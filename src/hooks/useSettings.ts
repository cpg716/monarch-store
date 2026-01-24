import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface Repository {
    id: string;
    name: string;
    enabled: boolean;
    description: string;
}

export function useSettings() {
    // 1. UI Preferences
    const [notificationsEnabled, setNotificationsEnabled] = useState(() => {
        return localStorage.getItem('notifications-enabled') !== 'false';
    });

    const [syncIntervalHours, setSyncIntervalHours] = useState<number>(() => {
        const saved = localStorage.getItem('sync-interval-hours');
        return saved ? parseInt(saved, 10) : 3;
    });

    // 2. Repository Management
    const [isAurEnabled, setIsAurEnabled] = useState(false);
    const [repos, setRepos] = useState<Repository[]>([]);

    // Repository order persistence
    const [repoOrder, setRepoOrder] = useState<string[]>(() => {
        const saved = localStorage.getItem('repo-priority-order');
        return saved ? JSON.parse(saved) : [];
    });

    // 3. System Sync
    const [isSyncing, setIsSyncing] = useState(false);
    const [repoCounts, setRepoCounts] = useState<Record<string, number>>({});

    const fetchRepoState = async () => {
        try {
            const isAur = await invoke<boolean>('is_aur_enabled');
            setIsAurEnabled(isAur);

            const backendRepos = await invoke<{ name: string; enabled: boolean; source: string }[]>('get_repo_states');

            // Map families
            const families: Record<string, { name: string; description: string; members: string[] }> = {
                'Chaotic-AUR': {
                    name: 'Chaotic-AUR',
                    description: 'Pre-built AUR packages - PRIMARY',
                    members: ['Chaotic-AUR'],
                },
                'CachyOS': {
                    name: 'CachyOS',
                    description: 'Performance-optimized packages',
                    members: ['cachyos', 'cachyos-v3', 'cachyos-core-v3', 'cachyos-extra-v3', 'cachyos-v4', 'cachyos-core-v4', 'cachyos-extra-v4', 'cachyos-znver4', 'cachyos-core-znver4', 'cachyos-extra-znver4'],
                },
                'Manjaro': {
                    name: 'Manjaro',
                    description: 'Stable, tested packages from Manjaro',
                    members: ['manjaro-core', 'manjaro-extra', 'manjaro-multilib'],
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
                    family.members.some(m => r.name.toLowerCase() === m.toLowerCase())
                );
                return {
                    id: key.toLowerCase().replace(/\s+/g, '-'),
                    name: family.name,
                    enabled: memberRepos.some(r => r.enabled),
                    description: family.description,
                };
            });

            // Sort by repoOrder if available
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

            const counts = await invoke<Record<string, number>>('get_repo_counts');
            setRepoCounts(counts);
        } catch (e) {
            console.error("[useSettings] Failed to fetch repo state", e);
        }
    };

    useEffect(() => {
        fetchRepoState();
    }, []);

    // Actions
    const updateNotifications = (enabled: boolean) => {
        setNotificationsEnabled(enabled);
        localStorage.setItem('notifications-enabled', String(enabled));
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
            // Soft Toggle: Updates UI state (repos.json) and clears cache (repo_manager.rs)
            // Does NOT touch system config (no password required)
            await invoke('toggle_repo_family', { family: repo.name, enabled: newEnabled });

            // If enabling, we might want to check if system backend exists?
            // But since Onboarding enables ALL, we assume it's there.
            // If missing (e.g. manual delete), user can use "Repair Config" or we can lazy-load if query fails.
            // For now, adhere to "Soft Disable" spec.

            await invoke('trigger_repo_sync');
            fetchRepoState();
        } catch (e) {
            // Revert
            setRepos(prev => prev.map(r => r.id === id ? { ...r, enabled: !newEnabled } : r));
        }
    };

    const reorderRepos = async (newRepos: Repository[]) => {
        setRepos(newRepos);
        const order = newRepos.map(r => r.id);
        setRepoOrder(order);
        localStorage.setItem('repo-priority-order', JSON.stringify(order));

        // Sync to backend if needed for Infrastructure 2.0 file naming
        try {
            await invoke('set_repo_priority', { order: newRepos.map(r => r.name) });
        } catch (e) {
            console.error("Priority sync failed", e);
        }
    };

    const triggerManualSync = async () => {
        setIsSyncing(true);
        try {
            await invoke('trigger_repo_sync', { syncIntervalHours });
            fetchRepoState();
        } finally {
            setIsSyncing(false);
        }
    };

    return {
        notificationsEnabled, updateNotifications,
        syncIntervalHours, updateSyncInterval,
        isAurEnabled, toggleAur,
        repos, toggleRepo, reorderRepos,
        isSyncing, triggerManualSync, repoCounts,
        refresh: fetchRepoState
    };
}
