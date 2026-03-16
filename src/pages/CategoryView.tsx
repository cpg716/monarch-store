import React, { useEffect, useState, useCallback, useMemo, useRef } from 'react';
import { useInfiniteScroll } from '../hooks/useInfiniteScroll';
import { ArrowLeft, LayoutGrid } from 'lucide-react';
import clsx from 'clsx';
import { commands, CategoryQuery } from '../services/bindings';
import type { Package } from '../services/bindings';
import PackageCardList from '../components/PackageCardList';
import PackageCardSkeleton from '../components/PackageCardSkeleton';
import EmptyState from '../components/EmptyState';
import { CATEGORIES } from '../components/CategoryGrid';
import { useErrorService } from '../context/ErrorContext';
import { useChaoticStatus, isOnlyChaoticSource } from '../hooks/useChaoticStatus';
import { useSettings } from '../hooks/useSettings';
import { getPackageListKey } from '../utils/packageKey';
import { useAppStore } from '../store/internal_store';
import { getSourceFamilyId, getSourceFamilyLabel } from '../utils/repoHelper';
import { usePackageCardList } from '../hooks/usePackageCardList';

import { unwrap } from '../utils/specta';
import { debugWarn } from '../utils/debugLog';

/** Stable empty array for store selectors to avoid getSnapshot reference changes (infinite loop). */
const EMPTY_CATEGORY_IDS: string[] = [];

// ... imports

interface CategoryViewProps {
    category: string;
    onBack: () => void;
    onSelectPackage: (pkg: Package, preferredSource?: string) => void;
    onOpenSettings?: () => void;
}

interface RepoState {
    name: string;
    enabled: boolean;
    source: any;
}

// ... imports

// ... types removed, using bindings.ts instead

