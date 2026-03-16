import { create } from 'zustand';
import { commands } from '../services/bindings';
import { unwrap } from '../utils/specta';
import { LazyStore } from '@tauri-apps/plugin-store';
import { getErrorService } from '../context/getErrorService';
import { friendlyError } from '../utils/friendlyError';
import { debugError, debugInfo, debugWarn } from '../utils/debugLog';
import {
    expandRatingLookupIds,
    getKnownAppIdsForPackage,
    getPackageListKey,
    normalizeCanonicalId
} from '../utils/packageKey';
import type { Package } from '../services/bindings';

const FAVORITES_STORE_PATH = 'favorites.json';
const FAVORITES_STORAGE_KEY = 'favorites';
const REGISTRY_MAX_SIZE = 5000;
const favoritesStore = typeof window !== 'undefined' ? new LazyStore(FAVORITES_STORE_PATH) : null;
let pendingUpdatesRequest: Promise<void> | null = null;


const isDecodeError = (raw: string): boolean =>
    /error decoding response body|decoding response body|invalid json|unexpected end of|expected value/i.test(raw);

const normalizeIdList = (ids: string[]): string[] =>
    Array.from(new Set(ids.map((id) => normalizeCanonicalId(String(id ?? ''))).filter(Boolean)));
/** Evict oldest entries not in protected set until size <= REGISTRY_MAX_SIZE. */
function evictRegistry(
    registry: Record<string, Package>,
    protectedIds: Set<string>
): Record<string, Package> {
    const keys = Object.keys(registry);
    if (keys.length <= REGISTRY_MAX_SIZE) return registry;

    const evictable = keys
        .filter((id) => !protectedIds.has(id));

    // Simple oldest-first eviction (based on key insertion order in JS)
    const overage = keys.length - REGISTRY_MAX_SIZE;
    const toRemove = evictable.slice(0, Math.min(overage, evictable.length));

    if (toRemove.length === 0) return registry;

    const next = { ...registry };
    for (const id of toRemove) delete next[id];
    debugInfo(`[REGISTRY] Evicted ${toRemove.length} packages. New size: ${Object.keys(next).length}`);
    return next;
}

export interface InfraStats {
    builders: number;
    users: number;
}

export interface UpdateProgress {
    phase: 'start' | 'refresh' | 'upgrade' | 'aur' | 'aur_build' | 'aur_install' | 'complete' | 'error';
    progress: number;
    message: string;
}

export interface AppState {
    // --- Global Package Registry (Single Source of Truth) ---
    packageRegistry: Record<string, Package>;
    /** Set when install/uninstall modal opens; cleared after install-complete success. Used to refresh registry installed state. */
    lastInstallTarget: { name: string; mode: 'install' | 'uninstall' } | null;
    activePackageId: string | null;
    // --- Dynamic Views (IDs only) ---
    trendingIds: string[];
    essentialsIds: string[];
    categoryIds: Record<string, string[]>;
    searchResultIds: string[];
    favorites: string[];
    /** True once initial metadata (essentials/trending) has loaded. */
    metadataInitialized: boolean;
    /** Prevents concurrent bulk registry syncs. */
    syncingRegistryBulk: boolean;

    // --- Actions ---
    upsertPackages: (pkgs: Package[]) => void;
    /** Atomic update: Upserts packages AND sets trending IDs in one go to prevent race conditions. */
    /** Atomic update: Upserts packages AND sets trending IDs in one go to prevent race conditions. */
    hydrateSection: (section: 'trending' | 'essentials', pkgs: Package[]) => void;
    /** @deprecated Use hydrateSection */
    setTrendingPackages: (pkgs: Package[]) => void;
    /** @deprecated Use hydrateSection */
    setEssentialsPackages: (pkgs: Package[]) => void;

    /** Atomic update from backend registry: fetches full details for these specific IDs. */
    syncRegistry: (ids: string[]) => Promise<void>;
    /** Full reset: usually after major state changes or cache clears. */
    syncRegistryBulk: () => Promise<void>;

    setLastInstallTarget: (v: { name: string; mode: 'install' | 'uninstall' } | null) => void;
    /** Update installed flag for all registry entries matching name/canonical_id/app_id (e.g. after install/uninstall). */
    updatePackageInstalledState: (nameOrId: string, installed: boolean) => void;
    setActivePackageId: (id: string | null) => void;
    /** @deprecated Use setTrendingPackages for atomic updates */
    setTrendingIds: (ids: string[]) => void;
    /** @deprecated Use setEssentialsPackages for atomic updates */
    setEssentialsIds: (ids: string[]) => void;
    setCategoryIds: (category: string, ids: string[]) => void;
    appendCategoryIds: (category: string, ids: string[]) => void;
    setSearchResultIds: (ids: string[]) => void;
    setFavorites: (ids: string[]) => void;
    toggleFavorite: (idOrName: string) => Promise<void>;
    hydrateFavorites: () => Promise<void>;
    /** Set when favorite persistence fails (show toast then clear). */
    favoriteError: string | null;
    clearFavoriteError: () => void;

