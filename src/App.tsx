import { lazy, Suspense, useState, useEffect, useRef, useCallback } from 'react';
import { commands } from './services/bindings';
import { unwrap } from './utils/specta';
import { ArrowLeft, Heart, AlertCircle, Database, Loader2 } from 'lucide-react';
import { useFavorites } from './hooks/useFavorites';
import Sidebar from './components/Sidebar';
import MobileNav from './components/MobileNav';
import SearchBar from './components/SearchBar';
import InstallMonitor from './components/InstallMonitor';
import type { DiscoveryIntent, Package, SearchSuggestion } from './services/bindings';
import { PackageSource } from './services/bindings';
import TrendingSection from './components/TrendingSection';
import { useAppStore } from './store/internal_store';
import { getPackageListKey } from './utils/packageKey';
import { useTheme } from './hooks/useTheme';
import './App.css';
import LoadingScreen from './components/LoadingScreen';
import OnboardingModal from './components/OnboardingModal';
import ErrorModal from './components/ErrorModal';
import ConfirmationModal from './components/ConfirmationModal';
import { useSearchHistory } from './hooks/useSearchHistory';
import { useUpdateChecker } from './hooks/useUpdateChecker';
import { listen } from '@tauri-apps/api/event';
import { UpdateProgress } from './store/internal_store';
import { useToast } from './context/ToastContext';
import { useSessionPassword } from './context/useSessionPassword';
import { useErrorService } from './context/ErrorContext';
import TitleBar from './components/TitleBar';
import { debugWarn } from './utils/debugLog';

const PackageDetails = lazy(() => import('./pages/PackageDetailsFresh'));
const CategoryView = lazy(() => import('./pages/CategoryView'));
const InstalledPage = lazy(() => import('./pages/InstalledPage'));
const UpdatesPage = lazy(() => import('./pages/UpdatesPage'));
const NewsPage = lazy(() => import('./pages/NewsPage'));
const SettingsPage = lazy(() => import('./pages/SettingsPage'));
const SearchPage = lazy(() => import('./pages/SearchPage'));
const HomePage = lazy(() => import('./pages/HomePage'));

const DEFAULT_HOME_QUICK_STARTS: DiscoveryIntent[] = [
  { id: 'web-browsers', label: 'Web Browsers', description: 'Find browsers and internet apps', query: 'browser', category: null },
  { id: 'office-school', label: 'Office & School', description: 'Documents, mail, and study tools', query: 'office suite', category: null },
  { id: 'gaming', label: 'Gaming', description: 'Launchers, emulators, and game clients', query: null, category: 'Game' },
  { id: 'chat-voice', label: 'Chat & Voice', description: 'Messaging and voice apps', query: 'discord telegram', category: null },
  { id: 'creative-tools', label: 'Creative Tools', description: 'Art, design, and editing apps', query: null, category: 'Graphics' },
  { id: 'system-utilities', label: 'System Utilities', description: 'Maintenance and system tools', query: null, category: 'System' },
];

