import { useMemo } from 'react';
import type { Package } from '../services/bindings';
import PackageCardSkeleton from './PackageCardSkeleton';
import { useChaoticStatus, isOnlyChaoticSource } from '../hooks/useChaoticStatus';
import { useAppStore } from '../store/internal_store';
import PackageCardList from './PackageCardList';
import { usePackageCardList } from '../hooks/usePackageCardList';


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

export default function TrendingSection({ title, onSelectPackage, filterIds, limit, onSeeAll, variant = 'grid', onOpenSettings, hideHeader = false, preloadedPackages = [], preloadInProgress = false }: TrendingSectionProps) {
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
    const { ids: dedupedVisibleIds } = usePackageCardList({
        source: { mode: 'ids', ids: visibleIds },
        packageRegistry,
        sort: 'preserve',
    });
    const { packages: directVisiblePackages } = usePackageCardList({
        source: { mode: 'packages', packages: preloadedPackages.slice(0, limit ?? 80) },
        packageRegistry,
        sort: 'preserve',
    });
    const shouldRenderDirect = directVisiblePackages.length > 0;

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
                        <div className="flex gap-6 overflow-x-auto pb-6 scrollbar-hide snap-x">
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

    if (dedupedVisibleIds.length === 0 && !shouldRenderDirect) {
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
                <PackageCardList
                    source={shouldRenderDirect
                        ? { mode: 'packages', packages: directVisiblePackages }
                        : { mode: 'ids', ids: dedupedVisibleIds }}
                    onSelectPackage={onSelectPackage}
                    variant="scroll"
                    onSeeAll={onSeeAll}
                    showViewAllCard={showSeeAll}
                    setupRequiredResolver={(pkg) => isOnlyChaoticSource(pkg) && !chaoticEnabled}
                    onConfigureSource={onOpenSettings}
                    surfaceName="TrendingSection"
                />
            ) : (
                <PackageCardList
                    source={shouldRenderDirect
                        ? { mode: 'packages', packages: directVisiblePackages }
                        : { mode: 'ids', ids: dedupedVisibleIds }}
                    onSelectPackage={onSelectPackage}
                    variant="grid"
                    onSeeAll={onSeeAll}
                    showViewAllCard={showSeeAll}
                    setupRequiredResolver={(pkg) => isOnlyChaoticSource(pkg) && !chaoticEnabled}
                    onConfigureSource={onOpenSettings}
                    surfaceName="TrendingSection"
                />
            )}
        </section>
    );
}
