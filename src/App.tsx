import { useState, useEffect, useRef } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, Heart, AlertCircle } from 'lucide-react';
import { useFavorites } from './hooks/useFavorites';
import Sidebar from './components/Sidebar';
import SearchBar from './components/SearchBar';
import InstallMonitor from './components/InstallMonitor';
import { Package } from './components/PackageCard';
import TrendingSection from './components/TrendingSection';
import HeroSection from './components/HeroSection';
import PackageDetails from './pages/PackageDetailsFresh';
import { useAppStore } from './store/internal_store';
import CategoryView from './pages/CategoryView';
import InstalledPage from './pages/InstalledPage';
import UpdatesPage from './pages/UpdatesPage';
import SettingsPage from './pages/SettingsPage';
import { useTheme } from './hooks/useTheme';
import './App.css';
import LoadingScreen from './components/LoadingScreen';
import OnboardingModal from './components/OnboardingModal';
import ErrorModal from './components/ErrorModal';
import SearchPage from './pages/SearchPage';
import { useSearchHistory } from './hooks/useSearchHistory';
import HomePage from './pages/HomePage';
import { ESSENTIALS_POOL } from './constants';
import { listen } from '@tauri-apps/api/event';
import { UpdateProgress } from './store/internal_store';
import { useToast } from './context/ToastContext';

