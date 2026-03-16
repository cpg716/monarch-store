import { useMemo, useState } from 'react';
import DOMPurify from 'dompurify';
import {
    ArrowLeft,
    CheckCircle2,
    Copy,
    ExternalLink,
    Globe,
    Heart,
    Play,
    ShieldCheck,
    Terminal,
    Trash2,
    AlertTriangle,
    Info,
    Search,
    MessageSquarePlus,
    Star,
} from 'lucide-react';
import { clsx } from 'clsx';
import RepoSelector from '../components/RepoSelector';
import RepoBadge from '../components/RepoBadge';
import { commands, Package as BackendPackage, PackageSource } from '../services/bindings';
import { usePackageDetailsModel } from '../hooks/usePackageDetailsModel';
import { useFavorites } from '../hooks/useFavorites';
import { useToast } from '../context/ToastContext';
import { useErrorService } from '../context/ErrorContext';
import { useAppStore } from '../store/internal_store';
import { getPackageListKey } from '../utils/packageKey';
import { unwrap } from '../utils/specta';
import { formatBytes } from '../utils/display';
import { resolveIconUrl, resolveImageUrl } from '../utils/iconHelper';
import { getPackageDisplayTitle } from '../utils/packagePresentation';
import { getSourceBrand } from '../utils/sourceBrand';
import archLogo from '../assets/arch-logo.png';
import { submitReview } from '../services/reviewService';

interface PackageDetailsProps {
    pkg?: BackendPackage;
    onBack: () => void;
    preferredSource?: PackageSource | string;
    installInProgress?: boolean;
    activeInstallPackage?: { name: string; mode: 'install' | 'uninstall' } | null;
    onInstall: (p: { name: string; source: PackageSource | string; repoName?: string; displayName?: string }) => void;
    onUninstall: (p: { name: string; source: PackageSource | string; repoName?: string; displayName?: string }) => void;
    onOpenSettings?: () => void;
}

function Section({
    title,
    children,
    compact = false,
}: {
    title: string;
    children: React.ReactNode;
    compact?: boolean;
}) {
    return (
        <section className={clsx('rounded-xl border border-app-border bg-app-card', compact ? 'p-4' : 'p-5')}>
            <h2 className="mb-3 text-sm font-bold text-white">{title}</h2>
            {children}
        </section>
    );
}

function sourceTuple(source: PackageSource | string | null | undefined): string {
    if (!source) return 'unknown';
    if (typeof source === 'string') return source;
    return `${source.source_type}:${source.id}`;
}

function isPlaceholderInstalledSource(source: PackageSource | string | null | undefined): boolean {
    return typeof source !== 'string' && !!source && source.source_type === 'repo' && source.id === 'installed';
}

function installCommand(pkgName: string, source: PackageSource | string | null | undefined): string {
    const target = pkgName.trim();
    if (!target) return '';
    if (!source || typeof source === 'string') return `sudo pacman -S ${target}`;
    if (source.source_type === 'flatpak') return `flatpak install flathub ${target}`;
    return `sudo pacman -S ${target}`;
}

function trustLabel(tier: string | null | undefined) {
    switch (tier) {
        case 'official':
            return 'Official repository';
        case 'distro_native':
            return 'Distribution repository';
        case 'third_party_repo':
            return 'Third-party repository';
        case 'community_build':
            return 'Community build';
        case 'sandboxed':
            return 'Sandboxed package';
        default:
            return 'Detected source';
    }
}

function securityHeadline(tier: string | null | undefined, access: string | null | undefined) {
    if (access === 'scoped') return 'Safer sandboxed app';
    switch (tier) {
        case 'official':
        case 'distro_native':
            return 'Trusted system package';
        case 'third_party_repo':
            return 'Extra review recommended';
        case 'community_build':
            return 'Built from community packaging';
        default:
            return 'Review this source before installing';
    }
}

function securityPlainLanguage(tier: string | null | undefined) {
    switch (tier) {
        case 'sandboxed':
            return 'This app runs inside a sandbox, so it usually has less access to your system.';
        case 'official':
            return 'This app comes from the main system repositories used by your distro.';
        case 'distro_native':
            return 'This app comes from your distribution’s own repositories.';
        case 'third_party_repo':
            return 'This app comes from an extra repository outside the main system repos.';
        case 'community_build':
            return 'This app is built from community packaging scripts, so it deserves a closer look.';
        default:
            return 'Check where this app comes from before you install it.';
    }
}

function accessLabel(access: string | null | undefined) {
    return access === 'scoped' ? 'Limited app permissions' : 'Full system package access';
}

