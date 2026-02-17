import React, { useEffect, useMemo, useState, useRef, useCallback } from 'react';
import { Download, Heart, Zap, Settings } from 'lucide-react';
import { motion } from 'framer-motion';
import { useFavorites } from '../hooks/useFavorites';
import { clsx } from 'clsx';
import { resolveIconUrl } from '../utils/iconHelper';
import RepoBadge from './RepoBadge';
import PackageCardSkeleton from './PackageCardSkeleton';
import { useAppStore } from '../store/internal_store';

import type { Package, PackageSource } from '../services/bindings';
import { getBestSource, getSourceTier, toPackageSource } from '../utils/repoHelper';
import { useDistro } from '../hooks/useDistro';
import archLogo from '../assets/arch-logo.png';



interface PackageCardProps {
    pkg?: Package;
    pkgId?: string;
    onClick: (pkg: Package) => void;
    skipMetadataFetch?: boolean;
    setupRequired?: boolean;
    onConfigureSource?: () => void;
}

/* ─── Pure display card ───────────────────────────────────── */



const PackageCardInner: React.FC<PackageCardProps & { pkg: Package }> = ({
    pkg,
    onClick,
    setupRequired = false,
    onConfigureSource,
}) => {
    const { distro } = useDistro();
    const distroId = typeof distro?.id === 'string' ? distro.id : (distro?.id as any)?.unknown ?? '';

    /* ── Derive best source from pre-enriched available_sources ── */
    const effectiveSources: PackageSource[] = useMemo(() => {
        const raw = (pkg.available_sources && pkg.available_sources.length > 0)
            ? pkg.available_sources
            : [pkg.source, ...(pkg.alternatives || []).map((a) => a.source)];
        const sources = raw.map(s => toPackageSource(s));
        const seen = new Set<string>();
        return sources.filter((s) => {
            const key = `${s.source_type}:${s.id}`;
            if (seen.has(key)) return false;
            seen.add(key);
            return true;
        }).sort((a, b) => getSourceTier(b, distroId) - getSourceTier(a, distroId));
    }, [pkg.available_sources, pkg.source, pkg.alternatives, distroId]);

    const bestSource = getBestSource(effectiveSources, distroId) ?? toPackageSource(pkg.source);
    const additionalCount = Math.max(0, effectiveSources.length - 1);

    /* ── Icon: read from pre-enriched pkg.icon (no per-card fetch) ── */
    const iconUrl = resolveIconUrl(pkg.icon || null);
    const [imgError, setImgError] = useState(false);
    useEffect(() => { setImgError(false); }, [iconUrl]);

    /* ── Rating: Restored ── */
    const rating = pkg.rating;
    const ratingAvg = rating && rating.total > 0
        ? ((rating.star1 + rating.star2 * 2 + rating.star3 * 3 + rating.star4 * 4 + rating.star5 * 5) / rating.total)
        : 0;

    /* ── Favorites (lightweight local-only hook) ── */
    const { toggleFavorite, isFavorite } = useFavorites();
    const isFav = isFavorite(pkg.name);

    /* ── Display name ── */
    const displayName = pkg.display_name || pkg.name;
    const showPkgName = pkg.display_name && pkg.display_name.toLowerCase() !== pkg.name.toLowerCase();

    /* ── Intersection Observer: defer rendering until near viewport ── */
    const cardRef = useRef<HTMLDivElement>(null);
    const [isVisible, setIsVisible] = useState(false);

    useEffect(() => {
        const el = cardRef.current;
        if (!el) return;
        const observer = new IntersectionObserver(
            ([entry]) => { if (entry.isIntersecting) { setIsVisible(true); observer.disconnect(); } },
            { rootMargin: '200px' }
        );
        observer.observe(el);
        return () => observer.disconnect();
    }, []);

    const syncRegistry = useAppStore((s) => s.syncRegistry);

    const handleMouseEnter = useCallback(() => {
        // Prefetch details on hover for instant feel
        syncRegistry([pkg.name]);
    }, [pkg.name, syncRegistry]);

    const handleClick = useCallback(() => onClick(pkg), [onClick, pkg]);
    const handleFavorite = useCallback((e: React.MouseEvent) => {
        e.stopPropagation();
        toggleFavorite(pkg.name);
    }, [toggleFavorite, pkg.name]);
    const handleConfigure = useCallback((e: React.MouseEvent) => {
        e.stopPropagation();
        onConfigureSource?.();
    }, [onConfigureSource]);

    return (
        <motion.div
            ref={cardRef}
            onClick={handleClick}
            onMouseEnter={handleMouseEnter}
            layout
            initial={{ opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.98 }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            className="group relative bg-app-card dark:bg-black/20 border border-app-border rounded-2xl p-5 hover:bg-app-card/80 dark:hover:bg-black/40 transition-all duration-300 hover:border-blue-300/50 dark:hover:border-white/10 hover:-translate-y-1 hover:shadow-xl dark:hover:shadow-2xl shadow-sm dark:shadow-none cursor-pointer overflow-hidden flex flex-col h-full min-w-[260px] max-w-full backdrop-blur-md card-gpu"
        >
            {!isVisible ? (
                /* Matches PackageCardSkeleton exactly to prevent layout shift */
                <div className="flex flex-col gap-3 animate-pulse h-[180px]">
                    <div className="flex items-center gap-3">
                        <div className="w-12 h-12 rounded-xl bg-slate-200/50 dark:bg-white/5 shrink-0" />
                        <div className="flex-1 space-y-2">
                            <div className="h-4 bg-slate-200/50 dark:bg-white/5 rounded w-3/4" />
                            <div className="h-3 bg-slate-200/50 dark:bg-white/5 rounded w-1/3" />
                        </div>
                    </div>
                    <div className="space-y-2 mb-auto">
                        <div className="h-3 bg-slate-200/50 dark:bg-white/5 rounded w-full" />
                        <div className="h-3 bg-slate-200/50 dark:bg-white/5 rounded w-[90%]" />
                    </div>
                    <div className="pt-3 border-t border-app-border/50 flex items-center justify-between">
                        <div className="h-5 w-12 bg-slate-200/50 dark:bg-white/5 rounded-lg" />
                        <div className="w-8 h-8 bg-slate-200/50 dark:bg-white/5 rounded-xl" />
                    </div>
                </div>
            ) : (
                <>
                    {/* ── Header: Icon + Name + Best Source Badge ── */}
                    <div className="flex items-start gap-3 mb-3">
                        <div className={clsx(
                            "w-12 h-12 rounded-xl flex items-center justify-center shrink-0 overflow-hidden transition-colors",
                            "text-slate-800 dark:text-white",
                            (!iconUrl || imgError)
                                ? "bg-slate-100 dark:bg-white/5 border border-slate-200 dark:border-white/10 p-1.5"
                                : "bg-transparent"
                        )}>
                            {iconUrl && !imgError ? (
                                <img
                                    src={iconUrl}
                                    alt={displayName}
                                    className="w-full h-full object-contain p-0.5 drop-shadow-md"
                                    loading="lazy"
                                    onError={() => setImgError(true)}
                                />
                            ) : (
                                <img
                                    src={archLogo}
                                    className="w-full h-full object-contain opacity-80 grayscale group-hover:grayscale-0 transition-all dark:invert"
                                    alt="Arch Linux"
                                />
                            )}
                        </div>

                        <div className="flex-1 min-w-0">
                            <h3
                                className="font-bold text-base leading-tight text-gray-900 dark:text-white group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors line-clamp-1 break-words"
                                title={displayName}
                            >
                                {displayName}
                            </h3>
                            {showPkgName && (
                                <span className="text-[10px] text-slate-500 dark:text-white/50 font-mono opacity-80 block truncate mt-0.5">
                                    {pkg.name}
                                </span>
                            )}
                        </div>

                        {/* Best source badge — top-right */}
                        <RepoBadge source={bestSource} compact />
                    </div>

                    {/* ── Description ── */}
                    <p className="text-sm text-gray-500 dark:text-gray-400 line-clamp-2 mb-auto min-h-[3rem] leading-relaxed">
                        {pkg.description}
                    </p>

                    {/* ── Footer: Rating + Sources + Actions ── */}
                    <div className="flex items-center justify-between mt-3 pt-3 border-t border-app-border/50 gap-2">
                        <div className="flex items-center gap-2 min-w-0 flex-1">
                            {/* Star rating (Removed in Iron Core Purge) */}
                            {rating && rating.total > 0 && (
                                <div className="flex items-center gap-1 bg-yellow-100 dark:bg-yellow-400/5 px-1.5 py-0.5 rounded-lg text-[10px] font-black text-yellow-600 dark:text-yellow-500 border border-yellow-200 dark:border-yellow-400/10 shrink-0">
                                    <span className="leading-none">★</span>
                                    <span className="tracking-tight">{ratingAvg.toFixed(1)}</span>
                                    <span className="text-[8px] opacity-70 font-medium">({rating.total})</span>
                                </div>
                            )}

                            {/* Optimized badge */}
                            {pkg.is_optimized && (
                                <div className="badge-hover px-1.5 py-0.5 rounded-full bg-amber-100 dark:bg-amber-500/10 border border-amber-200 dark:border-amber-500/20 text-amber-700 dark:text-amber-400 text-[9px] font-bold uppercase tracking-wider flex items-center gap-0.5 shrink-0">
                                    <Zap size={9} fill="currentColor" /> Opt
                                </div>
                            )}

                            {/* Installed badge */}
                            {pkg.installed && (
                                <span className="px-1.5 py-0.5 text-[9px] font-bold rounded-full bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 border border-emerald-500/25 shrink-0">
                                    Installed
                                </span>
                            )}

                            {/* Setup Required badge */}
                            {setupRequired && (
                                <span className="px-1.5 py-0.5 text-[9px] font-bold rounded-full bg-amber-500/15 text-amber-600 dark:text-amber-400 border border-amber-500/25 shrink-0">
                                    Setup
                                </span>
                            )}

                            {/* +N sources pill */}
                            {additionalCount > 0 && (
                                <span className="px-1.5 py-0.5 text-[9px] font-semibold rounded-full bg-blue-100 dark:bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-200 dark:border-blue-500/20 shrink-0">
                                    +{additionalCount} {additionalCount === 1 ? 'source' : 'sources'}
                                </span>
                            )}
                        </div>

                        {/* Action buttons */}
                        <div className="flex items-center gap-1.5 shrink-0">
                            <button
                                onClick={handleFavorite}
                                className={clsx(
                                    "p-2 rounded-xl transition-all border border-transparent active:scale-90",
                                    isFav
                                        ? "text-red-600 dark:text-red-500 bg-red-100 dark:bg-red-500/10 border-red-200 dark:border-red-500/20"
                                        : "text-slate-400 dark:text-white/50 bg-white dark:bg-white/5 hover:bg-red-500 hover:text-white"
                                )}
                                title={isFav ? "Remove from favorites" : "Add to favorites"}
                                aria-label={isFav ? "Remove from favorites" : "Add to favorites"}
                            >
                                <Heart size={14} fill={isFav ? "currentColor" : "none"} />
                            </button>

                            {setupRequired ? (
                                <button
                                    onClick={handleConfigure}
                                    className="p-2 rounded-xl bg-amber-500/80 hover:bg-amber-500 text-white transition-all active:scale-90 font-semibold flex items-center gap-1 text-[10px]"
                                    aria-label="Configure Source"
                                    title="Configure Chaotic-AUR in Settings"
                                >
                                    <Settings size={13} />
                                </button>
                            ) : (
                                <button
                                    className="p-2 rounded-xl bg-blue-600 hover:bg-blue-500 text-white transition-all active:scale-90 shadow-blue-900/20"
                                    aria-label="Install"
                                >
                                    <Download size={14} />
                                </button>
                            )}
                        </div>
                    </div>
                </>
            )}

            {/* Glow effect */}
            <div className="absolute inset-0 bg-gradient-to-br from-blue-500/5 to-purple-500/5 opacity-0 group-hover:opacity-100 pointer-events-none transition-opacity duration-500" />
        </motion.div>
    );
};

const PackageCardInnerMemo = React.memo(PackageCardInner);

/** PackageCard: pass pkgId to read from global registry, or pkg for direct data. */
const PackageCard: React.FC<PackageCardProps> = (props) => {
    const { pkg: pkgProp, pkgId, onClick } = props;
    const registryPkg = pkgId != null ? useAppStore((s) => s.packageRegistry[pkgId]) : undefined;

    // PRIORITY FIX: Prefer Registry Package if available, as it contains async updates (Ratings, etc.)
    // upsertPackages() merges search results into registry, so registry is always equal or better.
    const pkg = registryPkg ?? pkgProp;

    if (pkgId != null && pkg == null) return <PackageCardSkeleton />;
    if (pkg == null) return null;

    return (
        <PackageCardInnerMemo
            {...props}
            pkg={pkg}
            onClick={onClick}
        />
    );
};

export default PackageCard;
