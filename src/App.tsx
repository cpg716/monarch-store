import { useState, useEffect, useRef, useCallback } from 'react';
import { commands } from './services/bindings';
import { unwrap } from './utils/specta';
import { ArrowLeft, Heart, AlertCircle, Database, Loader2 } from 'lucide-react';
import { useFavorites } from './hooks/useFavorites';
import Sidebar from './components/Sidebar';
import MobileNav from './components/MobileNav';
import SearchBar from './components/SearchBar';
import InstallMonitor from './components/InstallMonitor';
import type { Package } from './services/bindings';
import { PackageSource } from './services/bindings';
import TrendingSection from './components/TrendingSection';
import HeroSection from './components/HeroSection';
import PackageDetails from './pages/PackageDetailsFresh';
import { useAppStore } from './store/internal_store';
import { getPackageListKey } from './utils/packageKey';
import CategoryView from './pages/CategoryView';
import InstalledPage from './pages/InstalledPage';
import UpdatesPage from './pages/UpdatesPage';
import NewsPage from './pages/NewsPage';
import SettingsPage from './pages/SettingsPage';
import { useTheme } from './hooks/useTheme';
import './App.css';
import LoadingScreen from './components/LoadingScreen';
import OnboardingModal from './components/OnboardingModal';
import ErrorModal from './components/ErrorModal';
import ConfirmationModal from './components/ConfirmationModal';
import SearchPage from './pages/SearchPage';
import { useSearchHistory } from './hooks/useSearchHistory';
import { useSettings } from './hooks/useSettings';
import { useUpdateChecker } from './hooks/useUpdateChecker';
import HomePage from './pages/HomePage';
import { ESSENTIAL_IDS } from './constants';
import { listen } from '@tauri-apps/api/event';
import { UpdateProgress } from './store/internal_store';
import { useToast } from './context/ToastContext';
import { useSessionPassword } from './context/useSessionPassword';
import { useErrorService } from './context/ErrorContext';
import TitleBar from './components/TitleBar';

