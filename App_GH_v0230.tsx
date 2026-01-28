import { useState, useEffect, useRef } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { trackEvent } from '@aptabase/tauri';
import { ArrowLeft, Heart } from 'lucide-react';
import { useFavorites } from './hooks/useFavorites';
import Sidebar from './components/Sidebar';
import SearchBar from './components/SearchBar';
import InstallMonitor from './components/InstallMonitor';
import { Package } from './components/PackageCard';
import TrendingSection from './components/TrendingSection';
import HeroSection from './components/HeroSection';
import PackageDetails from './pages/PackageDetails';
import { useAppStore } from './store/internal_store';
import CategoryView from './pages/CategoryView';
import InstalledPage from './pages/InstalledPage';
import UpdatesPage from './pages/UpdatesPage';
import SettingsPage from './pages/SettingsPage';
import { useTheme } from './hooks/useTheme';
import './App.css';
import LoadingScreen from './components/LoadingScreen';
import OnboardingModal from './components/OnboardingModal';
import SearchPage from './pages/SearchPage';
import { useSearchHistory } from './hooks/useSearchHistory';
import HomePage from './pages/HomePage';
import { ESSENTIALS_POOL } from './constants';

function App() {
  const [activeTab, setActiveTab] = useState(() => {
    return localStorage.getItem('monarch_active_tab') || 'explore';
  });
  const [activeInstall, setActiveInstall] = useState<{ name: string; source: string; repoName?: string; mode: 'install' | 'uninstall' } | null>(null);
  const [viewAll, setViewAll] = useState<'essentials' | 'trending' | null>(null);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [packages, setPackages] = useState<Package[]>([]);
  const [selectedPackage, setSelectedPackage] = useState<Package | null>(null);
  const [preferredSource, setPreferredSource] = useState<string | undefined>(undefined);
  const [onboardingReason, setOnboardingReason] = useState<string | undefined>(undefined);
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
    // 1. Get Repo States
    invoke<{ name: string; enabled: boolean; source: string }[]>('get_repo_states').then(repos => {
      setEnabledRepos(repos.filter(r => r.enabled));
    }).catch(console.error);
  }, []);

  const handleOnboardingComplete = () => {
    localStorage.setItem('monarch_onboarding_v3', 'true');
    setShowOnboarding(false);
  };

  useEffect(() => {
    const initializeStartup = async () => {
      const startTime = Date.now();
      try {
        // 1. Parallel background tasks
        fetchInfraStats();
        invoke<{ name: string; enabled: boolean; source: string }[]>('get_repo_states')
          .then(repos => setEnabledRepos(repos.filter(r => r.enabled)))
          .catch(console.error);

        // 2. Health & Onboarding status
        const status = await invoke<{
          needs_policy: boolean,
          needs_keyring: boolean,
          needs_migration: boolean,
          is_healthy: boolean
        }>('check_initialization_status');

        const isCompleted = localStorage.getItem('monarch_onboarding_v3');
        const legacyCompleted = localStorage.getItem('monarch_onboarding_v2_final') || localStorage.getItem('monarch_onboarding_completed');

        // 3. Simple Decision: Onboarding/Repair vs Normal Home
        let redoOnboarding = !isCompleted && !legacyCompleted;

        if (!status.is_healthy) {
          console.warn("System is unhealthy. Triggering repair flow.");
          setOnboardingReason("MonARCH detected system defects that require a quick maintenance. Your password will be needed once to fix the keyring and security policy.");
          redoOnboarding = true;
        }

        if (redoOnboarding) {
          setShowOnboarding(true);
        } else {
          // Healthy enough (or legacy user): background sync
          if (!isCompleted && legacyCompleted) {
            localStorage.setItem('monarch_onboarding_v3', 'true');
          }
          invoke('trigger_repo_sync', { syncIntervalHours: 3 }).catch(console.error);
        }

      } catch (e) {
        console.error("Critical Startup Logic Error", e);
      } finally {
        const elapsed = Date.now() - startTime;
        const remaining = Math.max(0, 1200 - elapsed);
        setTimeout(() => setIsRefreshing(false), remaining);
      }
    };
    initializeStartup();
  }, [fetchInfraStats]);

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
      setSelectedCategory(null);
      setSelectedPackage(null);
      setViewAll(null);
      setActiveTab('explore');
      setTimeout(() => {
        const input = document.querySelector('input') as HTMLInputElement;
        if (input) input.focus();
      }, 50);
    } else {
      if (activeTab === tab) {
        setSelectedPackage(null);
        setSelectedCategory(null);
        setViewAll(null);
        setSearchQuery('');
      }
      setActiveTab(tab);
      localStorage.setItem('monarch_active_tab', tab);
      setSearchQuery('');

      if (tab === 'settings') {
        setTimeout(() => {
          const el = document.getElementById('system-health');
          if (el) el.scrollIntoView({ behavior: 'smooth' });
        }, 100);
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

  if (isRefreshing) return <LoadingScreen />;

  return (
    <div className="flex h-screen w-screen bg-app-bg text-app-fg overflow-hidden font-sans transition-colors" style={{ '--tw-selection-bg': `${accentColor}4D` } as any}>
      {!showOnboarding && <Sidebar activeTab={activeTab} setActiveTab={handleTabChange} />}

      <main className="flex-1 flex flex-col h-full overflow-hidden relative">
        {showOnboarding ? (
          <div className="flex-1 bg-app-bg" /> /* Empty dark background while onboarding is active/animating */
        ) : selectedPackage ? (
          <PackageDetails
            pkg={selectedPackage}
            onBack={handleBack}
            preferredSource={preferredSource}
            onInstall={(p: { name: string; source: string; repoName?: string }) => setActiveInstall({ ...p, mode: 'install' })}
            onUninstall={(p: { name: string; source: string; repoName?: string }) => setActiveInstall({ ...p, mode: 'uninstall' })}
          />
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
                  <HeroSection onNavigateToFix={() => handleTabChange('settings')} />
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
                  <HomePage
                    onSelectPackage={setSelectedPackage}
                    onSeeAll={setViewAll}
                    onSelectCategory={setSelectedCategory}
                  />
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
                ) : null}
              </div>
            </div>
          </div>
        )}
      </main>
      {showOnboarding && <OnboardingModal onComplete={handleOnboardingComplete} reason={onboardingReason} />}
      {activeInstall && (
        <InstallMonitor
          pkg={activeInstall}
          mode={activeInstall.mode}
          onClose={() => setActiveInstall(null)}
          onSuccess={() => {
            // Global refresh logic if needed
          }}
        />
      )}
    </div>
  );
}

export default App;
