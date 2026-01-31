import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { getErrorService } from '../context/getErrorService';
import { friendlyError } from '../utils/friendlyError';
import type { Package } from '../components/PackageCard';

const isDecodeError = (raw: string): boolean =>
    /error decoding response body|decoding response body|invalid json|unexpected end of|expected value/i.test(raw);

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
    trendingPackages: Package[];
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

    /** When true, install modal shows detailed transaction logs by default (Glass Cockpit) */
    verboseLogsEnabled: boolean;
    setVerboseLogsEnabled: (enabled: boolean) => void;

    /** When true, user can enter password once in MonARCH (one dialog per session). Less secure than system prompt each time. */
    reducePasswordPrompts: boolean;
    setReducePasswordPrompts: (enabled: boolean) => void;

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
    verboseLogsEnabled: typeof localStorage !== 'undefined' ? (localStorage.getItem('monarch_verbose_logs') === 'true' || localStorage.getItem('monarch_debug_logs') === 'true') : false,
    setVerboseLogsEnabled: (enabled: boolean) => {
        if (typeof localStorage !== 'undefined') {
            if (enabled) localStorage.setItem('monarch_verbose_logs', 'true');
            else localStorage.removeItem('monarch_verbose_logs');
        }
        set({ verboseLogsEnabled: enabled });
    },
    // Default true: one password per session (Apple Store–like). User can turn off in Settings for system prompt each time.
    reducePasswordPrompts: typeof localStorage !== 'undefined' ? (localStorage.getItem('monarch_reduce_password_prompts') ?? 'true') !== 'false' : true,
    setReducePasswordPrompts: (enabled: boolean) => {
        if (typeof localStorage !== 'undefined') {
            if (enabled) localStorage.setItem('monarch_reduce_password_prompts', 'true');
            else localStorage.removeItem('monarch_reduce_password_prompts');
        }
        set({ reducePasswordPrompts: enabled });
    },
    fetchTrending: async () => {
        set({ loadingTrending: true, error: null });
        try {
            const trending = await invoke<Package[]>('get_trending');
            set({ trendingPackages: trending, loadingTrending: false });
        } catch (e) {
            const raw = e instanceof Error ? (e as Error).message : String(e);
            console.error('[MonARCH] invoke failed: get_trending', raw);
            if (isDecodeError(raw)) {
                set({ loadingTrending: false, trendingPackages: [], error: null });
            } else {
                getErrorService()?.reportError(e as Error | string);
                set({ loadingTrending: false, error: friendlyError(raw).description });
            }
        }
    },
    fetchInfraStats: async () => {
        set({ loadingStats: true });
        try {
            const stats = await invoke<InfraStats>('get_infra_stats');
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
            const enabled = await invoke<boolean>('is_telemetry_enabled');
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
            await invoke('set_telemetry_enabled', { enabled });
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
