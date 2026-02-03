import { useState, useEffect, useRef } from 'react';
import {
    ArrowLeft, Download, Play, Heart, Star, Code, X,
    AlertTriangle, Trash2, User, Globe, Calendar,
    ChevronRight, CheckCircle2,
    Loader2, ShieldCheck, MessageSquare, Cpu, ChevronDown, RefreshCw
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import DOMPurify from 'dompurify'; // Vector 1: HTML Injection Fix
import RepoSelector from '../components/RepoSelector';
import RepoBadge from '../components/RepoBadge';
import { Package } from '../components/PackageCard';
import { PackageSource } from '../types/alpm';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { clsx } from 'clsx';
import { resolveIconUrl } from '../utils/iconHelper';
import { useFavorites } from '../hooks/useFavorites';
import { submitReview } from '../services/reviewService';
import { useToast } from '../context/ToastContext';
import { useErrorService } from '../context/ErrorContext';
import archLogo from '../assets/arch-logo.svg';
import { usePackageReviews } from '../hooks/useRatings';
import { usePackageMetadata } from '../hooks/usePackageMetadata';
import { compareVersions } from '../utils/versionHelper';
import { useDistro } from '../hooks/useDistro';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { useFocusTrap } from '../hooks/useFocusTrap';

// --- Types ---
interface PackageDetailsProps {
    pkg: Package;
    onBack: () => void;
    preferredSource?: PackageSource | string;
    /** When true, disable Install/Uninstall to prevent concurrent ALPM operations. */
    installInProgress?: boolean;
    /** When set and name matches this pkg, show "Installing..." / "Uninstalling..." with spinner (no layout shift). */
    activeInstallPackage?: { name: string; mode: 'install' | 'uninstall' } | null;
    onInstall: (p: { name: string; source: PackageSource | string; repoName?: string }) => void;
    onUninstall: (p: { name: string; source: PackageSource | string; repoName?: string }) => void;
}

interface PackageVariant {
    source: PackageSource | string;
    version: string;
    repo_name?: string;
    pkg_name?: string;
}

interface InstallStatus {
    installed: boolean;
    version?: string;
    repo?: string;
    source?: PackageSource | string;
    actual_package_name?: string;
}


// --- Main Component ---

export default function PackageDetails({ pkg, onBack, preferredSource, installInProgress = false, activeInstallPackage = null, onInstall, onUninstall }: PackageDetailsProps) {
    const activeInstall = activeInstallPackage;
    // --- State & Hooks ---
    const { metadata: fullMeta } = usePackageMetadata(pkg.name);
    const { success } = useToast();
    const errorService = useErrorService();
    const { distro } = useDistro();

    const lookupId = pkg.app_id || fullMeta?.app_id || pkg.name;
    const { reviews, summary: rating, refresh: refreshReviews } = usePackageReviews(pkg.name, lookupId);

    const [variants, setVariants] = useState<PackageVariant[]>([]);
    const [selectedSource, setSelectedSource] = useState<PackageSource | string>(pkg.source);

    const isSameSource = (a: PackageSource | string, b: PackageSource | string) => {
        if (typeof a === 'string' && typeof b === 'string') return a === b;
        if (typeof a !== 'string' && typeof b !== 'string') return a.id === b.id && a.source_type === b.source_type;
        return false;
    };

    const [showReviewForm, setShowReviewForm] = useState(false);
    const [reviewTitle, setReviewTitle] = useState('');
    const [reviewBody, setReviewBody] = useState('');
    const [reviewRating, setReviewRating] = useState(5);
    const [isSubmittingReview, setIsSubmittingReview] = useState(false);

    // Pagination for reviews
    const [visibleReviewsCount, setVisibleReviewsCount] = useState(5);

    // Install/Status Logic
    const [installStatus, setInstallStatus] = useState<InstallStatus | null>(null);
    const [installedVariant, setInstalledVariant] = useState<InstallStatus | null>(null);
    const checkRequestId = useRef(0);

    // PKGBUILD Viewing
    const [showPkgbuild, setShowPkgbuild] = useState(false);
    useEscapeKey(() => setShowPkgbuild(false), showPkgbuild);
    const pkgbuildModalRef = useFocusTrap(showPkgbuild);
    const [pkgbuildContent, setPkgbuildContent] = useState<string | null>(null);
    const [pkgbuildLoading, setPkgbuildLoading] = useState(false);
    const [pkgbuildError, setPkgbuildError] = useState<string | null>(null);

    // Lightbox
    const [lightboxIndex, setLightboxIndex] = useState<number | null>(null);
    useEscapeKey(() => setLightboxIndex(null), lightboxIndex !== null);

    const { isFavorite, toggleFavorite } = useFavorites();
    const isFav = isFavorite(pkg.name);

    const reviewsRef = useRef<HTMLDivElement>(null);

    const scrollToReviews = () => {
        reviewsRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    };

    // --- Effects ---

    // 1. Fetch Variants & Initial Selection (use available_sources from unified search when present)
    useEffect(() => {
        // Pre-populate from pkg.available_sources when present (unified search pipeline)
        const fromAvailableSources = (pkg.available_sources || []).map(s => ({
            source: s,
            version: s.version,
            pkg_name: s.source_type === 'flatpak' ? (pkg.app_id || pkg.name) : pkg.name,
            repo_name: s.id === 'chaotic-aur' ? 'chaotic-aur' : undefined,
        } as PackageVariant));

        invoke<PackageVariant[]>('get_package_variants', { pkgName: pkg.name })
            .then(async (fetchedVars) => {
                const propAlternatives = (pkg.alternatives || []).map(a => ({
                    source: a.source,
                    version: a.version,
                    pkg_name: a.name,
                    repo_name: a.source === 'chaotic' ? 'chaotic-aur' : undefined
                } as PackageVariant));

                const combined = fetchedVars.length > 0
                    ? [...fetchedVars, ...propAlternatives]
                    : fromAvailableSources.length > 0
                        ? fromAvailableSources
                        : [...fetchedVars, ...propAlternatives];
                // Deduplicate
                let vars = combined.filter((v, index, self) =>
                    index === self.findIndex((t) => (
                        t.source === v.source && t.version === v.version && t.pkg_name === v.pkg_name
                    ))
                );
                // Repo availability: only show sources where the package actually exists (has a version)
                vars = vars.filter((v) => v.version != null && String(v.version).trim() !== '');
                setVariants(vars);

                // Check installed status to auto-select
                try {
                    const res = await invoke<InstallStatus>('check_installed_status', { name: pkg.name });
                    if (res.installed) {
                        setInstallStatus(res);
                        setInstalledVariant(res);
                        if (res.source) {
                            setSelectedSource(res.source);
                            return;
                        }
                        // When installed but backend has no source: prefer card's source (pkg.source) below
                    }
                } catch (e) { errorService.reportError(e as Error | string); }

                // Fallback selection: prefer card source so OFFICIAL on card shows OFFICIAL on details
                // Fallback selection: prefer card source so OFFICIAL on card shows OFFICIAL on details
                if (preferredSource && vars.some(v => isSameSource(v.source, preferredSource))) setSelectedSource(preferredSource);
                else if (vars.some(v => isSameSource(v.source, pkg.source))) setSelectedSource(pkg.source);
                else if (vars.some(v => typeof v.source === 'string' ? v.source === 'chaotic' : v.source.id === 'chaotic-aur')) setSelectedSource(vars.find(v => typeof v.source === 'string' ? v.source === 'chaotic' : v.source.id === 'chaotic-aur')!.source);
                else if (vars.some(v => typeof v.source === 'string' ? v.source === 'official' : v.source.id === 'core')) setSelectedSource(vars.find(v => typeof v.source === 'string' ? v.source === 'official' : v.source.id === 'core')!.source);
                else if (vars.length > 0) setSelectedSource(vars[0].source);
            });
    }, [pkg.name, preferredSource]);

    // 2. Status Checking Routine
    const checkStatus = (customName?: string) => {
        const reqId = ++checkRequestId.current;
        const nameToCheck = customName || installedVariant?.actual_package_name || installStatus?.actual_package_name || variants.find(v => isSameSource(v.source, selectedSource))?.pkg_name || pkg.name;

        invoke<InstallStatus>('check_installed_status', { name: nameToCheck })
            .then(res => {
                if (reqId !== checkRequestId.current) return;
                setInstallStatus(res);
                if (res.installed) setInstalledVariant(res);
            })
            .catch((e) => errorService.reportError(e));
    };

    useEffect(() => {
        checkStatus();
        const unlisten = listen('install-complete', () => checkStatus());
        return () => { unlisten.then((f: UnlistenFn) => f()); };
    }, [pkg.name, selectedSource, variants]);


    // --- Actions ---

    const handleInstallClick = () => {
        onInstall({
            name: variants.find(v => v.source === selectedSource)?.pkg_name || pkg.name,
            source: selectedSource,
            repoName: variants.find(v => v.source === selectedSource)?.repo_name
        });
    };

    const handleLaunch = async () => {
        const nameToLaunch = installedVariant?.actual_package_name || installStatus?.actual_package_name || variants.find(v => v.source === selectedSource)?.pkg_name || pkg.name;
        try {
            await invoke('launch_app', { pkgName: nameToLaunch });
            success("App launched");
        } catch (e) { errorService.reportError(e as Error | string); }
    };

    const handleReviewSubmit = async () => {
        if (!reviewBody.trim()) {
            errorService.reportError("Please write a comment.");
            return;
        }

        // Prevent duplicate (basic check)
        const alreadyReviewed = reviews.some(r => r.source === 'monarch' && r.comment.includes(reviewBody) && r.rating === reviewRating);
        if (alreadyReviewed) {
            success("You already submitted this review.");
            setShowReviewForm(false);
            return;
        }

        setIsSubmittingReview(true);
        try {
            // Note: We combine Title + Body because Supabase only has 'comment'. Ideally migration should separate them.
            const fullComment = reviewTitle ? `${reviewTitle}\n\n${reviewBody}` : reviewBody;

            await submitReview(pkg.name, reviewRating, fullComment, "MonArch User");
            setShowReviewForm(false);
            setReviewTitle(''); setReviewBody('');
            await refreshReviews(); // Force refresh via hook wrapper

            invoke('track_event', {
                event: 'review_submitted',
                payload: {
                    package: pkg.name,
                    rating: reviewRating,
                    rating_bucket: reviewRating <= 2 ? '1-2' : reviewRating <= 3 ? '3' : '4-5',
                },
            });
            success("Review submitted!");
        } catch (e) {
            errorService.reportError(e as Error | string);
        } finally {
            setIsSubmittingReview(false);
        }
    };

    const fetchPkgbuild = async () => {
        setPkgbuildLoading(true);
        setPkgbuildError(null);
        try {
            const content = await invoke<string>('fetch_pkgbuild', { pkgName: pkg.name });
            setPkgbuildContent(content);
            setShowPkgbuild(true);
        } catch (e) {
            errorService.reportError(e as Error | string);
            setPkgbuildError(String(e));
            setShowPkgbuild(true);
        } finally { setPkgbuildLoading(false); }
    };

    // --- Computed ---

    // Review Filtering & Sorting State
    const [sortOrder, setSortOrder] = useState<'newest' | 'oldest' | 'highest' | 'lowest'>('newest');
    const [filterRating, setFilterRating] = useState<number | null>(null);

    // Pipeline: pkg.screenshots first (passed from search), then metadata fetch
    const screenshots = (pkg.screenshots && pkg.screenshots.length > 0)
        ? pkg.screenshots
        : (fullMeta?.screenshots && fullMeta.screenshots.length > 0)
            ? fullMeta.screenshots
            : [];

    // Filter and Sort Logic
    const processedReviews = reviews
        .filter(r => filterRating === null || Math.round(r.rating) === filterRating)
        .sort((a, b) => {
            if (sortOrder === 'newest') return b.date.getTime() - a.date.getTime();
            if (sortOrder === 'oldest') return a.date.getTime() - b.date.getTime();
            if (sortOrder === 'highest') return b.rating - a.rating;
            if (sortOrder === 'lowest') return a.rating - b.rating;
            return 0;
        });

    const displayedReviews = processedReviews.slice(0, visibleReviewsCount);
    const hasMoreReviews = processedReviews.length > visibleReviewsCount;

    return (
        <motion.div
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            className="h-full flex flex-col bg-app-bg text-app-fg overflow-y-auto"
        >
            {/* --- HERO SECTION --- (pt-16 for compact top, h-auto to grow with content) */}
            <div className="relative h-auto md:min-h-[350px] pt-16 pb-10 md:pb-20 flex items-start z-30">
                {/* Background Gradient / Image */}
                <div className="absolute inset-0 z-0 overflow-hidden">
                    <div className="absolute inset-0 bg-gradient-to-b from-blue-900/40 to-app-bg z-10" />
                    <div className="absolute inset-0 bg-[url('/grid.svg')] opacity-10" />
                    {screenshots.length > 0 && (
                        <div className="absolute inset-0 blur-3xl opacity-30 scale-110">
                            <img src={screenshots[0]} alt="" className="w-full h-full object-cover" />
                        </div>
                    )}
                </div>

                {/* Back Button - Moved up to top-2 as requested */}
                <button
                    onClick={onBack}
                    className="absolute top-2 left-6 z-50 p-3 rounded-full bg-black/20 hover:bg-black/40 backdrop-blur-md text-white transition-all border border-white/10"
                >
                    <ArrowLeft size={24} />
                </button>

                {/* Header Info Container - Responsive layout */}
                <div className="relative z-20 w-full max-w-7xl mx-auto px-6 flex flex-col lg:flex-row items-start gap-6 lg:gap-10">

                    {/* Icon Card - Scalable & Robust */}
                    <motion.div
                        initial={{ scale: 0.9, opacity: 0 }}
                        animate={{ scale: 1, opacity: 1 }}
                        transition={{ delay: 0.1 }}
                        className="w-20 h-20 sm:w-24 sm:h-24 md:w-32 md:h-32 lg:w-48 lg:h-48 rounded-2xl md:rounded-3xl lg:rounded-4xl bg-app-card shadow-2xl shadow-black/50 border border-white/10 flex items-center justify-center p-2.5 md:p-4 lg:p-6 shrink-0 backdrop-blur-xl relative"
                    >
                        {(pkg.icon || fullMeta?.icon_url) ? (
                            <img src={resolveIconUrl(pkg.icon || fullMeta?.icon_url)} alt={pkg.name} className="w-full h-full object-contain filter drop-shadow-xl" />
                        ) : (
                            <img src={archLogo} className="w-full h-full object-contain opacity-80 grayscale dark:invert" alt="Arch Linux" />
                        )}
                    </motion.div>

                    {/* Text & Actions */}
                    <div className="flex-1 min-w-0 pb-1 w-full">
                        <motion.h1
                            initial={{ y: 20, opacity: 0 }}
                            animate={{ y: 0, opacity: 1 }}
                            transition={{ delay: 0.2 }}
                            className="text-4xl sm:text-5xl md:text-6xl lg:text-8xl font-black text-white tracking-tight leading-[1.1] md:leading-[0.9] mb-4 break-words max-w-3xl text-center lg:text-left"
                            style={{ textShadow: '0 0 20px rgba(0,0,0,0.8), 0 2px 4px rgba(0,0,0,0.9), 0 0 40px rgba(0,0,0,0.6)' }}
                        >
                            {pkg.display_name || fullMeta?.name || pkg.name}
                        </motion.h1>

                        <div className="flex flex-wrap items-center justify-center lg:justify-start gap-2 md:gap-4 mb-6 text-app-muted/80 font-medium">
                            <RepoBadge source={selectedSource} />
                            <div className="px-3 py-1 rounded-full bg-slate-100 dark:bg-white/5 border border-slate-200 dark:border-white/10 text-sm flex items-center gap-2 text-slate-700 dark:text-white/80">
                                <Cpu size={14} /> <span>v{variants.find(v => v.source === selectedSource)?.version || pkg.version}</span>
                            </div>

                            {/* Installed Badge - High Visibility (With Fallback) */}
                            {installedVariant?.installed && (
                                <div className="px-3 py-1 rounded-full bg-emerald-500/20 border border-emerald-500/50 text-emerald-400 text-sm font-bold flex items-center gap-2">
                                    <CheckCircle2 size={14} />
                                    <span>
                                        Installed: v{installedVariant.version}
                                        ({installedVariant.source ? (typeof installedVariant.source === 'string' ? installedVariant.source : installedVariant.source.label) : (installedVariant.repo || 'Unknown')})
                                    </span>
                                </div>
                            )}

                            <div className="px-3 py-1 rounded-full bg-slate-100 dark:bg-white/5 border border-slate-200 dark:border-white/10 text-sm flex items-center gap-2 text-slate-700 dark:text-white/80">
                                <MessageSquare size={14} /> <span>{reviews.length} Reviews</span>
                            </div>
                            {pkg.out_of_date && <span className="text-amber-400 flex items-center gap-1 font-bold"><AlertTriangle size={14} /> Outdated</span>}
                        </div>

                        {/* WARNINGS BLOCK - Compact and properly aligned */}
                        <div className="space-y-2 mb-6 max-w-2xl mx-auto lg:mx-0">

                            {(typeof selectedSource === 'string' ? selectedSource === 'aur' : selectedSource.source_type === 'aur') && (
                                <div className="flex items-center gap-3 px-4 py-3 rounded-xl bg-amber-500/10 border border-amber-500/20 backdrop-blur-sm">
                                    <AlertTriangle size={18} className="text-amber-500 shrink-0" />
                                    <div className="text-xs text-amber-200/80">
                                        <span className="font-bold text-amber-500">Community Package (AUR):</span> Not officially reviewed. Verify before installing.
                                    </div>
                                </div>
                            )}

                            {/* SAFETY NET: Cross-Pollination Warnings */}
                            {(() => {
                                const sourceId = typeof selectedSource === 'string' ? selectedSource.toLowerCase() : selectedSource.id;
                                // const sourceType = typeof selectedSource === 'string' ? 'repo' : selectedSource.source_type; // approx

                                const isArch = distro.id === 'arch' || distro.id === 'cachyos' || distro.id === 'endeavouros';
                                const isManjaro = distro.id === 'manjaro';

                                // Scenario A: Manjaro -> Chaotic/Arch (High Risk)
                                if (isManjaro && (sourceId === 'chaotic-aur' || sourceId === 'chaotic' || sourceId === 'core' || sourceId === 'extra')) {
                                    return (
                                        <div className="flex items-start gap-3 px-4 py-3 rounded-xl bg-red-500/10 border border-red-500/20 backdrop-blur-sm mt-2">
                                            <AlertTriangle size={18} className="text-red-500 shrink-0 mt-0.5" />
                                            <div className="text-xs text-red-200/80 leading-relaxed">
                                                <span className="font-bold text-red-500 block mb-0.5">⚠ High Risk (Glibc Mismatch)</span>
                                                Manjaro holds back core libraries (e.g. glibc) for stability. This package is built for Arch and may depend on newer versions—it can fail at runtime or break your system.
                                            </div>
                                        </div>
                                    );
                                }

                                // Scenario B: Arch -> Manjaro Repo (Compat Risk)
                                if (isArch && sourceId.includes('manjaro')) {
                                    return (
                                        <div className="flex items-start gap-3 px-4 py-3 rounded-xl bg-orange-500/10 border border-orange-500/20 backdrop-blur-sm mt-2">
                                            <AlertTriangle size={18} className="text-orange-500 shrink-0 mt-0.5" />
                                            <div className="text-xs text-orange-200/80 leading-relaxed">
                                                <span className="font-bold text-orange-500 block mb-0.5">⚠ Distro Mismatch</span>
                                                You are installing a Manjaro package on Arch. This may downgrade system packages (e.g. glibc, kernel) or introduce patched kernels not tested for your OS.
                                            </div>
                                        </div>
                                    );
                                }

                                return null;
                            })()}
                        </div>

                        {/* ACTION ROW */}
                        <motion.div
                            initial={{ y: 20, opacity: 0 }}
                            animate={{ y: 0, opacity: 1 }}
                            transition={{ delay: 0.3 }}
                            className="flex flex-col gap-4 w-full"
                        >
                            <div className="flex flex-col sm:flex-row sm:items-center gap-4 w-full">
                                {/* Repo Selector - Full Width on Mobile */}
                                {variants.length >= 1 && (
                                    <div className="relative z-50 w-full sm:w-auto sm:min-w-[300px]">
                                        <RepoSelector
                                            variants={variants}
                                            selectedSource={selectedSource}
                                            onChange={(s) => setSelectedSource(s as any)}
                                        />
                                    </div>
                                )}

                                {/* ACTION BUTTONS GROUP - Always together */}
                                <div className="flex flex-1 items-center justify-center sm:justify-start gap-3 flex-wrap">
                                    {(() => {
                                        // 1. Determine Identity
                                        const activeSource = selectedSource;
                                        const candidateVersion = variants.find(v => isSameSource(v.source, activeSource))?.version || pkg.version;

                                        // 2. Determine State
                                        const isInstalled = installedVariant?.installed;
                                        // Robust Source Check: Use source OR repo as fallback
                                        const installedSourceRaw = installedVariant?.source
                                            ? (typeof installedVariant.source === 'string' ? installedVariant.source : installedVariant.source.id)
                                            : installedVariant?.repo || '';

                                        const activeSourceId = typeof activeSource === 'string' ? activeSource : activeSource.id;
                                        const isSourceMismatch = isInstalled && installedSourceRaw && installedSourceRaw.toLowerCase() !== activeSourceId.toLowerCase();

                                        const isUpdateAvailable = isInstalled && !isSourceMismatch && installedVariant?.version && candidateVersion && compareVersions(candidateVersion, installedVariant.version) > 0;

                                        if (isInstalled) {
                                            return (
                                                <>
                                                    {/* Launch Button */}
                                                    <button
                                                        onClick={handleLaunch}
                                                        disabled={installInProgress}
                                                        className={clsx(
                                                            "h-14 px-8 rounded-2xl font-bold shadow-xl active:scale-95 transition-all flex items-center gap-3 text-lg border",
                                                            (isUpdateAvailable || isSourceMismatch)
                                                                ? "bg-slate-100 hover:bg-slate-200 dark:bg-white/10 dark:hover:bg-white/20 text-slate-900 dark:text-white border-slate-200 dark:border-white/10"
                                                                : "bg-emerald-500 hover:bg-emerald-400 text-white border-emerald-500/20 shadow-emerald-500/20"
                                                        )}
                                                    >
                                                        <Play size={24} fill="currentColor" /> Launch
                                                    </button>

                                                    {/* Update Button */}
                                                    {!isSourceMismatch && isUpdateAvailable && (() => {
                                                        const isThisUpdating = activeInstall?.name === pkg.name && activeInstall?.mode === 'install';
                                                        return (
                                                            <button
                                                                onClick={handleInstallClick}
                                                                disabled={installInProgress}
                                                                className="h-14 min-w-[12rem] px-8 bg-blue-600 hover:bg-blue-500 text-white rounded-2xl font-bold shadow-xl shadow-blue-600/20 active:scale-95 transition-all flex items-center justify-center gap-3 text-lg disabled:opacity-50 disabled:cursor-not-allowed"
                                                            >
                                                                {isThisUpdating ? <Loader2 size={24} className="animate-spin shrink-0" /> : <Download size={24} />}
                                                                <span className="truncate">{isThisUpdating ? "Updating…" : `Update (v${candidateVersion})`}</span>
                                                            </button>
                                                        );
                                                    })()}

                                                    {/* Uninstall Button */}
                                                    {(() => {
                                                        const isThisUninstalling = activeInstall?.name === (installedVariant?.actual_package_name || pkg.name) && activeInstall?.mode === 'uninstall';
                                                        return (
                                                            <button
                                                                onClick={() => onUninstall({
                                                                    name: installedVariant?.actual_package_name || pkg.name,
                                                                    source: installedVariant?.source || installedVariant?.repo || 'official'
                                                                })}
                                                                disabled={installInProgress}
                                                                className="h-14 min-w-[10rem] px-6 bg-slate-100 hover:bg-slate-200 dark:bg-white/5 dark:hover:bg-white/10 text-red-600 dark:text-red-400 border border-slate-200 dark:border-white/10 rounded-2xl font-bold active:scale-95 transition-all flex items-center justify-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
                                                            >
                                                                {isThisUninstalling ? <Loader2 size={20} className="animate-spin shrink-0" /> : <Trash2 size={20} />}
                                                                <span className="truncate">{isThisUninstalling ? "Uninstalling…" : "Uninstall"}</span>
                                                            </button>
                                                        );
                                                    })()}
                                                </>
                                            );
                                        } else {
                                            const sourceId = typeof selectedSource === 'string' ? selectedSource.toLowerCase() : selectedSource.id;
                                            const isManjaro = distro.id === 'manjaro';
                                            const isRisky = isManjaro && (sourceId === 'chaotic-aur' || sourceId === 'chaotic' || sourceId === 'core' || sourceId === 'extra');

                                            const isThisPackageInstalling = activeInstall?.name === pkg.name && activeInstall?.mode === 'install';
                                            return (
                                                <button
                                                    onClick={handleInstallClick}
                                                    disabled={installInProgress}
                                                    className={clsx(
                                                        "h-14 min-w-[12rem] px-10 rounded-2xl font-bold shadow-xl active:scale-95 transition-all flex items-center justify-center gap-3 text-lg border",
                                                        isRisky
                                                            ? "bg-amber-600 hover:bg-amber-500 text-white shadow-amber-600/20 border-amber-500/20"
                                                            : "bg-blue-600 hover:bg-blue-500 text-white shadow-blue-600/20 border-white/10",
                                                        "disabled:opacity-50 disabled:cursor-not-allowed"
                                                    )}
                                                >
                                                    {isThisPackageInstalling ? (
                                                        <Loader2 size={24} className="animate-spin shrink-0" aria-hidden />
                                                    ) : (
                                                        <Download size={24} className="shrink-0" />
                                                    )}
                                                    <span className="truncate">{isThisPackageInstalling ? "Installing…" : isRisky ? "Install (Unsafe)" : "Install"}</span>
                                                </button>
                                            );
                                        }
                                    })()}

                                    {/* Restore Missing Favorite Button */}
                                    <button
                                        onClick={() => toggleFavorite(pkg.name)}
                                        className={clsx(
                                            "h-14 w-14 rounded-2xl border flex items-center justify-center transition-colors active:scale-95 shrink-0",
                                            isFav ? "bg-red-500/20 border-red-500/50 text-red-500" : "bg-slate-100 dark:bg-white/5 border-slate-200 dark:border-white/10 text-slate-400 dark:text-white/50 hover:bg-slate-200 dark:hover:bg-white/10 hover:text-red-500 dark:hover:text-white"
                                        )}
                                        title={isFav ? "Remove from Favorites" : "Add to Favorites"}
                                    >
                                        <Heart size={24} className={isFav ? "fill-current" : ""} />
                                    </button>

                                    {(typeof selectedSource === 'string' ? selectedSource === 'aur' : selectedSource.source_type === 'aur') && (
                                        <button onClick={fetchPkgbuild} className="h-14 w-14 rounded-2xl border border-slate-200 dark:border-white/10 bg-slate-100 dark:bg-white/5 flex items-center justify-center text-slate-500 dark:text-white/50 hover:text-slate-900 dark:hover:text-white hover:bg-slate-200 dark:hover:bg-white/10 transition-colors shrink-0" title="View PKGBUILD">
                                            {pkgbuildLoading ? <Loader2 size={24} className="animate-spin" /> : <Code size={24} />}
                                        </button>
                                    )}
                                </div>
                            </div>

                            {/* CONFLICT / SWITCH UI (Compact) */}
                            {(() => {
                                const installedSourceRaw = installedVariant?.source
                                    ? (typeof installedVariant.source === 'string' ? installedVariant.source : installedVariant.source.id)
                                    : installedVariant?.repo || '';

                                const activeSourceId = typeof selectedSource === 'string' ? selectedSource : selectedSource.id;
                                const isSourceMismatch = installedVariant?.installed && installedSourceRaw && installedSourceRaw.toLowerCase() !== activeSourceId.toLowerCase();

                                if (isSourceMismatch) {
                                    return (
                                        <div className="w-full flex items-center justify-between gap-4 p-3 rounded-xl bg-app-bg/50 border border-app-border backdrop-blur-sm animate-in fade-in slide-in-from-top-2">
                                            <div className="flex items-center gap-3">
                                                <div className="shrink-0 text-red-500">
                                                    <AlertTriangle size={20} />
                                                </div>
                                                <div className="text-sm">
                                                    <span className="font-bold text-app-fg block">Version Conflict</span>
                                                    <span className="text-app-muted text-xs">Installed: <b>{installedSourceRaw}</b> vs Selected: <b>{typeof selectedSource === 'string' ? selectedSource : selectedSource.label}</b></span>
                                                </div>
                                            </div>
                                            <button
                                                onClick={() => {
                                                    const v = variants.find(variant => isSameSource(variant.source, selectedSource));
                                                    onInstall({
                                                        name: pkg.name,
                                                        source: selectedSource,
                                                        repoName: v?.repo_name
                                                    });
                                                }}
                                                disabled={installInProgress}
                                                className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-xs font-bold rounded-lg shadow-lg shadow-blue-500/20 transition-all active:scale-95 flex items-center gap-2 whitespace-nowrap disabled:opacity-50 disabled:cursor-not-allowed"
                                            >
                                                <RefreshCw size={14} /> Switch Source
                                            </button>
                                        </div>
                                    );
                                }
                                return null;
                            })()}

                        </motion.div>
                    </div>
                </div>
            </div>

            {/* --- MAIN CONTENT GRID --- */}
            <div className="flex-1 bg-app-bg">
                <div className="max-w-7xl mx-auto p-6 lg:p-10">
                    <div className="grid grid-cols-1 lg:grid-cols-12 gap-8 lg:gap-12">
                        {/* LEFT COLUMN (Details) */}
                        <div className="col-span-1 lg:col-span-8 space-y-8">

                            {/* NEW: Side-by-Side Metadata & Ratings Header - Forced grid-cols-2 */}
                            <div className="grid grid-cols-2 gap-3 md:gap-6">
                                <motion.div
                                    whileHover={{ scale: 1.01 }}
                                    whileTap={{ scale: 0.98 }}
                                    onClick={scrollToReviews}
                                    className="bg-gradient-to-br from-yellow-500/10 to-transparent rounded-3xl p-4 md:p-6 border border-yellow-500/20 cursor-pointer group transition-all"
                                >
                                    <h4 className="text-[10px] md:text-xs font-bold text-yellow-500 uppercase tracking-widest mb-1 md:mb-3 flex justify-between">
                                        Community Rating
                                        <ChevronRight size={14} className="hidden md:block opacity-0 group-hover:opacity-100 transition-opacity" />
                                    </h4>
                                    <div className="flex flex-col sm:flex-row items-start sm:items-center gap-2 sm:gap-4">
                                        <span className="text-3xl sm:text-5xl font-black text-slate-900 dark:text-white">{rating?.average.toFixed(1) || "0.0"}</span>
                                        <div className="flex flex-col">
                                            <div className="flex gap-0.5 md:gap-1 mb-1">
                                                {[1, 2, 3, 4, 5].map(s => <Star key={s} size={10} className="md:w-3.5 md:h-3.5 text-yellow-500" fill={s <= Math.round(rating?.average || 0) ? "#EAB308" : "none"} />)}
                                            </div>
                                            <span className="text-[9px] md:text-xs text-app-muted whitespace-nowrap">{rating?.count || 0} reviews</span>
                                        </div>
                                    </div>
                                </motion.div>

                                {/* Metadata Grid - Side Box */}
                                <div className="bg-white/50 dark:bg-app-card/40 rounded-3xl p-1 border border-slate-200 dark:border-white/5 overflow-hidden">
                                    <div className="grid grid-cols-1 divide-y divide-slate-100 dark:divide-white/5">
                                        <div className="px-3 py-2 md:px-4 md:py-3 flex items-center justify-between gap-2 md:gap-4">
                                            <span className="text-[10px] md:text-xs text-app-muted flex items-center gap-1.5 md:gap-2 shrink-0"><User size={12} className="md:w-[14px] md:h-[14px] text-blue-500" /> Maintainer</span>
                                            <span className="text-[10px] md:text-xs text-slate-900 dark:text-white font-medium truncate max-w-[80px] sm:max-w-[150px]">{fullMeta?.maintainer || "Community"}</span>
                                        </div>
                                        <div className="px-3 py-2 md:px-4 md:py-3 flex items-center justify-between gap-2 md:gap-4">
                                            <span className="text-[10px] md:text-xs text-app-muted flex items-center gap-1.5 md:gap-2 shrink-0"><ShieldCheck size={12} className="md:w-[14px] md:h-[14px] text-emerald-500" /> License</span>
                                            <span className="text-[10px] md:text-xs text-slate-900 dark:text-white font-medium truncate max-w-[80px] sm:max-w-[150px]">{fullMeta?.license || "Unknown"}</span>
                                        </div>
                                        <div className="px-3 py-2 md:px-4 md:py-3 flex items-center justify-between gap-2 md:gap-4">
                                            <span className="text-[10px] md:text-xs text-app-muted flex items-center gap-1.5 md:gap-2 shrink-0"><Calendar size={12} className="md:w-[14px] md:h-[14px] text-purple-500" /> Updated</span>
                                            <span className="text-[10px] md:text-xs text-slate-900 dark:text-white font-medium whitespace-nowrap">
                                                {fullMeta?.last_updated ? new Date(fullMeta.last_updated * 1000).toLocaleDateString() : 'Unknown'}
                                            </span>
                                        </div>
                                    </div>
                                </div>
                            </div>
                            {/* SCREENSHOTS GALLERY */}
                            <section>
                                <h3 className="text-xl font-bold text-white mb-6 flex items-center gap-2">
                                    <Globe size={24} className="text-blue-500" /> Preview
                                </h3>
                                {screenshots.length > 0 ? (
                                    <div className="flex gap-4 overflow-x-auto pb-6 snap-x scrollbar-thin scrollbar-thumb-white/10 scrollbar-track-transparent">
                                        {screenshots.map((url, i) => (
                                            <motion.div
                                                key={i}
                                                whileHover={{ scale: 1.02 }}
                                                onClick={() => setLightboxIndex(i)}
                                                className="shrink-0 w-[400px] aspect-video rounded-2xl overflow-hidden bg-slate-100 dark:bg-black/20 border border-slate-200 dark:border-white/10 cursor-pointer snap-center shadow-xl"
                                            >
                                                <img
                                                    src={url}
                                                    alt="Screenshot"
                                                    className="w-full h-full object-cover"
                                                    loading="lazy"
                                                    onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }}
                                                />
                                            </motion.div>
                                        ))}
                                    </div>
                                ) : (
                                    <div
                                        className="flex items-center justify-center w-full aspect-video max-h-[320px] rounded-2xl bg-gradient-to-br from-slate-800/50 to-slate-900/30 border border-slate-700/50 dark:border-white/10"
                                        style={{
                                            backgroundImage: 'radial-gradient(circle at 50% 50%, rgba(100,116,139,0.15) 0%, transparent 70%), linear-gradient(135deg, rgba(30,41,59,0.5) 0%, rgba(15,23,42,0.5) 100%)',
                                        }}
                                    >
                                        <div className="flex flex-col items-center gap-3 text-app-muted">
                                            <Globe size={48} className="opacity-40" />
                                            <span className="text-sm font-medium">No screenshots available</span>
                                            <span className="text-xs opacity-70">Preview images may be added in a future update</span>
                                        </div>
                                    </div>
                                )}
                            </section>

                            {/* DESCRIPTION */}
                            <section>
                                <h3 className="text-xl font-bold text-white mb-6">About this App</h3>
                                <div className="bg-app-card/30 rounded-3xl p-8 border border-white/5 leading-loose max-w-3xl mx-auto lg:mx-0">
                                    {/* VECTOR 1: HTML INJECTION DEFENSE */}
                                    <div
                                        className="prose prose-sm md:prose-base dark:prose-invert max-w-none text-slate-600 dark:text-white/70"
                                        dangerouslySetInnerHTML={{
                                            __html: DOMPurify.sanitize(fullMeta?.description || pkg.description || "No description available.")
                                        }}
                                    />
                                    {pkg.keywords && (
                                        <div className="flex flex-wrap gap-2 mt-8 pt-6 border-t border-white/5">
                                            {pkg.keywords.map(k => (
                                                <span key={k} className="px-3 py-1 bg-white/5 rounded-lg text-xs font-mono text-blue-300 border border-blue-500/20">#{k}</span>
                                            ))}
                                        </div>
                                    )}
                                </div>
                            </section>

                            {/* REVIEWS TAB - Attached reviewsRef for autoscroll */}
                            {/* REVIEWS TAB - Attached reviewsRef for autoscroll */}
                            <section ref={reviewsRef}>
                                <div className="flex flex-col md:flex-row md:items-center justify-between mb-8 gap-4">
                                    <h3 className="text-xl font-bold text-white">User Reviews ({reviews.length})</h3>

                                    <div className="flex items-center gap-3">
                                        {/* Sort Control */}
                                        <select
                                            value={sortOrder}
                                            onChange={(e) => setSortOrder(e.target.value as any)}
                                            className="bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm text-white outline-none focus:border-blue-500 transition-colors"
                                        >
                                            <option value="newest" className="bg-gray-800 text-white">Newest First</option>
                                            <option value="oldest" className="bg-gray-800 text-white">Oldest First</option>
                                            <option value="highest" className="bg-gray-800 text-white">Highest Rated</option>
                                            <option value="lowest" className="bg-gray-800 text-white">Lowest Rated</option>
                                        </select>

                                        {/* Filter Control */}
                                        <select
                                            value={filterRating || ''}
                                            onChange={(e) => setFilterRating(e.target.value ? Number(e.target.value) : null)}
                                            className="bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm text-white outline-none focus:border-blue-500 transition-colors"
                                        >
                                            <option value="" className="bg-gray-800 text-white">All Stars</option>
                                            <option value="5" className="bg-gray-800 text-white">5 Stars</option>
                                            <option value="4" className="bg-gray-800 text-white">4 Stars</option>
                                            <option value="3" className="bg-gray-800 text-white">3 Stars</option>
                                            <option value="2" className="bg-gray-800 text-white">2 Stars</option>
                                            <option value="1" className="bg-gray-800 text-white">1 Star</option>
                                        </select>

                                        <button onClick={() => setShowReviewForm(true)} className="px-4 py-2 bg-blue-600/10 text-blue-400 rounded-lg hover:bg-blue-600/20 font-bold transition-colors whitespace-nowrap">Write a Review</button>
                                    </div>
                                </div>

                                {/* Review Form */}
                                {showReviewForm && (
                                    <div className="bg-app-card p-6 rounded-2xl border border-blue-500/30 mb-8 animate-in slide-in-from-top-4">
                                        <h4 className="font-bold text-white mb-4">Write your review</h4>
                                        <div className="flex gap-2 mb-4">
                                            {[1, 2, 3, 4, 5].map(s => <Star key={s} size={28} fill={s <= reviewRating ? "#fbbf24" : "none"} className={s <= reviewRating ? "text-amber-400" : "text-zinc-600 stroke-2"} onClick={() => setReviewRating(s)} />)}
                                        </div>
                                        <input value={reviewTitle} onChange={e => setReviewTitle(e.target.value)} placeholder="Title (e.g. Works great!)" className="w-full bg-black/20 border border-white/10 rounded-xl p-4 mb-3 text-white focus:border-blue-500 outline-none transition-colors" />
                                        <textarea value={reviewBody} onChange={e => setReviewBody(e.target.value)} placeholder="Share your experience..." className="w-full bg-black/20 border border-white/10 rounded-xl p-4 mb-3 text-white focus:border-blue-500 outline-none transition-colors" rows={4} />
                                        <div className="flex justify-end gap-3">
                                            <button onClick={() => setShowReviewForm(false)} className="px-6 py-2 text-zinc-400 hover:text-white transition-colors">Cancel</button>
                                            <button
                                                onClick={handleReviewSubmit}
                                                disabled={isSubmittingReview}
                                                className="px-8 py-2 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-xl font-bold shadow-lg shadow-blue-600/20 flex items-center gap-2"
                                            >
                                                {isSubmittingReview ? <Loader2 size={16} className="animate-spin" /> : null}
                                                {isSubmittingReview ? "Submitting..." : "Submit"}
                                            </button>
                                        </div>
                                    </div>
                                )}

                                {/* Review List - PAGINATED */}
                                <div className="space-y-4">
                                    {reviews.length === 0 ? (
                                        <div className="p-8 text-center bg-white/5 rounded-2xl border border-white/5 text-app-muted">
                                            No reviews yet. Be the first to share your thoughts!
                                        </div>
                                    ) : (
                                        displayedReviews.map((review, idx) => (
                                            <div key={idx} className="p-6 bg-app-card rounded-2xl border border-white/5 hover:border-white/10 transition-colors">
                                                <div className="flex justify-between items-start mb-2">
                                                    <div className="flex items-center gap-3">
                                                        <div className="w-8 h-8 rounded-full bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center text-xs font-bold text-white shadow-lg shrink-0">
                                                            {review.userName.charAt(0)}
                                                        </div>
                                                        <div className="flex flex-col">
                                                            <div className="flex items-center gap-2">
                                                                <span className="font-bold text-white text-sm">{review.userName}</span>
                                                                {review.source === 'monarch' ? (
                                                                    <span className="px-1.5 py-0.5 rounded-md bg-purple-500/20 text-purple-300 text-[10px] font-bold border border-purple-500/30 uppercase tracking-wide">MonArch</span>
                                                                ) : (
                                                                    <span className="px-1.5 py-0.5 rounded-md bg-blue-500/20 text-blue-300 text-[10px] font-bold border border-blue-500/30 uppercase tracking-wide">ODRS</span>
                                                                )}
                                                            </div>
                                                            <span className="text-[10px] text-app-muted">{review.date ? new Date(review.date).toLocaleDateString() : 'Unknown Date'}</span>
                                                        </div>
                                                    </div>
                                                    <div className="flex gap-0.5">
                                                        {[1, 2, 3, 4, 5].map(s => <Star key={s} size={14} fill={s <= review.rating ? "#fbbf24" : "none"} className={s <= review.rating ? "text-amber-400" : "text-zinc-700"} />)}
                                                    </div>
                                                </div>
                                                {/* We don't have a distinct separate title field in the interface unless we parse it. For now, showing comment. */}
                                                <p className="text-app-fg/80 text-sm leading-relaxed whitespace-pre-line mt-3 pl-11">{review.comment}</p>
                                            </div>
                                        ))
                                    )}

                                    {/* Show More Button */}
                                    {hasMoreReviews && (
                                        <div className="pt-4 flex justify-center">
                                            <button
                                                onClick={() => setVisibleReviewsCount(prev => prev + 5)}
                                                className="px-6 py-3 rounded-xl bg-white/5 border border-white/10 hover:bg-white/10 text-white font-medium flex items-center gap-2 transition-colors"
                                            >
                                                Show More Reviews <ChevronDown size={16} />
                                            </button>
                                        </div>
                                    )}
                                </div>
                            </section>
                        </div>
                    </div>
                </div>
            </div>

            {/* LIGHTBOX PORTAL */}
            <AnimatePresence>
                {lightboxIndex !== null && (
                    <motion.div
                        initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
                        onClick={() => setLightboxIndex(null)}
                        className="fixed inset-0 z-40 bg-black/95 backdrop-blur-xl flex items-center justify-center p-4"
                    >
                        <button onClick={() => setLightboxIndex(null)} className="absolute top-6 right-6 p-4 text-white/50 hover:text-white" aria-label="Close"><X size={32} /></button>
                        <img
                            src={screenshots[lightboxIndex]}
                            className="max-h-[90vh] max-w-[90vw] rounded-lg shadow-2xl"
                            onClick={e => e.stopPropagation()}
                        />
                    </motion.div>
                )}
            </AnimatePresence>

            {/* PKGBUILD Modal */}
            <AnimatePresence>
                {showPkgbuild && (
                    <motion.div
                        initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
                        className="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4"
                        onClick={() => setShowPkgbuild(false)}
                    >
                        <motion.div
                            ref={pkgbuildModalRef}
                            initial={{ scale: 0.9 }} animate={{ scale: 1 }} exit={{ scale: 0.9 }}
                            onClick={e => e.stopPropagation()}
                            className="bg-app-card w-full max-w-4xl h-[80vh] rounded-2xl border border-white/10 flex flex-col overflow-hidden shadow-2xl"
                            role="dialog"
                            aria-modal="true"
                            aria-labelledby="pkgbuild-modal-title"
                        >
                            <div className="p-4 border-b border-white/10 flex justify-between items-center bg-white/5">
                                <h3 id="pkgbuild-modal-title" className="font-bold text-white flex items-center gap-2"><Code size={20} className="text-blue-400" /> PKGBUILD Preview</h3>
                                <button onClick={() => setShowPkgbuild(false)} aria-label="Close"><X size={24} className="text-white/50 hover:text-white" /></button>
                            </div>
                            <div className="flex-1 overflow-auto p-4 bg-[#1e1e1e]">
                                {pkgbuildLoading ? (
                                    <div className="h-full flex flex-col items-center justify-center text-white/50 gap-4">
                                        <Loader2 size={40} className="animate-spin text-blue-500" />
                                        <p>Fetching PKGBUILD...</p>
                                    </div>
                                ) : pkgbuildError ? (
                                    <div className="h-full flex flex-col items-center justify-center text-red-400 gap-4 p-8 text-center">
                                        <AlertTriangle size={40} />
                                        <p>{pkgbuildError}</p>
                                    </div>
                                ) : (
                                    <pre className="font-mono text-sm text-gray-300 whitespace-pre-wrap">{pkgbuildContent}</pre>
                                )}
                            </div>
                        </motion.div>
                    </motion.div>
                )}
            </AnimatePresence>
        </motion.div>
    );
}