function App() {
  const activeTab = useAppStore(s => s.activeTab);
  const setActiveTab = useAppStore(s => s.setActiveTab);
  const [activeInstall, setActiveInstall] = useState<{ name: string; source: PackageSource; repoName?: string; displayName?: string; mode: 'install' | 'uninstall' } | null>(null);
  const [viewAll, setViewAll] = useState<'essentials' | 'trending' | null>(null);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [packages, setPackages] = useState<Package[]>([]);
  const [preferredSource, setPreferredSource] = useState<string | undefined>(undefined);
  const [showSystemFixPopup, setShowSystemFixPopup] = useState(false);
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(true);
  const [pendingDbRepair, setPendingDbRepair] = useState(false);
  const [dbRepairInProgress, setDbRepairInProgress] = useState(false);
  const [systemHealth, setSystemHealth] = useState<{ is_healthy: boolean, reasons: string[] } | null>(null);
  const [onboardingReason, setOnboardingReason] = useState<string | undefined>(undefined);

  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const searchRequestIdRef = useRef(0);
  const updateTimerRef = useRef<number | null>(null);

  const { addSearch } = useSearchHistory();
  const activePackageId = useAppStore((s) => s.activePackageId);
  const setActivePackageId = useAppStore((s) => s.setActivePackageId);
  const {
    fetchInfraStats,
    setUpdateProgress,
    setUpdateStatus,
    setUpdatePhase,
    setUpdating,
    addUpdateLog,
    setRebootRequired,
    setPacnewWarnings,
    upsertPackages,
    syncRegistry,
    syncRegistryBulk,
    setLastInstallTarget,
    updatePackageInstalledState,
    setEssentialsPackages,
    setTrendingPackages,
    hydrateFavorites,
    essentialsIds,
    trendingIds,
    initializeSettings,
    onboardingCompleted,
    setOnboardingCompleted,
    declinedSystemSetup,
    setDeclinedSystemSetup
  } = useAppStore();

  const { accentColor } = useTheme();
  const { favorites } = useFavorites();
  const favoriteError = useAppStore((s) => s.favoriteError);
  const clearFavoriteError = useAppStore((s) => s.clearFavoriteError);
  const { show: showToast } = useToast();
  const { requestSessionPassword } = useSessionPassword();
  const errorService = useErrorService();
  const reducePasswordPrompts = useAppStore((s) => s.reducePasswordPrompts);
  const { isAurEnabled, isFlatpakEnabled, isChaoticEnabled } = useSettings();
  const [enabledRepos, setEnabledRepos] = useState<{ name: string; enabled: boolean; source: any }[]>([]);

  const initializeStartup = useCallback(async () => {
    const startTime = Date.now();
    try {
      await initializeSettings(); // Fetch all backend settings (parallel, clean, etc)

      // CRITICAL: Fetch fresh state AFTER settings are loaded to avoid closure staleness
      const state = useAppStore.getState();
      const {
        reducePasswordPrompts,
        isFlatpakEnabled,
        isAurEnabled,
        isChaoticEnabled,
        onboardingCompleted,
        declinedSystemSetup
      } = state;

      commands.emitSyncProgress('Checking system...').catch(() => { });
      const needsUnlock = await commands.needsStartupUnlock().catch(() => false);

      if (needsUnlock && reducePasswordPrompts) {
        try {
          commands.emitSyncProgress('Unlock may be needed...').catch(() => { });
          const pwd = await requestSessionPassword();
          unwrap(await commands.unlockPacmanIfStale(pwd ?? null));
        } catch (e) {
          errorService.reportWarning(e as Error | string);
        }
      } else if (needsUnlock) {
        commands.emitSyncProgress('Checking lock...').catch(() => { });
        commands.unlockPacmanIfStale(null).then(unwrap).catch((e) => errorService.reportWarning(e as Error | string));
      }

      commands.emitSyncProgress('Checking health...').catch(() => { });
      fetchInfraStats(); // No await needed, background
      await state.checkTelemetry();

      commands.getRepoStates()
        .then(unwrap)
        .then(repos => setEnabledRepos(repos.filter(r => r.enabled)))
        .catch((e) => errorService.reportError(e as Error | string));

      const systemStatus = unwrap(await commands.checkInitializationStatus());
      setSystemHealth(systemStatus);
      commands.emitSyncProgress('Preparing...').catch(() => { });

      const isCompleted = onboardingCompleted;
      let redoOnboarding = !isCompleted;

      const onlySyncDbRepair = systemStatus.needs_sync_db_repair && systemStatus.reasons.length <= 1 &&
        systemStatus.reasons.some((r: string) => r.toLowerCase().includes('sync') || r.toLowerCase().includes('database'));

      if (!systemStatus.is_healthy) {
        if (onlySyncDbRepair) {
          setPendingDbRepair(true);
        } else if (!declinedSystemSetup) {
          console.warn("System is unhealthy. Triggering repair flow.");
          const reasonText = systemStatus.reasons.join(" ");
          setOnboardingReason(`MonARCH detected system defects: ${reasonText} MonARCH will attempt to fix them on launch. In the next step you can choose to enter your password once to avoid multiple system prompts.`);
          setShowSystemFixPopup(true);
          redoOnboarding = true;
        }
      }

      commands.emitSyncProgress("Loading Essentials...").catch(() => { });
      const list = await commands.getEssentialsList().then(unwrap).catch(() => ESSENTIAL_IDS as string[]);

      // Use the FRESH state flags for these calls
      const essentialsPromise = commands.getPackagesByNames(list, {
        flatpak_enabled: isFlatpakEnabled,
        aur_enabled: isAurEnabled,
        chaotic_enabled: isChaoticEnabled,
        for_installed_lookup: false
      }, null).then(unwrap);

      const trendingPromise = commands.getTrending({
        flatpak_enabled: isFlatpakEnabled,
        aur_enabled: isAurEnabled,
        chaotic_enabled: isChaoticEnabled,
        for_installed_lookup: false
      }).then(unwrap);

      const [pkgs, trendingPkgs] = await Promise.all([
        essentialsPromise.catch(() => [] as Package[]),
        trendingPromise.catch(() => [] as Package[]),
      ]);

      setEssentialsPackages(pkgs);
      setTrendingPackages(trendingPkgs);
      commands.emitSyncProgress("Analyzing Trending Apps...").catch(() => { });

      if (redoOnboarding) {
        setShowOnboarding(true);
      } else {
        if (!isCompleted) {
          setOnboardingCompleted(true);
        }
        const refreshRequested = await commands.checkAndClearRefreshRequested().catch(() => false);
        const syncOnStartup = await commands.isSyncOnStartupEnabled().catch(() => true);
        const lastSyncAgeSecStr = await commands.getLastSyncAgeSeconds().catch(() => null);
        const lastSyncAgeSec = lastSyncAgeSecStr ? parseInt(lastSyncAgeSecStr) : null;
        const STALE_SECS = 6 * 3600;
        const needsSync = refreshRequested || (syncOnStartup && (lastSyncAgeSec == null || lastSyncAgeSec > STALE_SECS));

        if (needsSync) {
          (async () => {
            try {
              commands.emitSyncProgress('Syncing package databases...').catch(() => { });
              const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
              unwrap(await commands.applyOsConfig(pwd ?? null));
              unwrap(await commands.syncSystemDatabases(pwd));
            } catch (e) {
              errorService.reportWarning(e as Error | string);
            }
            commands.triggerRepoSync("3").catch((e) => errorService.reportError(e as Error | string));
          })();
        }
      }
    } catch (e) {
      errorService.reportError(e as Error | string);
    } finally {
      const elapsed = Date.now() - startTime;
      const minDelayMs = 1200;
      const remaining = Math.max(0, minDelayMs - elapsed);
      setTimeout(() => setIsRefreshing(false), remaining);
    }
  }, [
    // MINIMAL DEPENDENCIES: Only stable functions/refs. 
    // State variables (isFlatpakEnabled, onboardingCompleted, etc.) MUST NOT be here.
    requestSessionPassword,
    errorService,
    fetchInfraStats,
    setEssentialsPackages,
    setTrendingPackages,
    setSystemHealth,
    setPendingDbRepair,
    setOnboardingReason,
    setShowSystemFixPopup,
    setEnabledRepos,
    setShowOnboarding,
    setIsRefreshing,
    // Store actions are stable
    setOnboardingCompleted,
    initializeSettings
  ]);

  // Registry Sync Listeners
  useEffect(() => {
    const unlistenSync = listen<string[]>('registry-sync', (event) => {
      console.log('[REGISTRY] Throttled sync triggered for:', event.payload);
      syncRegistry(event.payload);
    });
    const unlistenBulk = listen('registry-sync-bulk', () => {
      console.log('[REGISTRY] Bulk sync triggered');
      syncRegistryBulk();
    });

    return () => {
      unlistenSync.then(fn => fn());
      unlistenBulk.then(fn => fn());
    };
  }, [syncRegistry, syncRegistryBulk, initializeStartup]);

  useEffect(() => {
    if (favoriteError) {
      showToast(favoriteError, 'error');
      clearFavoriteError();
    }
  }, [favoriteError, showToast, clearFavoriteError]);

  // Background update checker: respect Sources settings so badge matches what Update All would run
  useUpdateChecker(isAurEnabled, isFlatpakEnabled);

  useEffect(() => {
    hydrateFavorites();
  }, [hydrateFavorites]);

  const polkitCheckedRef = useRef(false);
  useEffect(() => {
    if (polkitCheckedRef.current || isRefreshing) return;
    polkitCheckedRef.current = true;
    commands.checkSecurityPolicy()
      .then((installed) => {
        if (!installed) {
          showToast(
            'Polkit rule not installed. Install and system actions may prompt for password. Enable One-Click in Settings to fix.',
            'warning'
          );
        }
      })
      .catch((e) => errorService.reportWarning(e as Error | string));
  }, [isRefreshing, showToast, errorService]);

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
              const reboot = unwrap(await commands.checkRebootRequired());
              setRebootRequired(reboot);
              const warnings = unwrap(await commands.getPacnewWarnings());
              setPacnewWarnings(warnings);
            } catch (e) {
              errorService.reportError(e as Error | string);
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

  const refreshSystemHealth = async () => {
    try {
      const status = unwrap(await commands.checkInitializationStatus());
      setSystemHealth(status);
      return status;
    } catch (e) {
      errorService.reportError(e as Error | string);
      return null;
    }
  };

  const handleOnboardingComplete = async () => {
    setOnboardingCompleted(true);
    setShowOnboarding(false);
    // Re-check so the infrastructure banner updates or disappears after repair
    await refreshSystemHealth();
  };

  useEffect(() => {
    // Ensure app becomes visible after at most 18s so Essentials/Trending can load in background
    const maxWaitMs = 18000;
    const timeoutId = setTimeout(() => setIsRefreshing(false), maxWaitMs);
    initializeStartup();
    return () => clearTimeout(timeoutId);
  }, [initializeStartup]);

  useEffect(() => {
    if (searchQuery) setActivePackageId(null);
  }, [searchQuery, setActivePackageId]);

  useEffect(() => {
    setActivePackageId(null);
    setSelectedCategory(null);
    setViewAll(null);
  }, [activeTab, setActivePackageId]);

  // On install/uninstall success, refresh package registry installed state so cards and details stay in sync
  useEffect(() => {
    const unlisten = listen<string>('install-complete', (event) => {
      if (event.payload !== 'success') return;
      const target = useAppStore.getState().lastInstallTarget;
      if (target) {
        updatePackageInstalledState(target.name, target.mode === 'install');
        setLastInstallTarget(null);
      }
    });
    return () => {
      unlisten.then((f) => f()).catch(() => { });
    };
  }, [updatePackageInstalledState, setLastInstallTarget]);

  useEffect(() => {
    // Increment request ID to track stale responses
    const currentRequestId = ++searchRequestIdRef.current;

    const search = async () => {
      if (!searchQuery) {
        setPackages([]);
        useAppStore.getState().setSearchResultIds([]);
        return;
      }
      try {
        const results = unwrap(await commands.searchPackages(searchQuery, {
          flatpak_enabled: isFlatpakEnabled,
          aur_enabled: isAurEnabled,
          chaotic_enabled: isChaoticEnabled,
          for_installed_lookup: false
        }));
        if (currentRequestId !== searchRequestIdRef.current) return;
        const { upsertPackages: upsert, setSearchResultIds: setIds, fetchRatingsForPackages } = useAppStore.getState();
        upsert(results);
        setIds([...new Set(results.map((p) => getPackageListKey(p)))]);

        // Safe batch rating fetch using IDs OR names (merges into live registry)
        const lookupIds = results.map(p => p.app_id || p.name).filter(id => !!id) as string[];
        fetchRatingsForPackages(lookupIds);

        setPackages(results);
        addSearch(searchQuery);
        commands.trackTelemetryEvent('search', {
          query: searchQuery,
          result_count: results.length,
          query_length: searchQuery.length,
          has_results: results.length > 0,
        }).catch(() => { });
      } catch (e) {
        errorService.reportError(e as Error | string);
      } finally {
        // Only update loading state if this is still the latest request
        if (currentRequestId === searchRequestIdRef.current) {
          setLoading(false);
        }
      }
    };

    const timeoutId = setTimeout(() => search(), 500);
    return () => clearTimeout(timeoutId);
  }, [searchQuery, addSearch, errorService]);

  const handleTabChange = (tab: string) => {
    if (tab === 'search') {
      if (activeTab === 'search') {
        setActivePackageId(null);
        setSelectedCategory(null);
        setViewAll(null);
        setSearchQuery('');
      } else {
        setSelectedCategory(null);
        setActivePackageId(null);
        setViewAll(null);
        setSearchQuery('');
      }
      setActiveTab('search');
      setTimeout(() => {
        const input = document.querySelector<HTMLInputElement>('input[data-monarch-search]');
        if (input) input.focus();
      }, 50);
      return;
    }

    if (activeTab === tab) {
      setActivePackageId(null);
      setSelectedCategory(null);
      setViewAll(null);
      setSearchQuery('');
    }
    setActiveTab(tab);
    setSearchQuery('');

    if (tab === 'settings') {
      setTimeout(() => {
        const el = document.getElementById('system-health');
        if (el) el.scrollIntoView({ behavior: 'smooth' });
      }, 100);
    }
  };

  const handleSelectPackage = useCallback(
    (pkg: Package, preferredSource?: string) => {
      // Always open the canonical (merged) package so details show one page with all sources/selector
      const id =
        typeof pkg.canonical_id === 'string' && pkg.canonical_id.trim() !== ''
          ? pkg.canonical_id
          : getPackageListKey(pkg);
      setActivePackageId(id);
      if (preferredSource !== undefined) setPreferredSource(preferredSource);
    },
    [setActivePackageId]
  );

  const handleBack = () => {
    if (activePackageId) {
      setActivePackageId(null);
      setPreferredSource(undefined);
    } else if (selectedCategory) {
      setSelectedCategory(null);
    } else if (viewAll) {
      setViewAll(null);
    }
  };

  const runDbRepair = async () => {
    if (dbRepairInProgress) return;
    setDbRepairInProgress(true);
    try {
      const pwd = await requestSessionPassword();
      unwrap(await commands.forceRefreshDatabases(pwd));
      await commands.clearSyncDbHealthCache();
      const refreshed = unwrap(await commands.checkInitializationStatus());
      setSystemHealth(
        refreshed.needs_sync_db_repair
          ? { ...refreshed, is_healthy: true, reasons: refreshed.reasons.filter((r: string) => !r.toLowerCase().includes('sync') && !r.toLowerCase().includes('database')) }
          : refreshed
      );
      setPendingDbRepair(false);
      showToast('Package databases fixed.', 'success');
    } catch (e) {
      errorService.reportError(e as Error | string);
      showToast('Repair failed. Try Settings → Refresh Databases or run: sudo pacman -Syy', 'error');
    } finally {
      setDbRepairInProgress(false);
    }
  };

  if (isRefreshing) return <LoadingScreen />;

  return (
    <div
      className="h-screen w-screen bg-app-bg text-app-fg overflow-hidden font-sans transition-colors"
      style={{ '--tw-selection-bg': `${accentColor}4D` } as any}
    >
      <div className="relative flex flex-col h-full border border-white/5 rounded-xl shadow-2xl overflow-hidden">
        <TitleBar />
        {/* Grandma-proof: one-step DB repair overlay when only issue is corrupt sync DBs */}
        {pendingDbRepair && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-6">
            <div className="bg-app-bg border border-app-border rounded-2xl shadow-2xl p-8 max-w-md w-full text-center">
              <div className="w-14 h-14 rounded-full bg-amber-500/20 flex items-center justify-center mx-auto mb-6">
                <Database size={28} className="text-amber-500" />
              </div>
              <h2 className="text-xl font-bold text-app-fg mb-2">Package databases need a quick fix</h2>
              <p className="text-app-muted text-sm mb-6">Enter your password once to repair. This only takes a moment.</p>
              <button
                onClick={runDbRepair}
                disabled={dbRepairInProgress}
                className="w-full py-3 px-6 rounded-xl bg-blue-600 hover:bg-blue-500 text-white font-bold text-sm disabled:opacity-50 flex items-center justify-center gap-2"
              >
                {dbRepairInProgress ? (
                  <>
                    <Loader2 size={18} className="animate-spin" />
                    Repairing…
                  </>
                ) : (
                  'Fix now'
                )}
              </button>
            </div>
          </div>
        )}
        <div className="flex flex-1 overflow-hidden pt-10">
          {!showOnboarding && (
            <div className="hidden md:flex">
              <Sidebar activeTab={activeTab} setActiveTab={handleTabChange} />
            </div>
          )}

          <main className="flex-1 flex flex-col h-full overflow-hidden relative min-w-0 pb-24 md:pb-0">
            {!showOnboarding && systemHealth && !systemHealth.is_healthy && !pendingDbRepair && (
              <div className="bg-red-600 text-white px-6 py-3 flex flex-col md:flex-row items-center justify-between gap-4 text-sm font-bold animate-in slide-in-from-top duration-300 z-30 shrink-0 shadow-lg">
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
            ) : activePackageId ? (
              <PackageDetails
                onBack={handleBack}
                preferredSource={preferredSource}
                installInProgress={activeInstall !== null}
                activeInstallPackage={activeInstall}
                onInstall={(p: { name: string; source: PackageSource | string; repoName?: string; displayName?: string }) => {
                  const srcArgs = typeof p.source === 'string'
                    ? { source_type: 'repo', id: p.source, version: '', label: p.source.toUpperCase() } as PackageSource
                    : p.source;
                  setActiveInstall({ name: p.name, source: srcArgs, repoName: p.repoName, displayName: p.displayName, mode: 'install' });
                  setLastInstallTarget({ name: p.name, mode: 'install' });
                }}
                onUninstall={(p: { name: string; source: PackageSource | string; repoName?: string; displayName?: string }) => {
                  const srcArgs = typeof p.source === 'string'
                    ? { source_type: 'repo', id: p.source, version: '', label: p.source.toUpperCase() } as PackageSource
                    : p.source;
                  setActiveInstall({ name: p.name, source: srcArgs, repoName: p.repoName, displayName: p.displayName, mode: 'uninstall' });
                  setLastInstallTarget({ name: p.name, mode: 'uninstall' });
                }}
                onOpenSettings={() => setActiveTab('settings')}
              />
            ) : selectedCategory ? (
              <CategoryView category={selectedCategory} onBack={handleBack} onSelectPackage={handleSelectPackage} onOpenSettings={() => setActiveTab('settings')} />
            ) : viewAll ? (
              <div className="flex-1 overflow-y-auto pb-32 scroll-gpu">
                <div className="p-10 pb-6 sticky top-0 bg-app-bg/95 backdrop-blur-xl z-20 border-b border-app-border/50 flex items-center gap-4">
                  <button onClick={handleBack} className="p-2 rounded-lg hover:bg-app-fg/10 transition-colors"><ArrowLeft size={24} /></button>
                  <h2 className="text-2xl font-bold">{viewAll === 'essentials' ? 'All Essentials' : 'Trending Applications'}</h2>
                </div>
                <div className="p-8 max-w-7xl mx-auto">
                  <TrendingSection
                    title=""
                    listKind={viewAll === 'essentials' ? 'essentials' : 'trending'}
                    filterIds={viewAll === 'essentials' ? essentialsIds : trendingIds}
                    onSelectPackage={handleSelectPackage}
                  />
                </div>
              </div>
            ) : (
              <div className="flex-1 overflow-hidden flex flex-col relative">
                <div className="absolute inset-0 bg-gradient-to-br from-purple-500/5 via-app-bg/50 to-blue-500/5 pointer-events-none transition-colors" />

                <div ref={scrollContainerRef} className="flex-1 overflow-y-auto min-h-0 pb-32 scroll-smooth scroll-gpu">
                  <div className="max-w-[1920px] mx-auto w-full">
                    {activeTab === 'explore' && !searchQuery && (
                      <div className="px-6 pt-6 animate-in fade-in slide-in-from-top-5 duration-700">
                        <HeroSection />
                      </div>
                    )}

                    <div className="sticky top-0 z-10 px-4 sm:px-6 py-4 bg-app-bg backdrop-blur-xl transition-all flex items-center justify-center gap-3">
                      <SearchBar
                        value={searchQuery}
                        onChange={setSearchQuery}
                        onBack={() => {
                          setSearchQuery('');
                          setActiveTab('explore');
                        }}
                      />
                    </div>

                    <div className="max-w-7xl mx-auto px-6 pb-16 min-h-[50vh]">
                      {(searchQuery || activeTab === 'search') ? (
                        <SearchPage
                          query={searchQuery}
                          onQueryChange={setSearchQuery}
                          packages={packages}
                          loading={loading}
                          onSelectPackage={handleSelectPackage}
                          enabledRepos={enabledRepos}
                          onOpenSettings={() => setActiveTab('settings')}
                        />
                      ) : activeTab === 'explore' ? (
                        <HomePage
                          onSelectPackage={handleSelectPackage}
                          onSeeAll={setViewAll}
                          onSelectCategory={setSelectedCategory}
                          onOpenSettings={() => setActiveTab('settings')}
                        />
                      ) : activeTab === 'installed' ? (
                        <InstalledPage onSelectPackage={handleSelectPackage} />
                      ) : activeTab === 'favorites' ? (
                        <div className="py-4">
                          <h2 className="text-2xl font-bold mb-2">Favorites</h2>
                          {favorites.length === 0 ? (
                            <div className="text-center text-app-muted py-20 flex flex-col items-center gap-4">
                              <div className="p-4 rounded-full bg-app-subtle"><Heart size={32} className="opacity-50" /></div>
                              <p className="font-bold">No favorites yet</p>
                            </div>
                          ) : (
                            <TrendingSection title="" listKind="favorites" filterIds={favorites} onSelectPackage={handleSelectPackage} limit={100} onOpenSettings={() => setActiveTab('settings')} />
                          )}
                        </div>
                      ) : activeTab === 'updates' ? (
                        <UpdatesPage />
                      ) : activeTab === 'news' ? (
                        <NewsPage />
                      ) : activeTab === 'settings' ? (
                        <SettingsPage
                          onRestartOnboarding={() => setShowOnboarding(true)}
                          onRepairComplete={async () => { await refreshSystemHealth(); }}
                        />
                      ) : null}
                    </div>
                  </div>
                </div>
              </div>
            )}
          </main>
        </div>
      </div>
      {/* System Fix Popup - Shows BEFORE onboarding when system defects detected */}
      {showSystemFixPopup && onboardingReason && (
        <ConfirmationModal
          isOpen={showSystemFixPopup}
          onClose={() => {
            setDeclinedSystemSetup(true);
            setShowSystemFixPopup(false);
            // Skip: do not show onboarding; user enters app and can use Repair banner
          }}
          onConfirm={() => {
            setShowSystemFixPopup(false);
            setShowOnboarding(true);
          }}
          title="System Setup Required"
          message={onboardingReason}
          confirmLabel="Continue to Setup"
          cancelLabel="Skip (Not Recommended)"
          variant="info"
        />
      )}

      {/* Onboarding - Only show after popup is dismissed or if no reason */}
      {showOnboarding && !showSystemFixPopup && <OnboardingModal onComplete={handleOnboardingComplete} reason={onboardingReason} />}
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
      <MobileNav activeTab={activeTab} setActiveTab={handleTabChange} />
      <ErrorModal />
    </div>
  );
}

export default App;