const CategoryView: React.FC<CategoryViewProps> = ({ category, onBack, onSelectPackage, onOpenSettings }) => {
    const errorService = useErrorService();
    const { enabled: chaoticEnabled } = useChaoticStatus();
    const { isFlatpakEnabled, isAurEnabled, isChaoticEnabled } = useSettings();
    const upsertPackages = useAppStore((s) => s.upsertPackages);
    const setCategoryIds = useAppStore((s) => s.setCategoryIds);
    const appendCategoryIds = useAppStore((s) => s.appendCategoryIds);
    const categoryIds = useAppStore((s) => s.categoryIds[category] ?? EMPTY_CATEGORY_IDS);
    const packageRegistry = useAppStore((s) => s.packageRegistry);
    const fetchRatingsForPackages = useAppStore((s) => s.fetchRatingsForPackages);

    const [totalPackages, setTotalPackages] = useState(0);
    const [loading, setLoading] = useState(true);
    const [initialLoad, setInitialLoad] = useState(true);
    const [sortBy, setSortBy] = useState<'featured' | 'name' | 'updated'>('featured');
    const [repoFilter, setRepoFilter] = useState<string[]>(['all']);
    const [page, setPage] = useState(1);
    const [hasMore, setHasMore] = useState(true);
    const [enabledRepos, setEnabledRepos] = useState<RepoState[]>([]);
    const [categoryPackages, setCategoryPackages] = useState<Package[]>([]);
    const [error, setError] = useState<string | null>(null);
    const requestSeqRef = useRef(0);
    const inFlightKeyRef = useRef<string | null>(null);

    // Constant limit for backend pagination
    const LIMIT = 50;

    const getRepoLabel = (sourceOrFamilyId: any) => {
        const id = typeof sourceOrFamilyId === 'string' ? sourceOrFamilyId : (sourceOrFamilyId?.id ?? 'other');
        return getSourceFamilyLabel(id);
    };

    // Helper for display labels
    const categoryResult = CATEGORIES.find(c => c.id === category || c.label === category);
    const Icon = categoryResult?.icon || LayoutGrid;
    const colorClass = categoryResult?.color || "text-blue-500";
    const displayLabel = categoryResult?.label || category;

    // ... (Repo fetch same)
    useEffect(() => {
        commands.getRepoStates().then(unwrap).then(repos => {
            const enabled = repos.filter(r => r.enabled);
            const uniqueSources = new Map<string, any>();
            for (const repo of enabled) {
                const sourceId = typeof repo.source === 'string' ? repo.source : (repo.source as any)?.id || 'other';
                if (!uniqueSources.has(sourceId)) uniqueSources.set(sourceId, repo);
            }
            setEnabledRepos(Array.from(uniqueSources.values()) as any);
        }).catch((e) => errorService.reportError(e as Error | string));
    }, [errorService]);

    // Fetch Logic
    const fetchApps = useCallback(async (reset: boolean = false) => {
        const currentPage = reset ? 1 : page;
        const repo_filter_val = repoFilter.includes('all') ? null : repoFilter;
        const requestKey = JSON.stringify({
            category,
            repo_filter: repo_filter_val,
            sort_by: sortBy,
            page: currentPage,
            limit: LIMIT,
            flatpak: isFlatpakEnabled ?? true,
            aur: isAurEnabled,
            chaotic: isChaoticEnabled,
            reset
        });

        if (inFlightKeyRef.current === requestKey) {
            return;
        }
        inFlightKeyRef.current = requestKey;

        if (reset) {
            setLoading(true);
            setInitialLoad(true);
            if (page !== 1) setPage(1);
            setError(null);
        }
        const requestId = ++requestSeqRef.current;

        try {
            let res: any;
            res = await commands.getCategoryPackagesPaginated({
                category,
                repo_filter: repo_filter_val,
                sort_by: sortBy,
                page: currentPage,
                limit: LIMIT,
                options: {
                    flatpak_enabled: isFlatpakEnabled ?? true,
                    aur_enabled: isAurEnabled,
                    chaotic_enabled: isChaoticEnabled,
                    for_installed_lookup: false
                }
            } as any).then(unwrap);
            if (requestId !== requestSeqRef.current) return;

            setTotalPackages(parseInt(res.total, 10));
            if ((res.packages?.length ?? 0) === 0) {
                debugWarn('[CategoryView] Backend returned no packages after fallback', {
                    category,
                    repoFilter: repo_filter_val,
                    sortBy,
                });
            }
            upsertPackages(res.packages as any);
            setCategoryPackages((prev) => (reset ? (res.packages as Package[]) : [...prev, ...(res.packages as Package[])]));

            // Safe batch rating fetch: use both app_id (ODRS canonical) and name (fallback)
            const appIds = Array.from(new Set(
                (res.packages as any[]).flatMap((p: any) =>
                    [p.app_id, p.name].filter((id: any): id is string => !!id && id.length > 0)
                )
            ));
            if (appIds.length > 0) {
                fetchRatingsForPackages(appIds);
            }

            // One card per app: dedupe by list key so Arch + Flatpak never show as two cards.
            const ids = Array.from(
                new Set(res.packages.map((p: any) => getPackageListKey(p as any)).filter(Boolean))
            ) as string[];
            if (reset) {
                setCategoryIds(category, ids);
            } else {
                appendCategoryIds(category, ids);
            }
            setHasMore(res.packages.length === LIMIT);
            setError(null);
        } catch (err: any) {
            if (requestId !== requestSeqRef.current) return;
            const errorMsg = err instanceof Error ? err.message : JSON.stringify(err);
            setError(errorMsg);
            errorService.reportError(errorMsg);
        } finally {
            if (inFlightKeyRef.current === requestKey) {
                inFlightKeyRef.current = null;
            }
            if (requestId !== requestSeqRef.current) return;
            setLoading(false);
            setInitialLoad(false);
        }
    }, [category, repoFilter, sortBy, page, isFlatpakEnabled, isAurEnabled, isChaoticEnabled, LIMIT, upsertPackages, setCategoryIds, appendCategoryIds]);

    // Triggers
    // 1. Reset when Category/Filter/Sort changes
    useEffect(() => {
        fetchApps(true);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [category, repoFilter, sortBy, isFlatpakEnabled, isAurEnabled, isChaoticEnabled]); // Removed fetchApps to break dependency loop

    // 2. Load More when page increments (but NOT on page 1, which is handled by reset)
    useEffect(() => {
        if (page > 1) {
            fetchApps(false);
        }
    }, [page, fetchApps]);
    // ^ Removing fetchApps from dependency to avoid loop? No, fetchApps depends on page.
    // Actually, putting fetchApps in dependency might cause loop if fetchApps changes.
    // Better: split the effect.
    // The previous useEffect handles the RESET correctly. 
    // But we need to handle pagination.

    // Let's restructure:
    // We shouldn't invoke in render or simple effect if we can help it.
    // But `useInfiniteScroll` just calls a callback.

    const loadMore = useCallback(() => {
        if (!loading && hasMore) {
            setPage((prev: number) => prev + 1);
        }
    }, [loading, hasMore]);

    const lastElementRef = useInfiniteScroll(loadMore, hasMore, loading);

    // Handlers
    const handleSelectPackage = useCallback((pkg: Package) => {
        if (!repoFilter.includes('all') && repoFilter.length === 1) {
            onSelectPackage(pkg, repoFilter[0]);
        } else {
            onSelectPackage(pkg);
        }
    }, [onSelectPackage, repoFilter]);

    // Filter chips: same family ids and labels as SearchPage (official, chaotic-aur, aur, flatpak, etc.)
    const filterOptions = useMemo(() => {
        const options: { id: string; label: string; count: number }[] = [
            { id: 'all', label: 'All', count: totalPackages }
        ];
        const seen = new Set<string>(['all']);
        enabledRepos.forEach(repo => {
            const sourceObj = typeof repo.source === 'object' && repo.source != null ? repo.source : { id: String(repo.source), source_type: String(repo.source), label: '', version: '' };
            const familyId = getSourceFamilyId(sourceObj);
            if (seen.has(familyId)) return;
            seen.add(familyId);
            options.push({ id: familyId, label: getSourceFamilyLabel(familyId), count: 0 });
        });
        if (isFlatpakEnabled && !seen.has('flatpak')) {
            options.push({ id: 'flatpak', label: getSourceFamilyLabel('flatpak'), count: 0 });
        }
        return options;
    }, [enabledRepos, isFlatpakEnabled, totalPackages]);
    const { ids: visibleCategoryIds } = usePackageCardList({
        source: { mode: 'ids', ids: categoryIds },
        packageRegistry,
        sort: 'preserve',
    });
    const { packages: directCategoryPackages } = usePackageCardList({
        source: { mode: 'packages', packages: categoryPackages },
        packageRegistry,
        sort: 'preserve',
    });
    const useDirectPackages = directCategoryPackages.length > 0;

    const toggleFilter = (id: string) => {
        if (id === 'all') {
            setRepoFilter(['all']);
        } else {
            // Match SearchPage behavior: single select for now, or multi?
            // Actually Category backend supports multi, but search is single.
            // Let's go with single select to MATCH SearchPage UI feel.
            setRepoFilter([id]);
        }
    };


    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 overflow-hidden transition-colors">
            {/* Header ... */}
            <div className="p-6 border-b border-app-border flex items-center justify-between bg-app-card z-10 transition-colors">
                {/* ... existing header code ... */}
                <div className="flex items-center gap-4">
                    <button
                        onClick={onBack}
                        className="p-2 hover:bg-app-fg/10 rounded-lg transition-colors"
                    >
                        <ArrowLeft size={20} className="text-app-muted" />
                    </button>
                    <div>
                        <h1 className="text-2xl font-bold flex items-center gap-2 text-app-fg">
                            <Icon className={colorClass} size={24} />
                            {displayLabel} Apps
                        </h1>
                        <p className="text-app-muted text-sm">
                            {totalPackages > 0
                                ? `${totalPackages} Packages Total - ${(useDirectPackages ? directCategoryPackages.length : visibleCategoryIds.length)} Showing`
                                : `${(useDirectPackages ? directCategoryPackages.length : categoryIds.length)} packages loaded`
                            }
                            {repoFilter.includes('all')
                                ? ''
                                : ` in ${repoFilter.length > 3
                                    ? `${repoFilter.length} Repos`
                                    : repoFilter.map(r => getRepoLabel(r)).join(', ')
                                }`
                            }
                        </p>
                    </div>
                </div>

                {/* Filter Controls */}
                <div className="flex flex-col gap-4">
                    <div className="flex items-center gap-2 overflow-x-auto pb-1 no-scrollbar">
                        {filterOptions.map((opt, idx) => (
                            <button
                                key={typeof opt.id === 'string' || typeof opt.id === 'number' ? String(opt.id) : `filter-${idx}`}
                                onClick={() => toggleFilter(opt.id)}
                                className={clsx(
                                    "px-4 py-2 rounded-full text-xs font-bold transition-all border whitespace-nowrap",
                                    repoFilter.includes(opt.id)
                                        ? "bg-blue-600 border-blue-600 text-white shadow-lg shadow-blue-500/20"
                                        : "bg-app-card border-app-border text-app-muted hover:border-app-fg/30"
                                )}
                            >
                                {opt.label}
                            </button>
                        ))}
                    </div>
                </div>

                {/* Sort & Settings */}
                <div className="flex items-center gap-4">
                    {/* Sort */}
                    <div className="flex items-center gap-2 bg-app-card border border-app-border rounded-xl px-3 py-1.5 shadow-sm">
                        <span className="text-[10px] font-bold text-app-muted uppercase tracking-wider">Sort:</span>
                        <select
                            className="bg-transparent text-sm font-bold text-app-fg outline-none cursor-pointer"
                            value={sortBy}
                            onChange={(e) => setSortBy(e.target.value as 'featured' | 'name' | 'updated')}
                        >
                            <option value="featured">Featured</option>
                            <option value="name">Name</option>
                            <option value="updated">Newest</option>
                        </select>
                    </div>
                </div>
            </div>

            <div className="flex-1 overflow-y-auto p-8">
                <div className="max-w-7xl mx-auto w-full">
                    {initialLoad && categoryIds.length === 0 && categoryPackages.length === 0 ? (
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                            {[...Array(24)].map((_, i) => (
                                <PackageCardSkeleton key={i} />
                            ))}
                        </div>
                    ) : error ? (
                        <EmptyState
                            variant="error"
                            title="Failed to load Apps"
                            description={`We couldn't load apps for ${displayLabel}.\n${error}`}
                            actionLabel="Retry"
                            onAction={() => fetchApps(true)}
                        />
                    ) : visibleCategoryIds.length === 0 && !useDirectPackages ? (
                        <EmptyState
                            title="No apps found"
                            description={`No applications found${!repoFilter.includes('all') ? ` in selected repos` : ' in this category'}. Try selecting a different repo.`}
                            actionLabel={!repoFilter.includes('all') ? "Show All Repos" : undefined}
                            onAction={!repoFilter.includes('all') ? () => setRepoFilter(['all']) : undefined}
                        />
                    ) : (
                        <>
                            {/* Conditional Featured Section */}
                            {(() => {
                                const showFeaturedSplit = sortBy === 'featured';
                                const featuredPackages = showFeaturedSplit
                                    ? (useDirectPackages
                                        ? directCategoryPackages.filter((pkg) => pkg.is_featured)
                                        : visibleCategoryIds
                                            .map((id) => packageRegistry[id])
                                            .filter((pkg): pkg is Package => !!pkg && !!pkg.is_featured))
                                    : [];
                                const otherPackages = showFeaturedSplit
                                    ? (useDirectPackages
                                        ? directCategoryPackages.filter((pkg) => !pkg.is_featured)
                                        : visibleCategoryIds
                                            .map((id) => packageRegistry[id])
                                            .filter((pkg): pkg is Package => !!pkg && !pkg.is_featured))
                                    : (useDirectPackages
                                        ? directCategoryPackages
                                        : visibleCategoryIds
                                            .map((id) => packageRegistry[id])
                                            .filter((pkg): pkg is Package => !!pkg));

                                return (
                                    <>
                                        {showFeaturedSplit && featuredPackages.length > 0 && (
                                            <div className="mb-8">
                                                <h2 className="text-lg font-bold text-app-fg mb-4 flex items-center gap-2">
                                                    <span className="text-yellow-500">★</span> Featured Applications
                                                </h2>
                                                <PackageCardList
                                                    source={{ mode: 'packages', packages: featuredPackages }}
                                                    onSelectPackage={handleSelectPackage}
                                                    variant="grid"
                                                    setupRequiredResolver={(pkg) => isOnlyChaoticSource(pkg) && !chaoticEnabled}
                                                    onConfigureSource={onOpenSettings}
                                                    surfaceName="CategoryViewFeatured"
                                                />
                                                <div className="h-px bg-app-border/50 my-6" />
                                                <h2 className="text-lg font-bold text-app-fg mb-4">All Applications</h2>
                                            </div>
                                        )}

                                        <PackageCardList
                                            source={{ mode: 'packages', packages: otherPackages }}
                                            onSelectPackage={handleSelectPackage}
                                            variant="grid"
                                            setupRequiredResolver={(pkg) => isOnlyChaoticSource(pkg) && !chaoticEnabled}
                                            onConfigureSource={onOpenSettings}
                                            surfaceName="CategoryView"
                                        />
                                        {hasMore && <div ref={lastElementRef} className="h-4" />}
                                    </>
                                );
                            })()}

                            {/* Loading More Indicator */}
                            {loading && !initialLoad && (
                                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 mt-4">
                                    {[...Array(5)].map((_, i) => (
                                        <PackageCardSkeleton key={`more-${i}`} />
                                    ))}
                                </div>
                            )}
                        </>
                    )}
                </div>
            </div>
        </div>
    );
};

export default CategoryView;
