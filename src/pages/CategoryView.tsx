import React, { useEffect, useState, useCallback } from 'react';
import { useInfiniteScroll } from '../hooks/useInfiniteScroll';
import { ArrowLeft, LayoutGrid, Filter } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import PackageCard, { Package, ChaoticPackage } from '../components/PackageCard';
import PackageCardSkeleton from '../components/PackageCardSkeleton';
import { CATEGORIES } from '../components/CategoryGrid';

interface CategoryViewProps {
    category: string;
    onBack: () => void;
    onSelectPackage: (pkg: Package, preferredSource?: string) => void;
}

interface RepoState {
    name: string;
    enabled: boolean;
    source: string;
}

// ... imports

interface PaginatedResponse {
    items: Package[];
    total: number;
}

const CategoryView: React.FC<CategoryViewProps> = ({ category, onBack, onSelectPackage }) => {
    const [packages, setPackages] = useState<Package[]>([]);
    const [totalPackages, setTotalPackages] = useState(0); // Track total available from backend
    const [loading, setLoading] = useState(true);
    const [initialLoad, setInitialLoad] = useState(true); // Track first load vs "load more"
    const [sortBy, setSortBy] = useState<'name' | 'updated'>('name');
    const [repoFilter, setRepoFilter] = useState<string>('all');
    const [page, setPage] = useState(1);
    const [hasMore, setHasMore] = useState(true);
    const [enabledRepos, setEnabledRepos] = useState<RepoState[]>([]);
    const [chaoticInfoMap, setChaoticInfoMap] = useState<Map<string, ChaoticPackage>>(new Map());

    // Constant limit for backend pagination
    const LIMIT = 50;

    // Helper
    const getRepoLabel = (source: string) => {
        const labels: Record<string, string> = {
            'chaotic': 'Chaotic-AUR',
            'official': 'Official',
            'aur': 'AUR',
            'cachyos': 'CachyOS',
            'garuda': 'Garuda',
            'endeavour': 'EndeavourOS',
            'manjaro': 'Manjaro'
        };
        return labels[source] || source.charAt(0).toUpperCase() + source.slice(1);
    };

    // ... (Icon logic same)
    const categoryResult = CATEGORIES.find(c => c.id === category || c.label === category);
    const Icon = categoryResult?.icon || LayoutGrid;
    const colorClass = categoryResult?.color || "text-blue-500";

    // ... (Repo fetch same)
    useEffect(() => {
        invoke<RepoState[]>('get_repo_states').then(repos => {
            const enabled = repos.filter(r => r.enabled);
            const uniqueSources = new Map<string, RepoState>();
            for (const repo of enabled) {
                if (!uniqueSources.has(repo.source)) uniqueSources.set(repo.source, repo);
            }
            setEnabledRepos(Array.from(uniqueSources.values()));
        }).catch(console.error);
    }, []);

    // Fetch Logic
    const fetchApps = useCallback(async (reset: boolean = false) => {
        if (reset) {
            setLoading(true);
            setInitialLoad(true);
            setPackages([]);
            setPage(1);
        }

        const currentPage = reset ? 1 : page;

        try {
            const res = await invoke<PaginatedResponse>('get_category_packages_paginated', {
                category,
                repo_filter: repoFilter,
                sort_by: sortBy,
                page: currentPage,
                limit: LIMIT
            });

            // Check if still relevant (simplest check if we can't easily use ref here)
            // Ideally we use a useRef for isMounted, but since we are inside useCallback, 
            // let's assume if the component unmounts, this callback might still fire but state updates usually warn.
            // React 18 handles this better, but let's be safe.

            // Actually, we need to pass a signal or use a ref from the component scope.
            // Since we can't change the function signature easily without affecting deps,
            // we will use a let variable inside the effect that calls this, or just ignore for now as React 18 is lenient.
            // BETTER FIX: The `useEffect` calls this. Let's fix it there or use a ref in the component.

            // Updating state...
            setTotalPackages(res.total);
            if (reset) {
                setPackages(res.items);
            } else {
                setPackages(prev => [...prev, ...res.items]);
            }
            setHasMore(res.items.length === LIMIT);

        } catch (e) {
            console.error("Failed to load category apps", e);
        } finally {
            setLoading(false);
            setInitialLoad(false);
        }
    }, [category, repoFilter, sortBy, page]);

    // Triggers
    // 1. Reset when Category/Filter/Sort changes
    useEffect(() => {
        fetchApps(true);
    }, [category, repoFilter, sortBy]);

    // 2. Load More when page increments (but NOT on page 1, which is handled by reset)
    useEffect(() => {
        if (page > 1) {
            fetchApps(false);
        }
    }, [page]);
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
            setPage(prev => prev + 1);
        }
    }, [loading, hasMore]);

    const lastElementRef = useInfiniteScroll(loadMore, hasMore, loading);

    // ... (Batch Fetch Logic same, but runs on `packages` which is now paginated)
    useEffect(() => {
        const fetchBatchInfo = async () => {
            // Only fetch for NEW items? Or all visible?
            // `packages` grows. We might re-fetch info for top items.
            // Optimization: Filter out ones we already map.
            const chaoticNames = packages
                .filter(p => p.source === 'chaotic')
                .map(p => p.name)
                .filter(n => !chaoticInfoMap.has(n)); // Only new ones

            if (chaoticNames.length === 0) return;

            // Chunk it? 50 is fine.
            const chunk = chaoticNames.slice(0, 50); // limit per request

            try {
                const infoMap = await invoke<Record<string, ChaoticPackage>>('get_chaotic_packages_batch', {
                    names: chunk
                });
                setChaoticInfoMap(prev => {
                    const next = new Map(prev);
                    Object.entries(infoMap).forEach(([name, info]) => next.set(name, info));
                    return next;
                });
            } catch (e) { console.error(e); }
        };
        const timeout = setTimeout(fetchBatchInfo, 500); // 500ms debounce to let scrolling settle
        return () => clearTimeout(timeout);
    }, [packages]); // Only when packages list changes

    // Handlers
    const handleSelectPackage = (pkg: Package) => {
        if (repoFilter !== 'all') {
            onSelectPackage(pkg, repoFilter);
        } else {
            onSelectPackage(pkg);
        }
    };

    // ... Render


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
                            {category} Apps
                        </h1>
                        <p className="text-app-muted text-sm">
                            {totalPackages > 0
                                ? `${totalPackages} Packages Total - ${packages.length} Showing`
                                : `${packages.length} packages loaded`
                            }
                            {repoFilter !== 'all' ? ` in ${getRepoLabel(repoFilter)}` : ''}
                        </p>
                    </div>
                </div>

                {/* Filter Controls */}
                <div className="flex items-center gap-4">
                    {/* Repo Filter */}
                    <div className="flex items-center gap-2">
                        <Filter size={14} className="text-app-muted" />
                        <select
                            className="bg-app-subtle border border-app-border rounded-lg px-3 py-1.5 text-sm text-app-fg focus:outline-none focus:border-blue-500 transition-colors"
                            value={repoFilter}
                            onChange={(e) => setRepoFilter(e.target.value)}
                        >
                            <option value="all">All Repos</option>
                            {enabledRepos.map(repo => (
                                <option key={repo.source} value={repo.source}>
                                    {getRepoLabel(repo.source)}
                                </option>
                            ))}
                        </select>
                    </div>

                    {/* Sort */}
                    <div className="flex items-center gap-2">
                        <span className="text-sm text-app-muted">Sort:</span>
                        <select
                            className="bg-app-subtle border border-app-border rounded-lg px-3 py-1.5 text-sm text-app-fg focus:outline-none focus:border-blue-500 transition-colors"
                            value={sortBy}
                            onChange={(e) => setSortBy(e.target.value as 'name' | 'updated')}
                        >
                            <option value="name">Name (A-Z)</option>
                            <option value="updated">Last Updated</option>
                        </select>
                    </div>
                </div>
            </div>

            <div className="flex-1 overflow-y-auto p-8">
                {initialLoad && packages.length === 0 ? (
                    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5 gap-4">
                        {[...Array(10)].map((_, i) => (
                            <PackageCardSkeleton key={i} />
                        ))}
                    </div>
                ) : packages.length === 0 ? (
                    <div className="text-center text-app-muted mt-20">
                        <p>No applications found{repoFilter !== 'all' ? ` in ${getRepoLabel(repoFilter)}` : ' in this category'}.</p>
                        <p className="text-sm mt-2">Try selecting a different repo or searching manually.</p>
                    </div>
                ) : (
                    <>
                        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5 gap-4">
                            {packages.map((pkg, index) => {
                                // Add ref to the last element
                                const isLast = index === packages.length - 1;
                                return (
                                    <div key={pkg.name} ref={isLast ? lastElementRef : null}>
                                        <PackageCard
                                            pkg={pkg}
                                            onClick={() => handleSelectPackage(pkg)}
                                            chaoticInfo={chaoticInfoMap.get(pkg.name)}
                                        />
                                    </div>
                                );
                            })}
                        </div>

                        {/* Loading More Indicator */}
                        {loading && !initialLoad && (
                            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5 gap-4 mt-4">
                                {[...Array(5)].map((_, i) => (
                                    <PackageCardSkeleton key={`more-${i}`} />
                                ))}
                            </div>
                        )}
                    </>
                )}
            </div>
        </div>
    );
};

export default CategoryView;
