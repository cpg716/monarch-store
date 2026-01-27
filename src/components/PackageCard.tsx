import React, { useEffect, useState } from 'react';
import { Download, ShieldCheck, Zap, Heart } from 'lucide-react';
import { motion } from 'framer-motion';
import { useFavorites } from '../hooks/useFavorites';
import { clsx } from 'clsx';
// import { invoke } from '@tauri-apps/api/core';
import { invoke } from '@tauri-apps/api/core';
import { resolveIconUrl } from '../utils/iconHelper';

export interface Package {
    name: string;
    display_name?: string;
    description: string;
    version: string;
    source: 'chaotic' | 'aur' | 'official' | 'cachyos' | 'garuda' | 'endeavour' | 'manjaro';
    maintainer?: string;
    votes?: number;
    url?: string; // Upstream URL
    license?: string[];
    keywords?: string[];
    last_modified?: number;
    first_submitted?: number;
    out_of_date?: number;
    num_votes?: number;
    icon?: string;
    app_id?: string;
    screenshots?: string[];
    is_optimized?: boolean;
    is_featured?: boolean;
    alternatives?: Package[];
}

interface PackageCardProps {
    pkg: Package;
    onClick: (pkg: Package) => void;
    skipMetadataFetch?: boolean;
    chaoticInfo?: ChaoticPackage | null;
}

export interface ChaoticPackage {
    id: number;
    pkgname: string;
    lastUpdated?: string;
    version?: string;
    metadata?: {
        buildDate?: string;
    }
}

import { usePackageRating } from '../hooks/useRatings';

import { usePackageMetadata } from '../hooks/usePackageMetadata';

import archLogo from '../assets/arch-logo.png';

