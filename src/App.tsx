import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, Filter, Heart } from 'lucide-react';
import { useFavorites } from './hooks/useFavorites';
import Sidebar from './components/Sidebar';
import SearchBar from './components/SearchBar';
import PackageCard, { Package } from './components/PackageCard';
import TrendingSection from './components/TrendingSection';
import HeroSection from './components/HeroSection';
import PackageDetails from './pages/PackageDetails';
import { useAppStore } from './store/internal_store';
import CategoryGrid from './components/CategoryGrid';
import CategoryView from './pages/CategoryView';
import InstalledPage from './pages/InstalledPage';
import UpdatesPage from './pages/UpdatesPage';
import SettingsPage from './pages/SettingsPage';
import { useTheme } from './hooks/useTheme';
import { useInfiniteScroll } from './hooks/useInfiniteScroll';
import './App.css';

import LoadingScreen from './components/LoadingScreen';
import PackageCardSkeleton from './components/PackageCardSkeleton';
import OnboardingModal from './components/OnboardingModal';

// Full pool of "Essentials" - Popular proprietary/chaotic apps
const ESSENTIALS_POOL = [
  "google-chrome", "visual-studio-code-bin", "spotify", "discord", "slack-desktop", "zoom", "sublime-text-4",
  "obsidian", "telegram-desktop-bin", "brave-bin", "edge-bin", "vlc", "gimp", "steam", "minecraft-launcher",
  "teams-for-linux", "notion-app", "postman-bin", "figma-linux-bin", "anydesk-bin"
];

// Helper to get a rotated subset based on the current week
const getRotatedEssentials = () => {
  // Get current week number (0-51)
  const now = new Date();
  const start = new Date(now.getFullYear(), 0, 0);
  const diff = (now.getTime() - start.getTime()) + ((start.getTimezoneOffset() - now.getTimezoneOffset()) * 60 * 1000);
  const oneDay = 1000 * 60 * 60 * 24;
  const day = Math.floor(diff / oneDay);
  const week = Math.floor(day / 7);

  // Use week number to rotate
  // We want 7 items.
  // Shift the pool by (week * 3) to slowly rotate? 
  // Or just (week * 7) % length?
  const poolSize = ESSENTIALS_POOL.length;
  const subsetSize = 7;

  // Create a rotating window
  const startIndex = (week * 3) % poolSize; // Shift by 3 each week

  let result: string[] = [];
  for (let i = 0; i < subsetSize; i++) {
    result.push(ESSENTIALS_POOL[(startIndex + i) % poolSize]);
  }
  return result;
};

console.log("App.tsx: Initializing...");
const ESSENTIAL_IDS = getRotatedEssentials();
console.log("App.tsx: Essentials initialized:", ESSENTIAL_IDS);

