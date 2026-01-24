import { useState, useEffect, useRef } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { trackEvent } from '@aptabase/tauri';
import { ArrowLeft, Zap, Heart } from 'lucide-react';
import { useFavorites } from './hooks/useFavorites';
import Sidebar from './components/Sidebar';
import SearchBar from './components/SearchBar';
import { Package } from './components/PackageCard';
import TrendingSection from './components/TrendingSection';
import HeroSection from './components/HeroSection';
import PackageDetails from './pages/PackageDetails';
import { useAppStore } from './store/internal_store';
import CategoryGrid from './components/CategoryGrid';
import CategoryView from './pages/CategoryView';
import InstalledPage from './pages/InstalledPage';
import UpdatesPage from './pages/UpdatesPage';
import SettingsPage from './pages/SettingsPage';
import SystemHealth from './pages/SystemHealth'; // [NEW]
import { useTheme } from './hooks/useTheme';
import './App.css';

import LoadingScreen from './components/LoadingScreen';
import OnboardingModal from './components/OnboardingModal';
import SearchPage from './pages/SearchPage';
import { useSearchHistory } from './hooks/useSearchHistory';

// Full pool of "Essentials" - Popular proprietary/chaotic apps
const ESSENTIALS_POOL = [
  "google-chrome", "visual-studio-code-bin", "spotify", "discord", "slack-desktop", "zoom", "sublime-text-4",
  "obsidian", "telegram-desktop-bin", "brave-bin", "edge-bin", "vlc", "gimp", "steam", "minecraft-launcher",
  "teams-for-linux", "notion-app", "postman-bin", "figma-linux-bin", "anydesk-bin"
];

const getRotatedEssentials = () => {
  const now = new Date();
  const start = new Date(now.getFullYear(), 0, 0);
  const diff = (now.getTime() - start.getTime()) + ((start.getTimezoneOffset() - now.getTimezoneOffset()) * 60 * 1000);
  const oneDay = 1000 * 60 * 60 * 24;
  const day = Math.floor(diff / oneDay);
  const week = Math.floor(day / 7);

  const poolSize = ESSENTIALS_POOL.length;
  const subsetSize = 7;
  const startIndex = (week * 3) % poolSize;

  let result: string[] = [];
  for (let i = 0; i < subsetSize; i++) {
    result.push(ESSENTIALS_POOL[(startIndex + i) % poolSize]);
  }
  return result;
};

const ESSENTIAL_IDS = getRotatedEssentials();

