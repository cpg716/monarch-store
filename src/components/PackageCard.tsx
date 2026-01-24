import React, { useEffect, useState } from 'react';
import { Download, Package as PackageIcon, ShieldCheck, Zap, Heart } from 'lucide-react';
import { useFavorites } from '../hooks/useFavorites';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';

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

const PackageCard: React.FC<PackageCardProps> = ({ pkg, onClick, skipMetadataFetch = false, chaoticInfo: initialChaoticInfo }) => {
    const isChaotic = pkg.source === 'chaotic';
    const isOfficial = pkg.source === 'official';
    const [chaoticInfo, setChaoticInfo] = useState<ChaoticPackage | null>(initialChaoticInfo || null);

    // Global Data Optimization (Source of Truth)
    const { metadata } = usePackageMetadata(pkg.name, pkg.url, skipMetadataFetch);
    const iconUrl = pkg.icon || metadata?.icon_url || null;

    // Unified Rating System (Source of Truth)
    const { rating } = usePackageRating(pkg.name, pkg.app_id || metadata?.app_id);

    // Favorites
    const { toggleFavorite, isFavorite } = useFavorites();
    const isFav = isFavorite(pkg.name);

    useEffect(() => {
        if (isChaotic && !initialChaoticInfo && !chaoticInfo) {
            invoke<ChaoticPackage>('get_chaotic_package_info', { name: pkg.name })
                .then(info => setChaoticInfo(info || null))
                .catch(() => { });
        }
    }, [pkg.name, isChaotic, initialChaoticInfo]);


    return (
        <div
            onClick={() => onClick(pkg)}
            className="group relative bg-app-card/40 border border-app-border rounded-2xl p-5 hover:bg-app-fg/5 transition-all duration-300 hover:border-app-fg/10 hover:-translate-y-1 cursor-pointer overflow-hidden flex flex-col h-full"
        >
            <div className="flex justify-between items-start mb-3 gap-3">
                <div className="flex items-center gap-3 min-w-0 flex-1">
                    <div className={clsx(
                        "w-10 h-10 rounded-lg flex items-center justify-center text-app-fg shadow-lg shrink-0 overflow-hidden relative transition-colors",
                        !iconUrl && isChaotic ? "bg-gradient-to-br from-purple-600/80 to-blue-600/80" :
                            !iconUrl && isOfficial ? "bg-gradient-to-br from-emerald-500/80 to-teal-600/80" :
                                !iconUrl ? "bg-app-bg/50" : "bg-transparent"
                    )}>
                        {iconUrl ? (
                            <img src={iconUrl || undefined} alt={pkg.name} className="w-full h-full object-contain p-1" loading="lazy" />
                        ) : (
                            isChaotic ? <Zap size={20} fill="currentColor" className="text-yellow-400" /> : <PackageIcon size={20} className="text-app-muted" />
                        )}
                    </div>
                    <div className="flex-1 min-w-0">
                        <div className="flex flex-col">
                            <h3 className="font-bold text-lg leading-tight group-hover:text-blue-500 transition-colors line-clamp-2 text-app-fg break-words">
                                {pkg.display_name || pkg.name}
                            </h3>
                        </div>
                        {pkg.display_name && pkg.display_name.toLowerCase() !== pkg.name.toLowerCase() && (
                            <span className="text-[10px] text-app-muted font-mono opacity-70 block truncate">
                                {pkg.name}
                            </span>
                        )}
                        <span className="text-xs text-app-muted font-mono">{pkg.version}</span>
                    </div>
                </div>
            </div>

            <p className="text-app-muted text-sm line-clamp-2 mb-4 h-10">
                {pkg.description}
            </p>

            <div className="flex items-center justify-between mt-auto">
                <div className="flex flex-col gap-1.5 items-start">
                    {isChaotic && (
                        <div className="px-2 py-0.5 rounded-full bg-violet-600/20 border border-violet-600/40 text-violet-600 text-[10px] font-bold uppercase tracking-wider flex items-center gap-1 shrink-0 whitespace-nowrap">
                            <ShieldCheck size={10} /> Chaotic
                        </div>
                    )}
                    <div className="flex items-center gap-2">
                        {rating && rating.count > 0 && typeof rating.average === 'number' && (
                            <div className="flex items-center gap-1.5 bg-yellow-400/10 backdrop-blur-md px-2 py-0.5 rounded-lg text-[10px] font-black text-yellow-500 border border-yellow-400/20 shadow-sm shadow-yellow-900/10">
                                <span className="text-[12px] leading-none">★</span>
                                <span className="tracking-tight">{rating.average.toFixed(1)}</span>
                                <span className="opacity-50 font-medium">({rating.count})</span>
                            </div>
                        )}
                        {chaoticInfo && chaoticInfo.lastUpdated && (
                            <span className="flex items-center gap-1 text-[10px] text-app-muted bg-app-subtle px-2 py-0.5 rounded">
                                {new Date(chaoticInfo.lastUpdated).toLocaleDateString()}
                            </span>
                        )}
                    </div>
                </div>
                <div className="flex items-center gap-2 relative z-10 self-end">
                    <button
                        onClick={(e) => {
                            e.stopPropagation();
                            toggleFavorite(pkg.name);
                        }}
                        className={clsx(
                            "p-2 rounded-lg transition-colors border border-transparent",
                            isFav
                                ? "text-red-500 bg-red-500/10 border-red-500/20"
                                : "text-app-muted bg-app-subtle hover:bg-red-500 hover:text-white"
                        )}
                        title={isFav ? "Remove from favorites" : "Add to favorites"}
                    >
                        <Heart size={18} fill={isFav ? "currentColor" : "none"} />
                    </button>
                    <button className="p-2 rounded-lg bg-app-subtle hover:bg-blue-600 hover:text-white transition-colors text-app-muted border border-transparent">
                        <Download size={18} />
                    </button>
                </div>
            </div>

            {/* Glow effect */}
            <div className="absolute inset-0 rounded-2xl bg-gradient-to-br from-blue-500/5 to-purple-500/5 opacity-0 group-hover:opacity-100 pointer-events-none transition-opacity duration-500" />
        </div>
    );
};

export default PackageCard;