    infraStats: InfraStats | null;
    loadingTrending: boolean;
    loadingStats: boolean;
    telemetryEnabled: boolean;
    error: string | null;

    // Update System State
    isUpdating: boolean;
    updateProgress: number;
    updateStatus: string;
    updatePhase: string;
    updateLogs: string[];
    rebootRequired: boolean;
    pacnewWarnings: string[];

    // Background Update Checking (NEW)
    pendingUpdates: { repo: number; aur: number; flatpak: number; total: number };
    lastUpdateCheck: number; // timestamp
    updateNotificationsEnabled: boolean;
    setUpdateNotificationsEnabled: (enabled: boolean) => void;
    setPendingUpdates: (updates: { repo: number; aur: number; flatpak: number }) => void;
    /** Pass includeAur/includeFlatpak to match Settings → Sources (default true when omitted). */
    refreshPendingUpdates: (includeAur?: boolean, includeFlatpak?: boolean) => Promise<void>;

    /** When true, install modal shows detailed transaction logs by default (Glass Cockpit) */
    verboseLogsEnabled: boolean;
    setVerboseLogsEnabled: (enabled: boolean) => void;

    /** When true, user can enter password once in MonARCH (one dialog per session). Less secure than system prompt each time. */
    reducePasswordPrompts: boolean;
    setReducePasswordPrompts: (enabled: boolean) => void;
    advancedMode: boolean;
    setAdvancedMode: (enabled: boolean) => Promise<void>;

    /** Builder Settings */
    cleanBuild: boolean;
    setCleanBuild: (enabled: boolean) => void;
    parallelDownloads: number;
    setParallelDownloads: (count: number) => void;

    /** Source Settings */
    isAurEnabled: boolean;
    setAurEnabled: (enabled: boolean) => Promise<void>;
    isFlatpakEnabled: boolean;
    setFlatpakEnabled: (enabled: boolean) => Promise<void>;
    oneClickEnabled: boolean;
    setOneClickEnabled: (enabled: boolean) => Promise<void>;
    isChaoticEnabled: boolean;
    setChaoticEnabled: (enabled: boolean) => Promise<void>;

    onboardingCompleted: boolean;
    /** Sets onboarding completed; returns true if it was already completed (so caller can skip store_installed). */
    setOnboardingCompleted: (completed: boolean) => Promise<boolean>;

    /** Theme Settings */
    themeMode: 'system' | 'light' | 'dark';
    setThemeMode: (mode: 'system' | 'light' | 'dark') => Promise<void>;
    accentColor: string;
    setAccentColor: (color: string) => Promise<void>;

    /** System Flags */
    declinedSystemSetup: boolean;
    setDeclinedSystemSetup: (declined: boolean) => Promise<void>;

    isSidebarExpanded: boolean;
    setSidebarExpanded: (expanded: boolean) => Promise<void>;

    alphaNoticeDismissed: boolean;
    setAlphaNoticeDismissed: (dismissed: boolean) => Promise<void>;

    searchHistory: string[];
    setSearchHistory: (history: string[]) => Promise<void>;

    readNewsIds: string[];
    setReadNewsIds: (ids: string[]) => Promise<void>;

    activeTab: string;
    setActiveTab: (tab: string) => Promise<void>;

    /** System Initializer: Syncs frontend store with backend StoredConfig */
    initializeSettings: () => Promise<void>;

    fetchTrending: () => Promise<void>;
    fetchInfraStats: () => Promise<void>;
    checkTelemetry: () => Promise<void>;
    setTelemetry: (enabled: boolean) => Promise<void>;

    /** Batch fetch ratings for a list of app IDs and update the store. */
    fetchRatingsForPackages: (packageIds: string[]) => Promise<void>;

    // Update Actions
    setUpdating: (val: boolean) => void;
    setUpdateProgress: (progress: number) => void;
    setUpdateStatus: (msg: string) => void;
    setUpdatePhase: (phase: string) => void;
    addUpdateLog: (log: string) => void;
    clearUpdateLogs: () => void;
    setRebootRequired: (val: boolean) => void;
    setPacnewWarnings: (warnings: string[]) => void;
    checkRebootStatus: () => Promise<void>;
    checkPacnewStatus: () => Promise<void>;
    pendingServiceRestarts: string[];
}

function matchesPackage(pkg: Package, nameOrId: string): boolean {
    const n = String(nameOrId).toLowerCase().trim();
    if (!n) return false;
    const name = (pkg.name ?? '').toLowerCase();
    const canonical = (typeof pkg.canonical_id === 'string' ? pkg.canonical_id : '').toLowerCase();
    const appId = (pkg.app_id ?? '').toLowerCase();
    return name === n || canonical === n || appId === n;
}

function getCanonicalIdOrWarn(pkg: Package, surface: string): string {
    const id = getPackageListKey(pkg);
    if (!id) {
        debugWarn(`[IRON-CORE] Package missing canonical_id from backend surface ${surface}:`, {
            name: pkg.name,
            app_id: pkg.app_id,
            source: pkg.source,
        });
    }
    return id;
}

