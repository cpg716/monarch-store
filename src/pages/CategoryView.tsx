import React, { useEffect, useState, useCallback, useMemo, useRef } from 'react';
import { useInfiniteScroll } from '../hooks/useInfiniteScroll';
import { ArrowLeft, LayoutGrid } from 'lucide-react';
import clsx from 'clsx';
import { commands, CategoryQuery } from '../services/bindings';
import type { Package } from '../services/bindings';
import PackageCard from '../components/PackageCard';
import PackageCardSkeleton from '../components/PackageCardSkeleton';
import EmptyState from '../components/EmptyState';
import { CATEGORIES } from '../components/CategoryGrid';
import { useErrorService } from '../context/ErrorContext';
import { useChaoticStatus, isOnlyChaoticSource } from '../hooks/useChaoticStatus';
import { useSettings } from '../hooks/useSettings';
import { getPackageListKey } from '../utils/packageKey';
import { useAppStore } from '../store/internal_store';
import { getSourceFamilyId, getSourceFamilyLabel } from '../utils/repoHelper';

import { unwrap } from '../utils/specta';

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
    const retryCountRef = useRef(0);
    const [sortBy, setSortBy] = useState<'featured' | 'name' | 'updated'>('featured');
    const [repoFilter, setRepoFilter] = useState<string[]>(['all']);
    const [page, setPage] = useState(1);
    const [hasMore, setHasMore] = useState(true);
    const [enabledRepos, setEnabledRepos] = useState<RepoState[]>([]);
    const [error, setError] = useState<string | null>(null);

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
    const CATEGORY_FETCH_TIMEOUT_MS = 45_000;
    const fetchApps = useCallback(async (reset: boolean = false) => {
        if (reset) {
            setLoading(true);
            setInitialLoad(true);
            setPage(1);
            setError(null);
        }

        const currentPage = reset ? 1 : page;

        const fetchWithTimeout = () => {
            const query: any = {
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
            };
            const req = commands.getCategoryPackagesPaginated(query).then(unwrap);
            return Promise.race([
                req,
                new Promise<never>((_, rej) =>
                    setTimeout(() => rej(new Error('Category load timed out')), CATEGORY_FETCH_TIMEOUT_MS)
                ),
            ]);
        };

        const repo_filter_val = repoFilter.includes('all') ? null : repoFilter;

        try {
            console.debug(`[CategoryView] Fetching ${category} page=${currentPage}...`);
            const res = (await fetchWithTimeout()) as any;
            console.debug(`[CategoryView] Fetch success for ${category}. Got ${res.packages.length} apps.`);

            setTotalPackages(parseInt(res.total, 10));
            upsertPackages(res.packages as any);

            // Safe batch rating fetch using IDs OR names (merges into live registry)
            const appIds = (res.packages as any[]).map(p => p.app_id || p.name).filter(id => !!id) as string[];
            if (appIds.length > 0) {
                fetchRatingsForPackages(appIds);
            }

            // One card per app: dedupe by list key so Arch + Flatpak never show as two cards.
            const ids = Array.from(new Set(res.packages.map((p: any) => getPackageListKey(p as any)))) as string[];
            if (reset) {
                setCategoryIds(category, ids);
                if (res.packages.length === 0 && parseInt(res.total, 10) === 0 && retryCountRef.current < 2) {
                    retryCountRef.current += 1;
                    console.warn(`[CategoryView] Empty results for ${category}. Retrying (${retryCountRef.current}/2)...`);
                    setTimeout(() => fetchApps(true), retryCountRef.current === 1 ? 3000 : 6000);
                }
            } else {
                appendCategoryIds(category, ids);
            }
            setHasMore(res.packages.length === LIMIT);
            setError(null);
        } catch (err: any) {
            console.error(`[CategoryView] Error loading category ${category}:`, err);
            const errorMsg = err instanceof Error ? err.message : JSON.stringify(err);
            setError(errorMsg);

            // If it's a timeout, we might want to automatically retry once more with a longer delay
            if (errorMsg.includes('timed out') && retryCountRef.current < 1) {
                retryCountRef.current += 1;
                console.warn(`[CategoryView] Timeout detected. Auto-retrying once...`);
                setTimeout(() => fetchApps(true), 2000);
            }
        } finally {
            setLoading(false);
            setInitialLoad(false);
        }
    }, [category, repoFilter, sortBy, page, isFlatpakEnabled, LIMIT, CATEGORY_FETCH_TIMEOUT_MS, upsertPackages, setCategoryIds, appendCategoryIds]);

    // Triggers
    // 1. Reset when Category/Filter/Sort changes
    useEffect(() => {
        retryCountRef.current = 0;
        fetchApps(true);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [category, repoFilter, sortBy, isFlatpakEnabled]); // Removed fetchApps to break dependency loop

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
            <div className="p-8 border-b border-app-border flex items-center justify-between bg-app-card/50 backdrop-blur-xl z-10 transition-colors">
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
                                ? `${totalPackages} Packages Total - ${categoryIds.length} Showing`
                                : `${categoryIds.length} packages loaded`
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
                    {initialLoad && categoryIds.length === 0 ? (
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
                    ) : categoryIds.length === 0 ? (
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
                                const featuredIds = showFeaturedSplit ? categoryIds.filter((id) => packageRegistry[id]?.is_featured) : [];
                                const otherIds = showFeaturedSplit ? categoryIds.filter((id) => !packageRegistry[id]?.is_featured) : categoryIds;

                                return (
                                    <>
                                        {showFeaturedSplit && featuredIds.length > 0 && (
                                            <div className="mb-8">
                                                <h2 className="text-lg font-bold text-app-fg mb-4 flex items-center gap-2">
                                                    <span className="text-yellow-500">★</span> Featured Applications
                                                </h2>
                                                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                                                    {featuredIds.map((id) => {
                                                        const pkg = packageRegistry[id];
                                                        return (
                                                            <PackageCard
                                                                key={id}
                                                                pkgId={id}
                                                                pkg={pkg}
                                                                onClick={() => handleSelectPackage(pkg)}
                                                                skipMetadataFetch={!!pkg?.icon}
                                                            />
                                                        );
                                                    })}
                                                </div>
                                                <div className="h-px bg-app-border/50 my-6" />
                                                <h2 className="text-lg font-bold text-app-fg mb-4">All Applications</h2>
                                            </div>
                                        )}

                                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                                            {otherIds.map((id, index) => {
                                                const pkg = packageRegistry[id];
                                                const isLast = index === otherIds.length - 1;
                                                return (
                                                    <div key={id} ref={isLast ? lastElementRef : null}>
                                                        <PackageCard
                                                            pkgId={id}
                                                            onClick={(p) => handleSelectPackage(p)}
                                                            setupRequired={pkg ? isOnlyChaoticSource(pkg) && !chaoticEnabled : false}
                                                            onConfigureSource={onOpenSettings}
                                                            skipMetadataFetch={!!pkg?.icon}
                                                        />
                                                    </div>
                                                );
                                            })}
                                        </div>
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