const PackageCard: React.FC<PackageCardProps> = ({ pkg, onClick, skipMetadataFetch = false, chaoticInfo: initialChaoticInfo }) => {
    // State to hold the currently selected variant (defaults to the main pkg)
    const [displayPkg, setDisplayPkg] = useState<Package>(pkg);

    // Sync when prop changes
    useEffect(() => {
        setDisplayPkg(pkg);
    }, [pkg]);

    const isChaotic = displayPkg.source === 'chaotic';
    const [chaoticInfo, setChaoticInfo] = useState<ChaoticPackage | null>(initialChaoticInfo || null);

    // Global Data Optimization (Source of Truth)
    const { metadata } = usePackageMetadata(displayPkg.name, displayPkg.url, skipMetadataFetch);
    const rawIcon = displayPkg.icon || metadata?.icon_url || null;
    const iconUrl = resolveIconUrl(rawIcon);

    // Unified Rating System (Source of Truth)
    const { rating } = usePackageRating(displayPkg.name, displayPkg.app_id || metadata?.app_id);

    // Favorites
    const { toggleFavorite, isFavorite } = useFavorites();
    const isFav = isFavorite(displayPkg.name);

    useEffect(() => {
        if (isChaotic && !initialChaoticInfo && !chaoticInfo) {
            invoke<ChaoticPackage>('get_chaotic_package_info', { name: displayPkg.name })
                .then(info => setChaoticInfo(info || null))
                .catch(() => { });
        }
    }, [displayPkg.name, isChaotic, initialChaoticInfo]);


    const [imgError, setImgError] = useState(false);

    // Reset error state when icon changes
    useEffect(() => {
        setImgError(false);
    }, [iconUrl]);

    // Construct variants list
    const variants = [pkg, ...(pkg.alternatives || [])];
    const hasVariants = variants.length > 1;

    return (
        <motion.div
            onClick={() => onClick(displayPkg)}
            className="group relative bg-white/70 dark:bg-black/20 border border-slate-200 dark:border-white/5 rounded-3xl p-6 hover:bg-white dark:hover:bg-black/40 transition-all duration-300 hover:border-blue-300/50 dark:hover:border-white/10 hover:-translate-y-1 hover:shadow-xl dark:hover:shadow-2xl shadow-sm dark:shadow-none cursor-pointer overflow-hidden flex flex-col h-full backdrop-blur-md"
        >
            <div className="flex justify-between items-start mb-4 gap-4">
                <div className="flex items-center gap-4 min-w-0 flex-1">
                    <div className={clsx(
                        "w-14 h-14 rounded-2xl flex items-center justify-center shadow-inner shrink-0 overflow-hidden relative transition-colors",
                        "text-slate-800 dark:text-white",
                        (!iconUrl || imgError) ? "bg-black/5 dark:bg-white/5 border border-black/5 dark:border-white/10 p-2" : "bg-transparent"
                    )}>
                        {iconUrl && !imgError ? (
                            <img
                                src={iconUrl}
                                alt={displayPkg.name}
                                className="w-full h-full object-contain p-1 drop-shadow-md"
                                loading="lazy"
                                onError={() => setImgError(true)}
                            />
                        ) : (
                            <img src={archLogo} className="w-full h-full object-contain opacity-80 grayscale group-hover:grayscale-0 transition-all dark:invert" alt="Arch Linux" />
                        )}
                    </div>
                    <div className="flex-1 min-w-0">
                        <div className="flex flex-col">
                            <h3 className="font-bold text-lg leading-tight group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors line-clamp-2 text-app-fg break-words max-w-[200px] md:max-w-none">
                                {displayPkg.display_name || displayPkg.name}
                            </h3>
                        </div>
                        {displayPkg.display_name && displayPkg.display_name.toLowerCase() !== displayPkg.name.toLowerCase() && (
                            <span className="text-[10px] text-slate-500 dark:text-white/50 font-mono opacity-80 block truncate mt-0.5">
                                {displayPkg.name}
                            </span>
                        )}

                        {/* VERSION SELECTOR */}
                        <div className="flex items-center gap-2 mt-1" onClick={(e) => e.stopPropagation()}>
                            {hasVariants ? (
                                <select
                                    className="text-[10px] font-mono bg-black/5 dark:bg-white/5 border border-black/5 dark:border-white/10 rounded px-1.5 py-0.5 outline-none focus:border-blue-500 text-slate-600 dark:text-white/70 max-w-[120px] cursor-pointer hover:bg-black/10 dark:hover:bg-white/10 transition-colors"
                                    value={variants.findIndex(v => v.source === displayPkg.source && v.version === displayPkg.version) !== -1 ? variants.findIndex(v => v.source === displayPkg.source && v.version === displayPkg.version) : 0}
                                    onChange={(e) => {
                                        const idx = parseInt(e.target.value);
                                        const selected = variants[idx];
                                        const siblings = variants.filter((_, i) => i !== idx);
                                        const enriched = { ...selected, alternatives: siblings };
                                        setDisplayPkg(enriched);
                                    }}
                                >
                                    {variants.map((v, i) => (
                                        <option key={i} value={i} className="bg-white dark:bg-slate-900 text-slate-900 dark:text-white">
                                            {v.version} ({v.source})
                                        </option>
                                    ))}
                                </select>
                            ) : (
                                <span className="text-[10px] text-slate-400 dark:text-white/40 font-mono">{displayPkg.version}</span>
                            )}
                        </div>
                    </div>
                </div>
            </div>

            <p className="text-slate-600 dark:text-indigo-100/70 text-sm line-clamp-2 mb-6 h-10 font-medium leading-relaxed">
                {displayPkg.description}
            </p>

            <div className="flex items-center justify-between mt-auto">
                <div className="flex flex-col gap-2 items-start">
                    {displayPkg.source === 'chaotic' ? (
                        <div className="px-2.5 py-1 rounded-full bg-violet-100 dark:bg-violet-500/10 border border-violet-200 dark:border-violet-500/20 text-violet-700 dark:text-violet-400 text-[10px] font-black uppercase tracking-widest flex items-center gap-1.5 shrink-0 whitespace-nowrap shadow-sm">
                            <ShieldCheck size={12} /> Chaotic
                        </div>
                    ) : displayPkg.source === 'official' ? (
                        <div className="px-2.5 py-1 rounded-full bg-teal-100 dark:bg-teal-500/10 border border-teal-200 dark:border-teal-500/20 text-teal-700 dark:text-teal-400 text-[10px] font-black uppercase tracking-widest flex items-center gap-1.5 shrink-0 whitespace-nowrap shadow-sm">
                            <ShieldCheck size={12} /> Official
                        </div>
                    ) : displayPkg.source === 'aur' ? (
                        <div className="px-2.5 py-1 rounded-full bg-amber-100 dark:bg-amber-500/10 border border-amber-200 dark:border-amber-500/20 text-amber-700 dark:text-amber-400 text-[10px] font-black uppercase tracking-widest flex items-center gap-1.5 shrink-0 whitespace-nowrap shadow-sm">
                            <Download size={12} /> AUR
                        </div>
                    ) : (
                        <div className="px-2.5 py-1 rounded-full bg-sky-100 dark:bg-sky-500/10 border border-sky-200 dark:border-sky-500/20 text-sky-700 dark:text-sky-400 text-[10px] font-black uppercase tracking-widest flex items-center gap-1.5 shrink-0 whitespace-nowrap shadow-sm">
                            <Zap size={12} /> {displayPkg.source.toUpperCase()}
                        </div>
                    )}

                    <div className="flex items-center gap-2">
                        {displayPkg.is_optimized && (
                            <div className="px-2 py-0.5 rounded-full bg-amber-100 dark:bg-amber-500/10 border border-amber-200 dark:border-amber-500/20 text-amber-700 dark:text-amber-400 text-[10px] font-bold uppercase tracking-wider flex items-center gap-1 shrink-0 whitespace-nowrap">
                                <Zap size={10} fill="currentColor" /> Opt
                            </div>
                        )}
                        {rating && rating.count > 0 && typeof rating.average === 'number' && (
                            <div className="flex items-center gap-1 bg-yellow-100 dark:bg-yellow-400/5 backdrop-blur-md px-1.5 py-0.5 rounded-lg text-[10px] font-black text-yellow-600 dark:text-yellow-500 border border-yellow-200 dark:border-yellow-400/10">
                                <span className="text-[10px] leading-none">★</span>
                                <span className="tracking-tight">{rating.average.toFixed(1)}</span>
                            </div>
                        )}
                    </div>
                </div>
                <div className="flex items-center gap-2 relative z-10 self-end translate-y-2 group-hover:translate-y-0 transition-transform duration-300 opacity-0 group-hover:opacity-100">
                    <button
                        onClick={(e) => {
                            e.stopPropagation();
                            toggleFavorite(displayPkg.name);
                        }}
                        className={clsx(
                            "p-2.5 rounded-xl transition-all border border-transparent shadow-lg active:scale-90",
                            isFav
                                ? "text-red-600 dark:text-red-500 bg-red-100 dark:bg-red-500/10 border-red-200 dark:border-red-500/20"
                                : "text-slate-400 dark:text-white/50 bg-white dark:bg-white/5 hover:bg-red-500 hover:text-white"
                        )}
                        title={isFav ? "Remove from favorites" : "Add to favorites"}
                    >
                        <Heart size={16} fill={isFav ? "currentColor" : "none"} />
                    </button>
                    <button className="p-2.5 rounded-xl bg-blue-600 hover:bg-blue-500 text-white transition-all shadow-lg active:scale-90 shadow-blue-900/20">
                        <Download size={16} />
                    </button>
                </div>
            </div>

            {/* Glow effect */}
            <div className="absolute inset-0 bg-gradient-to-br from-blue-500/5 to-purple-500/5 opacity-0 group-hover:opacity-100 pointer-events-none transition-opacity duration-500" />
        </motion.div>
    );
};

export default PackageCard;
