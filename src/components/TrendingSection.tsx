import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import PackageCard, { Package, ChaoticPackage } from './PackageCard';
import SkeletonCard from './SkeletonCard';
import { useErrorService } from '../context/ErrorContext';

interface TrendingSectionProps {
    title: string;
    onSelectPackage: (pkg: Package) => void;
    filterIds?: string[];
    limit?: number;
    onSeeAll?: () => void;
    variant?: 'scroll' | 'grid';
}

export default function TrendingSection({ title, onSelectPackage, filterIds, limit, onSeeAll, variant = 'grid' }: TrendingSectionProps) {
    const errorService = useErrorService();
    const [packages, setPackages] = useState<Package[]>([]);
    const [loading, setLoading] = useState(true);
    const [chaoticInfoMap, setChaoticInfoMap] = useState<Map<string, ChaoticPackage>>(new Map());

    useEffect(() => {
        const loadTrending = async () => {
            setLoading(true);
            try {
                // If we have specific filter IDs, we search for them specifically
                // Otherwise we get the generic trending list
                let result: Package[] = [];

                if (filterIds && filterIds.length > 0) {
                    // Fetch specific packages efficiently in one batch
                    try {
                        result = await invoke<Package[]>('get_packages_by_names', { names: filterIds });
                    } catch (e) {
                        errorService.reportError(e as Error | string);
                    }
                } else {
                    result = await invoke<Package[]>('get_trending');
                }

                setPackages(result);
            } catch (e) {
                console.error("Failed to load trending", e);
            } finally {
                setLoading(false);
            }
        };
        loadTrending();
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [JSON.stringify(filterIds)]);

    // Batch fetch chaotic info when packages load
    useEffect(() => {
        const fetchBatchInfo = async () => {
            const visiblePackages = limit ? packages.slice(0, limit) : packages;
            const chaoticNames = visiblePackages
                .filter(p => p.source === 'chaotic')
                .map(p => p.name);

            if (chaoticNames.length === 0) return;

            const neededNames = chaoticNames.filter(n => !chaoticInfoMap.has(n));
            if (neededNames.length === 0) return;

            try {
                const infoMap = await invoke<Record<string, ChaoticPackage>>('get_chaotic_packages_batch', {
                    names: neededNames
                });

                setChaoticInfoMap(prev => {
                    const next = new Map(prev);
                    Object.entries(infoMap).forEach(([name, info]) => {
                        next.set(name, info);
                    });
                    return next;
                });
            } catch (e) {
                errorService.reportError(e as Error | string);
            }
        };

        if (packages.length > 0) {
            fetchBatchInfo();
        }
    }, [packages, limit]);

    if (loading) {
        return (
            <section>
                <div className="flex items-center justify-between mb-6">
                    {title && <div className="h-8 w-48 rounded bg-gray-200 dark:bg-gray-700 animate-pulse" />}
                </div>
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5 gap-6 max-w-7xl mx-auto w-full">
                    {[...Array(8)].map((_, i) => (
                        <SkeletonCard key={i} />
                    ))}
                </div>
            </section>
        );
    }

    if ((packages || []).length === 0) return null;

    const displayedPackages = limit ? (packages || []).slice(0, limit) : (packages || []);
    const showSeeAll = limit && onSeeAll && (packages || []).length > limit;

    // Use the explicit variant, or infer 'scroll' if a limit is set (heuristic for homepage rows)
    const isScroll = variant === 'scroll';

    return (
        <section>
            <div className="flex items-center justify-between mb-6">
                {title && <h2 className="text-2xl font-bold text-app-fg flex items-center gap-2">{title}</h2>}
                {showSeeAll && !filterIds && (
                    <button onClick={onSeeAll} className="text-sm font-bold text-blue-500 hover:text-blue-400 transition-colors flex items-center gap-1">
                        See All <span className="text-xs">→</span>
                    </button>
                )}
            </div>

            {isScroll ? (
                <div className="relative group/scroll max-w-7xl mx-auto">
                    <div
                        className="flex gap-6 overflow-x-auto pb-6 scrollbar-hide snap-x relative z-0"
                        style={{
                            maskImage: 'linear-gradient(to right, black 85%, transparent 100%)',
                            WebkitMaskImage: 'linear-gradient(to right, black 85%, transparent 100%)'
                        }}
                    >
                        {displayedPackages.map((pkg) => (
                            <div key={`${pkg.name}-${pkg.source}`} className="snap-start flex-shrink-0 w-[280px]">
                                <PackageCard
                                    pkg={pkg}
                                    onClick={() => onSelectPackage(pkg)}
                                    chaoticInfo={chaoticInfoMap.get(pkg.name)}
                                />
                            </div>
                        ))}
                        {showSeeAll && (
                            <div className="snap-start flex-shrink-0 w-[280px] flex">
                                <button
                                    onClick={onSeeAll}
                                    className="w-full h-full bg-app-card/30 border-2 border-dashed border-app-border rounded-2xl flex flex-col items-center justify-center gap-4 group hover:border-blue-500/50 hover:bg-blue-500/5 transition-all min-h-[200px]"
                                >
                                    <div className="w-12 h-12 rounded-full bg-app-subtle flex items-center justify-center group-hover:bg-blue-500/20 text-app-muted group-hover:text-blue-500 transition-colors">
                                        <span className="text-2xl">→</span>
                                    </div>
                                    <span className="font-bold text-app-fg group-hover:text-blue-400">View All</span>
                                </button>
                            </div>
                        )}
                    </div>
                </div>
            ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5 gap-6 max-w-7xl mx-auto w-full">
                    {displayedPackages.map((pkg) => (
                        <PackageCard
                            key={`${pkg.name}-${pkg.source}`}
                            pkg={pkg}
                            onClick={() => onSelectPackage(pkg)}
                            chaoticInfo={chaoticInfoMap.get(pkg.name)}
                        />
                    ))}
                    {/* For Grid, we can add a card if space permits or just rely on the header button. 
                        Header button is cleaner for grid. But let's add a card if it fits the grid pattern? 
                        7 items + 1 See All = 8 items (perfect 4x2 grid). */}
                    {showSeeAll && (
                        <button
                            onClick={onSeeAll}
                            className="bg-app-card/30 border-2 border-dashed border-app-border rounded-2xl flex flex-col items-center justify-center gap-4 group hover:border-blue-500/50 hover:bg-blue-500/5 transition-all p-8 h-full min-h-[220px]"
                        >
                            <div className="w-12 h-12 rounded-full bg-app-fg/5 flex items-center justify-center group-hover:bg-blue-500/20 text-app-muted group-hover:text-blue-500 transition-colors">
                                <span className="text-2xl">→</span>
                            </div>
                            <span className="font-bold text-app-fg group-hover:text-blue-400">View All Trending</span>
                        </button>
                    )}
                </div>
            )}
        </section>
    );
}