function deriveSecurityFromSource(
    source: PackageSource | string | null | undefined,
    maintainerKnown: boolean
) {
    if (!source || typeof source === 'string') {
        return {
            trust_tier: 'official',
            system_access: 'full',
            maintainer_known: maintainerKnown,
            verification_note: 'Check where this app comes from before installing it.',
            user_action_note: 'Review the package source before installing.',
        };
    }

    const id = source.id.toLowerCase();
    if (source.source_type === 'flatpak') {
        return {
            trust_tier: 'sandboxed',
            system_access: 'scoped',
            maintainer_known: maintainerKnown,
            verification_note: 'This Flatpak is sandboxed, so it usually has less access to your system.',
            user_action_note: 'Check the app permissions before installing.',
        };
    }
    if (source.source_type === 'aur') {
        return {
            trust_tier: 'community_build',
            system_access: 'full',
            maintainer_known: maintainerKnown,
            verification_note: 'This source is built from community packaging scripts.',
            user_action_note: 'Review the package details and PKGBUILD before installing.',
        };
    }
    if (id.includes('chaotic')) {
        return {
            trust_tier: 'third_party_repo',
            system_access: 'full',
            maintainer_known: maintainerKnown,
            verification_note: 'This package comes from an extra third-party repository.',
            user_action_note: 'Confirm you trust this repository before installing.',
        };
    }
    if (id.includes('cachyos') || id.includes('manjaro') || id.includes('garuda') || id.includes('endeavour')) {
        return {
            trust_tier: 'distro_native',
            system_access: 'full',
            maintainer_known: maintainerKnown,
            verification_note: 'This package comes from your distribution repositories.',
            user_action_note: 'Review package details before installing.',
        };
    }
    return {
        trust_tier: 'official',
        system_access: 'full',
        maintainer_known: maintainerKnown,
        verification_note: 'This package comes from the main system repositories used by your distro.',
        user_action_note: 'Review package details before installing.',
    };
}

function normalizeMarkupText(value: string | null | undefined): string {
    if (!value) return '';
    return value
        .replace(/<[^>]+>/g, ' ')
        .replace(/&nbsp;/g, ' ')
        .replace(/\s+/g, ' ')
        .trim();
}