function App() {
  const [activeTab, setActiveTab] = useState('explore');
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
  const [systemHealth, setSystemHealth] = useState<{ is_healthy: boolean, reasons: string[] } | null>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const searchRequestIdRef = useRef(0);
  const updateTimerRef = useRef<number | null>(null);

  const { addSearch } = useSearchHistory();
  const {
    fetchInfraStats,
    setUpdateProgress,
    setUpdateStatus,
    setUpdatePhase,
    setUpdating,
    addUpdateLog,
    setRebootRequired,
    setPacnewWarnings
  } = useAppStore();
  const { accentColor } = useTheme();
  const { favorites } = useFavorites();
  const { show: showToast } = useToast();

  const [enabledRepos, setEnabledRepos] = useState<{ name: string; enabled: boolean; source: string }[]>([]);

  // Polkit rule pre-check at startup: flag immediately if missing so user isn't surprised at first install
  const polkitCheckedRef = useRef(false);
  useEffect(() => {
    if (polkitCheckedRef.current || isRefreshing) return;
    polkitCheckedRef.current = true;
    invoke<boolean>('check_security_policy')
      .then((installed) => {
        if (!installed) {
          showToast(
            'Polkit rule not installed. Install and system actions may prompt for password. Enable One-Click in Settings to fix.',
            'warning'
          );
        }
      })
      .catch(() => {});
  }, [isRefreshing, showToast]);

  // Global Update Listeners
  useEffect(() => {
    const unlistenProgress = listen<UpdateProgress>('update-progress', (event) => {
      setUpdateProgress(event.payload.progress);
      setUpdateStatus(event.payload.message);
      setUpdatePhase(event.payload.phase);

      if (event.payload.phase === 'complete') {
        // Clear any existing timer before setting a new one
        if (updateTimerRef.current) window.clearTimeout(updateTimerRef.current);
        updateTimerRef.current = window.setTimeout(() => {
          (async () => {
            try {
              setUpdating(false);
              setUpdateProgress(100);

              // Check for post-update states
              const reboot = await invoke<boolean>('check_reboot_required');
              setRebootRequired(reboot);
              const warnings = await invoke<string[]>('get_pacnew_warnings');
              setPacnewWarnings(warnings);
            } catch (e) {
              console.error("Post-update checks failed", e);
            }
          })();
        }, 1500);
      } else if (event.payload.phase === 'error') {
        if (updateTimerRef.current) window.clearTimeout(updateTimerRef.current);
        updateTimerRef.current = window.setTimeout(() => {
          setUpdating(false);
          setUpdateProgress(0);
        }, 3000);
      }
    });

    const unlistenLogs = listen<string>('install-output', (event) => {
      const msg = event.payload;
      addUpdateLog(msg);
      setUpdateStatus(msg);
    });

    const unlistenStatus = listen<string>('update-status', (event) => {
      setUpdateStatus(event.payload);
    });

    return () => {
      // Clean up timers on unmount
      if (updateTimerRef.current) window.clearTimeout(updateTimerRef.current);
      unlistenProgress.then(fn => fn()).catch(() => { });
      unlistenLogs.then(fn => fn()).catch(() => { });
      unlistenStatus.then(fn => fn()).catch(() => { });
    };
  }, [setUpdateProgress, setUpdateStatus, setUpdatePhase, setUpdating, addUpdateLog, setRebootRequired, setPacnewWarnings]);

  // Removed duplicate get_repo_states call - now only fetched in initializeStartup

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
        useAppStore.getState().checkTelemetry(); // Initialize privacy state
        invoke<{ name: string; enabled: boolean; source: string }[]>('get_repo_states')
          .then(repos => setEnabledRepos(repos.filter(r => r.enabled)))
          .catch(console.error);

        // 2. Health & Onboarding status
        const status = await invoke<{
          needs_policy: boolean,
          needs_keyring: boolean,
          needs_migration: boolean,
          is_healthy: boolean,
          reasons: string[]
        }>('check_initialization_status');

        setSystemHealth(status);

        const isCompleted = localStorage.getItem('monarch_onboarding_v3');
        const legacyCompleted = localStorage.getItem('monarch_onboarding_v2_final') || localStorage.getItem('monarch_onboarding_completed');

        // 3. Simple Decision: Onboarding/Repair vs Normal Home
        let redoOnboarding = !isCompleted && !legacyCompleted;

        if (!status.is_healthy) {
          console.warn("System is unhealthy. Triggering repair flow.");
          const reasonText = status.reasons.join(" ");
          setOnboardingReason(`MonARCH detected system defects: ${reasonText} Your password will be needed once to fix the keyring and security policy.`);
          redoOnboarding = true;
        }

        if (redoOnboarding) {
          setShowOnboarding(true);
        } else {
          // Healthy enough (or legacy user): sync only if refresh requested (pacman hook) or "Sync on startup" is on
          if (!isCompleted && legacyCompleted) {
            localStorage.setItem('monarch_onboarding_v3', 'true');
          }
          const refreshRequested = await invoke<boolean>('check_and_clear_refresh_requested').catch(() => false);
          const syncOnStartup = await invoke<boolean>('is_sync_on_startup_enabled').catch(() => true);
          if (refreshRequested || syncOnStartup) {
            invoke('trigger_repo_sync', { syncIntervalHours: 3 }).catch(console.error);
          }

          // --- PRE-WARM CACHE (Performance Optimization) ---
          try {
            const { ESSENTIAL_IDS } = await import('./constants');
            const { prewarmRatings } = await import('./hooks/useRatings'); // Dynamic import to avoid cycles if any

            await invoke('emit_sync_progress', { status: "Loading Essentials..." });
            const warmEssentials = invoke('get_packages_by_names', { names: ESSENTIAL_IDS }); // fire & forget or wait

            await invoke('emit_sync_progress', { status: "Analyzing Trending Apps..." });
            // Fetch trending AND warm their ratings
            const warmTrending = invoke<Package[]>('get_trending').then(pkgs => {
              prewarmRatings(pkgs.map(p => p.name));
              return pkgs;
            });

            // Also warm ratings for essentials (we know the IDs)
            prewarmRatings(ESSENTIAL_IDS);

            // Wait for critical data (parallel) while splash screen is up
            await Promise.all([warmEssentials, warmTrending]);
          } catch (e) {
            console.warn("Pre-warm failed", e);
          }
          // -------------------------------------------------
        }

      } catch (e) {
        console.error("Critical Startup Logic Error", e);
      } finally {
        const elapsed = Date.now() - startTime;
        const remaining = Math.max(0, 1500 - elapsed);
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
    // Increment request ID to track stale responses
    const currentRequestId = ++searchRequestIdRef.current;

    const search = async () => {
      if (!searchQuery) {
        setPackages([]);
        return;
      }
      setLoading(true);
      try {
        const results = await invoke<Package[]>('search_packages', { query: searchQuery });
        // Only update if this is still the latest request (prevents race conditions)
        if (currentRequestId !== searchRequestIdRef.current) return;
        setPackages(results);
        addSearch(searchQuery);
        invoke('track_event', { event: 'search', payload: { query: searchQuery, result_count: results.length } }).catch(() => { });
      } catch (e) {
        console.error("Search failed", e);
      } finally {
        // Only update loading state if this is still the latest request
        if (currentRequestId === searchRequestIdRef.current) {
          setLoading(false);
        }
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
        {!showOnboarding && systemHealth && !systemHealth.is_healthy && (
          <div className="bg-red-600 text-white px-6 py-3 flex flex-col md:flex-row items-center justify-between gap-4 text-sm font-bold animate-in slide-in-from-top duration-300 z-[100] shrink-0 shadow-lg">
            <div className="flex items-start gap-3">
              <AlertCircle size={20} className="shrink-0 mt-0.5" />
              <div>
                <span className="block mb-1 font-black uppercase tracking-tighter text-[10px] opacity-70">Infrastructure Issues Detected</span>
                <p className="font-bold leading-tight">
                  {systemHealth.reasons[0] || "Repository access or security policy may be broken."}
                  {systemHealth.reasons.length > 1 && <span className="ml-2 opacity-70 font-medium">(+{systemHealth.reasons.length - 1} more issues)</span>}
                </p>
              </div>
            </div>
            <button onClick={() => handleTabChange('settings')} className="bg-white/20 hover:bg-white/30 px-6 py-2 rounded-xl transition-all active:scale-95 whitespace-nowrap shadow-inner border border-white/10 uppercase tracking-widest text-[10px]">
              Repair Now
            </button>
          </div>
        )}

        {showOnboarding ? (
          <div className="flex-1 bg-app-bg" /> /* Empty dark background while onboarding is active/animating */
        ) : selectedPackage ? (
          <PackageDetails
            pkg={selectedPackage}
            onBack={handleBack}
            preferredSource={preferredSource}
            installInProgress={activeInstall !== null}
            onInstall={(p: { name: string; source: string; repoName?: string }) => setActiveInstall({ ...p, mode: 'install' })}
            onUninstall={(p: { name: string; source: string; repoName?: string }) => setActiveInstall({ ...p, mode: 'uninstall' })}
          />
        ) : selectedCategory ? (
          <CategoryView category={selectedCategory} onBack={handleBack} onSelectPackage={setSelectedPackage} />
        ) : viewAll ? (
          <div className="flex-1 overflow-y-auto pb-32 scroll-gpu">
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

            <div ref={scrollContainerRef} className="flex-1 overflow-y-auto min-h-0 pb-32 scroll-smooth scroll-gpu">
              {activeTab === 'explore' && !searchQuery && (
                <div className="px-6 pt-6 animate-in fade-in slide-in-from-top-5 duration-700">
                  <HeroSection />
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
                  <InstalledPage onSelectPackage={setSelectedPackage} />
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
      <ErrorModal />
    </div>
  );
}

export default App;
