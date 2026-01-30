import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export interface TrendingPackage {
    pkgbase_pkgname: string;
    count: string;
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

interface AppState {
    trendingPackages: TrendingPackage[];
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

    fetchTrending: () => Promise<void>;
    fetchInfraStats: () => Promise<void>;
    checkTelemetry: () => Promise<void>;
    setTelemetry: (enabled: boolean) => Promise<void>;

    // Update Actions
    setUpdating: (val: boolean) => void;
    setUpdateProgress: (progress: number) => void;
    setUpdateStatus: (msg: string) => void;
    setUpdatePhase: (phase: string) => void;
    addUpdateLog: (log: string) => void;
    clearUpdateLogs: () => void;
    setRebootRequired: (val: boolean) => void;
    setPacnewWarnings: (warnings: string[]) => void;
}

export const useAppStore = create<AppState>((set) => ({
    trendingPackages: [],
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
    fetchTrending: async () => {
        set({ loadingTrending: true, error: null });
        try {
            const trending = await invoke<TrendingPackage[]>('get_trending');
            set({ trendingPackages: trending, loadingTrending: false });
        } catch (e) {
            console.error("Failed to fetch trending:", e);
            set({ loadingTrending: false, error: String(e) });
        }
    },
    fetchInfraStats: async () => {
        set({ loadingStats: true });
        try {
            const stats = await invoke<InfraStats>('get_infra_stats');
            set({ infraStats: stats, loadingStats: false });
        } catch (e) {
            console.error("Failed to fetch stats:", e);
            set({ loadingStats: false });
        }
    },
    checkTelemetry: async () => {
        try {
            const enabled = await invoke<boolean>('is_telemetry_enabled');
            set({ telemetryEnabled: enabled });
        } catch (e) {
            console.error("Failed to check telemetry:", e);
        }
    },
    setTelemetry: async (enabled: boolean) => {
        const previousState = useAppStore.getState().telemetryEnabled;
        set({ telemetryEnabled: enabled });

        try {
            await invoke('set_telemetry_enabled', { enabled });
        } catch (e) {
            console.error("[Store] Failed to set telemetry:", e);
            // Rollback on error
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
