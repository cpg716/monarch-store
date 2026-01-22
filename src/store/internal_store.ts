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

interface AppState {
    trendingPackages: TrendingPackage[];
    infraStats: InfraStats | null;
    loadingTrending: boolean;
    loadingStats: boolean;
    error: string | null;
    fetchTrending: () => Promise<void>;
    fetchInfraStats: () => Promise<void>;
}

export const useAppStore = create<AppState>((set) => ({
    trendingPackages: [],
    infraStats: null,
    loadingTrending: false,
    loadingStats: false,
    error: null,
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
}));
