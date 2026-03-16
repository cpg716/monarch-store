import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Download, Heart, Play, RefreshCw, Settings2, Trash2 } from 'lucide-react';
import { clsx } from 'clsx';
import { useFavorites } from '../hooks/useFavorites';
import { resolveIconUrl } from '../utils/iconHelper';
import RepoBadge from './RepoBadge';
import PackageCardSkeleton from './PackageCardSkeleton';
import { useAppStore } from '../store/internal_store';
import { getPackageListKey } from '../utils/packageKey';
import { toPackageSource } from '../utils/repoHelper';
import { getSourceBrand } from '../utils/sourceBrand';
import {
    getPackageDisplayTitle,
    getPackagePrimaryActionLabel,
    getPackageSourceLabel,
    getPackageSourceSummary,
} from '../utils/packagePresentation';

import type { Package, PackageSource } from '../services/bindings';
import archLogo from '../assets/arch-logo.png';

interface PackageCardProps {
    pkg?: Package;
    pkgId?: string;
    onClick: (pkg: Package) => void;
    onPrimaryAction?: (pkg: Package) => void;
    onSecondaryAction?: (pkg: Package) => void;
    secondaryActionLabel?: string;
    skipMetadataFetch?: boolean;
    setupRequired?: boolean;
    onConfigureSource?: () => void;
    viewMode?: 'browse' | 'installed' | 'update';
}

function getVisibleSources(pkg: Package, flags: { flatpak: boolean; aur: boolean; chaotic: boolean }): PackageSource[] {
    const raw = (pkg.available_sources && pkg.available_sources.length > 0)
        ? pkg.available_sources
        : [pkg.source, ...(pkg.alternatives || []).map((alt) => alt.source)];

    const seen = new Set<string>();
    return raw
        .map((source) => toPackageSource(source))
        .filter((source) => {
            if (source.source_type === 'flatpak' && !flags.flatpak) return false;
            if (source.source_type === 'aur' && !flags.aur) return false;
            if (source.id === 'chaotic-aur' && !flags.chaotic) return false;
            const key = `${source.source_type}:${source.id}`;
            if (seen.has(key)) return false;
            seen.add(key);
            return true;
        });
}

function getStateSummary(pkg: Package, setupRequired: boolean, visibleSources: PackageSource[]): string {
    if (setupRequired) return 'Setup required before this source can be used';
    if (pkg.installed) return 'Installed on this system';
    const fallback = visibleSources.length > 1
        ? `Using ${visibleSources[0]?.label || 'this source'} by default • ${visibleSources.length - 1} more source${visibleSources.length - 1 === 1 ? '' : 's'} available`
        : pkg.is_optimized
            ? `Optimized for ${visibleSources[0]?.label || 'your distro'}`
            : visibleSources[0]?.label || 'Available now';
    return getPackageSourceSummary(pkg, fallback);
}

