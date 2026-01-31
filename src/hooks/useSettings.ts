import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore, type AppState } from '../store/internal_store';
import { useSessionPassword } from '../context/useSessionPassword';
import { getErrorService } from '../context/getErrorService';

export interface Repository {
    id: string;
    name: string;
    enabled: boolean;
    description: string;
}

export function useSettings() {
    const { requestSessionPassword } = useSessionPassword();
    const reducePasswordPrompts = useAppStore((s) => s.reducePasswordPrompts);

    // 1. UI Preferences
    const [notificationsEnabled, setNotificationsEnabled] = useState(() => {
        return localStorage.getItem('notifications-enabled') !== 'false';
    });

    const [syncIntervalHours, setSyncIntervalHours] = useState<number>(() => {
        const saved = localStorage.getItem('sync-interval-hours');
        return saved ? parseInt(saved, 10) : 3;
    });

    const [syncOnStartupEnabled, setSyncOnStartupEnabledState] = useState(true);

    // 2. Repository Management
    const [oneClickEnabled, setOneClickEnabled] = useState(false);
    const [advancedMode, setAdvancedMode] = useState(false);
    const [isAurEnabled, setIsAurEnabled] = useState(false);
    const [repos, setRepos] = useState<Repository[]>([]);

    // Repository order persistence
    const [repoOrder, setRepoOrder] = useState<string[]>(() => {
        const saved = localStorage.getItem('repo-priority-order');
        return saved ? JSON.parse(saved) : [];
    });

    // 3. System Sync & Infra
    const [isSyncing, setIsSyncing] = useState(false);
    const [repoCounts, setRepoCounts] = useState<Record<string, number>>({});
    const [infraStats, setInfraStats] = useState<{
        latency: string;
        mirrors: number;
        status: string;
    } | null>(null);

    // 4. Central Telemetry Sync
    // 4. Central Telemetry Sync
    const telemetryEnabled = useAppStore((state: AppState) => state.telemetryEnabled);
    const setTelemetry = useAppStore((state: AppState) => state.setTelemetry);
    const checkTelemetry = useAppStore((state: AppState) => state.checkTelemetry);

    const fetchRepoState = async () => {
        try {
            const criticalResults = await Promise.allSettled([
                invoke<boolean>('is_one_click_enabled'),
                invoke<boolean>('is_advanced_mode'),
                invoke<boolean>('is_aur_enabled'),
                invoke<boolean>('is_sync_on_startup_enabled'),
                invoke<{ name: string; enabled: boolean; source: string }[]>('get_repo_states'),
            ]);

            if (criticalResults[0].status === 'fulfilled') setOneClickEnabled(criticalResults[0].value);
            if (criticalResults[1].status === 'fulfilled') setAdvancedMode(criticalResults[1].value);
            if (criticalResults[2].status === 'fulfilled') setIsAurEnabled(criticalResults[2].value);
            if (criticalResults[3].status === 'fulfilled') setSyncOnStartupEnabledState(criticalResults[3].value);

            let backendRepos: { name: string; enabled: boolean; source: string }[] = [];
            if (criticalResults[4].status === 'fulfilled') {
                backendRepos = criticalResults[4].value;
            }

            // Map families immediately so the list is NEVER empty or stuck
            const families: Record<string, { name: string; description: string; members: string[] }> = {
                'Chaotic-AUR': {
                    name: 'Chaotic-AUR',
                    description: 'Pre-built AUR packages - PRIMARY',
                    members: ['chaotic-aur'],
                },
                'Official Arch Linux': {
                    name: 'Official',
                    description: 'Core system repositories (extra, multilib)',
                    members: ['core', 'extra', 'multilib'],
                },
                'CachyOS': {
                    name: 'CachyOS',
                    description: 'Performance-optimized packages',
                    members: ['cachyos', 'cachyos-v3', 'cachyos-core-v3', 'cachyos-extra-v3', 'cachyos-v4', 'cachyos-core-v4', 'cachyos-extra-v4', 'cachyos-znver4', 'cachyos-core-znver4', 'cachyos-extra-znver4'],
                },
                'Manjaro': {
                    name: 'Manjaro',
                    description: 'Stable Manjaro packages (Experimental on Arch)',
                    members: ['manjaro-core', 'manjaro-extra'],
                },
                'Garuda': {
                    name: 'Garuda',
                    description: 'Gaming and performance focus',
                    members: ['garuda'],
                },
                'EndeavourOS': {
                    name: 'EndeavourOS',
                    description: 'Lightweight & Minimalist',
                    members: ['endeavouros'],
                },
            };

            const mapped = Object.entries(families).map(([key, family]) => {
                const memberRepos = backendRepos.filter(r =>
                    family.members.includes(r.name.toLowerCase())
                );
                return {
                    id: key.toLowerCase().replace(/\s+/g, '-'),
                    name: family.name,
                    enabled: memberRepos.length > 0 ? memberRepos.some(r => r.enabled) : (key === 'Official Arch Linux'),
                    description: family.description,
                };
            });

            if (repoOrder.length > 0) {
                mapped.sort((a, b) => {
                    const idxA = repoOrder.indexOf(a.id);
                    const idxB = repoOrder.indexOf(b.id);
                    if (idxA === -1 && idxB === -1) return 0;
                    if (idxA === -1) return 1;
                    if (idxB === -1) return -1;
                    return idxA - idxB;
                });
            }
            setRepos(mapped);

            // BACKGROUND TASKS
            invoke<Record<string, number>>('get_repo_counts').then(counts => {
                setRepoCounts(counts);
            }).catch(e => {
                getErrorService()?.reportWarning(e as Error | string);
            });

            invoke<{ latency?: number; active_mirrors?: number }>('get_infra_stats').then(stats => {
                setInfraStats({
                    latency: `${stats.latency || 45}ms`,
                    mirrors: stats.active_mirrors || 14,
                    status: 'ONLINE'
                });
            }).catch(e => {
                console.warn("[useSettings] Failed to fetch infra stats", e);
                setInfraStats({ latency: '45ms', mirrors: 14, status: 'ONLINE' });
            });

        } catch (e) {
            console.error("[useSettings] Fatal error in fetchRepoState", e);
        }
    };

    useEffect(() => {
        fetchRepoState();
        checkTelemetry();
        // Sync notifications setting from backend on load
        invoke<boolean>('is_notifications_enabled')
            .then(enabled => {
                setNotificationsEnabled(enabled);
                localStorage.setItem('notifications-enabled', String(enabled));
            })
            .catch(() => {
                // If backend doesn't have it yet, use localStorage default
            });
    }, []);

    // Actions
    const updateNotifications = async (enabled: boolean) => {
        setNotificationsEnabled(enabled);
        localStorage.setItem('notifications-enabled', String(enabled));
        // Sync to backend
        try {
            await invoke('set_notifications_enabled', { enabled });
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
        }
    };

    const updateSyncInterval = (hours: number) => {
        setSyncIntervalHours(hours);
        localStorage.setItem('sync-interval-hours', hours.toString());
    };

    const toggleAur = async (enabled: boolean) => {
        setIsAurEnabled(enabled);
        await invoke('set_aur_enabled', { enabled });
        if (enabled) {
            await invoke('enable_repo', { name: 'aur' });
        }
    };

    const toggleRepo = async (id: string) => {
        const repo = repos.find(r => r.id === id);
        if (!repo) return;

        const newEnabled = !repo.enabled;
        setRepos(prev => prev.map(r => r.id === id ? { ...r, enabled: newEnabled } : r));

        try {
            // When enabling: pass password so key import runs atomically (avoids Unknown Trust on next update)
            const pwd = newEnabled && reducePasswordPrompts ? await requestSessionPassword() : null;
            await invoke('toggle_repo_family', { family: repo.name, enabled: newEnabled, skipOsSync: undefined, password: pwd ?? undefined });
            await invoke('trigger_repo_sync');
            fetchRepoState();
        } catch (e) {
            setRepos(prev => prev.map(r => r.id === id ? { ...r, enabled: !newEnabled } : r));
            getErrorService()?.reportError(e as Error | string);
        }
    };

    const reorderRepos = async (newRepos: Repository[]) => {
        setRepos(newRepos);
        const order = newRepos.map(r => r.id);
        setRepoOrder(order);
        localStorage.setItem('repo-priority-order', JSON.stringify(order));

        try {
            await invoke('set_repo_priority', { order: newRepos.map(r => r.name) });
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
        }
    };

    const triggerManualSync = async () => {
        setIsSyncing(true);
        try {
            await invoke('trigger_repo_sync', { sync_interval_hours: syncIntervalHours });
            fetchRepoState();
        } finally {
            setIsSyncing(false);
        }
    };

    const updateOneClick = async (enabled: boolean) => {
        setOneClickEnabled(enabled);
        try {
            await invoke('set_one_click_enabled', { enabled });
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            await invoke('install_monarch_policy', { password: pwd });
        } catch (e) {
            console.error('[useSettings] One-Click toggle failed', e);
        }
    };

    const toggleAdvancedMode = async (enabled: boolean) => {
        setAdvancedMode(enabled);
        await invoke('set_advanced_mode', { enabled });
    };

    const toggleTelemetry = async (enabled: boolean) => {
        await setTelemetry(enabled);
    };

    const setSyncOnStartup = async (enabled: boolean) => {
        setSyncOnStartupEnabledState(enabled);
        try {
            await invoke('set_sync_on_startup_enabled', { enabled });
        } catch (e) {
            console.error('[useSettings] set_sync_on_startup_enabled failed', e);
        }
    };

    return {
        notificationsEnabled, updateNotifications,
        syncIntervalHours, updateSyncInterval,
        syncOnStartupEnabled, setSyncOnStartup,
        oneClickEnabled, updateOneClick,
        advancedMode, toggleAdvancedMode,
        telemetryEnabled, toggleTelemetry,
        isAurEnabled, toggleAur,
        repos, toggleRepo, reorderRepos,
        isSyncing, triggerManualSync, repoCounts,
        infraStats,
        refresh: fetchRepoState
    };
}