export default function PackageDetails({
    pkg,
    onBack,
    preferredSource,
    installInProgress = false,
    activeInstallPackage = null,
    onInstall,
    onUninstall,
}: PackageDetailsProps) {
    const {
        packageData,
        loading,
        variants,
        selectedSource,
        setSelectedSource,
        activeVariant,
        details,
        presentation,
        flatpakPermissions,
        installStatus,
        selectedInstalledVariant,
        isSelectedSourceInstalled,
        displayTitle,
        primaryAction,
        primaryActionLabel,
        sourceSummary,
        securitySummary,
        installedSourceLabel,
        sourceSwitchNotice,
        security,
        reviews,
        rating,
        refreshReviews,
    } = usePackageDetailsModel(pkg, preferredSource);
    const { toggleFavorite, isFavorite } = useFavorites();
    const { success } = useToast();
    const errorService = useErrorService();
    const advancedMode = useAppStore((s) => s.advancedMode);
    const [reviewQuery, setReviewQuery] = useState('');
    const [reviewSourceFilter, setReviewSourceFilter] = useState<'all' | 'odrs' | 'monarch'>('all');
    const [reviewName, setReviewName] = useState('');
    const [reviewComment, setReviewComment] = useState('');
    const [reviewStars, setReviewStars] = useState(5);
    const [submittingReview, setSubmittingReview] = useState(false);
    const [reviewStarFilter, setReviewStarFilter] = useState<'all' | 5 | 4 | 3 | 2 | 1>('all');
    const [reviewSort, setReviewSort] = useState<'recent' | 'highest' | 'lowest'>('recent');
    const [showAllReviews, setShowAllReviews] = useState(false);
    const [lightboxIndex, setLightboxIndex] = useState<number | null>(null);

    const packageRecord = packageData || pkg;
    const favoriteId = useMemo(() => (packageRecord ? getPackageListKey(packageRecord) : ''), [packageRecord]);
    const isFav = favoriteId ? isFavorite(favoriteId) : false;
    const sharedIcon = presentation?.icon || packageRecord?.icon || null;
    const sharedShortDescription = activeVariant?.description || presentation?.short_description || packageRecord?.description || '';
    const sharedLongDescription = presentation?.long_description || packageRecord?.long_description || null;

    const screenshots = useMemo(() => {
        const raw = presentation?.screenshots?.length
            ? presentation.screenshots
            : (activeVariant?.screenshots || packageRecord?.screenshots || []);
        return raw.map((value) => resolveImageUrl(value) ?? value);
    }, [activeVariant?.screenshots, packageRecord?.screenshots, presentation?.screenshots]);

    if (!packageRecord?.name) {
        return (
            <div className="flex h-full items-center justify-center bg-app-bg">
                <p className="text-sm text-app-muted">Loading package details…</p>
            </div>
        );
    }

    const displayName = presentation?.display_title || displayTitle || getPackageDisplayTitle(packageRecord);
    const usingVariantData = !!activeVariant;
    const displayVersion = usingVariantData
        ? (activeVariant?.version || packageRecord.version || installStatus?.version || 'Not provided by source')
        : (packageRecord.version || installStatus?.version || 'latest');
    const displaySize = usingVariantData
        ? (
            formatBytes(activeVariant?.installed_size)
            || formatBytes(activeVariant?.download_size)
            || formatBytes(packageRecord.installed_size_bytes || packageRecord.installed_size)
            || formatBytes(packageRecord.download_size_bytes || packageRecord.download_size)
            || 'Not provided by source'
        )
        : (
            formatBytes(packageRecord.installed_size_bytes || packageRecord.installed_size)
            || formatBytes(packageRecord.download_size_bytes || packageRecord.download_size)
            || 'Not provided by source'
        );
    const maintainer = usingVariantData ? (activeVariant?.maintainer || packageRecord.maintainer || null) : (packageRecord.maintainer || null);
    const license = usingVariantData ? (activeVariant?.license?.join(', ') || packageRecord.license?.join(', ') || null) : (packageRecord.license?.join(', ') || null);
    const isBusy = !!(
        activeInstallPackage &&
        activeInstallPackage.name === (activeVariant?.pkg_name || packageRecord.name) &&
        installInProgress
    );
    const fallbackSource = isPlaceholderInstalledSource(packageRecord.source) ? null : packageRecord.source;
    const resolvedSource = selectedSource || installStatus?.source || fallbackSource;
    const maintainerKnown = !!maintainer;
    const currentSecurity = activeVariant
        ? (activeVariant.security || deriveSecurityFromSource(activeVariant.source, maintainerKnown))
        : (security || deriveSecurityFromSource(resolvedSource || packageRecord.source, maintainerKnown));
    const SecurityIcon = currentSecurity?.system_access === 'scoped' ? ShieldCheck : AlertTriangle;
    const packageNameForAction = activeVariant?.pkg_name || selectedInstalledVariant?.actual_package_name || packageRecord.name;
    const canLaunch = isSelectedSourceInstalled || packageRecord.installed;
    const sourceIsResolving = canLaunch && !resolvedSource;
    const primaryButtonLabel = canLaunch
        ? (primaryActionLabel || 'Launch')
        : (primaryActionLabel || 'Install');
    const normalizedShortDescription = normalizeMarkupText(sharedShortDescription);
    const normalizedLongDescription = normalizeMarkupText(sharedLongDescription);
    const hasExtendedOverview = !!normalizedLongDescription && normalizedLongDescription !== normalizedShortDescription;
    const overviewHtml = hasExtendedOverview
        ? DOMPurify.sanitize(sharedLongDescription || '')
        : '';
    const sourceSelectionLocked = canLaunch;
    const securityTrustTier = currentSecurity?.trust_tier || packageRecord.trust_level || null;
    const securityAccess = currentSecurity?.system_access || null;
    const securityTitle = securityHeadline(securityTrustTier, securityAccess);
    const securityOverview = securityPlainLanguage(securityTrustTier);
    const selectedSourceBrand = getSourceBrand(resolvedSource || packageRecord.source, '');
    const sourceTupleValue = resolvedSource ? sourceTuple(resolvedSource) : 'detecting-source';
    const installedSourceTitle = installedSourceLabel || (resolvedSource && typeof resolvedSource !== 'string' ? resolvedSource.label : null) || 'Installed package';
    const securityMessage = currentSecurity?.verification_note || securitySummary || 'Review the package source before installing.';
    const displaySourceSummary = variants.length > 1
        ? `${variants.length} sources available`
        : (resolvedSource && typeof resolvedSource !== 'string'
            ? resolvedSource.label
            : sourceSummary);
    const projectWebsite = packageRecord.url || null;
    const developerName = presentation?.developer_name || details?.developer_name || maintainer || null;
    const donationUrl = presentation?.donation_url || details?.donation_url || null;
    const filteredReviews = useMemo(() => {
        const q = reviewQuery.trim().toLowerCase();
        const next = reviews.filter((review) => {
            if (reviewSourceFilter !== 'all' && review.source !== reviewSourceFilter) return false;
            if (reviewStarFilter !== 'all' && Math.round(review.rating) !== reviewStarFilter) return false;
            if (!q) return true;
            return (
                review.userName.toLowerCase().includes(q)
                || review.comment.toLowerCase().includes(q)
                || review.source.toLowerCase().includes(q)
            );
        });
        if (reviewSort === 'highest') {
            next.sort((a, b) => b.rating - a.rating || b.date.getTime() - a.date.getTime());
        } else if (reviewSort === 'lowest') {
            next.sort((a, b) => a.rating - b.rating || b.date.getTime() - a.date.getTime());
        } else {
            next.sort((a, b) => b.date.getTime() - a.date.getTime());
        }
        return next;
    }, [reviewQuery, reviewSourceFilter, reviewSort, reviewStarFilter, reviews]);
    const visibleReviews = showAllReviews ? filteredReviews : filteredReviews.slice(0, 8);
    const supportLinkLabel = projectWebsite ? 'Support / Donate via project site' : null;

    const handleInstall = () => {
        onInstall({
            name: packageNameForAction,
            source: resolvedSource || packageRecord.source,
            repoName: activeVariant?.repo_name || undefined,
            displayName: packageRecord.display_name || undefined,
        });
    };

    const handleUninstall = () => {
        onUninstall({
            name: packageNameForAction,
            source: resolvedSource || packageRecord.source,
            repoName: activeVariant?.repo_name || undefined,
            displayName: packageRecord.display_name || undefined,
        });
    };

    const handleLaunch = async () => {
        try {
            await commands.launchPackage({
                package_name: packageNameForAction,
                app_id: packageRecord.app_id || null,
                desktop_entry: null,
                launch_target: packageRecord.launch_target || null,
                source: typeof resolvedSource === 'string' ? packageRecord.source : (resolvedSource || packageRecord.source),
            }).then(unwrap);
            success('App launched');
        } catch (error) {
            errorService.reportError(error as Error | string);
        }
    };

    const copyValue = async (value: string, label: string) => {
        try {
            await navigator.clipboard.writeText(value);
            success(`${label} copied`);
        } catch (error) {
            errorService.reportError(error as Error | string);
        }
    };

    const handleSubmitReview = async () => {
        if (!reviewComment.trim()) {
            errorService.reportWarning('Please write a short review before submitting.');
            return;
        }
        try {
            setSubmittingReview(true);
            // Tauri command does local save + Supabase; fallback to Supabase-only when not in Tauri
            if (typeof (window as any).__TAURI_INVOKE__ === 'function') {
                unwrap(await commands.submitReview(packageRecord.name, reviewStars, '', reviewComment.trim(), reviewName.trim() || 'MonARCH User'));
            } else {
                await submitReview(packageRecord.name, reviewStars, reviewComment.trim(), reviewName.trim() || 'MonARCH User');
            }
            setReviewComment('');
            setReviewName('');
            setReviewStars(5);
            await refreshReviews();
            success('Review submitted');
            commands.trackTelemetryEvent('review_submitted', {
                package_name: packageRecord.name,
                rating: reviewStars,
                source: 'supabase',
            }).catch(() => {});
        } catch (error) {
            errorService.reportError(error as Error | string);
        } finally {
            setSubmittingReview(false);
        }
    };

    return (
        <div className="flex-1 overflow-y-auto bg-app-bg px-6 py-6">
            <div className="mx-auto flex w-full max-w-6xl flex-col gap-6">
                <div className="flex items-center justify-between gap-3">
                    <button
                        onClick={onBack}
                        className="inline-flex items-center gap-2 rounded-lg border border-app-border bg-app-card px-3 py-2 text-xs font-bold text-slate-300 transition-colors hover:text-white"
                    >
                        <ArrowLeft size={14} />
                        Back to Store
                    </button>
                    <button
                        onClick={() => toggleFavorite(favoriteId)}
                        className={clsx(
                            'inline-flex items-center gap-2 rounded-lg border px-3 py-2 text-xs font-bold transition-colors',
                            isFav
                                ? 'border-red-500/30 bg-red-500/10 text-red-300'
                                : 'border-app-border bg-app-card text-slate-300 hover:text-white'
                        )}
                    >
                        <Heart size={14} className={isFav ? 'fill-current' : ''} />
                        {isFav ? 'Saved' : 'Save'}
                    </button>
                </div>

                <div className="grid gap-6 lg:grid-cols-[minmax(0,2fr)_minmax(280px,1fr)]">
                    <div className="rounded-xl border border-app-border bg-app-card p-5">
                        <div className="flex flex-col gap-5 lg:flex-row">
                            <div className="flex h-24 w-24 shrink-0 items-center justify-center rounded-xl border border-white/5 bg-black/20 p-3">
                                <img
                                    src={resolveIconUrl(sharedIcon) || archLogo}
                                    alt={displayName}
                                    className="h-full w-full object-contain"
                                />
                            </div>

                            <div className="min-w-0 flex-1">
                                <div className="mb-3 flex flex-wrap items-center gap-2">
                                    {resolvedSource ? (
                                        <RepoBadge source={resolvedSource} />
                                    ) : (
                                        <span className="rounded-md bg-slate-700/40 px-2 py-1 text-[11px] font-bold text-slate-300">
                                            Detecting Source
                                        </span>
                                    )}
                                    <span
                                        className={clsx(
                                            'rounded-md px-2 py-1 text-[11px] font-bold',
                                            canLaunch ? 'bg-emerald-500/15 text-emerald-300' : 'bg-blue-600/15 text-blue-300'
                                        )}
                                    >
                                        {canLaunch ? 'Installed' : 'Available'}
                                    </span>
                                    {loading && !details && <span className="text-xs text-app-muted">Refreshing details…</span>}
                                </div>

                                <h1 className="text-3xl font-black tracking-tight text-white lg:text-4xl">{displayName}</h1>
                                <p className="mt-2 text-sm leading-6 text-slate-300">
                                    {sharedShortDescription}
                                </p>
                                {displaySourceSummary && (
                                    <p className="mt-2 text-xs font-medium text-app-muted">
                                        {displaySourceSummary}
                                    </p>
                                )}

                                <div className="mt-4 grid gap-3 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-6">
                                    <div>
                                        <div className="text-[11px] font-semibold text-app-muted">Version</div>
                                        <div className="mt-1 text-sm font-medium text-white">{displayVersion}</div>
                                    </div>
                                    <div>
                                        <div className="text-[11px] font-semibold text-app-muted">Size</div>
                                        <div className="mt-1 text-sm font-medium text-white">{displaySize}</div>
                                    </div>
                                    <div>
                                        <div className="text-[11px] font-semibold text-app-muted">Maintainer</div>
                                        <div className={clsx('mt-1 truncate text-sm font-medium', maintainer ? 'text-white' : 'text-app-muted')}>
                                            {maintainer || 'Not published by source'}
                                        </div>
                                    </div>
                                    <div>
                                        <div className="text-[11px] font-semibold text-app-muted">Developer</div>
                                        <div className={clsx('mt-1 truncate text-sm font-medium', developerName ? 'text-white' : 'text-app-muted')}>
                                            {developerName || 'Not listed'}
                                        </div>
                                    </div>
                                    <div>
                                        <div className="text-[11px] font-semibold text-app-muted">License</div>
                                        <div className={clsx('mt-1 truncate text-sm font-medium', license ? 'text-white' : 'text-app-muted')}>
                                            {license || 'Not provided by source'}
                                        </div>
                                    </div>
                                    <div>
                                        <div className="text-[11px] font-semibold text-app-muted">Rating</div>
                                        <div className="mt-1 flex items-center gap-2 text-sm font-medium text-white">
                                            <Star size={14} className="fill-amber-400 text-amber-400" />
                                            {rating ? rating.average.toFixed(1) : 'No rating'}
                                            <span className="text-xs font-medium text-app-muted">
                                                {rating ? `${rating.count} reviews` : ''}
                                            </span>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>

                        <div className="mt-5 grid gap-4 lg:grid-cols-[minmax(0,1fr)_auto]">
                            {sourceSelectionLocked ? (
                                <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/10 px-5 py-4">
                                    <div className="flex flex-wrap items-center gap-2">
                                        {resolvedSource ? (
                                            <RepoBadge source={resolvedSource} />
                                        ) : (
                                            <span className="rounded-md bg-black/20 px-2 py-1 text-[11px] font-bold text-emerald-200">
                                                Installed Package
                                            </span>
                                        )}
                                        <span className="text-sm font-bold text-emerald-300">
                                            {installedSourceTitle}
                                        </span>
                                    </div>
                                    <div className="mt-2 text-xs font-medium text-emerald-100/80">
                                        {sourceIsResolving
                                            ? 'Checking which source this installed app came from…'
                                            : 'Installed apps stay on their current source. Compare other sources below, then uninstall first if you want to switch.'}
                                    </div>
                                </div>
                            ) : (
                                <div className="space-y-2">
                                    <RepoSelector
                                        variants={variants.map((variant) => ({
                                            ...variant,
                                            repo_name: variant.repo_name || undefined,
                                            pkg_name: variant.pkg_name || undefined,
                                        }))}
                                        selectedSource={selectedSource || packageRecord.source}
                                        onChange={setSelectedSource}
                                    />
                                    <div className="px-1 text-xs font-medium text-app-muted">
                                        {selectedSourceBrand.hint}
                                    </div>
                                </div>
                            )}

                            <div className="flex flex-wrap gap-2">
                                {canLaunch ? (
                                    <button
                                        onClick={handleLaunch}
                                        disabled={isBusy}
                                        className="inline-flex items-center gap-2 rounded-lg bg-emerald-600 px-4 py-2 text-sm font-bold text-white transition-colors hover:bg-emerald-500 disabled:opacity-60"
                                    >
                                        {primaryAction === 'launch' ? <Play size={15} /> : <CheckCircle2 size={15} />}
                                        {primaryButtonLabel}
                                    </button>
                                ) : (
                                    <button
                                        onClick={handleInstall}
                                        disabled={isBusy}
                                        className="inline-flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-bold text-white transition-colors hover:bg-blue-500 disabled:opacity-60"
                                    >
                                        <CheckCircle2 size={15} />
                                        {primaryButtonLabel}
                                    </button>
                                )}
                                {canLaunch && (
                                    <button
                                        onClick={handleUninstall}
                                        disabled={isBusy}
                                        className="inline-flex items-center gap-2 rounded-lg bg-red-600/15 px-4 py-2 text-sm font-bold text-red-300 transition-colors hover:bg-red-600/25 disabled:opacity-60"
                                    >
                                        <Trash2 size={15} />
                                        Uninstall
                                    </button>
                                )}
                            </div>
                        </div>
                        {sourceSwitchNotice && (
                            <div className="mt-4 rounded-lg border border-blue-500/20 bg-blue-500/10 p-3 text-sm text-blue-100">
                                <div className="flex items-start gap-2">
                                    <Info size={16} className="mt-0.5 shrink-0" />
                                    <div>
                                        <div className="font-semibold">
                                            {installedSourceLabel ? `Installed from ${installedSourceLabel}` : 'Installed app'}
                                        </div>
                                        <div className="mt-1 text-blue-100/90">{sourceSwitchNotice}</div>
                                    </div>
                                </div>
                            </div>
                        )}
                        {variants.length > 1 && sourceSelectionLocked && (
                            <div className="mt-4 grid gap-2">
                                {variants.map((variant) => {
                                    const variantKey = `${sourceTuple(variant.source)}:${variant.pkg_name || variant.version}`;
                                    const isInstalledSource = resolvedSource
                                        ? sourceTuple(variant.source) === sourceTuple(resolvedSource)
                                        : false;
                                    const variantLabel = typeof variant.source === 'string'
                                        ? variant.source
                                        : variant.source.label;
                                    return (
                                        <div key={variantKey} className="flex items-center justify-between rounded-lg border border-white/5 bg-black/20 px-3 py-3 text-sm">
                                            <div>
                                                <div className="font-medium text-white">{variantLabel}</div>
                                                <div className="text-xs text-app-muted">v{variant.version || 'latest'}</div>
                                            </div>
                                            <div className={clsx('text-xs font-semibold', isInstalledSource ? 'text-emerald-300' : 'text-app-muted')}>
                                                {isInstalledSource ? 'Installed source' : 'Available after uninstall'}
                                            </div>
                                        </div>
                                    );
                                })}
                            </div>
                        )}
                    </div>

                    <div className="space-y-4">
                        <Section title="Source Safety" compact>
                            {sourceIsResolving && !currentSecurity ? (
                                <div className="rounded-lg border border-slate-500/20 bg-slate-500/10 p-4 text-slate-200">
                                    <div className="flex items-center gap-2 text-sm font-bold">
                                        <Info size={16} />
                                        Resolving installed source…
                                    </div>
                                    <div className="mt-3 text-sm leading-6 text-slate-300">
                                        MonARCH is checking which source this installed app came from so it can show the correct trust guidance, source actions, and switch rules.
                                    </div>
                                </div>
                            ) : (
                                <div
                                    className={clsx(
                                        'rounded-lg border p-4',
                                        currentSecurity?.system_access === 'scoped'
                                            ? 'border-emerald-500/15 bg-emerald-500/5 text-emerald-100'
                                            : 'border-slate-500/20 bg-slate-500/10 text-slate-200'
                                    )}
                                >
                                    <div className="flex items-center gap-2 text-sm font-bold">
                                        <SecurityIcon size={16} />
                                        {securityTitle}
                                    </div>
                                    <div className="mt-3 grid gap-2 text-sm leading-6">
                                        <div>
                                            <span className="font-semibold">In plain language:</span>{' '}
                                            {securityOverview}
                                        </div>
                                        <div>
                                            <span className="font-semibold">Access level:</span>{' '}
                                            {accessLabel(currentSecurity?.system_access || null)}
                                        </div>
                                        <div className="opacity-90">
                                            {securityMessage}
                                        </div>
                                        {advancedMode && (
                                            <div className="mt-1 rounded-lg border border-white/10 bg-black/20 p-3 text-xs text-slate-300">
                                                <div><span className="font-semibold text-white">Trust tier:</span> {trustLabel(securityTrustTier)}</div>
                                                <div className="mt-1"><span className="font-semibold text-white">Access model:</span> {currentSecurity?.system_access || 'unknown'}</div>
                                                <div className="mt-1"><span className="font-semibold text-white">Source tuple:</span> {sourceTupleValue}</div>
                                            </div>
                                        )}
                                    </div>
                                </div>
                            )}
                            {flatpakPermissions.length > 0 && (
                                <div className="mt-4 space-y-2">
                                    <div className="text-xs font-semibold text-app-muted">Requested permissions</div>
                                    <div className="flex flex-wrap gap-2">
                                        {flatpakPermissions.map((permission) => (
                                            <span
                                                key={permission}
                                                className="rounded-md border border-white/5 bg-black/20 px-2 py-1 text-xs text-slate-300"
                                            >
                                                {permission}
                                            </span>
                                        ))}
                                    </div>
                                </div>
                            )}
                        </Section>

                        <Section title="Source Actions" compact>
                            <div className="space-y-2">
                                <button
                                    onClick={() => copyValue(packageRecord.name, 'Package name')}
                                    className="flex w-full items-center justify-between rounded-lg border border-app-border bg-black/20 px-3 py-3 text-left text-sm text-slate-300"
                                >
                                    <span className="inline-flex items-center gap-2"><Copy size={14} /> Copy package identifier</span>
                                    <span className="truncate pl-3 text-xs text-app-muted">{packageRecord.name}</span>
                                </button>
                                <button
                                    onClick={() => copyValue(sourceTupleValue, 'Source identifier')}
                                    disabled={!resolvedSource}
                                    className="flex w-full items-center justify-between rounded-lg border border-app-border bg-black/20 px-3 py-3 text-left text-sm text-slate-300"
                                >
                                    <span className="inline-flex items-center gap-2"><Copy size={14} /> Copy source tuple</span>
                                    <span className="truncate pl-3 text-xs text-app-muted">{resolvedSource ? sourceTupleValue : 'Resolving…'}</span>
                                </button>
                                <button
                                    onClick={() => copyValue(installCommand(packageNameForAction, resolvedSource || packageRecord.source), 'Install command')}
                                    className="flex w-full items-center justify-between rounded-lg border border-app-border bg-black/20 px-3 py-3 text-left text-sm text-slate-300"
                                >
                                    <span className="inline-flex items-center gap-2"><Terminal size={14} /> Copy install command</span>
                                    <span className="truncate pl-3 text-xs text-app-muted">{installCommand(packageNameForAction, resolvedSource || packageRecord.source)}</span>
                                </button>
                                {packageRecord.url && (
                                    <a
                                        href={packageRecord.url}
                                        target="_blank"
                                        rel="noreferrer"
                                        className="flex w-full items-center justify-between rounded-lg border border-app-border bg-black/20 px-3 py-3 text-left text-sm text-slate-300"
                                    >
                                        <span className="inline-flex items-center gap-2"><Globe size={14} /> Open official website</span>
                                        <ExternalLink size={14} />
                                    </a>
                                )}
                                {supportLinkLabel && donationUrl && (
                                    <a
                                        href={donationUrl}
                                        target="_blank"
                                        rel="noreferrer"
                                        className="flex w-full items-center justify-between rounded-lg border border-emerald-500/20 bg-emerald-500/5 px-3 py-3 text-left text-sm text-emerald-200"
                                    >
                                        <span className="inline-flex items-center gap-2"><Heart size={14} /> {supportLinkLabel}</span>
                                        <ExternalLink size={14} />
                                    </a>
                                )}
                            </div>
                        </Section>
                    </div>
                </div>

                {screenshots.length > 0 && (
                    <Section title="Screenshots">
                        <div className="-mx-1 flex gap-3 overflow-x-auto px-1 pb-2">
                            {screenshots.map((shot, index) => (
                                <button
                                    key={shot}
                                    type="button"
                                    onClick={() => setLightboxIndex(index)}
                                    className="group relative min-w-[320px] shrink-0 overflow-hidden rounded-lg border border-app-border bg-black/30 text-left sm:min-w-[420px]"
                                >
                                    <div className="aspect-video w-full overflow-hidden bg-black/40">
                                        <img
                                            src={shot}
                                            alt={`${displayName} screenshot ${index + 1}`}
                                            className="h-full w-full object-contain transition-transform duration-200 group-hover:scale-[1.02]"
                                        />
                                    </div>
                                    <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/70 to-transparent px-3 py-2 text-xs font-semibold text-white/90">
                                        Click to enlarge
                                    </div>
                                </button>
                            ))}
                        </div>
                    </Section>
                )}

                <div className="grid gap-6">
                    <Section title="Overview">
                        {hasExtendedOverview ? (
                            <div
                                className="prose prose-invert max-w-none text-sm leading-7 text-slate-300"
                                dangerouslySetInnerHTML={{ __html: overviewHtml }}
                            />
                        ) : (
                            <div className="space-y-3 text-sm leading-7 text-slate-300">
                                <p>
                                    This source does not provide extended package notes yet. The summary above gives the quick purpose of the app.
                                </p>
                                <p className="text-app-muted">
                                    New users: review the source, security, and permissions before installing.
                                    Advanced users: use the source actions and technical details below to verify the package identity quickly.
                                </p>
                            </div>
                        )}
                    </Section>
                </div>

                <div className={clsx('grid gap-6', advancedMode && 'xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]')}>
                    {advancedMode && (
                    <Section title="Technical Details">
                        <div className="grid gap-3 sm:grid-cols-2">
                            <div className="rounded-lg border border-white/5 bg-black/20 p-3">
                                <div className="text-[11px] font-semibold text-app-muted">Package</div>
                                <div className="mt-1 text-sm text-white">{packageRecord.name}</div>
                            </div>
                            <div className="rounded-lg border border-white/5 bg-black/20 p-3">
                                <div className="text-[11px] font-semibold text-app-muted">Source</div>
                                <div className="mt-1 text-sm text-white">{sourceTupleValue}</div>
                            </div>
                            <div className="rounded-lg border border-white/5 bg-black/20 p-3">
                                <div className="text-[11px] font-semibold text-app-muted">App ID</div>
                                <div className="mt-1 truncate text-sm text-white">{packageRecord.app_id || 'Unavailable'}</div>
                            </div>
                            <div className="rounded-lg border border-white/5 bg-black/20 p-3">
                                <div className="text-[11px] font-semibold text-app-muted">Trust</div>
                                <div className="mt-1 text-sm text-white">{trustLabel(securityTrustTier)}</div>
                            </div>
                            <div className="rounded-lg border border-white/5 bg-black/20 p-3">
                                <div className="text-[11px] font-semibold text-app-muted">Provided by</div>
                                <div className="mt-1 truncate text-sm text-white">{activeVariant?.repo_name || 'Detected source'}</div>
                            </div>
                        </div>
                    </Section>
                    )}

                    <Section title="Reviews">
                        <div className="mb-4 flex items-center justify-between">
                            <div>
                                <div className="text-xl font-bold text-white">
                                    {rating ? rating.average.toFixed(1) : '0.0'}
                                </div>
                                <div className="text-xs text-app-muted">
                                    {rating ? `${rating.count} user reviews (last 24 months)` : 'No reviews yet'}
                                </div>
                            </div>
                            <div className="text-right text-xs text-app-muted">
                                Ratings combine ODRS and MonARCH community feedback from the last 24 months.
                            </div>
                        </div>
                        <div className="mb-4 grid gap-3 md:grid-cols-[minmax(0,1fr)_auto_auto_auto]">
                            <label className="flex items-center gap-2 rounded-lg border border-app-border bg-black/20 px-3 py-2 text-sm text-slate-300">
                                <Search size={14} className="text-app-muted" />
                                <input
                                    value={reviewQuery}
                                    onChange={(event) => setReviewQuery(event.target.value)}
                                    placeholder="Search reviews"
                                    className="w-full bg-transparent outline-none placeholder:text-app-muted"
                                />
                            </label>
                            <select
                                value={reviewSourceFilter}
                                onChange={(event) => setReviewSourceFilter(event.target.value as 'all' | 'odrs' | 'monarch')}
                                className="rounded-lg border border-app-border bg-black/20 px-3 py-2 text-sm text-slate-300 outline-none"
                            >
                                <option value="all">All sources</option>
                                <option value="odrs">ODRS</option>
                                <option value="monarch">MonARCH</option>
                            </select>
                            <select
                                value={reviewStarFilter}
                                onChange={(event) => setReviewStarFilter(event.target.value === 'all' ? 'all' : Number(event.target.value) as 5 | 4 | 3 | 2 | 1)}
                                className="rounded-lg border border-app-border bg-black/20 px-3 py-2 text-sm text-slate-300 outline-none"
                            >
                                <option value="all">All ratings</option>
                                <option value={5}>5 stars</option>
                                <option value={4}>4 stars</option>
                                <option value={3}>3 stars</option>
                                <option value={2}>2 stars</option>
                                <option value={1}>1 star</option>
                            </select>
                            <select
                                value={reviewSort}
                                onChange={(event) => setReviewSort(event.target.value as 'recent' | 'highest' | 'lowest')}
                                className="rounded-lg border border-app-border bg-black/20 px-3 py-2 text-sm text-slate-300 outline-none"
                            >
                                <option value="recent">Newest first</option>
                                <option value="highest">Highest rated</option>
                                <option value="lowest">Lowest rated</option>
                            </select>
                            <button
                                onClick={handleSubmitReview}
                                disabled={submittingReview}
                                className="inline-flex items-center gap-2 rounded-lg bg-blue-600 px-3 py-2 text-sm font-bold text-white transition-colors hover:bg-blue-500 disabled:opacity-60"
                            >
                                <MessageSquarePlus size={14} />
                                {submittingReview ? 'Sending…' : 'Leave Review'}
                            </button>
                        </div>
                        <div className="mb-4 grid gap-3 md:grid-cols-[160px_minmax(0,1fr)_minmax(0,2fr)]">
                            <input
                                value={reviewName}
                                onChange={(event) => setReviewName(event.target.value)}
                                placeholder="Your name"
                                className="rounded-lg border border-app-border bg-black/20 px-3 py-2 text-sm text-slate-300 outline-none placeholder:text-app-muted"
                            />
                            <select
                                value={reviewStars}
                                onChange={(event) => setReviewStars(Number(event.target.value))}
                                className="rounded-lg border border-app-border bg-black/20 px-3 py-2 text-sm text-slate-300 outline-none"
                            >
                                <option value={5}>5 stars</option>
                                <option value={4}>4 stars</option>
                                <option value={3}>3 stars</option>
                                <option value={2}>2 stars</option>
                                <option value={1}>1 star</option>
                            </select>
                            <input
                                value={reviewComment}
                                onChange={(event) => setReviewComment(event.target.value)}
                                placeholder="Share what worked well (or didn’t)"
                                className="rounded-lg border border-app-border bg-black/20 px-3 py-2 text-sm text-slate-300 outline-none placeholder:text-app-muted"
                            />
                        </div>
                        <div className="space-y-3">
                            {filteredReviews.length === 0 ? (
                                <p className="text-sm text-app-muted">No recent reviews available.</p>
                            ) : (
                                visibleReviews.map((review) => (
                                    <div key={`${review.source}-${review.id}`} className="rounded-lg border border-white/5 bg-black/20 p-3">
                                        <div className="flex items-center justify-between gap-3">
                                            <div className="text-sm font-semibold text-white">{review.userName}</div>
                                            <div className="text-xs text-app-muted">{review.date.toLocaleDateString()}</div>
                                        </div>
                                        <div className="mt-1 text-xs text-amber-300">{'★'.repeat(Math.max(1, Math.round(review.rating)))}</div>
                                        <p className="mt-2 text-sm leading-6 text-slate-300">{review.comment || 'No written review provided.'}</p>
                                    </div>
                                ))
                            )}
                        </div>
                        {filteredReviews.length > 8 && (
                            <div className="mt-4">
                                <button
                                    onClick={() => setShowAllReviews((current) => !current)}
                                    className="rounded-lg border border-app-border bg-black/20 px-3 py-2 text-sm font-semibold text-slate-300 transition-colors hover:text-white"
                                >
                                    {showAllReviews ? 'Show fewer reviews' : `View all ${filteredReviews.length} reviews`}
                                </button>
                            </div>
                        )}
                    </Section>
                </div>
            </div>
            {lightboxIndex !== null && screenshots[lightboxIndex] && (
                <div
                    className="fixed inset-0 z-50 flex items-center justify-center bg-black/85 p-6"
                    role="dialog"
                    aria-modal="true"
                    onClick={() => setLightboxIndex(null)}
                >
                    <div
                        className="relative w-full max-w-6xl overflow-hidden rounded-2xl border border-white/10 bg-black"
                        onClick={(event) => event.stopPropagation()}
                    >
                        <button
                            onClick={() => setLightboxIndex(null)}
                            className="absolute right-3 top-3 z-10 rounded-lg bg-black/60 px-3 py-2 text-sm font-semibold text-white transition-colors hover:bg-black/80"
                        >
                            Close
                        </button>
                        <div className="flex max-h-[85vh] items-center justify-center bg-black p-4">
                            <img
                                src={screenshots[lightboxIndex]}
                                alt={`${displayName} screenshot ${lightboxIndex + 1}`}
                                className="max-h-[80vh] w-full object-contain"
                            />
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