const PackageCardInner: React.FC<PackageCardProps & { pkg: Package }> = ({
    pkg,
    onClick,
    onPrimaryAction,
    onSecondaryAction,
    setupRequired = false,
    onConfigureSource,
    secondaryActionLabel = 'Remove',
    viewMode = 'browse',
}) => {
    const isFlatpakEnabled = useAppStore((s) => s.isFlatpakEnabled);
    const isAurEnabled = useAppStore((s) => s.isAurEnabled);
    const isChaoticEnabled = useAppStore((s) => s.isChaoticEnabled);
    const { toggleFavorite, isFavorite } = useFavorites();
    const cardRef = useRef<HTMLDivElement>(null);
    const [isVisible, setIsVisible] = useState(false);
    const [imgError, setImgError] = useState(false);

    const visibleSources = useMemo(
        () => getVisibleSources(pkg, { flatpak: isFlatpakEnabled, aur: isAurEnabled, chaotic: isChaoticEnabled }),
        [pkg, isFlatpakEnabled, isAurEnabled, isChaoticEnabled]
    );
    const badgeSource = pkg.installed ? toPackageSource(pkg.source) : (visibleSources[0] ?? toPackageSource(pkg.source));
    const sourceBrand = getSourceBrand(badgeSource, '');
    const displayName = getPackageDisplayTitle(pkg);
    const favoriteId = getPackageListKey(pkg);
    const isFav = isFavorite(favoriteId);
    const iconUrl = resolveIconUrl(pkg.icon || null);
    const stateSummary = getStateSummary(pkg, setupRequired, visibleSources);
    const summaryText = pkg.description?.trim() || 'No description available yet.';
    const primaryLabel = viewMode === 'installed'
        ? 'Launch'
        : viewMode === 'update'
            ? 'Update'
            : getPackagePrimaryActionLabel(pkg, { setupRequired });
    const PrimaryIcon = setupRequired
        ? Settings2
        : viewMode === 'installed'
            ? Play
            : viewMode === 'update'
                ? RefreshCw
                : Download;

    useEffect(() => {
        setImgError(false);
    }, [iconUrl]);

    useEffect(() => {
        const node = cardRef.current;
        if (!node) return;
        const observer = new IntersectionObserver(
            ([entry]) => {
                if (entry.isIntersecting) {
                    setIsVisible(true);
                    observer.disconnect();
                }
            },
            { rootMargin: '160px' }
        );
        observer.observe(node);
        return () => observer.disconnect();
    }, []);

    if (!pkg?.name) return null;

    return (
        <div
            ref={cardRef}
            onClick={() => onClick(pkg)}
            className="group flex h-full min-h-[208px] cursor-pointer flex-col rounded-xl border border-app-border bg-app-card px-4 py-4 transition-colors hover:border-blue-500/30 hover:bg-app-card/90"
        >
            {!isVisible ? (
                <PackageCardSkeleton />
            ) : (
                <>
                    <div className="flex items-start gap-3">
                        <div className="flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-white/5 bg-black/20 p-1.5">
                            {iconUrl && !imgError ? (
                                <img
                                    src={iconUrl}
                                    alt={displayName}
                                    className="h-full w-full object-contain"
                                    loading="lazy"
                                    onError={() => setImgError(true)}
                                />
                            ) : (
                                <img
                                    src={sourceBrand.logoAsset || archLogo}
                                    alt={sourceBrand.altText || 'Application'}
                                    className="h-full w-full object-contain opacity-70"
                                />
                            )}
                        </div>

                        <div className="min-w-0 flex-1">
                            <div className="min-w-0">
                                <h3 className="line-clamp-2 text-base font-bold leading-6 text-white">{displayName}</h3>
                                <p className="mt-0.5 truncate text-xs font-medium text-app-muted">
                                    {pkg.display_name && pkg.display_name.toLowerCase() !== pkg.name.toLowerCase()
                                        ? pkg.name
                                        : getPackageSourceLabel(badgeSource)}
                                </p>
                            </div>
                            <div className="mt-2">
                                <RepoBadge source={badgeSource} compact />
                            </div>
                        </div>
                    </div>

                    <p className="mt-3 line-clamp-2 min-h-[2.5rem] text-sm leading-6 text-slate-300">
                        {summaryText}
                    </p>

                    <div className="mt-3 flex min-h-[1.25rem] items-center">
                        <span
                            className={clsx(
                                'text-xs font-medium',
                                setupRequired && 'text-amber-400',
                                pkg.installed && 'text-emerald-400',
                                !setupRequired && !pkg.installed && 'text-slate-400'
                            )}
                        >
                            {stateSummary}
                        </span>
                    </div>
                    {visibleSources.length > 1 && !pkg.installed && (
                        <div className="mt-2 text-[11px] font-medium text-app-muted">
                            Compare sources in details before installing.
                        </div>
                    )}

                    <div className="mt-auto flex items-center justify-between gap-2 border-t border-white/5 pt-4">
                        <div className="flex items-center gap-2">
                            <button
                                type="button"
                                onClick={(event) => {
                                    event.stopPropagation();
                                    if (setupRequired) {
                                        onConfigureSource?.();
                                        return;
                                    }
                                    if (onPrimaryAction) {
                                        onPrimaryAction(pkg);
                                        return;
                                    }
                                    onClick(pkg);
                                }}
                                className={clsx(
                                    'inline-flex items-center gap-2 rounded-lg px-3 py-2 text-xs font-bold transition-colors',
                                    setupRequired
                                        ? 'bg-amber-500/15 text-amber-300 hover:bg-amber-500/25'
                                        : pkg.installed
                                            ? 'bg-emerald-500/15 text-emerald-300 hover:bg-emerald-500/25'
                                            : 'bg-blue-600 text-white hover:bg-blue-500'
                                )}
                            >
                                <PrimaryIcon size={14} />
                                {primaryLabel}
                            </button>

                            {onSecondaryAction && (
                                <button
                                    type="button"
                                    onClick={(event) => {
                                        event.stopPropagation();
                                        onSecondaryAction(pkg);
                                    }}
                                    className="inline-flex items-center gap-2 rounded-lg px-3 py-2 text-xs font-bold transition-colors bg-red-500/10 text-red-300 hover:bg-red-500/20"
                                >
                                    <Trash2 size={14} />
                                    {secondaryActionLabel}
                                </button>
                            )}
                        </div>

                        {!onSecondaryAction && (
                            <button
                                type="button"
                                onClick={(event) => {
                                    event.stopPropagation();
                                    toggleFavorite(favoriteId);
                                }}
                                className={clsx(
                                    'rounded-lg border p-2 transition-colors',
                                    isFav
                                        ? 'border-red-500/30 bg-red-500/10 text-red-400'
                                        : 'border-white/5 bg-black/20 text-app-muted hover:border-white/10 hover:text-white'
                                )}
                                aria-label={isFav ? 'Remove from favorites' : 'Add to favorites'}
                                title={isFav ? 'Remove from favorites' : 'Add to favorites'}
                            >
                                <Heart size={14} className={isFav ? 'fill-current' : ''} />
                            </button>
                        )}
                    </div>
                </>
            )}
        </div>
    );
};

const PackageCard: React.FC<PackageCardProps> = (props) => {
    const { pkg: pkgProp, pkgId } = props;
    const registryPkg = pkgId ? useAppStore((s) => s.packageRegistry[pkgId]) : undefined;
    const pkg = useMemo(() => {
        if (registryPkg) return registryPkg;
        if (pkgProp) return pkgProp;
        return undefined;
    }, [registryPkg, pkgProp]);

    if (pkgId && !pkg) return <PackageCardSkeleton />;
    if (!pkg) return null;

    return <PackageCardInner {...props} pkg={pkg} />;
};

export default React.memo(PackageCard);
