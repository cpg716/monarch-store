import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { commands, FullPackageDetails, Package as BackendPackage, PackageInstallStatus, PackageSecuritySummary, PackageSource } from '../services/bindings';
import { unwrap } from '../utils/specta';
import { isSameSource } from '../utils/repoHelper';
import { getPackageReviews, RatingSummary, Review } from '../services/reviewService';
import { useSettings } from './useSettings';
import { useAppStore } from '../store/internal_store';
import { getPackageListKey } from '../utils/packageKey';
import { useErrorService } from '../context/ErrorContext';

export interface PackageDetailsVariant {
    source: PackageSource | string;
    version: string;
    repo_name?: string | null;
    pkg_name?: string | null;
    download_size?: string | null;
    installed_size?: string | null;
    maintainer?: string | null;
    license?: string[] | null;
    description?: string | null;
    screenshots?: string[] | null;
    security?: PackageSecuritySummary | null;
}

function isPlaceholderInstalledSource(source: PackageSource | string | null | undefined): boolean {
    return typeof source !== 'string' && !!source && source.source_type === 'repo' && source.id === 'installed';
}

function resolveInitialSource(
    pkg: BackendPackage | undefined,
    preferredSource?: PackageSource | string
): PackageSource | string | null {
    if (preferredSource) return preferredSource;
    if (!pkg?.source || isPlaceholderInstalledSource(pkg.source)) return null;
    return pkg.source;
}

function filterVariants(
    input: PackageDetailsVariant[],
    flags: { flatpak: boolean; aur: boolean; chaotic: boolean }
): PackageDetailsVariant[] {
    return input.filter((variant) => {
        const source = variant.source;
        if (typeof source === 'string') {
            if (source === 'aur' && !flags.aur) return false;
            if (source === 'flatpak' && !flags.flatpak) return false;
            if (source === 'chaotic' && !flags.chaotic) return false;
            return true;
        }
        if (source.source_type === 'aur' && !flags.aur) return false;
        if (source.source_type === 'flatpak' && !flags.flatpak) return false;
        if (source.id === 'chaotic-aur' && !flags.chaotic) return false;
        return true;
    });
}

function buildFallbackVariants(pkg: BackendPackage): PackageDetailsVariant[] {
    const sources = (pkg.available_sources && pkg.available_sources.length > 0)
        ? pkg.available_sources
        : [pkg.source];

    return sources.map((source) => ({
        source,
        version: source.version || pkg.version || 'latest',
        pkg_name: source.package_name || pkg.name,
        repo_name: source.id === 'chaotic-aur' ? 'chaotic-aur' : null,
        download_size: pkg.download_size || null,
        installed_size: pkg.installed_size || null,
        maintainer: pkg.maintainer || null,
        license: pkg.license || null,
        description: pkg.description || null,
        screenshots: pkg.screenshots || null,
        security: null,
    }));
}

