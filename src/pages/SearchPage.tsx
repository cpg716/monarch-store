import { useState, useRef, useEffect, useMemo, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Search, Clock, X, Sparkles, TrendingUp, Heart } from 'lucide-react';
import { useSearchHistory } from '../hooks/useSearchHistory';
import { useFavorites } from '../hooks/useFavorites';
import PackageCardList from '../components/PackageCardList';
import type { Package, SearchSuggestion } from '../services/bindings';
import { useAppStore } from '../store/internal_store';
import PackageCardSkeleton from '../components/PackageCardSkeleton';
import EmptyState from '../components/EmptyState';
import { clsx } from 'clsx';
import { useChaoticStatus, isOnlyChaoticSource } from '../hooks/useChaoticStatus';
import { getSourceFamilyId, getSourceFamilyLabel } from '../utils/repoHelper';
import { getPackageDisplayTitle } from '../utils/packagePresentation';
import { usePackageCardList } from '../hooks/usePackageCardList';

interface SearchPageProps {
    query: string;
    onQueryChange: (query: string) => void;
    packages: Package[];
    loading: boolean;
    onSelectPackage: (pkg: Package) => void;
    enabledRepos: { name: string; enabled: boolean; source: any }[];
    suggestions?: SearchSuggestion[];
    queryInterpretation?: string | null;
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
    suggestions = [],
    queryInterpretation,
    error,
    onRetry,
    onOpenSettings
}: SearchPageProps) {
    const { history, removeSearch, clearHistory } = useSearchHistory();
    const { favorites } = useFavorites();
    const { enabled: chaoticEnabled } = useChaoticStatus();
    const packageRegistry = useAppStore((s) => s.packageRegistry);
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
        else if (magic === '@chaotic') currentFilter = 'chaotic-aur';
        else if (magic === '@official') currentFilter = 'official';
        else if (magic === '@flatpak') currentFilter = 'flatpak';
    } else if (query.trim().toLowerCase().startsWith('in:')) {
        const scope = query.trim().split(' ')[0].slice(3).toLowerCase();
        if (scope === 'games') currentFilter = 'all';
    }

    const { packages: directPackages } = usePackageCardList({
        source: { mode: 'packages', packages: _packagesProp },
        packageRegistry,
        sourceFamilyFilter: currentFilter,
        sort: sortBy === 'best_match' ? 'preserve' : sortBy,
        query,
    });
    const displayResultCount = directPackages.length;
    const displayPackages = directPackages;

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
                            {query ? `${displayResultCount} apps matching "${query}"` : 'Discover your next favorite app'}
                        </p>
                        {queryInterpretation && (
                            <p className="mt-1 text-xs font-medium text-blue-300">{queryInterpretation}</p>
                        )}
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
                    <div ref={filterChipsRef} className="space-y-3">
                        {didYouMean && (
                            <div className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-sm text-amber-100">
                                <span className="font-semibold">Popular alternative:</span> {didYouMean}
                            </div>
                        )}
                        <div className="flex items-center gap-2 overflow-x-auto pb-2 no-scrollbar">
                        <button
                            onClick={() => setActiveFilter('all')}
                            className={clsx(
                                "px-4 py-2 rounded-full text-xs font-bold transition-all border whitespace-nowrap",
                                activeFilter === 'all'
                                    ? "bg-blue-600 border-blue-600 text-white shadow-lg shadow-blue-500/20"
                                    : "bg-app-card border-app-border text-app-muted hover:border-app-fg/30"
                            )}
                        >
                            All ({displayResultCount})
                        </button>
                        {(() => {
                            // Unify repos into families (same ids as Category backend: official, chaotic-aur, aur, flatpak, etc.)
                            const families = new Map<string, { label: string; count: number }>();
                            enabledRepos.forEach(repo => {
                                const sourceObj = typeof repo.source === 'object' && repo.source != null ? repo.source : { id: String(repo.source), source_type: String(repo.source), label: '', version: '' };
                                const familyId = getSourceFamilyId(sourceObj);
                                const label = getSourceFamilyLabel(familyId);
                                const count = displayPackages.filter((p) => {
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

                            const flatpakCount = displayPackages.filter((p) => {
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
                    </div>
                )}

                {!query && (
                    <div className="space-y-3 pt-2">
                        <div className="text-xs text-app-muted">
                            Try app names, tasks, or categories like "browser", "video editor", "office", or "music player".
                        </div>
                        <div className="flex flex-wrap gap-2 text-[11px] text-app-muted">
                        {[
                            { token: '@official', label: 'Official Repos' },
                            { token: '@flatpak', label: 'Flatpak Apps' },
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
                                            onClick={() => onQueryChange("browser")}
                                            className="p-4 rounded-2xl border flex flex-col items-center gap-2 hover:scale-[1.02] transition-all group accent-hover-outline"
                                            style={{
                                                background: 'linear-gradient(135deg, color-mix(in srgb, var(--app-accent) 18%, transparent), transparent 70%)',
                                                borderColor: 'color-mix(in srgb, var(--app-accent) 30%, transparent)'
                                            }}
                                        >
                                            <TrendingUp className="text-accent group-hover:scale-110 transition-transform" />
                                            <span className="text-xs font-bold text-app-fg">Popular Browsers</span>
                                        </button>
                                        <button
                                            onClick={() => onQueryChange("video editor")}
                                            className="p-4 rounded-2xl bg-gradient-to-br from-purple-500/10 to-pink-500/5 border border-purple-500/20 flex flex-col items-center gap-2 hover:scale-[1.02] transition-all group"
                                        >
                                            <Sparkles className="text-purple-500 group-hover:scale-110 transition-transform" />
                                            <span className="text-xs font-bold text-app-fg">Video Editors</span>
                                        </button>
                                    </div>
                                </div>

                                {favorites.length > 0 && (
                                    <div className="space-y-4">
                                        <h3 className="text-sm font-bold text-app-muted uppercase tracking-widest flex items-center gap-2">
                                            <Heart size={16} className="text-red-500" /> From Your Favorites
                                        </h3>
                                        <div className="flex flex-wrap gap-2">
                                            {favorites.slice(0, 8).map((fav, i) => {
                                                const favPkg = packageRegistry[fav];
                                                const label = getPackageDisplayTitle(favPkg) || fav;
                                                return (
                                                <button
                                                    key={typeof fav === 'string' ? fav : `fav-${i}`}
                                                    onClick={() => onQueryChange(fav)}
                                                    className="px-3 py-1.5 rounded-full bg-app-card border border-app-border text-xs text-app-fg hover:border-red-500/30 hover:bg-red-500/5 transition-all"
                                                >
                                                    {label}
                                                </button>
                                                );
                                            })}
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
                            ) : (loading && activeFilter === 'all') ? (
                                <div className="grid gap-6 max-w-7xl mx-auto w-full grid-cols-[repeat(auto-fill,minmax(260px,1fr))]">
                                    {[...Array(8)].map((_, i) => (
                                        <PackageCardSkeleton key={i} />
                                    ))}
                                </div>
                            ) : directPackages.length === 0 ? (
                                <div className="space-y-4">
                                    <EmptyState
                                        title="No apps found"
                                        description={didYouMean ? `Did you mean '${didYouMean}'? Arch uses different apps than Windows.` : `We couldn't find any packages matching "${query}"${activeFilter !== 'all' ? ` in the ${activeFilter} source` : ''}.`}
                                        actionLabel={didYouMean ? `Search for ${didYouMean}` : "Clear filters & search again"}
                                        onAction={() => {
                                            if (didYouMean) onQueryChange(didYouMean);
                                            else { onQueryChange(''); setActiveFilter('all'); }
                                        }}
                                    />
                                    {suggestions.length > 0 && (
                                        <div className="rounded-xl border border-app-border bg-app-card p-4">
                                            <div className="text-xs font-bold uppercase tracking-widest text-app-muted">Try instead</div>
                                            <div className="mt-3 flex flex-wrap gap-2">
                                                {suggestions.map((suggestion) => (
                                                    <button
                                                        key={`${suggestion.reason}:${suggestion.query}`}
                                                        type="button"
                                                        onClick={() => onQueryChange(suggestion.query)}
                                                        className="rounded-full border border-app-border bg-black/20 px-3 py-2 text-xs font-bold text-white transition-colors hover:border-blue-500/40 hover:bg-blue-500/10"
                                                        title={suggestion.reason}
                                                    >
                                                        {suggestion.label}
                                                    </button>
                                                ))}
                                            </div>
                                        </div>
                                    )}
                                </div>
                            ) : (
                                <PackageCardList
                                    source={{ mode: 'packages', packages: directPackages }}
                                    onSelectPackage={handleSelectPackage}
                                    variant="grid"
                                    setupRequiredResolver={(pkg) => isOnlyChaoticSource(pkg) && !chaoticEnabled}
                                    onConfigureSource={onOpenSettings}
                                    surfaceName="SearchPage"
                                />
                            )}
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>
        </div>
    );
}