function App() {
  const [activeTab, setActiveTab] = useState('explore');
  const [viewAll, setViewAll] = useState<'essentials' | 'trending' | null>(null);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [packages, setPackages] = useState<Package[]>([]);
  const [selectedPackage, setSelectedPackage] = useState<Package | null>(null);
  const [preferredSource, setPreferredSource] = useState<string | undefined>(undefined);
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(true);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const { addSearch } = useSearchHistory();
  const { fetchInfraStats } = useAppStore();
  const { accentColor } = useTheme();
  const { favorites } = useFavorites();

  const [enabledRepos, setEnabledRepos] = useState<{ name: string; enabled: boolean; source: string }[]>([]);

  useEffect(() => {
    invoke<{ name: string; enabled: boolean; source: string }[]>('get_repo_states').then(repos => {
      setEnabledRepos(repos.filter(r => r.enabled));
    }).catch(console.error);
  }, []);

  useEffect(() => {
    const completed = localStorage.getItem('monarch_onboarding_completed');
    if (!completed) setShowOnboarding(true);
  }, []);

  const handleOnboardingComplete = () => {
    localStorage.setItem('monarch_onboarding_completed', 'true');
    setShowOnboarding(false);
  };

  useEffect(() => {
    const initInfo = async () => {
      try {
        // Start stats immediately
        // Start stats immediately
        fetchInfraStats();

        // [UX FIX] Re-enabled safe background sync (No Root Required)
        // This ensures the app has content on launch without blocking UI.
        const savedInterval = localStorage.getItem('sync-interval-hours');
        const interval = savedInterval ? parseInt(savedInterval, 10) : 3;

        // Await the sync so the Loading Screen persists until data is ready
        await invoke('trigger_repo_sync', { syncIntervalHours: interval });

      } catch (e) {
        console.error("Startup Logic Error", e);
      } finally {
        // Always show the UI, even if sync is still running or failed
        setIsRefreshing(false);
      }
    };
    initInfo();
  }, [fetchInfraStats]);

  // Migration: Infrastructure 2.0 (v0.2.25)
  // [UX FIX] Disabled auto-bootstrap to prevent immediate root prompt on launch.
  // This should be moved to a "Fix System" button in Settings or a non-blocking toast.
  /*
  useEffect(() => {
    const migrateInfra = async () => {
      const migrated = localStorage.getItem('monarch_infra_2_0');
      if (!migrated) {
        console.log("Migrating to Infrastructure 2.0...");
        try {
          // 1. Run Bootstrap (Keyring, Modular Configs)
          await invoke('bootstrap_infrastructure');

          // 2. Enable All Supported Repos (System Level)
          const allSupportedRepos = ['cachyos', 'garuda', 'endeavouros', 'manjaro', 'chaotic-aur'];
          await invoke('enable_repos_batch', { names: allSupportedRepos });

          // 3. Mark Complete
          localStorage.setItem('monarch_infra_2_0', 'true');
          console.log("Migration Complete");
        } catch (e) {
          console.error("Migration Failed (will retry next launch)", e);
        }
      }
    };
    // Helper to run after app load
    setTimeout(migrateInfra, 2000);
  }, []);
  */

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
      if (!searchQuery) {
        setPackages([]);
        return;
      }
      setLoading(true);
      try {
        const results = await invoke<Package[]>('search_packages', { query: searchQuery });
        setPackages(results);
        addSearch(searchQuery);
        trackEvent('search', { query: searchQuery, result_count: results.length });
      } catch (e) {
        console.error("Search failed", e);
      } finally {
        setLoading(false);
      }
    };

    const timeoutId = setTimeout(() => search(), 300);
    return () => clearTimeout(timeoutId);
  }, [searchQuery, addSearch]);

  const handleTabChange = (tab: string) => {
    if (tab === 'search') {
      setActiveTab('explore');
      setTimeout(() => {
        const input = document.querySelector('input') as HTMLInputElement;
        if (input) input.focus();
      }, 50);
    } else {
      // Logic: If user clicks the SAME tab they are already on, we want to reset the view
      // back to the "root" of that tab (clear selection, search, etc.)
      if (activeTab === tab) {
        setSelectedPackage(null);
        setSelectedCategory(null);
        setViewAll(null);
        setSearchQuery('');
      }
      setActiveTab(tab);
      setSearchQuery(''); // Always clear search when switching tabs to ensure view changes
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

  if (isRefreshing) return <LoadingScreen />;

  return (
    <div className="flex h-screen w-screen bg-app-bg text-app-fg overflow-hidden font-sans transition-colors" style={{ '--tw-selection-bg': `${accentColor}4D` } as any}>
      <Sidebar activeTab={activeTab} setActiveTab={handleTabChange} />

      <main className="flex-1 flex flex-col h-full overflow-hidden relative">
        {selectedPackage ? (
          <PackageDetails pkg={selectedPackage} onBack={handleBack} preferredSource={preferredSource} />
        ) : selectedCategory ? (
          <CategoryView category={selectedCategory} onBack={handleBack} onSelectPackage={setSelectedPackage} />
        ) : viewAll ? (
          <div className="flex-1 overflow-y-auto pb-32">
            <div className="p-10 pb-6 sticky top-0 bg-app-bg/95 backdrop-blur-xl z-20 border-b border-app-border/50 flex items-center gap-4">
              <button onClick={handleBack} className="p-2 rounded-lg hover:bg-app-fg/10 transition-colors"><ArrowLeft size={24} /></button>
              <h2 className="text-2xl font-bold">{viewAll === 'essentials' ? 'All Essentials' : 'Trending Applications'}</h2>
            </div>
            <div className="p-8 max-w-7xl mx-auto">
              <TrendingSection title="" filterIds={viewAll === 'essentials' ? ESSENTIALS_POOL : undefined} onSelectPackage={setSelectedPackage} />
            </div>
          </div>
        ) : (
          <div className="flex-1 overflow-hidden flex flex-col relative">
            <div className="absolute inset-0 bg-gradient-to-br from-purple-500/5 via-app-bg/50 to-blue-500/5 pointer-events-none transition-colors" />

            <div ref={scrollContainerRef} className="flex-1 overflow-y-auto min-h-0 pb-32 scroll-smooth">
              {activeTab === 'explore' && !searchQuery && (
                <div className="px-6 pt-6 animate-in fade-in slide-in-from-top-5 duration-700">
                  <HeroSection onNavigateToFix={() => handleTabChange('system')} />
                </div>
              )}

              <div className="sticky top-0 z-30 px-6 py-4 bg-app-bg/80 backdrop-blur-xl transition-all flex justify-center">
                <SearchBar value={searchQuery} onChange={setSearchQuery} />
              </div>

              <div className="max-w-7xl mx-auto px-6 pb-16 min-h-[50vh]">
                {(searchQuery || activeTab === 'search') ? (
                  <SearchPage
                    query={searchQuery}
                    onQueryChange={setSearchQuery}
                    packages={packages}
                    loading={loading}
                    onSelectPackage={setSelectedPackage}
                    enabledRepos={enabledRepos}
                  />
                ) : activeTab === 'explore' ? (
                  <div className="space-y-12 mt-4 animate-in fade-in duration-500">
                    <section>
                      <div className="flex items-center justify-between mb-4 px-2">
                        <div className="flex items-center gap-3">
                          <div className="p-2 rounded-xl bg-violet-600/10 text-violet-600"><Zap size={20} /></div>
                          <div><h2 className="text-xl font-bold">Recommended Essentials</h2><p className="text-xs text-app-muted">Optimized for your system.</p></div>
                        </div>
                        <button onClick={() => setViewAll('essentials')} className="text-sm font-bold text-blue-500">See All</button>
                      </div>
                      <TrendingSection title="" filterIds={ESSENTIAL_IDS} onSelectPackage={setSelectedPackage} onSeeAll={() => setViewAll('essentials')} variant="scroll" />
                    </section>
                    <TrendingSection title="Trending Applications" onSelectPackage={setSelectedPackage} limit={7} onSeeAll={() => setViewAll('trending')} variant="scroll" />
                    <CategoryGrid onSelectCategory={setSelectedCategory} />
                  </div>
                ) : activeTab === 'installed' ? (
                  <InstalledPage />
                ) : activeTab === 'favorites' ? (
                  <div className="py-4">
                    <h2 className="text-2xl font-bold mb-2">Favorites</h2>
                    {favorites.length === 0 ? (
                      <div className="text-center text-app-muted py-20 flex flex-col items-center gap-4">
                        <div className="p-4 rounded-full bg-app-subtle"><Heart size={32} className="opacity-50" /></div>
                        <p className="font-bold">No favorites yet</p>
                      </div>
                    ) : (
                      <TrendingSection title="" filterIds={favorites} onSelectPackage={setSelectedPackage} limit={100} />
                    )}
                  </div>
                ) : activeTab === 'updates' ? (
                  <UpdatesPage />
                ) : activeTab === 'settings' ? (
                  <SettingsPage onRestartOnboarding={() => setShowOnboarding(true)} />
                ) : activeTab === 'system' ? (
                  <SystemHealth />
                ) : null}
              </div>
            </div>
          </div>
        )}
      </main>
      {showOnboarding && <OnboardingModal onComplete={handleOnboardingComplete} />}
    </div>
  );
}

export default App;
