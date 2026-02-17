
import { useMemo } from 'react';
import PackageCard from './PackageCard';
import type { Package } from '../services/bindings';
import PackageCardSkeleton from './PackageCardSkeleton';
import { useChaoticStatus, isOnlyChaoticSource } from '../hooks/useChaoticStatus';
import { useAppStore } from '../store/internal_store';


interface TrendingSectionProps {
    title: string;
    onSelectPackage: (pkg: Package) => void;
    filterIds?: string[];
    limit?: number;
    onSeeAll?: () => void;
    variant?: 'scroll' | 'grid';
    onOpenSettings?: () => void;
    /** When true, do not render the section header (title + See All); parent provides it. */
    hideHeader?: boolean;
    /** When set, use these packages instead of fetching (instant load for See All / home essentials). */
    preloadedPackages?: Package[];
    /** When true (e.g. essentials from App prewarm), do not fetch; show skeletons until preloadedPackages arrive. Avoids duplicate get_packages_by_names. */
    preloadInProgress?: boolean;
    /** Which list to update in store: essentials vs trending vs favorites. */
    listKind?: 'trending' | 'essentials' | 'favorites';
}

export default function TrendingSection({ title, onSelectPackage, filterIds, limit, onSeeAll, variant = 'grid', onOpenSettings, hideHeader = false, preloadInProgress = false }: TrendingSectionProps) {
    const { enabled: chaoticEnabled } = useChaoticStatus();
    const packageRegistry = useAppStore((s) => s.packageRegistry);

    // Simplification: Rely purely on parent providing filterIds (managed by App.tsx)
    const safeIds = useMemo(() => filterIds ?? [], [filterIds]);
    const loading = preloadInProgress;

    // Legacy support removal: No internal fetching.

    const visibleIds = useMemo(() => {
        const DEFAULT_MAX_ITEMS = 80;
        const effectiveLimit = limit ?? DEFAULT_MAX_ITEMS;
        return safeIds.slice(0, effectiveLimit);
    }, [safeIds, limit]);

    const showSeeAll = limit != null && onSeeAll != null && safeIds.length > limit;

    if (loading) {
        const isScroll = variant === 'scroll';
        const skeletonCount = isScroll ? 7 : 8;
        return (
            <section>
                {!hideHeader && (
                    <div className="flex items-center justify-between mb-6">
                        {title && <div className="h-8 w-48 rounded bg-gray-200 dark:bg-gray-700 animate-pulse" />}
                    </div>
                )}
                {isScroll ? (
                    <div className="relative group/scroll max-w-7xl mx-auto">
                        <div
                            className="flex gap-6 overflow-x-auto pb-6 scrollbar-hide snap-x"
                            style={{
                                maskImage: 'linear-gradient(to right, black 85%, transparent 100%)',
                                WebkitMaskImage: 'linear-gradient(to right, black 85%, transparent 100%)'
                            }}
                        >
                            {[...Array(skeletonCount)].map((_, i) => (
                                <div key={i} className="snap-start flex-shrink-0 w-[280px]">
                                    <PackageCardSkeleton />
                                </div>
                            ))}
                        </div>
                    </div>
                ) : (
                    <div className="grid gap-6 max-w-7xl mx-auto w-full grid-cols-[repeat(auto-fill,minmax(260px,1fr))]">
                        {[...Array(skeletonCount)].map((_, i) => (
                            <PackageCardSkeleton key={i} />
                        ))}
                    </div>
                )}
            </section>
        );
    }

    if (visibleIds.length === 0) {
        return (
            <section>
                {!hideHeader && title && <h2 className="text-2xl font-bold text-app-fg mb-6">{title}</h2>}
                <p className="text-app-muted text-sm py-6">No trending applications to show right now. Try again in a moment.</p>
            </section>
        );
    }

    const isScroll = variant === 'scroll';

    return (
        <section>
            {!hideHeader && (
                <div className="flex items-center justify-between mb-6">
                    {title && <h2 className="text-2xl font-bold text-app-fg flex items-center gap-2">{title}</h2>}
                    {showSeeAll && onSeeAll && (
                        <button onClick={onSeeAll} className="text-sm font-bold text-accent hover:opacity-80 transition-colors flex items-center gap-1">
                            See All <span className="text-xs">→</span>
                        </button>
                    )}
                </div>
            )}

            {isScroll ? (
                <div className="relative group/scroll max-w-7xl mx-auto">
                    <div
                        className="flex gap-6 overflow-x-auto pb-6 scrollbar-hide snap-x relative z-0"
                        style={{
                            maskImage: 'linear-gradient(to right, black 85%, transparent 100%)',
                            WebkitMaskImage: 'linear-gradient(to right, black 85%, transparent 100%)'
                        }}
                    >
                        {visibleIds.map((id) => {
                            const pkg = packageRegistry[id];
                            return (
                                <div key={id} className="snap-start flex-shrink-0 w-[280px]">
                                    <PackageCard
                                        pkgId={id}
                                        onClick={(p) => onSelectPackage(p)}

                                        setupRequired={pkg ? isOnlyChaoticSource(pkg) && !chaoticEnabled : false}
                                        onConfigureSource={onOpenSettings}
                                        skipMetadataFetch={!!pkg?.icon}
                                    />
                                </div>
                            );
                        })}
                        {showSeeAll && (
                            <div className="snap-start flex-shrink-0 w-[280px] flex">
                                <button
                                    onClick={onSeeAll}
                                    className="w-full h-full bg-app-card/30 border-2 border-dashed border-app-border rounded-2xl flex flex-col items-center justify-center gap-4 group transition-all min-h-[200px] accent-hover-outline"
                                >
                                    <div className="w-12 h-12 rounded-full bg-app-subtle flex items-center justify-center transition-opacity group-hover:opacity-90">
                                        <span className="text-2xl">→</span>
                                    </div>
                                    <span className="font-bold text-app-fg transition-opacity group-hover:opacity-90">View All</span>
                                </button>
                            </div>
                        )}
                    </div>
                </div>
            ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 max-w-7xl mx-auto w-full">
                    {visibleIds.map((id) => {
                        const pkg = packageRegistry[id];
                        return (
                            <PackageCard
                                key={id}
                                pkgId={id}
                                onClick={(p) => onSelectPackage(p)}

                                setupRequired={pkg ? isOnlyChaoticSource(pkg) && !chaoticEnabled : false}
                                onConfigureSource={onOpenSettings}
                                skipMetadataFetch={!!pkg?.icon}
                            />
                        );
                    })}
                    {showSeeAll && (
                        <button
                            onClick={onSeeAll}
                            className="bg-app-card/30 border-2 border-dashed border-app-border rounded-2xl flex flex-col items-center justify-center gap-4 group transition-all p-8 h-full min-h-[220px] accent-hover-outline"
                        >
                            <div className="w-12 h-12 rounded-full bg-app-fg/5 flex items-center justify-center transition-opacity group-hover:opacity-90">
                                <span className="text-2xl">→</span>
                            </div>
                            <span className="font-bold text-app-fg transition-opacity group-hover:opacity-90">View All Trending</span>
                        </button>
                    )}
                </div>
            )}
        </section>
    );
}