export function usePackageDetailsModel(pkg?: BackendPackage, preferredSource?: PackageSource | string) {
    const [details, setDetails] = useState<FullPackageDetails | null>(null);
    const [selectedSource, setSelectedSource] = useState<PackageSource | string | null>(() => resolveInitialSource(pkg, preferredSource));
    const [reviews, setReviews] = useState<Review[]>([]);
    const [rating, setRating] = useState<RatingSummary | null>(null);
    const [loading, setLoading] = useState(false);
    const { isFlatpakEnabled, isAurEnabled, isChaoticEnabled } = useSettings();
    const upsertPackages = useAppStore((s) => s.upsertPackages);
    const setActivePackageId = useAppStore((s) => s.setActivePackageId);
    const errorService = useErrorService();
    const pkgRef = useRef(pkg);

    useEffect(() => {
        pkgRef.current = pkg;
        setSelectedSource(resolveInitialSource(pkg, preferredSource));
        setDetails(null);
    }, [pkg, preferredSource]);

    const refreshDetails = useCallback(async () => {
        const current = pkgRef.current;
        if (!current?.canonical_id) {
            errorService.reportWarning(`Package details requested without canonical_id: ${current?.name ?? 'unknown'}`);
            return;
        }

        setLoading(true);
        try {
            const nextDetails = unwrap(await commands.getFullPackageDetailsByCanonicalId(current.canonical_id));
            setDetails(nextDetails);

            if (nextDetails.package) {
                upsertPackages([nextDetails.package]);
                const nextId = getPackageListKey(nextDetails.package);
                const currentId = getPackageListKey(current);
                if (nextId && currentId && nextId !== currentId) {
                    setActivePackageId(nextId);
                }
            }

            const candidate =
                (nextDetails.all_installed_variants?.find((variant) => !!variant.source)?.source || null) ||
                ((nextDetails.installed_status?.installed && nextDetails.installed_status.source)
                    ? nextDetails.installed_status.source
                    : null) ||
                nextDetails.selected_default_source ||
                preferredSource || nextDetails.all_variants?.[0]?.source || current.source;
            setSelectedSource((currentSelected) => {
                if (
                    currentSelected &&
                    !isPlaceholderInstalledSource(currentSelected) &&
                    nextDetails.all_variants?.some((variant) => isSameSource(variant.source, currentSelected))
                ) {
                    return currentSelected;
                }
                return candidate;
            });
        } catch (error) {
            errorService.reportError(error as Error | string);
        } finally {
            setLoading(false);
        }
    }, [errorService, preferredSource, setActivePackageId, upsertPackages]);

    const refreshReviews = useCallback(async () => {
        const current = pkgRef.current;
        if (!current?.name) return;
        try {
            const result = await getPackageReviews(current.name, current.app_id || current.name);
            setReviews(result.reviews);
            setRating(result.summary);
        } catch (error) {
            errorService.reportError(error as Error | string);
        }
    }, [errorService]);

    useEffect(() => {
        if (!pkg?.canonical_id) return;
        void refreshDetails();
        void refreshReviews();

        const unlisten = listen('install-complete', () => {
            void refreshDetails();
        });

        return () => {
            unlisten.then((fn: UnlistenFn) => fn()).catch(() => undefined);
        };
    }, [pkg?.canonical_id, refreshDetails, refreshReviews]);

    const variants = useMemo(() => {
        if (!pkg) return [];
        const sourceVariants = details?.all_variants && details.all_variants.length > 0
            ? details.all_variants
            : buildFallbackVariants(pkg);
        return filterVariants(sourceVariants, {
            flatpak: isFlatpakEnabled,
            aur: isAurEnabled,
            chaotic: isChaoticEnabled,
        });
    }, [details?.all_variants, isAurEnabled, isChaoticEnabled, isFlatpakEnabled, pkg]);

    useEffect(() => {
        if (!variants.length) return;
        if (selectedSource && variants.some((variant) => isSameSource(variant.source, selectedSource))) return;
        setSelectedSource(
            details?.all_installed_variants?.find((variant) => !!variant.source)?.source ||
            (details?.installed_status?.installed && details.installed_status.source) ||
            details?.selected_default_source ||
            preferredSource ||
            variants[0].source
        );
    }, [details?.all_installed_variants, details?.installed_status, details?.selected_default_source, preferredSource, selectedSource, variants]);

    const activeVariant = useMemo(() => {
        if (!selectedSource) return variants[0] || null;
        return variants.find((variant) => isSameSource(variant.source, selectedSource)) || variants[0] || null;
    }, [selectedSource, variants]);

    const allInstalledVariants = details?.all_installed_variants || [];
    const selectedInstalledVariant = useMemo(() => (
        selectedSource
            ? allInstalledVariants.find((variant) => isSameSource(variant.source as any, selectedSource))
            : undefined
    ), [allInstalledVariants, selectedSource]);

    const isSelectedSourceInstalled = useMemo(() => {
        if (selectedInstalledVariant) return true;
        return allInstalledVariants.length === 0 && !!pkg?.installed;
    }, [allInstalledVariants.length, pkg?.installed, selectedInstalledVariant]);

    const effectivePackage = details?.package || pkg || null;

    return {
        packageData: effectivePackage,
        loading,
        variants,
        selectedSource,
        setSelectedSource,
        activeVariant,
        details,
        presentation: details?.presentation || null,
        displayTitle: details?.display_title || effectivePackage?.display_title || null,
        primaryAction: details?.primary_action || effectivePackage?.primary_action || null,
        primaryActionLabel: details?.primary_action_label || effectivePackage?.primary_action_label || null,
        sourceSummary: details?.source_summary || effectivePackage?.source_summary || null,
        securitySummary: details?.security_summary || effectivePackage?.security_summary || null,
        installedSourceLabel: details?.installed_source_label || null,
        sourceSwitchPolicy: details?.source_switch_policy || null,
        sourceSwitchNotice: details?.source_switch_notice || null,
        security: details?.security || null,
        flatpakPermissions: details?.flatpak_permissions || [],
        installStatus: details?.installed_status || null,
        allInstalledVariants,
        selectedInstalledVariant,
        isSelectedSourceInstalled,
        reviews,
        rating,
        refreshDetails,
        refreshReviews,
    };
}
