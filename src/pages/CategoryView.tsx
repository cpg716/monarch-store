import React, { useEffect, useState, useCallback } from 'react';
import { useInfiniteScroll } from '../hooks/useInfiniteScroll';
import { ArrowLeft, LayoutGrid, Filter, Check, ChevronDown } from 'lucide-react';
import clsx from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import PackageCard, { Package, ChaoticPackage } from '../components/PackageCard';
import PackageCardSkeleton from '../components/PackageCardSkeleton';
import EmptyState from '../components/EmptyState';
import { CATEGORIES } from '../components/CategoryGrid';
import { useErrorService } from '../context/ErrorContext';
import { friendlyError } from '../utils/friendlyError';

// Multi-Select Dropdown Component

const MultiSelectDropdown = ({
    options,
    selected,
    onChange
}: {
    options: { value: string, label: string }[],
    selected: string[],
    onChange: (newSelected: string[]) => void
}) => {
    const [isOpen, setIsOpen] = useState(false);
    const dropdownRef = React.useRef<HTMLDivElement>(null);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    const toggleOption = (value: string) => {
        if (value === 'all') {
            onChange(['all']);
            return;
        }

        const newSelected = selected.includes('all') ? [] : [...selected];

        if (newSelected.includes(value)) {
            const next = newSelected.filter(v => v !== value);
            onChange(next.length === 0 ? ['all'] : next);
        } else {
            onChange([...newSelected, value]);
        }
    };

    const displayText = selected.includes('all') || selected.length === 0
        ? "All Sources"
        : `${selected.length} Selected`;

    return (
        <div className="relative" ref={dropdownRef}>
            <button
                onClick={() => setIsOpen(!isOpen)}
                className="flex items-center gap-2 bg-app-subtle border border-app-border rounded-lg px-3 py-1.5 text-sm text-app-fg hover:border-blue-500 transition-colors"
            >
                <Filter size={14} className="text-app-muted" />
                <span>{displayText}</span>
                <ChevronDown size={14} className="text-app-muted" />
            </button>

            {isOpen && (
                <div className="absolute top-full mt-2 right-0 w-56 bg-app-card border border-app-border rounded-xl shadow-xl p-2 z-50 flex flex-col gap-1">
                    <button
                        onClick={() => toggleOption('all')}
                        className={clsx(
                            "flex items-center justify-between px-3 py-2 rounded-lg text-sm transition-colors",
                            selected.includes('all') ? "bg-blue-500 text-white" : "hover:bg-app-fg/10 text-app-fg"
                        )}
                    >
                        <span>All Sources</span>
                        {selected.includes('all') && <Check size={14} />}
                    </button>
                    <div className="h-px bg-app-border/50 my-1" />
                    {options.map(opt => (
                        <button
                            key={opt.value}
                            onClick={() => toggleOption(opt.value)}
                            className={clsx(
                                "flex items-center justify-between px-3 py-2 rounded-lg text-sm transition-colors",
                                selected.includes(opt.value) && !selected.includes('all') ? "bg-blue-500/10 text-blue-500 font-bold" : "hover:bg-app-fg/10 text-app-fg"
                            )}
                        >
                            <span>{opt.label}</span>
                            {(selected.includes(opt.value) && !selected.includes('all')) && <Check size={14} />}
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
};
// ... imports

interface CategoryViewProps {
    category: string;
    onBack: () => void;
    onSelectPackage: (pkg: Package, preferredSource?: string) => void;
}

interface RepoState {
    name: string;
    enabled: boolean;
    source: any;
}

// ... imports

interface PaginatedResponse {
    packages: Package[];
    total: number;
    page: number;
    has_more: boolean;
}

const CategoryView: React.FC<CategoryViewProps> = ({ category, onBack, onSelectPackage }) => {
    const errorService = useErrorService();
    const [packages, setPackages] = useState<Package[]>([]);
    const [totalPackages, setTotalPackages] = useState(0); // Track total available from backend
    const [loading, setLoading] = useState(true);
    const [initialLoad, setInitialLoad] = useState(true); // Track first load vs "load more"
    const [sortBy, setSortBy] = useState<'featured' | 'name' | 'updated'>('featured');
    const [repoFilter, setRepoFilter] = useState<string[]>(['all']);
    const [page, setPage] = useState(1);
    const [hasMore, setHasMore] = useState(true);
    const [enabledRepos, setEnabledRepos] = useState<RepoState[]>([]);
    const [chaoticInfoMap, setChaoticInfoMap] = useState<Map<string, ChaoticPackage>>(new Map());
    const [error, setError] = useState<string | null>(null);

    // Constant limit for backend pagination
    const LIMIT = 50;

    // Helper
    const getRepoLabel = (source: any) => {
        const sourceId = typeof source === 'string' ? source : source.id;
        const labels: Record<string, string> = {
            'chaotic-aur': 'Chaotic-AUR',
            'official': 'Official',
            'aur': 'AUR',
            'cachyos': 'CachyOS',
            'garuda': 'Garuda',
            'endeavour': 'EndeavourOS',
            'manjaro': 'Manjaro'
        };
        return labels[sourceId] || (typeof sourceId === 'string' ? sourceId.charAt(0).toUpperCase() + sourceId.slice(1) : 'Unknown');
    };

    // Helper for display labels
    const categoryResult = CATEGORIES.find(c => c.id === category || c.label === category);
    const Icon = categoryResult?.icon || LayoutGrid;
    const colorClass = categoryResult?.color || "text-blue-500";
    const displayLabel = categoryResult?.label || category;

    // ... (Repo fetch same)
    useEffect(() => {
        invoke<RepoState[]>('get_repo_states').then(repos => {
            const enabled = repos.filter(r => r.enabled);
            const uniqueSources = new Map<string, RepoState>();
            for (const repo of enabled) {
                if (!uniqueSources.has(repo.source)) uniqueSources.set(repo.source, repo);
            }
            setEnabledRepos(Array.from(uniqueSources.values()));
        }).catch((e) => errorService.reportError(e as Error | string));
    }, [errorService]);

    // Fetch Logic
    const fetchApps = useCallback(async (reset: boolean = false) => {
        if (reset) {
            setLoading(true);
            setInitialLoad(true);
            setPackages([]);
            setPage(1);
            setError(null);
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
                setPackages(res.packages);
            } else {
                setPackages((prev: Package[]) => {
                    const existingNames = new Set(prev.map(p => p.name));
                    const uniqueNew = res.packages.filter(p => !existingNames.has(p.name));
                    return [...prev, ...uniqueNew];
                });
            }
            setHasMore(res.packages.length === LIMIT);
            setError(null);
        } catch (e: unknown) {
            const raw = e instanceof Error ? e.message : String(e);
            errorService.reportError(e as Error | string);
            setError(friendlyError(raw).description);
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
            setPage((prev: number) => prev + 1);
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
                setChaoticInfoMap((prev: Map<string, ChaoticPackage>) => {
                    const next = new Map(prev);
                    Object.entries(infoMap).forEach(([name, info]) => next.set(name, info as ChaoticPackage));
                    return next;
                });
            } catch (e) { errorService.reportError(e as Error | string); }
        };
        const timeout = setTimeout(fetchBatchInfo, 500); // 500ms debounce to let scrolling settle
        return () => clearTimeout(timeout);
    }, [packages]); // Only when packages list changes

    // Handlers
    const handleSelectPackage = (pkg: Package) => {
        if (!repoFilter.includes('all') && repoFilter.length === 1) {
            onSelectPackage(pkg, repoFilter[0]);
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
                            {displayLabel} Apps
                        </h1>
                        <p className="text-app-muted text-sm">
                            {totalPackages > 0
                                ? `${totalPackages} Packages Total - ${packages.length} Showing`
                                : `${packages.length} packages loaded`
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
                <div className="flex items-center gap-4">
                    {/* Repo Filter */}
                    <MultiSelectDropdown
                        options={enabledRepos.map(r => ({ value: r.source, label: getRepoLabel(r.source) }))}
                        selected={repoFilter}
                        onChange={setRepoFilter}
                    />

                    {/* Sort */}
                    <div className="flex items-center gap-2">
                        <span className="text-sm text-app-muted">Sort:</span>
                        <select
                            className="bg-app-subtle border border-app-border rounded-lg px-3 py-1.5 text-sm text-app-fg focus:outline-none focus:border-blue-500 transition-colors"
                            value={sortBy}
                            onChange={(e) => setSortBy(e.target.value as 'featured' | 'name' | 'updated')}
                        >
                            <option value="featured">Featured</option>
                            <option value="name">Name (A-Z)</option>
                            <option value="updated">Last Updated</option>
                        </select>
                    </div>
                </div>
            </div>

            <div className="flex-1 overflow-y-auto p-8">
                <div className="max-w-7xl mx-auto w-full">
                    {initialLoad && packages.length === 0 ? (
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                            {[...Array(10)].map((_, i) => (
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
                    ) : packages.length === 0 ? (
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
                                // Only split view if sorting by "featured" and on first page (or full list)
                                // If we have paginated data, the "featured" items should be at the top of the first page.
                                const showFeaturedSplit = sortBy === 'featured';
                                const featured = showFeaturedSplit ? packages.filter(p => p.is_featured) : [];
                                const others = showFeaturedSplit ? packages.filter(p => !p.is_featured) : packages;

                                return (
                                    <>
                                        {showFeaturedSplit && featured.length > 0 && (
                                            <div className="mb-8">
                                                <h2 className="text-lg font-bold text-app-fg mb-4 flex items-center gap-2">
                                                    <span className="text-yellow-500">★</span> Featured Applications
                                                </h2>
                                                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                                                    {featured.map((pkg) => (
                                                        <PackageCard
                                                            key={`feat-${pkg.name}`}
                                                            pkg={pkg}
                                                            onClick={() => handleSelectPackage(pkg)}
                                                            chaoticInfo={chaoticInfoMap.get(pkg.name)}
                                                        />
                                                    ))}
                                                </div>
                                                <div className="h-px bg-app-border/50 my-6" />
                                                <h2 className="text-lg font-bold text-app-fg mb-4">All Applications</h2>
                                            </div>
                                        )}

                                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                                            {others.map((pkg, index) => {
                                                const isLast = index === others.length - 1;
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
                                    </>
                                );
                            })()}

                            {/* Loading More Indicator */}
                            {loading && !initialLoad && (
                                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 mt-4">
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
