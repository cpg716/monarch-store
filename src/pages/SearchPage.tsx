import { useState, useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Search, Clock, X, Sparkles, TrendingUp, Heart } from 'lucide-react';
import { useSearchHistory } from '../hooks/useSearchHistory';
import { useFavorites } from '../hooks/useFavorites';
import PackageCard, { Package } from '../components/PackageCard';
import SkeletonCard from '../components/SkeletonCard';
import EmptyState from '../components/EmptyState';
import { clsx } from 'clsx';

interface SearchPageProps {
    query: string;
    onQueryChange: (query: string) => void;
    packages: Package[];
    loading: boolean;
    onSelectPackage: (pkg: Package) => void;
    enabledRepos: { name: string; enabled: boolean; source: any }[];
    error?: string | null;
    onRetry?: () => void;
}

export default function SearchPage({
    query,
    onQueryChange,
    packages,
    loading,
    onSelectPackage,
    enabledRepos,
    error,
    onRetry
}: SearchPageProps) {
    const { history, removeSearch, clearHistory } = useSearchHistory();
    const { favorites } = useFavorites();
    const [activeFilter, setActiveFilter] = useState('all');
    const [sortBy, setSortBy] = useState<'best_match' | 'name' | 'updated'>('best_match');
    const [displayLimit, setDisplayLimit] = useState(50);
    const filterChipsRef = useRef<HTMLDivElement>(null);

    // When magic keyword (@aur, @chaotic, @official) is used, scroll filter chips into view
    useEffect(() => {
        if (query.trim().startsWith('@') && filterChipsRef.current) {
            filterChipsRef.current.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
        }
    }, [query]);

    // Filtered & Sorted results
    const getFilteredResults = () => {
        const safePackages = packages || [];
        let results = [...safePackages];
        let currentFilter = activeFilter;

        // Magic Keyword Detection (@aur, @chaotic, @official)
        if (query.trim().startsWith('@')) {
            const parts = query.trim().split(' ');
            const magic = parts[0].toLowerCase();
            if (magic === '@aur') currentFilter = 'aur';
            else if (magic === '@chaotic') currentFilter = 'chaotic';
            else if (magic === '@official') currentFilter = 'official';
        }

        if (currentFilter !== 'all') {
            results = results.filter(p => {
                const sourceId = typeof p.source === 'string' ? p.source : p.source.id;
                const matchesSource = sourceId === currentFilter || (typeof p.source !== 'string' && p.source.source_type === currentFilter);

                const matchesAvailable = p.available_sources?.some(s => s.id === currentFilter || s.source_type === currentFilter);

                return matchesSource || matchesAvailable;
            });
        }
        return results;
    };

    const sortedResults = getFilteredResults().sort((a, b) => {
        if (sortBy === 'name') {
            return (a.display_name || a.name).localeCompare(b.display_name || b.name);
        } else if (sortBy === 'updated') {
            return (b.last_modified || 0) - (a.last_modified || 0);
        }
        return 0; // Default backend order
    });

    const displayed = sortedResults.slice(0, displayLimit);

    // [NOVICE] Windows App Aliases
    const aliases: Record<string, string> = {
        'photoshop': 'GIMP',
        'illustrator': 'Inkscape',
        'word': 'LibreOffice',
        'excel': 'LibreOffice',
        'powerpoint': 'LibreOffice',
        'outlook': 'Thunderbird',
        'notepad': 'Gedit',
        'chrome': 'Google Chrome',
        'edge': 'Microsoft Edge',
        'paint': 'Krita',
        'task manager': 'Stacer'
    };
    const didYouMean = aliases[query.toLowerCase()];

    return (
        <div className="flex-1 flex flex-col h-full overflow-hidden bg-app-bg">
            <div className="p-8 pb-4 space-y-6">
                {/* Search Header Info */}
                <div className="flex items-center justify-between">
                    <div>
                        <h2 className="text-2xl font-black text-app-fg flex items-center gap-2">
                            <Search className="text-accent" size={24} />
                            {query ? `Search Results` : 'Explore'}
                        </h2>
                        <p className="text-app-muted text-sm capitalize">
                            {query ? `${(packages || []).length} apps matching "${query}"` : 'Discover your next favorite app'}
                        </p>
                    </div>

                    {query && (
                        <div className="flex items-center gap-4">
                            {/* Sort select */}
                            <div className="flex items-center gap-2 bg-app-card border border-app-border rounded-xl px-3 py-1.5 shadow-sm">
                                <span className="text-[10px] font-bold text-app-muted uppercase tracking-wider">Sort:</span>
                                <select
                                    value={sortBy}
                                    onChange={(e) => setSortBy(e.target.value as any)}
                                    className="bg-transparent text-sm font-bold text-app-fg outline-none cursor-pointer"
                                >
                                    <option value="best_match">Relevant</option>
                                    <option value="name">Name</option>
                                    <option value="updated">Newest</option>
                                </select>
                            </div>
                        </div>
                    )}
                </div>

                {/* Filter Chips */}
                {query && (
                    <div ref={filterChipsRef} className="flex items-center gap-2 overflow-x-auto pb-2 no-scrollbar">
                        <button
                            onClick={() => setActiveFilter('all')}
                            className={clsx(
                                "px-4 py-2 rounded-full text-xs font-bold transition-all border whitespace-nowrap",
                                activeFilter === 'all'
                                    ? "bg-blue-600 border-blue-600 text-white shadow-lg shadow-blue-500/20"
                                    : "bg-app-card border-app-border text-app-muted hover:border-app-fg/30"
                            )}
                        >
                            All ({(packages || []).length})
                        </button>
                        {(() => {
                            // Unify Repos into Families
                            const families = new Map<string, { label: string; count: number; sources: string[] }>();
                            enabledRepos.forEach(repo => {
                                const sourceObj = typeof repo.source === 'string' ? { id: repo.source, label: repo.source, source_type: repo.source } : repo.source;
                                const family = sourceObj.id;
                                let label = sourceObj.label || (family.charAt(0).toUpperCase() + family.slice(1));

                                if (family === 'chaotic-aur') label = 'AUR-Binaries';
                                if (family === 'aur') label = 'AUR-Source';
                                if (family === 'official' || family === 'core') label = 'Official';
                                if (family === 'manjaro') label = 'Manjaro';
                                if (family === 'cachyos') label = 'CachyOS';

                                const count = (packages || []).filter(p => {
                                    const pSourceId = typeof p.source === 'string' ? p.source : p.source.id;
                                    return pSourceId === family || (typeof p.source !== 'string' && p.source.source_type === family);
                                }).length;
                                if (count === 0) return;

                                if (families.has(family)) {
                                    families.get(family)!.count += count;
                                    families.get(family)!.sources.push(repo.source);
                                } else {
                                    families.set(family, { label, count, sources: [repo.source] });
                                }
                            });

                            // [Manual] Add Flatpak if present in results
                            const flatpakCount = (packages || []).filter(p =>
                                (typeof p.source === 'string' ? p.source === 'flatpak' : p.source.source_type === 'flatpak') ||
                                (p.available_sources && p.available_sources.some(s => s.source_type === 'flatpak'))
                            ).length;

                            if (flatpakCount > 0) {
                                families.set('flatpak', { label: 'Flatpak', count: flatpakCount, sources: ['flatpak'] });
                            }

                            return Array.from(families.entries()).map(([id, family]) => (
                                <button
                                    key={id}
                                    onClick={() => setActiveFilter(id)}
                                    className={clsx(
                                        "px-4 py-2 rounded-full text-xs font-bold transition-all border whitespace-nowrap",
                                        activeFilter === id
                                            ? "bg-blue-600 border-blue-600 text-white shadow-lg shadow-blue-500/20"
                                            : "bg-app-card border-app-border text-app-muted hover:border-app-fg/30"
                                    )}
                                >
                                    {family.label} ({family.count})
                                </button>
                            ));
                        })()}
                    </div>
                )}

                {!query && (
                    <div className="flex flex-wrap gap-2 pt-2 text-[11px] text-app-muted">
                        {[
                            { token: '@official', label: 'Official Repos' },
                            { token: '@aur', label: 'AUR Source' },
                            { token: '@chaotic', label: 'Chaotic-AUR' }
                        ].map((shortcut) => (
                            <button
                                key={shortcut.token}
                                onClick={() => onQueryChange(`${shortcut.token} `)}
                                className="px-3 py-1.5 rounded-full border border-app-border text-app-fg/80 accent-hover-outline"
                                type="button"
                                aria-label={`Filter results by ${shortcut.label}`}
                            >
                                <span className="font-mono text-xs">{shortcut.token}</span>
                                <span className="ml-2 text-[10px] uppercase tracking-wide opacity-70">{shortcut.label}</span>
                            </button>
                        ))}
                    </div>
                )}
            </div>

            <div className="flex-1 overflow-y-auto p-8 pt-0 custom-scrollbar">
                <AnimatePresence mode="wait">
                    {!query ? (
                        <motion.div
                            key="pre-search"
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            exit={{ opacity: 0, scale: 0.98 }}
                            className="grid grid-cols-1 md:grid-cols-2 gap-10 max-w-5xl mx-auto pt-4"
                        >
                            {/* Recent Searches */}
                            {history.length > 0 && (
                                <div className="space-y-4">
                                    <div className="flex items-center justify-between">
                                        <h3 className="text-sm font-bold text-app-muted uppercase tracking-widest flex items-center gap-2">
                                            <Clock size={16} /> Recent Searches
                                        </h3>
                                        <button onClick={clearHistory} className="text-[10px] font-bold text-red-500 hover:text-red-400">Clear All</button>
                                    </div>
                                    <div className="space-y-2">
                                        {history.map(item => (
                                            <div
                                                key={item}
                                                onClick={() => onQueryChange(item)}
                                                className="group flex items-center justify-between p-3 rounded-xl bg-app-card/30 border border-app-border/50 hover:bg-app-card/60 cursor-pointer transition-all accent-hover-outline"
                                            >
                                                <div className="flex items-center gap-3">
                                                    <Search size={14} className="text-app-muted group-hover:text-accent transition-colors" />
                                                    <span className="text-sm text-app-fg">{item}</span>
                                                </div>
                                                <button
                                                    onClick={(e) => { e.stopPropagation(); removeSearch(item); }}
                                                    className="p-1 rounded-md hover:bg-red-500/10 text-app-muted hover:text-red-500 transition-colors opacity-0 group-hover:opacity-100"
                                                >
                                                    <X size={12} />
                                                </button>
                                            </div>
                                        ))}
                                    </div>
                                </div>
                            )}

                            {/* Suggestions / Quick Actions */}
                            <div className="space-y-6">
                                <div className="space-y-4">
                                    <h3 className="text-sm font-bold text-app-muted uppercase tracking-widest flex items-center gap-2">
                                        <Sparkles size={16} /> Quick Filters
                                    </h3>
                                    <div className="grid grid-cols-2 gap-3">
                                        <button
                                            onClick={() => onQueryChange("top:trending")}
                                            className="p-4 rounded-2xl border flex flex-col items-center gap-2 hover:scale-[1.02] transition-all group accent-hover-outline"
                                            style={{
                                                background: 'linear-gradient(135deg, color-mix(in srgb, var(--app-accent) 18%, transparent), transparent 70%)',
                                                borderColor: 'color-mix(in srgb, var(--app-accent) 30%, transparent)'
                                            }}
                                        >
                                            <TrendingUp className="text-accent group-hover:scale-110 transition-transform" />
                                            <span className="text-xs font-bold text-app-fg">Browse Trending</span>
                                        </button>
                                        <button
                                            onClick={() => onQueryChange("top:new")}
                                            className="p-4 rounded-2xl bg-gradient-to-br from-purple-500/10 to-pink-500/5 border border-purple-500/20 flex flex-col items-center gap-2 hover:scale-[1.02] transition-all group"
                                        >
                                            <Sparkles className="text-purple-500 group-hover:scale-110 transition-transform" />
                                            <span className="text-xs font-bold text-app-fg">New Arrivals</span>
                                        </button>
                                    </div>
                                </div>

                                {favorites.length > 0 && (
                                    <div className="space-y-4">
                                        <h3 className="text-sm font-bold text-app-muted uppercase tracking-widest flex items-center gap-2">
                                            <Heart size={16} className="text-red-500" /> From Your Favorites
                                        </h3>
                                        <div className="flex flex-wrap gap-2">
                                            {favorites.slice(0, 8).map(fav => (
                                                <button
                                                    key={fav}
                                                    onClick={() => onQueryChange(fav)}
                                                    className="px-3 py-1.5 rounded-full bg-app-card border border-app-border text-xs text-app-fg hover:border-red-500/30 hover:bg-red-500/5 transition-all"
                                                >
                                                    {fav}
                                                </button>
                                            ))}
                                        </div>
                                    </div>
                                )}
                            </div>
                        </motion.div>
                    ) : (
                        <motion.div
                            key="results"
                            initial={{ opacity: 0 }}
                            animate={{ opacity: 1 }}
                            className="space-y-8"
                        >
                            {error ? (
                                <EmptyState
                                    icon={X}
                                    title="Search Failed"
                                    description={`We encountered an error while searching for "${query}".\n${error}`}
                                    actionLabel="Try Again"
                                    onAction={() => {
                                        (onRetry || (() => onQueryChange('')))();
                                    }}
                                    variant="error"
                                />
                            ) : loading && packages.length === 0 ? (
                                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6 max-w-7xl mx-auto w-full">
                                    {[...Array(8)].map((_, i) => (
                                        <SkeletonCard key={i} />
                                    ))}
                                </div>
                            ) : displayed.length === 0 ? (
                                <EmptyState
                                    title="No apps found"
                                    description={didYouMean ? `Did you mean '${didYouMean}'? Arch uses different apps than Windows.` : `We couldn't find any packages matching "${query}"${activeFilter !== 'all' ? ` in the ${activeFilter} source` : ''}.`}
                                    actionLabel={didYouMean ? `Search for ${didYouMean}` : "Clear filters & search again"}
                                    onAction={() => {
                                        if (didYouMean) onQueryChange(didYouMean);
                                        else { onQueryChange(''); setActiveFilter('all'); }
                                    }}
                                />
                            ) : (
                                <>
                                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6 max-w-7xl mx-auto w-full">
                                        {displayed.map((pkg) => (
                                            <PackageCard
                                                key={`${pkg.name}-${pkg.source}`}
                                                pkg={pkg}
                                                onClick={() => onSelectPackage(pkg)}
                                            />
                                        ))}
                                    </div>
                                    {sortedResults.length > displayLimit && (
                                        <div className="flex justify-center pt-8">
                                            <button
                                                onClick={() => setDisplayLimit(prev => prev + 50)}
                                                className="px-10 py-3 rounded-2xl bg-app-card border border-app-border text-app-fg font-bold hover:bg-app-card/60 transition-all active:scale-95 shadow-lg"
                                            >
                                                Load More Results
                                            </button>
                                        </div>
                                    )}
                                </>
                            )}
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>
        </div>
    );
}