function App() {
  useTheme();
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
  const [homeQuickStarts, setHomeQuickStarts] = useState<DiscoveryIntent[]>([]);
  const [homeEssentialsPackages, setHomeEssentialsPackages] = useState<Package[]>([]);
  const [homeTrendingPackages, setHomeTrendingPackages] = useState<Package[]>([]);
  const [homeDiscoveryLoading, setHomeDiscoveryLoading] = useState(false);
  const [homeDiscoveryError, setHomeDiscoveryError] = useState<string | null>(null);
  const [searchSuggestions, setSearchSuggestions] = useState<SearchSuggestion[]>([]);
  const [queryInterpretation, setQueryInterpretation] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(true);
  const [pendingDbRepair, setPendingDbRepair] = useState(false);
  const [dbRepairInProgress, setDbRepairInProgress] = useState(false);
  const [systemHealth, setSystemHealth] = useState<{ is_healthy: boolean, reasons: string[] } | null>(null);
  const [onboardingReason, setOnboardingReason] = useState<string | undefined>(undefined);

  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const searchRequestIdRef = useRef(0);
  const updateTimerRef = useRef<number | null>(null);
  const startupInitRef = useRef(false);
  const homeDiscoveryInitRef = useRef(false);
  const homeDiscoveryInFlightRef = useRef<Promise<void> | null>(null);
  const registrySyncQueueRef = useRef<Set<string>>(new Set());
  const registrySyncFlushTimerRef = useRef<number | null>(null);
  const registryBulkTimerRef = useRef<number | null>(null);

  const { addSearch } = useSearchHistory();
  const activePackageId = useAppStore((s) => s.activePackageId);
  const setActivePackageId = useAppStore((s) => s.setActivePackageId);
  const packageRegistry = useAppStore((s) => s.packageRegistry);
  const fetchInfraStats = useAppStore(s => s.fetchInfraStats);
  const setUpdateProgress = useAppStore(s => s.setUpdateProgress);
  const setUpdateStatus = useAppStore(s => s.setUpdateStatus);
  const setUpdatePhase = useAppStore(s => s.setUpdatePhase);
  const setUpdating = useAppStore(s => s.setUpdating);
  const addUpdateLog = useAppStore(s => s.addUpdateLog);
  const setRebootRequired = useAppStore(s => s.setRebootRequired);
  const setPacnewWarnings = useAppStore(s => s.setPacnewWarnings);
  const upsertPackages = useAppStore(s => s.upsertPackages);
  const syncRegistry = useAppStore(s => s.syncRegistry);
  const syncRegistryBulk = useAppStore(s => s.syncRegistryBulk);
  const setLastInstallTarget = useAppStore(s => s.setLastInstallTarget);
  const updatePackageInstalledState = useAppStore(s => s.updatePackageInstalledState);
  const setEssentialsPackages = useAppStore(s => s.setEssentialsPackages);
  const setTrendingPackages = useAppStore(s => s.setTrendingPackages);
  const hydrateFavorites = useAppStore(s => s.hydrateFavorites);
  const essentialsIds = useAppStore(s => s.essentialsIds);
  const trendingIds = useAppStore(s => s.trendingIds);
  const initializeSettings = useAppStore(s => s.initializeSettings);
  const onboardingCompleted = useAppStore(s => s.onboardingCompleted);
  const setOnboardingCompleted = useAppStore(s => s.setOnboardingCompleted);
  const declinedSystemSetup = useAppStore(s => s.declinedSystemSetup);
  const setDeclinedSystemSetup = useAppStore(s => s.setDeclinedSystemSetup);

  const accentColor = useAppStore(s => s.accentColor);
  const favorites = useAppStore(s => s.favorites);
  const favoriteError = useAppStore((s) => s.favoriteError);
  const clearFavoriteError = useAppStore((s) => s.clearFavoriteError);
  const { show: showToast } = useToast();
  const { requestSessionPassword } = useSessionPassword();
  const { reportError, reportWarning } = useErrorService();
  const reducePasswordPrompts = useAppStore((s) => s.reducePasswordPrompts);
  const isAurEnabled = useAppStore(s => s.isAurEnabled);
  const isFlatpakEnabled = useAppStore(s => s.isFlatpakEnabled);
  const isChaoticEnabled = useAppStore(s => s.isChaoticEnabled);
  const [enabledRepos, setEnabledRepos] = useState<{ name: string; enabled: boolean; source: any }[]>([]);

  const loadHomeDiscovery = useCallback(async () => {
    if (homeDiscoveryInFlightRef.current) {
      return homeDiscoveryInFlightRef.current;
    }

    const run = (async () => {
      setHomeDiscoveryLoading(true);
      setHomeDiscoveryError(null);
      setHomeQuickStarts(DEFAULT_HOME_QUICK_STARTS);

      try {
        const snapshot = unwrap(await commands.getDiscoveryHomeSnapshot());
        const essentials = snapshot.essentials ?? [];
        const trendingPkgs = snapshot.trending ?? [];
        const quickStarts = snapshot.quick_starts?.length ? snapshot.quick_starts : DEFAULT_HOME_QUICK_STARTS;

        if (essentials.length === 0) {
          debugWarn('[HomeDiscovery] Snapshot returned no essentials packages');
        }
        if (trendingPkgs.length === 0) {
          debugWarn('[HomeDiscovery] Snapshot returned no trending packages');
        }

        setHomeQuickStarts(quickStarts);
        setHomeEssentialsPackages(essentials);
        setEssentialsPackages(essentials);
        setHomeTrendingPackages(trendingPkgs);
        setTrendingPackages(trendingPkgs);
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        debugWarn('[HomeDiscovery] Snapshot unavailable', message);
        setHomeDiscoveryError(message);
        setHomeQuickStarts(DEFAULT_HOME_QUICK_STARTS);
        setHomeEssentialsPackages([]);
        setEssentialsPackages([]);
        setHomeTrendingPackages([]);
        setTrendingPackages([]);
      } finally {
        setHomeDiscoveryLoading(false);
        homeDiscoveryInFlightRef.current = null;
      }
    })();

    homeDiscoveryInFlightRef.current = run;
    return run;
  }, [
    setEssentialsPackages,
    setTrendingPackages,
  ]);

  const initializeStartup = useCallback(async () => {
    const startTime = Date.now();
    try {
      commands.emitSyncProgress('Loading saved settings').catch(() => { });
      await initializeSettings(); // Fetch all backend settings (parallel, clean, etc)

      // CRITICAL: Fetch fresh state AFTER settings are loaded to avoid closure staleness
      const state = useAppStore.getState();
      const {
        reducePasswordPrompts,
        onboardingCompleted,
        declinedSystemSetup,
        oneClickEnabled
      } = state;
      const useOneClickSessionAuth = reducePasswordPrompts || oneClickEnabled;

      const needsUnlock = await commands.needsStartupUnlock().catch(() => false);

      if (needsUnlock) {
        commands.emitSyncProgress('Checking for a stale package-manager lock').catch(() => { });

        (async () => {
          try {
            let pwd: string | null = null;
            if (useOneClickSessionAuth) {
              commands.emitSyncProgress('Authorization needed to clear a stale package-manager lock').catch(() => { });
              pwd = await Promise.race<string | null>([
                requestSessionPassword(),
                new Promise<string | null>((resolve) => setTimeout(() => resolve(null), 4000)),
              ]);
            }
            await commands.unlockPacmanIfStale(pwd ?? null).then(unwrap);
          } catch (e) {
            reportWarning(e as Error | string);
          }
        })();
      }

      commands.emitSyncProgress('Checking package manager health').catch(() => { });
      fetchInfraStats();
      void state.checkTelemetry();

      commands.getRepoStates()
        .then(unwrap)
        .then(repos => setEnabledRepos(repos.filter(r => r.enabled)))
        .catch((e) => reportError(e as Error | string));

      const systemStatus = unwrap(await commands.checkInitializationStatus());
      setSystemHealth(systemStatus);
      commands.emitSyncProgress('Loading your software catalog').catch(() => { });

      const isCompleted = onboardingCompleted;
      let redoOnboarding = !isCompleted;

      const onlySyncDbRepair = systemStatus.needs_sync_db_repair && systemStatus.reasons.length <= 1 &&
        systemStatus.reasons.some((r: string) => r.toLowerCase().includes('sync') || r.toLowerCase().includes('database'));

      if (!systemStatus.is_healthy) {
        if (onlySyncDbRepair) {
          setPendingDbRepair(true);
        } else if (!declinedSystemSetup) {
          const reasonText = systemStatus.reasons.join(" ");
          setOnboardingReason(`MonARCH detected system defects: ${reasonText} MonARCH will attempt to fix them on launch. In the next step you can choose to enter your password once to avoid multiple system prompts.`);
          setShowSystemFixPopup(true);
          redoOnboarding = true;
        }
      }

      commands.emitSyncProgress('Restoring featured apps').catch(() => { });
      void (async () => {
        try {
          const installedPkgs = unwrap(await commands.getInstalledCatalog());
          upsertPackages(installedPkgs);
        } catch (e) {
          reportWarning(e as Error | string);
        }
      })();
      commands.emitSyncProgress('Ready').catch(() => { });

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
              commands.emitSyncProgress('Refreshing package sources in background').catch(() => { });
              const pwd = useOneClickSessionAuth ? await requestSessionPassword() : null;
              unwrap(await commands.triggerRepoSync(3, pwd ?? null));
            } catch (e) {
              reportWarning(e as Error | string);
            }
          })();
        }
      }
    } catch (e) {
      reportError(e as Error | string);
      try {
        setEssentialsPackages([]);
        setTrendingPackages([]);
        setHomeEssentialsPackages([]);
        setHomeTrendingPackages([]);
        setHomeQuickStarts(DEFAULT_HOME_QUICK_STARTS);
      } catch {
      }
    } finally {
      const elapsed = Date.now() - startTime;
      const minDelayMs = 1200;
      const remaining = Math.max(0, minDelayMs - elapsed);
      setTimeout(() => setIsRefreshing(false), remaining);
    }
  }, [
    requestSessionPassword,
    reportError,
    reportWarning,
    fetchInfraStats,
    setEssentialsPackages,
    setTrendingPackages,
    setSystemHealth,
    setPendingDbRepair,
    setOnboardingReason,
    setShowSystemFixPopup,
    setEnabledRepos,
    setHomeQuickStarts,
    setShowOnboarding,
    setIsRefreshing,
    loadHomeDiscovery,
    syncRegistry,
    setOnboardingCompleted,
    initializeSettings
  ]);

  // Registry Sync Listeners
  useEffect(() => {
    const unlistenSync = listen<string[]>('registry-sync', (event) => {
      for (const id of event.payload ?? []) {
        if (id) registrySyncQueueRef.current.add(id);
      }
      if (registrySyncFlushTimerRef.current) {
        window.clearTimeout(registrySyncFlushTimerRef.current);
      }
      registrySyncFlushTimerRef.current = window.setTimeout(() => {
        const ids = Array.from(registrySyncQueueRef.current);
        registrySyncQueueRef.current.clear();
        registrySyncFlushTimerRef.current = null;
        if (ids.length > 0) {
          syncRegistry(ids);
        }
      }, 120);
    });
    const unlistenBulk = listen('registry-sync-bulk', () => {
      if (registryBulkTimerRef.current) {
        window.clearTimeout(registryBulkTimerRef.current);
      }
      registryBulkTimerRef.current = window.setTimeout(() => {
        registryBulkTimerRef.current = null;
        syncRegistryBulk();
      }, 150);
    });

    return () => {
      if (registrySyncFlushTimerRef.current) window.clearTimeout(registrySyncFlushTimerRef.current);
      if (registryBulkTimerRef.current) window.clearTimeout(registryBulkTimerRef.current);
      unlistenSync.then(fn => fn());
      unlistenBulk.then(fn => fn());
    };
  }, [syncRegistry, syncRegistryBulk]);

  useEffect(() => {
    if (favoriteError) {
      showToast(favoriteError, 'error');
      clearFavoriteError();
    }
  }, [favoriteError, showToast, clearFavoriteError]);

  // Background update checker includes installed updates from repo/AUR/Flatpak.
  // Source toggles affect discovery only (browse/search), not update detection.
  useUpdateChecker(!isRefreshing);

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
      .catch((e) => reportWarning(e as Error | string));
  }, [isRefreshing, showToast, reportWarning]);

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
              reportError(e as Error | string);
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
      reportError(e as Error | string);
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
    if (!startupInitRef.current) {
      startupInitRef.current = true;
      initializeStartup();
    }
    return () => clearTimeout(timeoutId);
  }, [initializeStartup]);

  useEffect(() => {
    if (homeDiscoveryInitRef.current) return;
    homeDiscoveryInitRef.current = true;
    void loadHomeDiscovery();
  }, [loadHomeDiscovery]);

  useEffect(() => {
    if (!isRefreshing && !useAppStore.getState().metadataInitialized) {
      useAppStore.setState({ metadataInitialized: true });
    }
  }, [isRefreshing]);

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

    if (!searchQuery) {
      setPackages([]);
      setSearchSuggestions([]);
      setQueryInterpretation(null);
      useAppStore.getState().setSearchResultIds([]);
      setLoading(false);
      return;
    }

    // Prepare for search: set loading immediately if we're active
    setLoading(true);

    const search = async () => {
      try {
        const searchResponse = unwrap(await commands.searchPackagesRich(searchQuery, {
          flatpak_enabled: isFlatpakEnabled,
          aur_enabled: isAurEnabled,
          chaotic_enabled: isChaoticEnabled,
          for_installed_lookup: false
        }));
        const results = searchResponse.packages ?? [];

        if (currentRequestId !== searchRequestIdRef.current) return;

        // Backend is SSOT: upsert backend payload as-is, then map current result IDs.
        upsertPackages(results);
        const ids = Array.from(new Set(results.map((p) => getPackageListKey(p)).filter(Boolean)));
        useAppStore.getState().setSearchResultIds(ids);
        setSearchSuggestions(searchResponse.suggestions ?? []);
        setQueryInterpretation(searchResponse.query_interpretation ?? null);

        // Safe batch rating fetch: use both app_id (ODRS canonical) and name (fallback)
        const lookupIds = Array.from(new Set(
          results.flatMap(p => [p.app_id, p.name].filter((id): id is string => !!id && id.length > 0))
        ));
        useAppStore.getState().fetchRatingsForPackages(lookupIds);

        setPackages(results);
        addSearch(searchQuery);
        commands.trackTelemetryEvent('search', {
          query: searchQuery,
          result_count: results.length,
          query_length: searchQuery.length,
          has_results: results.length > 0,
        }).catch(() => { });
      } catch (e) {
        // Ignore stale request failures (user typed a new query before this one resolved).
        if (currentRequestId !== searchRequestIdRef.current) return;
        setSearchSuggestions([]);
        setQueryInterpretation(null);
        reportError(e as Error | string);
      } finally {
        // Only update loading state if this is still the latest request
        if (currentRequestId === searchRequestIdRef.current) {
          setLoading(false);
        }
      }
    };

    const timeoutId = setTimeout(() => search(), 200);
    return () => clearTimeout(timeoutId);
  }, [searchQuery, addSearch, reportError, isFlatpakEnabled, isAurEnabled, isChaoticEnabled, upsertPackages]);

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
      const id = getPackageListKey(pkg);
      if (!id) {
        reportWarning(`Package missing canonical_id: ${pkg.name}`);
        return;
      }
      upsertPackages([pkg]);
      setActivePackageId(id);
      if (preferredSource !== undefined) setPreferredSource(preferredSource);
    },
    [reportWarning, setActivePackageId, upsertPackages]
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

  const handleQuickStart = useCallback((intent: DiscoveryIntent) => {
    setActivePackageId(null);
    setViewAll(null);
    if (intent.category) {
      setSearchQuery('');
      setSelectedCategory(intent.category);
      return;
    }
    if (intent.query) {
      setSelectedCategory(null);
      setActiveTab('search');
      setSearchQuery(intent.query);
    }
  }, [setActivePackageId, setActiveTab]);

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
      reportError(e as Error | string);
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
      <div className="relative flex flex-col h-full bg-app-bg border border-white/5 rounded-xl shadow-2xl overflow-hidden">
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
              <Suspense fallback={<div className="flex-1 bg-app-bg" />}>
                <PackageDetails
                  pkg={packageRegistry[activePackageId] as any}
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
              </Suspense>
            ) : selectedCategory ? (
              <Suspense fallback={<div className="flex-1 bg-app-bg" />}>
                <CategoryView category={selectedCategory} onBack={handleBack} onSelectPackage={handleSelectPackage} onOpenSettings={() => setActiveTab('settings')} />
              </Suspense>
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
                    preloadedPackages={viewAll === 'essentials' ? homeEssentialsPackages : homeTrendingPackages}
                    onSelectPackage={handleSelectPackage}
                  />
                </div>
              </div>
            ) : (
              <div className="flex-1 overflow-hidden flex flex-col relative">
                <div className="absolute inset-0 bg-gradient-to-br from-blue-500/5 via-transparent to-transparent pointer-events-none transition-colors" />

                <div ref={scrollContainerRef} className="flex-1 overflow-y-auto min-h-0 pb-32 scroll-smooth scroll-gpu">
                  <div className="max-w-[1920px] mx-auto w-full">
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
                      <Suspense fallback={<div className="py-12 text-center text-sm text-app-muted">Loading…</div>}>
                        {(searchQuery || activeTab === 'search') ? (
                          <SearchPage
                            query={searchQuery}
                            onQueryChange={setSearchQuery}
                            packages={packages}
                            loading={loading}
                            onSelectPackage={handleSelectPackage}
                            enabledRepos={enabledRepos}
                            suggestions={searchSuggestions}
                            queryInterpretation={queryInterpretation}
                            onOpenSettings={() => setActiveTab('settings')}
                          />
                        ) : activeTab === 'explore' ? (
                          <HomePage
                            onSelectPackage={handleSelectPackage}
                            onSeeAll={setViewAll}
                            onSelectCategory={setSelectedCategory}
                            quickStarts={homeQuickStarts}
                            essentialsPackages={homeEssentialsPackages}
                            trendingPackages={homeTrendingPackages}
                            homeDiscoveryLoading={homeDiscoveryLoading}
                            homeDiscoveryError={homeDiscoveryError}
                            onQuickStart={handleQuickStart}
                            onOpenSettings={() => setActiveTab('settings')}
                          />
                        ) : activeTab === 'installed' ? (
                          <InstalledPage
                            onSelectPackage={handleSelectPackage}
                            onUninstallPackage={(pkg) => {
                              setActiveInstall({ name: pkg.name, source: pkg.source, displayName: pkg.display_name ?? undefined, mode: 'uninstall' });
                              setLastInstallTarget({ name: pkg.name, mode: 'uninstall' });
                            }}
                          />
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
                      </Suspense>
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
            // Modal stays open so the user can click 'Launch' or 'Done'
          }}
        />
      )}
      <MobileNav activeTab={activeTab} setActiveTab={handleTabChange} />
      <ErrorModal />
    </div>
  );
}

export default App;
