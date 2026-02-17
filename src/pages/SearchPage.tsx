import { useState, useRef, useEffect, useMemo, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { VList } from 'virtua';
import { Search, Clock, X, Sparkles, TrendingUp, Heart } from 'lucide-react';
import { useSearchHistory } from '../hooks/useSearchHistory';
import { getPackageListKey } from '../utils/packageKey';
import { useFavorites } from '../hooks/useFavorites';
import PackageCard from '../components/PackageCard';
import type { Package } from '../services/bindings';
import { useAppStore } from '../store/internal_store';
import PackageCardSkeleton from '../components/PackageCardSkeleton';
import EmptyState from '../components/EmptyState';
import { clsx } from 'clsx';
import { useChaoticStatus, isOnlyChaoticSource } from '../hooks/useChaoticStatus';
import { getSourceFamilyId, getSourceFamilyLabel } from '../utils/repoHelper';


const GRID_COLS = 3;
function chunk<T>(arr: T[], size: number): T[][] {
    const out: T[][] = [];
    for (let i = 0; i < arr.length; i += size) {
        out.push(arr.slice(i, i + size));
    }
    return out;
}

interface SearchPageProps {
    query: string;
    onQueryChange: (query: string) => void;
    packages: Package[];
    loading: boolean;
    onSelectPackage: (pkg: Package) => void;
    enabledRepos: { name: string; enabled: boolean; source: any }[];
    error?: string | null;
    onRetry?: () => void;
    onOpenSettings?: () => void;
}

export default function SearchPage({
    query,
    onQueryChange,
    packages: _packagesProp,
    loading,
    onSelectPackage,
    enabledRepos,
    error,
    onRetry,
    onOpenSettings
}: SearchPageProps) {
    const { history, removeSearch, clearHistory } = useSearchHistory();
    const { favorites } = useFavorites();
    const { enabled: chaoticEnabled } = useChaoticStatus();
    const searchResultIds = useAppStore((s) => s.searchResultIds);
    const packageRegistry = useAppStore((s) => s.packageRegistry);
    const favoriteSet = useMemo(() => new Set(favorites.map((f) => f.toLowerCase())), [favorites]);
    const [activeFilter, setActiveFilter] = useState('all');
    const [sortBy, setSortBy] = useState<'best_match' | 'name' | 'updated'>('best_match');
    const filterChipsRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (query.trim().startsWith('@') && filterChipsRef.current) {
            filterChipsRef.current.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
        }
    }, [query]);

    let currentFilter = activeFilter;
    if (query.trim().startsWith('@')) {
        const magic = query.trim().split(' ')[0].toLowerCase();
        if (magic === '@aur') currentFilter = 'aur';
        else if (magic === '@chaotic') currentFilter = 'chaotic';
        else if (magic === '@official') currentFilter = 'official';
    }

    const sortedIds = useMemo(() => {
        const pkgs = searchResultIds
            .map((id) => packageRegistry[id])
            .filter((p): p is Package => p != null);
        let filtered = pkgs;
        if (currentFilter !== 'all') {
            filtered = pkgs.filter((p) => {
                const pFamily = getSourceFamilyId(typeof p.source === 'string' ? p.source : p.source);
                if (pFamily === currentFilter) return true;
                const matchesAvailable = p.available_sources?.some((s) => getSourceFamilyId(s) === currentFilter);
                return !!matchesAvailable;
            });
        }
        const seen = new Set<string>();
        const deduped = filtered.filter((pkg) => {
            const key = getPackageListKey(pkg);
            if (seen.has(key)) return false;
            seen.add(key);
            return true;
        });
        return deduped
            .sort((a, b) => {
                if (sortBy === 'name') return (a.display_name || a.name).localeCompare(b.display_name || b.name);
                if (sortBy === 'updated') {
                    const getT = (t: number | string | null | undefined) => typeof t === 'string' ? parseInt(t, 10) : (t || 0);
                    return getT(b.last_modified) - getT(a.last_modified);
                }
                if (sortBy === 'best_match') {
                    const idA = getPackageListKey(a);
                    const idB = getPackageListKey(b);
                    const score = (p: Package, id: string) =>
                        (p.installed ? 2 : 0) + (favoriteSet.has(id) ? 1 : 0);
                    const sA = score(a, idA);
                    const sB = score(b, idB);
                    if (sB !== sA) return sB - sA;
                    return (a.display_name || a.name).localeCompare(b.display_name || b.name);
                }
                return 0;
            })
            .map((p) => getPackageListKey(p));
    }, [searchResultIds, packageRegistry, currentFilter, sortBy, query, favoriteSet]);

    const rows = useMemo(() => chunk(sortedIds, GRID_COLS), [sortedIds]);
    const scrollContainerRef = useRef<HTMLDivElement>(null);

    const handleSelectPackage = useCallback((pkg: Package) => {
        onSelectPackage(pkg);
    }, [onSelectPackage]);

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
                            {query ? `${searchResultIds.length} apps matching "${query}"` : 'Discover your next favorite app'}
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
                            All ({searchResultIds.length})
                        </button>
                        {(() => {
                            // Unify repos into families (same ids as Category backend: official, chaotic-aur, aur, flatpak, etc.)
                            const families = new Map<string, { label: string; count: number }>();
                            enabledRepos.forEach(repo => {
                                const sourceObj = typeof repo.source === 'object' && repo.source != null ? repo.source : { id: String(repo.source), source_type: String(repo.source), label: '', version: '' };
                                const familyId = getSourceFamilyId(sourceObj);
                                const label = getSourceFamilyLabel(familyId);
                                const count = searchResultIds.filter((id) => {
                                    const p = packageRegistry[id];
                                    if (!p) return false;
                                    return getSourceFamilyId(typeof p.source === 'string' ? p.source : p.source) === familyId ||
                                        (p.available_sources?.some((s) => getSourceFamilyId(s) === familyId) ?? false);
                                }).length;
                                if (count === 0) return;
                                if (families.has(familyId)) {
                                    families.get(familyId)!.count += count;
                                } else {
                                    families.set(familyId, { label, count });
                                }
                            });

                            const flatpakCount = searchResultIds.filter((id) => {
                                const p = packageRegistry[id];
                                if (!p) return false;
                                return getSourceFamilyId(typeof p.source === 'string' ? p.source : p.source) === 'flatpak' ||
                                    (p.available_sources?.some((s) => getSourceFamilyId(s) === 'flatpak') ?? false);
                            }).length;
                            if (flatpakCount > 0) {
                                families.set('flatpak', { label: getSourceFamilyLabel('flatpak'), count: flatpakCount });
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
                            className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 max-w-7xl mx-auto w-full"
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
                                        {history.map((item, i) => (
                                            <div
                                                key={typeof item === 'string' ? item : `hist-${i}`}
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
                                            {favorites.slice(0, 8).map((fav, i) => (
                                                <button
                                                    key={typeof fav === 'string' ? fav : `fav-${i}`}
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
                            ) : loading && searchResultIds.length === 0 ? (
                                <div className="grid gap-6 max-w-7xl mx-auto w-full grid-cols-[repeat(auto-fill,minmax(260px,1fr))]">
                                    {[...Array(8)].map((_, i) => (
                                        <PackageCardSkeleton key={i} />
                                    ))}
                                </div>
                            ) : sortedIds.length === 0 ? (
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
                                <div ref={scrollContainerRef} className="flex-1 min-h-0 -mx-8 px-8 max-w-7xl mx-auto w-full">
                                    <VList
                                        data={rows}
                                        style={{ height: '100%', minHeight: 400 }}
                                        className="custom-scrollbar"
                                    >
                                        {(row, rowIndex) => (
                                            <div
                                                key={rowIndex}
                                                className="grid gap-6 pb-6 grid-cols-1 md:grid-cols-2 lg:grid-cols-3"
                                            >
                                                {row.map((id) => {
                                                    const pkg = packageRegistry[id];
                                                    return (
                                                        <PackageCard
                                                            key={id}
                                                            pkgId={id}
                                                            onClick={(p) => handleSelectPackage(p)}
                                                            setupRequired={pkg ? isOnlyChaoticSource(pkg) && !chaoticEnabled : false}
                                                            onConfigureSource={onOpenSettings}
                                                        />
                                                    );
                                                })}
                                            </div>
                                        )}
                                    </VList>
                                </div>
                            )}
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>
        </div>
    );
}