function applyBackendPackage(existing: Package | undefined, incoming: Package): { pkg: Package; changed: boolean } {
    if (!existing) return { pkg: incoming, changed: true };

    if (JSON.stringify(existing) === JSON.stringify(incoming)) {
        return { pkg: existing, changed: false };
    }

    // Backend remains SSOT for identity/source/install state.
    // But different backend surfaces return different payload richness.
    // Preserve richer universal metadata from an existing backend payload when
    // the incoming payload omits it, so a lightweight list fetch cannot erase
    // details already hydrated from getFullPackageDetails.
    const merged = { ...incoming };

    const incomingSources = incoming.available_sources || [incoming.source];
    const existingSourceStillAvailable = incomingSources.some((src) =>
        src.source_type === existing.source.source_type &&
        src.id === existing.source.id &&
        (src.package_name || '') === (existing.source.package_name || '')
    );

    // Keep the already-published visible source identity stable for the session
    // as long as that source variant still exists in the incoming backend payload.
    if (existingSourceStillAvailable) {
        merged.source = existing.source;
        if (existing.source_summary) {
            merged.source_summary = existing.source_summary;
        }
        if (existing.trust_level) {
            merged.trust_level = existing.trust_level;
        }
        if (existing.security_summary) {
            merged.security_summary = existing.security_summary;
        }
        if (existing.primary_action) {
            merged.primary_action = existing.primary_action;
        }
        if (existing.primary_action_label) {
            merged.primary_action_label = existing.primary_action_label;
        }
    }

    if ((!incoming.icon || incoming.icon.length === 0) && existing.icon) {
        merged.icon = existing.icon;
    }
    if ((!incoming.display_name || incoming.display_name.length === 0) && existing.display_name) {
        merged.display_name = existing.display_name;
    }
    if ((!incoming.app_id || incoming.app_id.length === 0) && existing.app_id) {
        merged.app_id = existing.app_id;
    }
    if ((!incoming.description || incoming.description.length === 0) && existing.description) {
        merged.description = existing.description;
    }
    if (
        (!incoming.long_description || incoming.long_description.length === 0) &&
        existing.long_description
    ) {
        merged.long_description = existing.long_description;
    }
    if (
        (!incoming.screenshots || incoming.screenshots.length === 0) &&
        existing.screenshots &&
        existing.screenshots.length > 0
    ) {
        merged.screenshots = existing.screenshots;
    }
    if ((!incoming.maintainer || incoming.maintainer.length === 0) && existing.maintainer) {
        merged.maintainer = existing.maintainer;
    }
    if ((!incoming.license || incoming.license.length === 0) && existing.license) {
        merged.license = existing.license;
    }

    if (!incoming.rating && existing.rating) {
        merged.rating = existing.rating;
    }

    // Preserve installed truth from a richer installed-catalog payload when a lighter
    // backend surface (e.g. trending/search) omits or misclassifies installed state.
    merged.installed = Boolean(incoming.installed || existing.installed);
    if ((!incoming.installed_sources || incoming.installed_sources.length === 0) && existing.installed_sources) {
        merged.installed_sources = existing.installed_sources;
    }
    if ((!incoming.launch_target || incoming.launch_target.length === 0) && existing.launch_target) {
        merged.launch_target = existing.launch_target;
    }

    return { pkg: merged, changed: true };
}