function App() {
  console.log("App.tsx: Rendering App...");
  const [activeTab, setActiveTab] = useState('explore');
  const [viewAll, setViewAll] = useState<'essentials' | 'trending' | null>(null);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [packages, setPackages] = useState<Package[]>([]);
  const [selectedPackage, setSelectedPackage] = useState<Package | null>(null);
  const [preferredSource, setPreferredSource] = useState<string | undefined>(undefined);
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(true); // Startup refresh state
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [displayLimit, setDisplayLimit] = useState(50); // For infinite scroll logic

  // Infinite Scroll Hook
  const loadMore = useCallback(() => {
    setDisplayLimit(prev => prev + 50);
  }, []);

  // Compute hasMore based on current view
  const hasMore = () => {
    // Simplified check - if we have packets and haven't shown all
    return packages.length > displayLimit;
  };

  const lastElementRef = useInfiniteScroll(loadMore, hasMore(), false);
  const { fetchInfraStats } = useAppStore();
  const { accentColor } = useTheme();
  const { favorites } = useFavorites();

  // Repo filter state for search
  const [searchRepoFilter, setSearchRepoFilter] = useState<string>('all');
  const [sortBy, setSortBy] = useState<'best_match' | 'name' | 'updated'>('best_match');
  const [enabledRepos, setEnabledRepos] = useState<{ name: string; enabled: boolean; source: string }[]>([]);

  // Sorting Logic Helper
  const sortPackages = (pkgs: Package[], criterion: 'best_match' | 'name' | 'updated') => {
    return [...pkgs].sort((a, b) => {
      if (criterion === 'name') {
        return (a.display_name || a.name).localeCompare(b.display_name || b.name);
      } else if (criterion === 'updated') {
        // Sort by last_modified (newer first)
        return (b.last_modified || 0) - (a.last_modified || 0);
      }
      return 0; // Best Match preserves original order (backend sorted)
    });
  };

  useEffect(() => {
    invoke<{ name: string; enabled: boolean; source: string }[]>('get_repo_states').then(repos => {
      const enabled = repos.filter(r => r.enabled);
      // Deduplicate by source - only keep one entry per source type
      const uniqueSources = new Map<string, { name: string; enabled: boolean; source: string }>();
      for (const repo of enabled) {
        if (!uniqueSources.has(repo.source)) {
          uniqueSources.set(repo.source, repo);
        }
      }
      setEnabledRepos(Array.from(uniqueSources.values()));
    }).catch(console.error);
  }, []);

  // Check for onboarding status
  useEffect(() => {
    const completed = localStorage.getItem('monarch_onboarding_completed');
    if (!completed) {
      setShowOnboarding(true);
    }
  }, []);

  const handleOnboardingComplete = () => {
    localStorage.setItem('monarch_onboarding_completed', 'true');
    setShowOnboarding(false);
  };

  // Startup: Trigger Repo Sync
  useEffect(() => {
    const initInfo = async () => {
      try {
        setIsRefreshing(true);


        // Parallelize metadata fetch and repo sync
        // We want sync to finish before letting user browse to ensure cache is hot
        // But we can let infra stats load in background
        fetchInfraStats();

        // Read user preference for sync interval (default 3 hours)
        const savedInterval = localStorage.getItem('sync-interval-hours');
        const interval = savedInterval ? parseInt(savedInterval, 10) : 3;

        // Trigger sync with user's interval
        await invoke('trigger_repo_sync', { syncIntervalHours: interval });


        // Optional: artificial delay if it's TOO fast (to show off the cool screen? nah, speed is king)
        // But user asked for "give time to make the app load better", so maybe 1s minimum?
        // Let's just let it be natural. If cached, it's fast.
      } catch (e) {
        console.error("Startup Sync Failed", e);
      } finally {
        setIsRefreshing(false);
      }
    };

    initInfo();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  console.log("App.tsx: Render state:", { isRefreshing, activeTab, showOnboarding });

  // Reset selection when searching or changing tabs
  useEffect(() => {
    if (searchQuery) setSelectedPackage(null);
  }, [searchQuery]);

  useEffect(() => {
    setSelectedPackage(null);
    setSelectedCategory(null);
    setViewAll(null);
  }, [activeTab]);

  useEffect(() => {
    const search = async () => {
      setLoading(true);
      try {
        const results = await invoke<Package[]>('search_packages', { query: searchQuery });
        setPackages(results);
      } catch (e) {
        console.error("Search failed", e);
      } finally {
        setLoading(false);
        setDisplayLimit(50);
      }
    };

    const timeoutId = setTimeout(() => {
      search();
    }, 300);

    return () => clearTimeout(timeoutId);
  }, [searchQuery]);

  const handleTabChange = (tab: string) => {
    if (tab === 'search') {
      // Switch to explore and focus search bar
      setActiveTab('explore');
      setSelectedCategory(null);
      setSelectedPackage(null);
      setViewAll(null);
      // Focus search input after a brief delay
      setTimeout(() => {
        const searchInput = document.querySelector('input[type="text"]') as HTMLInputElement;
        if (searchInput) searchInput.focus();
      }, 100);
    } else {
      setActiveTab(tab);
      if (tab === 'explore') {
        setSearchQuery('');
        setSelectedCategory(null);
        setSelectedPackage(null);
        setViewAll(null);
      }
    }
  };

  const handleBack = () => {
    if (selectedPackage) {
      setSelectedPackage(null);
      setPreferredSource(undefined);
    } else if (selectedCategory) {
      setSelectedCategory(null);
    } else if (viewAll) {
      setViewAll(null);
    }
  };

  if (isRefreshing) {
    console.log("App.tsx: Render -> LoadingScreen");
    return <LoadingScreen />;
  }

  return (
    <div
      className="flex h-screen w-screen bg-app-bg text-app-fg overflow-hidden font-sans transition-colors"
      style={{ '--tw-selection-bg': `${accentColor}4D` } as any}
    >
      <Sidebar activeTab={activeTab} setActiveTab={handleTabChange} />

      <main className="flex-1 flex flex-col h-full overflow-hidden relative">
        {/* Background Gradients */}
        <div className="absolute top-0 left-0 w-full h-96 bg-gradient-to-b from-blue-500/5 to-transparent pointer-events-none -z-10" />

        {selectedPackage ? (
          <PackageDetails pkg={selectedPackage} onBack={handleBack} preferredSource={preferredSource} />
        ) : selectedCategory ? (
          <CategoryView
            category={selectedCategory}
            onBack={handleBack}
            onSelectPackage={(pkg, source) => { setSelectedPackage(pkg); setPreferredSource(source); }}
          />
        ) : viewAll ? (
          /* View All Page (Essentials or Trending) */
          <div className="flex-1 overflow-y-auto pb-32 scroll-smooth">
            <div className="p-10 pb-6 sticky top-0 bg-app-bg/95 backdrop-blur-xl z-20 border-b border-app-border/50 flex items-center gap-4">
              <button onClick={handleBack} className="p-2 rounded-lg hover:bg-app-fg/10 transition-colors">
                <ArrowLeft size={24} />
              </button>
              <h2 className="text-2xl font-bold">{viewAll === 'essentials' ? 'All Essentials' : 'Trending Applications'}</h2>
            </div>
            <div className="p-8 max-w-7xl mx-auto">
              <TrendingSection
                title=""
                // For essentials full view, we show EVERYTHING in the pool
                filterIds={viewAll === 'essentials' ? ESSENTIALS_POOL : undefined}
                onSelectPackage={setSelectedPackage}
              // No limit here
              />
            </div>
          </div>
        ) : (
          <div className="flex-1 overflow-hidden flex flex-col relative">
            <div className="absolute inset-0 bg-gradient-to-br from-purple-500/5 via-app-bg/50 to-blue-500/5 pointer-events-none transition-colors" />

            {/* Main Scroll Container */}
            <div
              ref={scrollContainerRef}
              className="flex-1 overflow-y-auto min-h-0 pb-32 scroll-smooth"
            >

              {/* 1. HERO SECTION (Only on Explore Home) */}
              {activeTab === 'explore' && !searchQuery && (
                <div className="px-6 pt-6 animate-in fade-in slide-in-from-top-5 duration-700">
                  <HeroSection />
                </div>
              )}

              {/* 2. STICKY SEARCH BAR */}
              <div className="sticky top-0 z-30 px-6 py-4 bg-app-bg/80 backdrop-blur-xl border-b border-app-border/0 transition-all flex justify-center">
                <SearchBar value={searchQuery} onChange={setSearchQuery} />
              </div>

              {/* 3. CONTENT AREA */}
              <div className="max-w-7xl mx-auto px-6 pb-16 min-h-[50vh]">

                {activeTab === 'explore' && !searchQuery ? (
                  <div className="space-y-12 mt-4 animate-in fade-in duration-500 delay-150">

                    {/* 1. Essentials Collection (Rotated Weekly) */}
                    <section>
                      <div className="flex items-center justify-between mb-4">
                        <div className="flex items-center gap-3">
                          <div className="p-2 rounded-lg bg-violet-600/20 text-violet-600">
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" /></svg>
                          </div>
                          <div>
                            <h2 className="text-xl font-bold text-app-fg">Essentials</h2>
                            <p className="text-xs text-app-muted">Popular apps, pre-compiled for speed.</p>
                          </div>
                        </div>
                        <button onClick={() => setViewAll('essentials')} className="text-sm font-bold text-blue-500 hover:text-blue-400 transition-colors">
                          See All
                        </button>
                      </div>

                      <TrendingSection
                        title=""
                        filterIds={ESSENTIAL_IDS}
                        onSelectPackage={setSelectedPackage}
                        onSeeAll={() => setViewAll('essentials')}
                        variant="scroll"
                      />
                    </section>

                    {/* 2. Trending Now (Limited to 7 items) */}
                    <TrendingSection
                      title="Trending Now"
                      onSelectPackage={setSelectedPackage}
                      limit={7}
                      onSeeAll={() => setViewAll('trending')}
                      variant="scroll"
                    />

                    {/* 3. Browse by Category (Big Cards) */}
                    <section>
                      <CategoryGrid onSelectCategory={setSelectedCategory} />
                    </section>
                  </div>
                ) : activeTab === 'installed' ? (
                  <InstalledPage />
                ) : activeTab === 'favorites' ? (
                  <div className="py-4">
                    <div className="mb-6">
                      <h2 className="text-2xl font-bold">Your Favorites</h2>
                      <p className="text-app-muted text-sm">{favorites.length} items saved</p>
                    </div>
                    {favorites.length === 0 ? (
                      <div className="text-center text-app-muted py-20 flex flex-col items-center gap-4">
                        <div className="p-4 rounded-full bg-app-subtle">
                          <Heart size={32} className="opacity-50" />
                        </div>
                        <div>
                          <p className="font-bold">No favorites yet</p>
                          <p className="text-sm">Click the heart icon on any package to save it here.</p>
                        </div>
                      </div>
                    ) : (
                      <TrendingSection
                        title=""
                        filterIds={favorites}
                        onSelectPackage={setSelectedPackage}
                        limit={1000}
                      />
                    )}
                  </div>
                ) : activeTab === 'updates' ? (
                  <UpdatesPage />
                ) : activeTab === 'settings' ? (
                  <SettingsPage onRestartOnboarding={() => setShowOnboarding(true)} />
                ) : (
                  /* Search Results */
                  <div className="py-4 animate-in fade-in duration-300">
                    <div className="flex items-center justify-between mb-6">
                      <h2 className="text-2xl font-bold">
                        {loading ? 'Searching...' : `Found ${searchRepoFilter === 'all' ? packages.length : packages.filter(p => p.source === searchRepoFilter).length} results for "${searchQuery}"`}
                      </h2>
                      <div className="flex items-center gap-4">
                        {/* Repo Filter */}
                        <div className="flex items-center gap-2">
                          <Filter size={14} className="text-app-muted" />
                          <select
                            className="bg-app-subtle border border-app-border rounded-lg px-3 py-1.5 text-sm text-app-fg focus:outline-none focus:border-blue-500 transition-colors"
                            value={searchRepoFilter}
                            onChange={(e) => setSearchRepoFilter(e.target.value)}
                          >
                            <option value="all">All Repos</option>
                            {enabledRepos.map(repo => (
                              <option key={repo.source} value={repo.source}>
                                {repo.source === 'chaotic' ? 'Chaotic-AUR' :
                                  repo.source === 'official' ? 'Official' :
                                    repo.source === 'aur' ? 'AUR' :
                                      repo.source.charAt(0).toUpperCase() + repo.source.slice(1)}
                              </option>
                            ))}
                          </select>
                        </div>

                        {/* Sort Control */}
                        <div className="flex items-center gap-2">
                          <span className="text-sm text-app-muted">Sort:</span>
                          <select
                            className="bg-app-subtle border border-app-border rounded-lg px-3 py-1.5 text-sm text-app-fg focus:outline-none focus:border-blue-500 transition-colors"
                            value={sortBy}
                            onChange={(e) => setSortBy(e.target.value as 'best_match' | 'name' | 'updated')}
                          >
                            <option value="best_match">Best Match</option>
                            <option value="name">Name (A-Z)</option>
                            <option value="updated">Last Updated</option>
                          </select>
                        </div>
                      </div>
                    </div>

                    {loading && packages.length === 0 ? (
                      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5 gap-4">
                        {[...Array(10)].map((_, i) => (
                          <PackageCardSkeleton key={i} />
                        ))}
                      </div>
                    ) : (
                      <>
                        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5 gap-4">
                          {(() => {
                            const filtered = searchRepoFilter === 'all' ? packages : packages.filter(p => p.source === searchRepoFilter);
                            // Best Match (backend order) is default, no client-side sort needed
                            const sorted = sortBy === 'best_match' ? filtered : sortPackages(filtered, sortBy);

                            return sorted.slice(0, displayLimit).map((pkg, index) => {
                              const isLast = index === displayLimit - 1;
                              return (
                                <div key={`${pkg.name}-${pkg.source}`} ref={isLast ? lastElementRef : null}>
                                  <PackageCard
                                    pkg={pkg}
                                    onClick={() => { setSelectedPackage(pkg); if (searchRepoFilter !== 'all') setPreferredSource(searchRepoFilter); }}
                                  />
                                </div>
                              )
                            });
                          })()}
                        </div>

                        {(() => {
                          const total = searchRepoFilter === 'all' ? packages.length : packages.filter(p => p.source === searchRepoFilter).length;
                          return total > displayLimit && (
                            <div className="flex justify-center mt-8 py-4">
                              <div className="w-6 h-6 border-2 border-app-fg/20 border-t-blue-500 rounded-full animate-spin" />
                            </div>
                          );
                        })()}
                      </>
                    )}
                  </div>
                )}
              </div>
            </div>
          </div>
        )}
      </main>

      {/* Onboarding Modal */}
      {showOnboarding && <OnboardingModal onComplete={handleOnboardingComplete} />}

    </div>
  );
}

export default App;
