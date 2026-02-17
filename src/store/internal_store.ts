import { create } from 'zustand';
import { commands } from '../services/bindings';
import { unwrap } from '../utils/specta';
import { LazyStore } from '@tauri-apps/plugin-store';
import { getErrorService } from '../context/getErrorService';
import { friendlyError } from '../utils/friendlyError';
import { getPackageListKey } from '../utils/packageKey';
import { toPackageSource } from '../utils/repoHelper';
import type { Package } from '../services/bindings';
import type { PackageSource } from '../services/bindings';

const FAVORITES_STORE_PATH = 'favorites.json';
const FAVORITES_STORAGE_KEY = 'favorites';
const REGISTRY_MAX_SIZE = 5000;
const favoritesStore = typeof window !== 'undefined' ? new LazyStore(FAVORITES_STORE_PATH) : null;


const isDecodeError = (raw: string): boolean =>
    /error decoding response body|decoding response body|invalid json|unexpected end of|expected value/i.test(raw);
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
    console.debug(`[REGISTRY] Evicted ${toRemove.length} packages. New size: ${Object.keys(next).length}`);
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
    setOnboardingCompleted: (completed: boolean) => Promise<void>;

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

// --- Helpers ---
function deepMergePackage(existing: Package | undefined, incoming: Package): { pkg: Package; changed: boolean } {
    if (!existing) return { pkg: incoming, changed: true };

    // Identity Check: If incoming and existing are identical, skip.
    if (JSON.stringify(existing) === JSON.stringify(incoming)) {
        return { pkg: existing, changed: false };
    }

    const merged: Package = {
        // Core Identity
        name: incoming.name,
        source: incoming.source, // allow source to shift (repo -> flatpak -> etc)
        version: incoming.version,
        installed: incoming.installed ?? existing.installed,
        installed_sources: incoming.installed_sources || existing.installed_sources,

        // Identity Fallbacks
        app_id: incoming.app_id || existing.app_id,
        canonical_id: incoming.canonical_id || existing.canonical_id,
        url: incoming.url || existing.url,

        // Display Metadata - Strict Preservation of "Golden Data"
        display_name: (() => {
            const oldDN = existing.display_name;
            const newDN = incoming.display_name;

            // 1. Strict Null/Empty Check
            if (!newDN) return oldDN;
            if (!oldDN) return newDN;

            // 2. GOLDEN RULE: Never overwrite Title Case with Lowercase
            // (e.g. Keep "Calibre" if incoming is "calibre")
            const oldHasUpper = /[A-Z]/.test(oldDN);
            const newHasUpper = /[A-Z]/.test(newDN);

            if (oldHasUpper && !newHasUpper) {
                // Incoming is inferior (lowercase only). Keep existing.
                return oldDN;
            }

            // 3. Length Heuristic: Prefer "Firefox Web Browser" over "Firefox"
            if (newDN.length > oldDN.length && newHasUpper) return newDN;

            // 4. Default: Updates are essentially equal in quality, take new to be safe
            return newDN;
        })(),

        // Strict Deep Merge: Never overwrite valid longer description with shorter/empty
        description: (() => {
            const inc = incoming.description;
            const ext = existing.description;
            if (!inc) return ext || "";
            if (!ext) return inc;
            // Prefer the longer one, as it's likely more descriptive
            return inc.length >= ext.length ? inc : ext;
        })(),

        long_description: (() => {
            const inc = incoming.long_description;
            const ext = existing.long_description;
            if (!inc) return ext || "";
            if (!ext) return inc;
            return inc.length >= ext.length ? inc : ext;
        })(),

        // Icon Priority: HTTP (Remote) > Local
        icon: (() => {
            const newIcon = incoming.icon;
            const oldIcon = existing.icon;

            if (!newIcon) return oldIcon;
            if (!oldIcon) return newIcon;

            const newIsRemote = newIcon.startsWith('http');
            const oldIsRemote = oldIcon.startsWith('http');

            // If we have a remote icon, keep it! Don't let a local path overwrite it.
            if (oldIsRemote && !newIsRemote) return oldIcon;

            return newIcon;
        })(),

        // Collections (Merge unique)
        screenshots: (incoming.screenshots?.length ?? 0) > 0 ? incoming.screenshots : existing.screenshots,
        keywords: (incoming.keywords?.length ?? 0) > 0 ? incoming.keywords : existing.keywords,

        // Metadata
        maintainer: incoming.maintainer || existing.maintainer,
        license: (incoming.license?.length ?? 0) > 0 ? incoming.license : existing.license,

        // Tech
        provides: (incoming.provides?.length ?? 0) > 0 ? incoming.provides : existing.provides,
        depends: (incoming.depends?.length ?? 0) > 0 ? incoming.depends : existing.depends,
        make_depends: (incoming.make_depends?.length ?? 0) > 0 ? incoming.make_depends : existing.make_depends,
        alternatives: (incoming.alternatives?.length ?? 0) > 0 ? incoming.alternatives : existing.alternatives,

        // Metrics
        download_size: incoming.download_size ?? existing.download_size,
        installed_size: incoming.installed_size ?? existing.installed_size,
        last_modified: incoming.last_modified ?? existing.last_modified,
        first_submitted: incoming.first_submitted ?? existing.first_submitted,
        out_of_date: incoming.out_of_date ?? existing.out_of_date,
        num_votes: incoming.num_votes ?? existing.num_votes,

        // Flags
        is_featured: incoming.is_featured ?? existing.is_featured,
        is_optimized: incoming.is_optimized ?? existing.is_optimized,

        // Rating Protection: Don't zero out valid ratings
        rating: (() => {
            // If incoming has valid data, use it
            if (incoming.rating && incoming.rating.total > 0) return incoming.rating;
            // If incoming is empty/null, keep existing
            return existing.rating;
        })(),

        // Sources (Merged)
        available_sources: (() => {
            const seen = new Set<string>();
            const mergedSources: PackageSource[] = [];
            [...(incoming.available_sources || []), ...(existing?.available_sources || [])].forEach(s => {
                const key = `${s.source_type}:${s.id}`;
                if (!seen.has(key)) {
                    seen.add(key);
                    mergedSources.push(s);
                }
            });
            if (mergedSources.length > 0) return mergedSources;
            // Fallback: Use incoming source if available_sources was empty
            if (incoming.source) mergedSources.push(incoming.source);
            return mergedSources.length > 0 ? mergedSources : null;
        })()
    };

    // Optimization: check for equality again after merge to prevent blink-inducing state updates
    const isDifferent = JSON.stringify(merged) !== JSON.stringify(existing);

    return { pkg: isDifferent ? merged : existing, changed: isDifferent };
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
                const id = getPackageListKey(p);
                if (!id) continue;
                idsBeingUpserted.push(id);

                const { pkg: merged, changed } = deepMergePackage(nextRegistry[id], p);
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
        if (!pkgs) return;

        // 1. Upsert packages first so they exist in registry
        set((state) => {
            const nextRegistry = { ...state.packageRegistry };
            const newIds: string[] = [];
            let anyChanged = false;

            for (const p of pkgs) {
                if (!p) continue;
                const id = getPackageListKey(p);
                if (id) {
                    newIds.push(id);
                    const { pkg: merged, changed } = deepMergePackage(nextRegistry[id], p);
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
                metadataInitialized: true,
                loadingTrending: section === 'trending' ? false : state.loadingTrending,
                error: section === 'trending' ? null : state.error
            };
        });

        // 2. Fire-and-forget batch rating fetch (safe mode)
        const appIds = pkgs.map(p => p.app_id).filter(id => !!id) as string[];
        if (appIds.length > 0) {
            get().fetchRatingsForPackages(appIds);
        }
    },

    fetchRatingsForPackages: async (packageIds: string[]) => {
        if (!packageIds || packageIds.length === 0) return;

        // Filter valid IDs to avoid empty calls
        const validIds = packageIds.filter(id => !!id && id.length > 0);
        if (validIds.length === 0) return;

        try {
            // 1. Fetch ratings from backend (ODRS)
            const res = await commands.getAppRatingsBatch(validIds);
            const ratingsMap = unwrap(res);

            // 2. Safely merge into EXISTING registry packages
            // We use functional state update to ensure we have latest registry
            set((state) => {
                const nextRegistry = { ...state.packageRegistry };
                let anyChanged = false;
                let matchCount = 0;

                Object.entries(ratingsMap).forEach(([appId, rating]) => {
                    const searchId = appId.toLowerCase();
                    // Iterate registry.
                    // PERF: This is O(N * M) where N=registry size, M=ratings count.
                    // Registry size is usually <2000. Batch size M is usually <50. 
                    // 100k iters is fine for JS.
                    for (const key in nextRegistry) {
                        const pkg = nextRegistry[key];
                        // Robust match: Check app_id (case-insensitive) OR fallback to name exact match
                        const matchesId = pkg.app_id && pkg.app_id.toLowerCase() === searchId;
                        const matchesName = pkg.name === searchId;

                        if (matchesId || matchesName) {
                            // Only update if rating changed
                            if (JSON.stringify(pkg.rating) !== JSON.stringify(rating)) {
                                nextRegistry[key] = { ...pkg, rating };
                                anyChanged = true;
                                matchCount++;
                            } else {
                                // matched but no change
                                matchCount++;
                            }
                        }
                    }
                });

                if (anyChanged) {
                    // console.debug(`[MonARCH] Applied ratings to ${matchCount} packages.`);
                } else if (matchCount === 0 && Object.keys(ratingsMap).length > 0) {
                    console.warn(`[MonARCH] Fetched ${Object.keys(ratingsMap).length} ratings but matched ZERO in registry! keys=${Object.keys(ratingsMap).join(',')}`);
                }

                return anyChanged ? { packageRegistry: nextRegistry } : state;
            });

        } catch (e) {
            console.warn('[MonARCH] Batch rating fetch failed (safe ignore):', e);
        }
    },

    setTrendingPackages: (pkgs: Package[]) => get().hydrateSection('trending', pkgs),
    setEssentialsPackages: (pkgs: Package[]) => get().hydrateSection('essentials', pkgs),

    syncRegistry: async (ids: string[]) => {
        if (!ids.length) return;
        // Protection against Bridge Flooding: if many IDs, do a Bulk Sync instead
        if (ids.length > 100) {
            console.warn(`[REGISTRY] syncRegistry called with ${ids.length} IDs. Redirecting to Bulk Sync for performance.`);
            get().syncRegistryBulk();
            return;
        }
        try {
            const { isFlatpakEnabled, isAurEnabled, isChaoticEnabled } = get();
            // Fetch the fully joined metadata from the SQLite backend
            const pkgs = unwrap(await commands.getPackagesByNames(ids, {
                flatpak_enabled: isFlatpakEnabled,
                aur_enabled: isAurEnabled,
                chaotic_enabled: isChaoticEnabled,
                for_installed_lookup: false
            }, null));
            if (pkgs && pkgs.length > 0) {
                get().upsertPackages(pkgs);
            }
        } catch (e) {
            console.error('[REGISTRY] Throttled sync failed:', e);
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

        console.debug(`[REGISTRY] Performing Bulk Sync for ${idList.length} relevant objects...`);

        // Fetch in chunks of 100 to avoid IPC limits or DB locks
        const CHUNK_SIZE = 100;
        try {
            for (let i = 0; i < idList.length; i += CHUNK_SIZE) {
                const chunk = idList.slice(i, i + CHUNK_SIZE);
                try {
                    const { isFlatpakEnabled, isAurEnabled, isChaoticEnabled } = get();
                    const pkgs = unwrap(await commands.getPackagesByNames(chunk, {
                        flatpak_enabled: isFlatpakEnabled,
                        aur_enabled: isAurEnabled,
                        chaotic_enabled: isChaoticEnabled,
                        for_installed_lookup: false
                    }, null));
                    if (pkgs && pkgs.length > 0) {
                        get().upsertPackages(pkgs);
                    }
                } catch (p) {
                    console.error(`[REGISTRY] Bulk sync chunk ${i / CHUNK_SIZE} failed:`, p);
                }
            }
        } finally {
            set({ syncingRegistryBulk: false });
            console.debug(`[REGISTRY] Bulk sync completed.`);
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

    setActivePackageId: (id: string | null) => set({ activePackageId: id }),
    setTrendingIds: (ids: string[]) => set({ trendingIds: ids }),
    setEssentialsIds: (ids: string[]) => set({ essentialsIds: ids }),
    setCategoryIds: (category: string, ids: string[]) =>
        set((state) => ({ categoryIds: { ...state.categoryIds, [category]: ids } })),
    appendCategoryIds: (category: string, ids: string[]) =>
        set((state) => {
            const current = state.categoryIds[category] ?? [];
            return { categoryIds: { ...state.categoryIds, [category]: [...current, ...ids] } };
        }),
    setSearchResultIds: (ids: string[]) => set({ searchResultIds: ids }),
    setFavorites: (ids: string[]) => set({ favorites: ids }),
    favoriteError: null,
    clearFavoriteError: () => set({ favoriteError: null }),

    toggleFavorite: async (idOrName: string) => {
        const norm = idOrName.toLowerCase().trim();
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
            const migrated = raw.map((s) => {
                if (typeof s !== 'string') return '';
                const trimmed = String(s).trim().toLowerCase();
                return trimmed;
            }).filter(Boolean);
            const unique = Array.from(new Set(migrated));
            set({ favorites: unique });
        } catch (e) {
            console.error('Failed to load favorites from store', e);
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
            console.error('[MonARCH] Failed to set notifications:', e);
        }
    },
    setPendingUpdates: (updates: { repo: number; aur: number; flatpak: number }) => {
        set({
            pendingUpdates: { ...updates, total: updates.repo + updates.aur + updates.flatpak },
            lastUpdateCheck: Date.now()
        });
    },
    refreshPendingUpdates: async (includeAur?: boolean, includeFlatpak?: boolean) => {
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
            console.error('[MonARCH] Failed to refresh pending updates:', e);
            // Don't report as error - this is a background check
        }
    },

    checkRebootStatus: async () => {
        try {
            const required = unwrap(await commands.checkRebootRequired());
            set({ rebootRequired: required });
        } catch (e) {
            console.error('[MonARCH] Failed to check reboot status:', e);
        }
    },

    checkPacnewStatus: async () => {
        try {
            const warnings = unwrap(await commands.getPacnewWarnings());
            set({ pacnewWarnings: warnings });
        } catch (e) {
            console.error('[MonARCH] Failed to check pacnew status:', e);
        }
    },

    verboseLogsEnabled: false,
    setVerboseLogsEnabled: async (enabled: boolean) => {
        try {
            unwrap(await commands.setVerboseLogsEnabled(enabled));
            set({ verboseLogsEnabled: enabled });
        } catch (e) {
            console.error('[MonARCH] Failed to set verbose logs:', e);
        }
    },
    // Default true: one password per session (Apple Store–like). User can turn off in Settings for system prompt each time.
    reducePasswordPrompts: true,
    setReducePasswordPrompts: async (enabled: boolean) => {
        try {
            unwrap(await commands.setAdvancedMode(enabled)); // Advanced mode in backend maps to reducePasswordPrompts
            set({ reducePasswordPrompts: enabled });
        } catch (e) {
            console.error('[MonARCH] Failed to set advanced mode:', e);
        }
    },
    cleanBuild: false,
    setCleanBuild: async (enabled: boolean) => {
        try {
            unwrap(await commands.setCleanBuildEnabled(enabled));
            set({ cleanBuild: enabled });
        } catch (e) {
            console.error('[MonARCH] Failed to set clean build:', e);
        }
    },
    parallelDownloads: 5,
    setParallelDownloads: async (count: number) => {
        try {
            unwrap(await commands.setParallelDownloads(count));
            set({ parallelDownloads: count });
        } catch (e) {
            console.error('[MonARCH] Failed to set parallel downloads:', e);
        }
    },
    setAurEnabled: async (enabled: boolean) => {
        try {
            unwrap(await commands.setAurEnabled(enabled));
            if (enabled) unwrap(await commands.toggleRepo('aur', true, null));
            set({ isAurEnabled: enabled });
        } catch (e) {
            console.error('[MonARCH] Failed to set AUR enabled:', e);
        }
    },
    setFlatpakEnabled: async (enabled: boolean) => {
        try {
            unwrap(await commands.setFlatpakEnabled(enabled));
            set({ isFlatpakEnabled: enabled });
        } catch (e) {
            console.error('[MonARCH] Failed to set Flatpak enabled:', e);
        }
    },
    setOneClickEnabled: async (enabled: boolean) => {
        try {
            unwrap(await commands.setOneClickEnabled(enabled));
            set({ oneClickEnabled: enabled });
        } catch (e) {
            console.error('[MonARCH] Failed to set one-click enabled:', e);
        }
    },
    setChaoticEnabled: async (enabled: boolean) => {
        try {
            // Chaotic is managed via toggleRepo in backend
            unwrap(await commands.toggleRepo('chaotic-aur', enabled, null));
            set({ isChaoticEnabled: enabled });
        } catch (e) {
            console.error('[MonARCH] Failed to set Chaotic-AUR enabled:', e);
        }
    },
    onboardingCompleted: false,
    setOnboardingCompleted: async (completed: boolean) => {
        try {
            unwrap(await commands.setOnboardingCompleted(completed));
            set({ onboardingCompleted: completed });
        } catch (e) {
            console.error('[MonARCH] Failed to set onboarding completed:', e);
        }
    },
    themeMode: 'system',
    setThemeMode: async (mode: 'system' | 'light' | 'dark') => {
        const previous = get().themeMode;
        set({ themeMode: mode }); // Optimistic update
        try {
            unwrap(await commands.setThemeMode(mode));
        } catch (e) {
            console.error('[MonARCH] Failed to set theme mode:', e);
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
            console.error('[MonARCH] Failed to set accent color:', e);
            set({ accentColor: previous }); // Revert on failure
        }
    },
    declinedSystemSetup: false,
    setDeclinedSystemSetup: async (declined: boolean) => {
        try {
            unwrap(await commands.setDeclinedSystemSetup(declined));
            set({ declinedSystemSetup: declined });
        } catch (e) {
            console.error('[MonARCH] Failed to set declined system setup:', e);
        }
    },
    isSidebarExpanded: true,
    setSidebarExpanded: async (expanded: boolean) => {
        const previous = get().isSidebarExpanded;
        set({ isSidebarExpanded: expanded }); // Optimistic update
        try {
            unwrap(await commands.setSidebarExpanded(expanded));
        } catch (e) {
            console.error('[MonARCH] Failed to set sidebar expanded:', e);
            set({ isSidebarExpanded: previous }); // Revert on failure
        }
    },
    alphaNoticeDismissed: false,
    setAlphaNoticeDismissed: async (dismissed: boolean) => {
        try {
            unwrap(await commands.setAlphaNoticeDismissed(dismissed));
            set({ alphaNoticeDismissed: dismissed });
        } catch (e) {
            console.error('[MonARCH] Failed to set alpha notice dismissed:', e);
        }
    },
    searchHistory: [],
    setSearchHistory: async (history: string[]) => {
        try {
            unwrap(await commands.setSearchHistory(history));
            set({ searchHistory: history });
        } catch (e) {
            console.error('[MonARCH] Failed to set search history:', e);
        }
    },
    readNewsIds: [],
    setReadNewsIds: async (ids: string[]) => {
        try {
            unwrap(await commands.setReadNewsIds(ids));
            set({ readNewsIds: ids });
        } catch (e) {
            console.error('[MonARCH] Failed to set read news ids:', e);
        }
    },
    activeTab: 'explore',
    setActiveTab: async (tab: string) => {
        try {
            unwrap(await commands.setActiveTab(tab));
            set({ activeTab: tab });
        } catch (e) {
            console.error('[MonARCH] Failed to set active tab:', e);
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
            const chaotic = repoStates.find(r => r.name.toLowerCase() === 'chaotic-aur')?.enabled ?? false;

            set({
                telemetryEnabled: telemetry,
                updateNotificationsEnabled: notifications,
                verboseLogsEnabled: verbose,
                cleanBuild: clean,
                parallelDownloads: parallel,
                reducePasswordPrompts: advanced,
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
            console.debug('[MonARCH] Settings initialized from backend.');
        } catch (e) {
            console.error('[MonARCH] Failed to initialize settings:', e);
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
            console.error('[MonARCH] invoke failed: get_trending', raw);
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
            console.error('[MonARCH] invoke failed: get_infra_stats', raw);
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
            console.error('[MonARCH] invoke failed: is_telemetry_enabled', raw);
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