export const useAppStore = create<AppState>()((set, get) => ({
    packageRegistry: {},
    lastInstallTarget: null,
    activePackageId: null,
    trendingIds: [],
    essentialsIds: [],
    categoryIds: {},
    searchResultIds: [],
    favorites: [],
    isAurEnabled: false,
    isFlatpakEnabled: true,
    isChaoticEnabled: false,
    oneClickEnabled: false,
    advancedMode: false,
    metadataInitialized: false,
    syncingRegistryBulk: false,

    upsertPackages: (pkgs: Package[]) => {
        if (!pkgs || !pkgs.length) return;
        set((state) => {
            const nextRegistry = { ...state.packageRegistry };
            const idsBeingUpserted: string[] = [];
            let anyChanged = false;

            for (const p of pkgs) {
                if (!p) continue;
                const id = getCanonicalIdOrWarn(p, 'upsertPackages');
                if (!id) continue;
                idsBeingUpserted.push(id);

                const { pkg: merged, changed } = applyBackendPackage(nextRegistry[id], p);
                if (changed) {
                    nextRegistry[id] = merged;
                    anyChanged = true;
                }
            }

            if (!anyChanged) return state; // SKIP RENDER if data is identical

            const protectedIds = new Set<string>([
                ...state.trendingIds,
                ...state.essentialsIds,
                ...state.searchResultIds,
                ...(state.activePackageId ? [state.activePackageId] : []),
                ...Object.keys(state.categoryIds).flatMap((c) => state.categoryIds[c] ?? []),
                ...state.favorites,
                ...idsBeingUpserted
            ]);

            return { packageRegistry: evictRegistry(nextRegistry, protectedIds) };
        });
    },

    hydrateSection: (section: 'trending' | 'essentials', pkgs: Package[]) => {
        if (!pkgs) {
            // Never leave home sections in permanent skeleton mode.
            set({ metadataInitialized: true });
            return;
        }

        // 1. Upsert packages first so they exist in registry
        set((state) => {
            const nextRegistry = { ...state.packageRegistry };
            const newIds: string[] = [];
            let anyChanged = false;

            for (const p of pkgs) {
                if (!p) continue;
                const id = getCanonicalIdOrWarn(p, `hydrateSection:${section}`);
                if (id) {
                    // Prevent duplicates in the IDs array
                    if (!newIds.includes(id)) {
                        newIds.push(id);
                    }
                    const { pkg: merged, changed } = applyBackendPackage(nextRegistry[id], p);
                    if (changed) {
                        nextRegistry[id] = merged;
                        anyChanged = true;
                    }
                }
            }

            // Map section to ID key
            const sectionKey = section === 'trending' ? 'trendingIds' : 'essentialsIds';

            // Check if IDs list actually changed
            const idsChanged = JSON.stringify(newIds) !== JSON.stringify(state[sectionKey]);

            if (!anyChanged && !idsChanged && state.metadataInitialized) return state;

            // Protect new IDs + existing vital IDs
            const protectedIds = new Set<string>([
                ...newIds,
                ...(section === 'trending' ? state.essentialsIds : state.trendingIds),
                ...state.searchResultIds,
                ...(state.activePackageId ? [state.activePackageId] : []),
                ...Object.keys(state.categoryIds).flatMap((c) => state.categoryIds[c] ?? []),
                ...state.favorites
            ]);

            return {
                packageRegistry: evictRegistry(nextRegistry, protectedIds),
                [sectionKey]: newIds,
                // Mark initialized even when section is empty to prevent infinite skeletons.
                metadataInitialized: true,
                loadingTrending: section === 'trending' ? false : state.loadingTrending,
                error: section === 'trending' ? null : state.error
            };
        });

        // 2. Fire-and-forget batch rating fetch.
        // Collect EVERY possible lookup key: app_id takes priority (ODRS canonical),
        // but fall back to pkg name so packages with no app_id still get rated.
        const ratingLookupIds = Array.from(
            new Set(
                pkgs.flatMap(p => [
                    p.app_id,
                    // Also include name as fallback for ODRS matching
                    p.name,
                ].filter((id): id is string => !!id && id.length > 0))
            )
        );
        if (ratingLookupIds.length > 0) {
            get().fetchRatingsForPackages(ratingLookupIds);
        }
    },

    fetchRatingsForPackages: async (packageIds: string[]) => {
        if (!packageIds || packageIds.length === 0) return;

        // Filter valid IDs to avoid empty calls
        const validIds = expandRatingLookupIds(
            packageIds.filter(id => !!id && id.length > 0)
        );
        if (validIds.length === 0) return;

        try {
            // 1. Fetch ratings from backend (ODRS)
            const res = await commands.getAppRatingsBatch(validIds);
            const ratingsMap = unwrap(res);

            if (Object.keys(ratingsMap).length === 0) return;

            // 2. Build a fast lookup index from the ratings map
            // ODRS keys are typically app_id (e.g. "org.mozilla.Firefox") or pkg name.
            // We need case-insensitive matching in both directions.
            const ratingsByNormalizedKey = new Map<string, typeof ratingsMap[string]>();
            for (const [appId, rating] of Object.entries(ratingsMap)) {
                ratingsByNormalizedKey.set(appId.toLowerCase(), rating);
            }

            // 3. Safely merge into EXISTING registry packages
            set((state) => {
                const nextRegistry = { ...state.packageRegistry };
                let anyChanged = false;

                for (const key in nextRegistry) {
                    const pkg = nextRegistry[key];
                    if (!pkg) continue;

                    // Match against app_id (canonical) OR name (fallback)
                    const lookupKeys = new Set<string>();
                    if (pkg.app_id) lookupKeys.add(pkg.app_id.toLowerCase());
                    if (pkg.name) lookupKeys.add(pkg.name.toLowerCase());
                    for (const knownAppId of getKnownAppIdsForPackage(pkg)) {
                        lookupKeys.add(knownAppId.toLowerCase());
                    }

                    let rating = null;
                    for (const lookupKey of lookupKeys) {
                        const candidate = ratingsByNormalizedKey.get(lookupKey);
                        if (candidate) {
                            rating = candidate;
                            break;
                        }
                    }

                    if (rating && JSON.stringify(pkg.rating) !== JSON.stringify(rating)) {
                        nextRegistry[key] = { ...pkg, rating };
                        anyChanged = true;
                    }
                }

                return anyChanged ? { packageRegistry: nextRegistry } : state;
            });

        } catch (e) {
            debugWarn('[MonARCH] Batch rating fetch failed (safe ignore):', e);
        }
    },

    setTrendingPackages: (pkgs: Package[]) => get().hydrateSection('trending', pkgs),
    setEssentialsPackages: (pkgs: Package[]) => get().hydrateSection('essentials', pkgs),

    syncRegistry: async (ids: string[]) => {
        if (!ids.length) return;
        // Protection against Bridge Flooding: if many IDs, do a Bulk Sync instead
        if (ids.length > 100) {
            debugWarn(`[REGISTRY] syncRegistry called with ${ids.length} IDs. Redirecting to Bulk Sync for performance.`);
            get().syncRegistryBulk();
            return;
        }
        try {
            // Fetch the fully joined metadata from the backend Registry by Canonical IDs
            const canonicalPkgs = unwrap(await commands.getPackagesByCanonicalIds(ids));
            if ((canonicalPkgs ?? []).length > 0) get().upsertPackages(canonicalPkgs ?? []);
        } catch (e) {
            debugError('[REGISTRY] Throttled sync failed:', e);
        }
    },

    syncRegistryBulk: async () => {
        if (get().syncingRegistryBulk) return;
        set({ syncingRegistryBulk: true });

        const state = get();
        // Identify what's actually visible or "hot" to minimize IPC traffic
        const relevantIds = new Set<string>([
            ...state.essentialsIds,
            ...state.trendingIds,
            ...state.searchResultIds,
            ...(state.activePackageId ? [state.activePackageId] : [])
        ]);

        // Also add current category results
        Object.values(state.categoryIds).forEach((ids) => {
            ids.forEach((id) => relevantIds.add(id));
        });

        const idList = Array.from(relevantIds);
        if (idList.length === 0) {
            set({ syncingRegistryBulk: false });
            return;
        }

        debugInfo(`[REGISTRY] Performing Bulk Sync for ${idList.length} relevant objects...`);

        // Fetch in chunks of 100 to avoid IPC limits or DB locks
        const CHUNK_SIZE = 100;
        try {
            for (let i = 0; i < idList.length; i += CHUNK_SIZE) {
                const chunk = idList.slice(i, i + CHUNK_SIZE);
                try {
                    // Fetch the fully joined metadata from the backend Registry by Canonical IDs
                    const canonicalPkgs = unwrap(await commands.getPackagesByCanonicalIds(chunk));
                    if ((canonicalPkgs ?? []).length > 0) get().upsertPackages(canonicalPkgs ?? []);
                } catch (p) {
                    debugError(`[REGISTRY] Bulk sync chunk ${i / CHUNK_SIZE} failed:`, p);
                }
            }
        } finally {
            set({ syncingRegistryBulk: false });
            debugInfo(`[REGISTRY] Bulk sync completed.`);
        }
    },

    setLastInstallTarget: (v: { name: string; mode: 'install' | 'uninstall' } | null) => set({ lastInstallTarget: v }),

    updatePackageInstalledState: (nameOrId: string, installed: boolean) => {
        set((state) => {
            const next = { ...state.packageRegistry };
            let changed = false;
            for (const id of Object.keys(next)) {
                const pkg = next[id];
                if (pkg && matchesPackage(pkg, nameOrId)) {
                    next[id] = { ...pkg, installed };
                    changed = true;
                }
            }
            return changed ? { packageRegistry: next } : state;
        });
    },

    setActivePackageId: (id: string | null) => set({ activePackageId: id ? normalizeCanonicalId(id) : null }),
    setTrendingIds: (ids: string[]) => set({ trendingIds: normalizeIdList(ids) }),
    setEssentialsIds: (ids: string[]) => set({ essentialsIds: normalizeIdList(ids) }),
    setCategoryIds: (category: string, ids: string[]) =>
        set((state) => ({ categoryIds: { ...state.categoryIds, [category]: normalizeIdList(ids) } })),
    appendCategoryIds: (category: string, ids: string[]) =>
        set((state) => {
            const current = state.categoryIds[category] ?? [];
            return { categoryIds: { ...state.categoryIds, [category]: normalizeIdList([...current, ...ids]) } };
        }),
    setSearchResultIds: (ids: string[]) => set({ searchResultIds: normalizeIdList(ids) }),
    setFavorites: (ids: string[]) => set({ favorites: normalizeIdList(ids) }),
    favoriteError: null,
    clearFavoriteError: () => set({ favoriteError: null }),

    toggleFavorite: async (idOrName: string) => {
        const norm = normalizeCanonicalId(idOrName);
        if (!norm) return;
        const current = get().favorites;
        const newFavorites = current.includes(norm)
            ? current.filter((f) => f.toLowerCase() !== norm)
            : [...current.filter((f) => f.toLowerCase() !== norm), norm];
        set({ favorites: newFavorites, favoriteError: null });
        if (!favoritesStore) return;
        try {
            await favoritesStore.set(FAVORITES_STORAGE_KEY, newFavorites);
            await favoritesStore.save();
        } catch (e) {
            const message = e instanceof Error ? e.message : 'Failed to save favorites';
            set({ favorites: current, favoriteError: 'Persistence Error: ' + message });
        }
    },
    hydrateFavorites: async () => {
        if (!favoritesStore) return;
        try {
            const saved = await favoritesStore.get<string[]>(FAVORITES_STORAGE_KEY);
            const raw = saved ?? [];
            const migrated = raw
                .map((s) => (typeof s === 'string' ? normalizeCanonicalId(s) : ''))
                .filter(Boolean);
            const unique = Array.from(new Set(migrated));
            set({ favorites: unique });
        } catch (e) {
            debugError('Failed to load favorites from store', e);
        }
    },

    infraStats: null,
    loadingTrending: false,
    loadingStats: false,
    telemetryEnabled: false,
    error: null,

    // Update System Initial State
    isUpdating: false,
    updateProgress: 0,
    updateStatus: '',
    updatePhase: '',
    updateLogs: [],
    rebootRequired: false,
    pacnewWarnings: [],
    pendingServiceRestarts: [],

    // Background Update Checking (NEW)
    pendingUpdates: { repo: 0, aur: 0, flatpak: 0, total: 0 },
    lastUpdateCheck: 0,
    updateNotificationsEnabled: true,
    setUpdateNotificationsEnabled: async (enabled: boolean) => {
        try {
            unwrap(await commands.setNotificationsEnabled(enabled));
            set({ updateNotificationsEnabled: enabled });
        } catch (e) {
            debugError('[MonARCH] Failed to set notifications:', e);
        }
    },
    setPendingUpdates: (updates: { repo: number; aur: number; flatpak: number }) => {
        set({
            pendingUpdates: { ...updates, total: updates.repo + updates.aur + updates.flatpak },
            lastUpdateCheck: Date.now()
        });
    },
    refreshPendingUpdates: async (includeAur?: boolean, includeFlatpak?: boolean) => {
        if (pendingUpdatesRequest) {
            return pendingUpdatesRequest;
        }
        pendingUpdatesRequest = (async () => {
            try {
                const updates = unwrap(await commands.checkUpdates(includeAur ?? true, includeFlatpak ?? true));
                const breakdown = { repo: 0, aur: 0, flatpak: 0 };
                for (const u of updates) {
                    // source is now PackageSource object
                    const srcType = u.source.source_type.toLowerCase();
                    if (srcType === 'repo') breakdown.repo++;
                    else if (srcType === 'aur') breakdown.aur++;
                    else if (srcType === 'flatpak') breakdown.flatpak++;
                }
                set({
                    pendingUpdates: { ...breakdown, total: updates.length },
                    lastUpdateCheck: Date.now()
                });

                // check system status too
                const reboot = unwrap(await commands.checkRebootRequired());
                const pacnew = unwrap(await commands.getPacnewWarnings());
                let svcs: string[] = [];
                try {
                    svcs = unwrap(await commands.checkServicesRestart());
                } catch {
                    // Command may not be in capabilities; keep existing or empty
                }
                set({ rebootRequired: reboot, pacnewWarnings: pacnew, pendingServiceRestarts: svcs });
            } catch (e) {
                debugError('[MonARCH] Failed to refresh pending updates:', e);
                // Don't report as error - this is a background check
            } finally {
                pendingUpdatesRequest = null;
            }
        })();
        return pendingUpdatesRequest;
    },

    checkRebootStatus: async () => {
        try {
            const required = unwrap(await commands.checkRebootRequired());
            set({ rebootRequired: required });
        } catch (e) {
            debugError('[MonARCH] Failed to check reboot status:', e);
        }
    },

    checkPacnewStatus: async () => {
        try {
            const warnings = unwrap(await commands.getPacnewWarnings());
            set({ pacnewWarnings: warnings });
        } catch (e) {
            debugError('[MonARCH] Failed to check pacnew status:', e);
        }
    },

    verboseLogsEnabled: false,
    setVerboseLogsEnabled: async (enabled: boolean) => {
        try {
            unwrap(await commands.setVerboseLogsEnabled(enabled));
            set({ verboseLogsEnabled: enabled });
        } catch (e) {
            debugError('[MonARCH] Failed to set verbose logs:', e);
        }
    },
    // One-click/silent-guard prompt behavior; kept in sync with oneClickEnabled.
    reducePasswordPrompts: false,
    setReducePasswordPrompts: async (enabled: boolean) => {
        try {
            unwrap(await commands.setOneClickEnabled(enabled));
            set({ reducePasswordPrompts: enabled, oneClickEnabled: enabled });
        } catch (e) {
            debugError('[MonARCH] Failed to set one-click mode:', e);
        }
    },
    setAdvancedMode: async (enabled: boolean) => {
        try {
            unwrap(await commands.setAdvancedMode(enabled));
            set({ advancedMode: enabled });
        } catch (e) {
            debugError('[MonARCH] Failed to set advanced mode:', e);
        }
    },
    cleanBuild: false,
    setCleanBuild: async (enabled: boolean) => {
        try {
            unwrap(await commands.setCleanBuildEnabled(enabled));
            set({ cleanBuild: enabled });
        } catch (e) {
            debugError('[MonARCH] Failed to set clean build:', e);
        }
    },
    parallelDownloads: 5,
    setParallelDownloads: async (count: number) => {
        try {
            unwrap(await commands.setParallelDownloads(count));
            set({ parallelDownloads: count });
        } catch (e) {
            debugError('[MonARCH] Failed to set parallel downloads:', e);
        }
    },
    setAurEnabled: async (enabled: boolean) => {
        try {
            unwrap(await commands.setAurEnabled(enabled));
            if (enabled) unwrap(await commands.toggleRepo('aur', true, null));
            set({ isAurEnabled: enabled });
        } catch (e) {
            debugError('[MonARCH] Failed to set AUR enabled:', e);
        }
    },
    setFlatpakEnabled: async (enabled: boolean) => {
        try {
            unwrap(await commands.setFlatpakEnabled(enabled));
            set({ isFlatpakEnabled: enabled });
        } catch (e) {
            debugError('[MonARCH] Failed to set Flatpak enabled:', e);
        }
    },
    setOneClickEnabled: async (enabled: boolean) => {
        try {
            unwrap(await commands.setOneClickEnabled(enabled));
            set({ oneClickEnabled: enabled, reducePasswordPrompts: enabled });
        } catch (e) {
            debugError('[MonARCH] Failed to set one-click enabled:', e);
        }
    },
    setChaoticEnabled: async (enabled: boolean) => {
        try {
            // Chaotic is managed via toggleRepo in backend
            unwrap(await commands.toggleRepo('chaotic-aur', enabled, null));
            set({ isChaoticEnabled: enabled });
        } catch (e) {
            debugError('[MonARCH] Failed to set Chaotic-AUR enabled:', e);
        }
    },
    onboardingCompleted: false,
    setOnboardingCompleted: async (completed: boolean) => {
        try {
            const wasAlreadyCompleted = unwrap(await commands.setOnboardingCompleted(completed));
            set({ onboardingCompleted: completed });
            return wasAlreadyCompleted;
        } catch (e) {
            debugError('[MonARCH] Failed to set onboarding completed:', e);
            return true; // assume already completed so we don't double-send
        }
    },
    themeMode: 'system',
    setThemeMode: async (mode: 'system' | 'light' | 'dark') => {
        const previous = get().themeMode;
        set({ themeMode: mode }); // Optimistic update
        try {
            unwrap(await commands.setThemeMode(mode));
        } catch (e) {
            debugError('[MonARCH] Failed to set theme mode:', e);
            set({ themeMode: previous }); // Revert on failure
        }
    },
    accentColor: '#3b82f6',
    setAccentColor: async (color: string) => {
        const previous = get().accentColor;
        set({ accentColor: color }); // Optimistic update
        try {
            unwrap(await commands.setAccentColor(color));
        } catch (e) {
            debugError('[MonARCH] Failed to set accent color:', e);
            set({ accentColor: previous }); // Revert on failure
        }
    },
    declinedSystemSetup: false,
    setDeclinedSystemSetup: async (declined: boolean) => {
        try {
            unwrap(await commands.setDeclinedSystemSetup(declined));
            set({ declinedSystemSetup: declined });
        } catch (e) {
            debugError('[MonARCH] Failed to set declined system setup:', e);
        }
    },
    isSidebarExpanded: true,
    setSidebarExpanded: async (expanded: boolean) => {
        const previous = get().isSidebarExpanded;
        set({ isSidebarExpanded: expanded }); // Optimistic update
        try {
            unwrap(await commands.setSidebarExpanded(expanded));
        } catch (e) {
            debugError('[MonARCH] Failed to set sidebar expanded:', e);
            set({ isSidebarExpanded: previous }); // Revert on failure
        }
    },
    alphaNoticeDismissed: false,
    setAlphaNoticeDismissed: async (dismissed: boolean) => {
        try {
            unwrap(await commands.setAlphaNoticeDismissed(dismissed));
            set({ alphaNoticeDismissed: dismissed });
        } catch (e) {
            debugError('[MonARCH] Failed to set alpha notice dismissed:', e);
        }
    },
    searchHistory: [],
    setSearchHistory: async (history: string[]) => {
        try {
            unwrap(await commands.setSearchHistory(history));
            set({ searchHistory: history });
        } catch (e) {
            debugError('[MonARCH] Failed to set search history:', e);
        }
    },
    readNewsIds: [],
    setReadNewsIds: async (ids: string[]) => {
        try {
            unwrap(await commands.setReadNewsIds(ids));
            set({ readNewsIds: ids });
        } catch (e) {
            debugError('[MonARCH] Failed to set read news ids:', e);
        }
    },
    activeTab: 'explore',
    setActiveTab: async (tab: string) => {
        try {
            unwrap(await commands.setActiveTab(tab));
            set({ activeTab: tab });
        } catch (e) {
            debugError('[MonARCH] Failed to set active tab:', e);
        }
    },
    initializeSettings: async () => {
        try {
            const telemetry = unwrap(await commands.isTelemetryEnabled());
            const notifications = unwrap(await commands.isNotificationsEnabled());
            const verbose = unwrap(await commands.isVerboseLogsEnabled());
            const clean = unwrap(await commands.isCleanBuildEnabled());
            const parallel = unwrap(await commands.getParallelDownloads());
            const advanced = unwrap(await commands.isAdvancedMode());
            const aur = unwrap(await commands.isAurEnabled());
            const flatpak = unwrap(await commands.isFlatpakEnabled());
            const oneClick = unwrap(await commands.isOneClickEnabled());
            const onboarding = unwrap(await commands.isOnboardingCompleted());
            const theme = unwrap(await commands.getThemeMode()) as 'system' | 'light' | 'dark';
            const accent = unwrap(await commands.getAccentColor());
            const declined = unwrap(await commands.isDeclinedSystemSetup());
            const sidebar = unwrap(await commands.isSidebarExpanded());
            const alpha = unwrap(await commands.isAlphaNoticeDismissed());
            const searchHistory = unwrap(await commands.getSearchHistory());
            const readNews = unwrap(await commands.getReadNewsIds());
            const tab = unwrap(await commands.getActiveTab());

            const repoStates = unwrap(await commands.getRepoStates());
            const chaoticRepo = repoStates.find(
                (r) => r.name.toLowerCase() === 'chaotic-aur'
            );
            // Preserve current toggle when backend repo list has no explicit chaotic row.
            const chaotic = chaoticRepo ? chaoticRepo.enabled : get().isChaoticEnabled;

            set({
                telemetryEnabled: telemetry,
                updateNotificationsEnabled: notifications,
                verboseLogsEnabled: verbose,
                cleanBuild: clean,
                parallelDownloads: parallel,
                reducePasswordPrompts: oneClick,
                advancedMode: advanced,
                isAurEnabled: aur,
                isFlatpakEnabled: flatpak,
                isChaoticEnabled: chaotic,
                oneClickEnabled: oneClick,
                onboardingCompleted: onboarding,
                themeMode: theme,
                accentColor: accent,
                declinedSystemSetup: declined,
                isSidebarExpanded: sidebar,
                alphaNoticeDismissed: alpha,
                searchHistory: searchHistory,
                readNewsIds: readNews,
                activeTab: tab,
            });
            debugInfo('[MonARCH] Settings initialized from backend.');
        } catch (e) {
            debugError('[MonARCH] Failed to initialize settings:', e);
        }
    },
    fetchTrending: async () => {
        set({ loadingTrending: true, error: null });
        try {
            const { isFlatpakEnabled, isAurEnabled, isChaoticEnabled } = get();
            const trending = unwrap(await commands.getTrending({
                flatpak_enabled: isFlatpakEnabled,
                aur_enabled: isAurEnabled,
                chaotic_enabled: isChaoticEnabled,
                for_installed_lookup: false
            }));
            // Atomic update:
            get().setTrendingPackages(trending);
            // setTrendingIds is handled by setTrendingPackages
        } catch (e) {
            const raw = e instanceof Error ? (e as Error).message : String(e);
            debugError('[MonARCH] invoke failed: get_trending', raw);
            if (isDecodeError(raw)) {
                set({ loadingTrending: false, trendingIds: [], error: null });
            } else {
                getErrorService()?.reportError(e as Error | string);
                set({ loadingTrending: false, error: friendlyError(raw).description });
            }
        }
    },
    fetchInfraStats: async () => {
        set({ loadingStats: true });
        try {
            const stats = unwrap(await commands.getInfraStats());
            set({ infraStats: stats, loadingStats: false });
        } catch (e) {
            const raw = e instanceof Error ? (e as Error).message : String(e);
            debugError('[MonARCH] invoke failed: get_infra_stats', raw);
            if (isDecodeError(raw)) {
                set({ infraStats: null, loadingStats: false });
            } else {
                getErrorService()?.reportError(e as Error | string);
                set({ loadingStats: false });
            }
        }
    },
    checkTelemetry: async () => {
        try {
            const enabled = unwrap(await commands.isTelemetryEnabled());
            set({ telemetryEnabled: enabled });
        } catch (e) {
            const raw = e instanceof Error ? (e as Error).message : String(e);
            debugError('[MonARCH] invoke failed: is_telemetry_enabled', raw);
            if (isDecodeError(raw)) {
                set({ telemetryEnabled: false });
            } else {
                getErrorService()?.reportError(e as Error | string);
            }
        }
    },
    setTelemetry: async (enabled: boolean) => {
        const previousState = useAppStore.getState().telemetryEnabled;
        set({ telemetryEnabled: enabled });

        try {
            unwrap(await commands.setTelemetryEnabled(enabled));
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
            set({ telemetryEnabled: previousState });
            throw e;
        }
    },

    setUpdating: (val) => set({ isUpdating: val }),
    setUpdateProgress: (progress) => set({ updateProgress: progress }),
    setUpdateStatus: (msg) => set({ updateStatus: msg }),
    setUpdatePhase: (phase) => set({ updatePhase: phase }),
    addUpdateLog: (log) => set((state) => ({
        updateLogs: [...state.updateLogs.slice(-499), log]
    })),
    clearUpdateLogs: () => set({ updateLogs: [] }),
    setRebootRequired: (val) => set({ rebootRequired: val }),
    setPacnewWarnings: (warnings) => set({ pacnewWarnings: warnings }),
}));
