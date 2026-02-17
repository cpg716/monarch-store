import { useState, useEffect } from 'react';
import { commands } from '../services/bindings';
import { unwrap } from '../utils/specta';
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

    // Unified State from App Store
    const {
        isAurEnabled, setAurEnabled,
        isFlatpakEnabled, setFlatpakEnabled,
        oneClickEnabled, setOneClickEnabled,
        reducePasswordPrompts, setReducePasswordPrompts,
        verboseLogsEnabled, setVerboseLogsEnabled,
        cleanBuild: cleanBuildEnabled, setCleanBuild: setCleanBuildEnabled,
        updateNotificationsEnabled: notificationsEnabled, setUpdateNotificationsEnabled: updateNotifications,
        parallelDownloads, setParallelDownloads,
        isChaoticEnabled, setChaoticEnabled
    } = useAppStore();

    // 1. UI Preferences (Remaining local or combined)
    const [syncIntervalHours, setSyncIntervalHours] = useState<number>(3);
    const [syncOnStartupEnabled, setSyncOnStartupEnabledState] = useState(true);
    const [automaticHousekeepingEnabled, setAutomaticHousekeepingEnabled] = useState(false);

    // 2. Repository Management
    const [repos, setRepos] = useState<Repository[]>([]);
    const [repoOrder, setRepoOrder] = useState<string[]>([]);

    // 3. System Sync & Infra
    const [isSyncing, setIsSyncing] = useState(false);
    const [repoCounts, setRepoCounts] = useState<Record<string, number>>({});
    const [infraStats, setInfraStats] = useState<{
        builders: number;
        users: number;
        status: string;
    } | null>(null);

    // 4. Central Telemetry Sync
    const telemetryEnabled = useAppStore((state: AppState) => state.telemetryEnabled);
    const setTelemetry = useAppStore((state: AppState) => state.setTelemetry);
    const checkTelemetry = useAppStore((state: AppState) => state.checkTelemetry);

    const fetchRepoState = async () => {
        try {
            const [
                syncOnStartup,
                interval,
                housekeeping,
                rOrder,
                backendRepos
            ] = await Promise.all([
                commands.isSyncOnStartupEnabled().then(unwrap),
                commands.getSyncIntervalHours().then(unwrap),
                commands.isAutomaticHousekeepingEnabled().then(unwrap),
                commands.getRepoPriorityOrder().then(unwrap),
                commands.getRepoStates().then(unwrap),
            ]);

            setSyncOnStartupEnabledState(syncOnStartup);
            setSyncIntervalHours(interval);
            setAutomaticHousekeepingEnabled(housekeeping);
            setRepoOrder(rOrder);

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
                const memberRepos = (backendRepos as any[]).filter(r =>
                    family.members.includes(r.name.toLowerCase())
                );
                return {
                    id: key.toLowerCase().replace(/\s+/g, '-'),
                    name: family.name,
                    enabled: memberRepos.length > 0 ? memberRepos.some(r => r.enabled) : (key === 'Official Arch Linux'),
                    description: family.description,
                };
            });

            if (rOrder.length > 0) {
                mapped.sort((a, b) => {
                    const idxA = rOrder.indexOf(a.id);
                    const idxB = rOrder.indexOf(b.id);
                    if (idxA === -1 && idxB === -1) return 0;
                    if (idxA === -1) return 1;
                    if (idxB === -1) return -1;
                    return idxA - idxB;
                });
            }
            setRepos(mapped);

            commands.getRepoCounts().then(unwrap).then(counts => {
                const numericCounts: Record<string, number> = {};
                Object.entries(counts).forEach(([k, v]) => numericCounts[k] = parseInt(v, 10));
                setRepoCounts(numericCounts);
            }).catch(e => {
                getErrorService()?.reportWarning(e as Error | string);
            });

            commands.getInfraStats().then(unwrap).then(stats => {
                setInfraStats({
                    builders: (stats as any).builders || 0,
                    users: (stats as any).users || 0,
                    status: 'ONLINE'
                });
            }).catch(e => {
                console.warn("[useSettings] Failed to fetch infra stats", e);
                setInfraStats({ builders: 0, users: 0, status: 'ONLINE' });
            });

        } catch (e) {
            console.error("[useSettings] Fatal error in fetchRepoState", e);
        }
    };

    useEffect(() => {
        fetchRepoState();
        checkTelemetry();
    }, []);

    // Actions
    const toggleAur = async (enabled: boolean) => {
        try {
            await setAurEnabled(enabled);
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
        }
    };

    const toggleFlatpak = async (enabled: boolean) => {
        try {
            await setFlatpakEnabled(enabled);
            if (enabled) {
                const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
                unwrap(await commands.prepareFlatpak(pwd ?? null));
            }
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
        }
    };

    const toggleRepo = async (id: string) => {
        const repo = repos.find(r => r.id === id);
        if (!repo) return;

        const newEnabled = !repo.enabled;
        setRepos(prev => prev.map(r => r.id === id ? { ...r, enabled: newEnabled } : r));

        try {
            const pwd = newEnabled && reducePasswordPrompts ? await requestSessionPassword() : null;
            unwrap(await commands.toggleRepoFamily(repo.name, newEnabled, null, pwd ?? null));
            unwrap(await commands.triggerRepoSync(null));
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

        try {
            unwrap(await commands.setRepoPriorityOrder(order));
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
        }
    };

    const triggerManualSync = async () => {
        setIsSyncing(true);
        try {
            unwrap(await commands.triggerRepoSync(syncIntervalHours.toString()));
            fetchRepoState();
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
        } finally {
            setIsSyncing(false);
        }
    };

    const updateOneClick = async (enabled: boolean) => {
        try {
            await setOneClickEnabled(enabled);
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            unwrap(await commands.installMonarchPolicy(pwd ?? null));
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
        }
    };

    const toggleAdvancedMode = async (enabled: boolean) => {
        try {
            await setReducePasswordPrompts(enabled);
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
        }
    };

    const toggleTelemetry = async (enabled: boolean) => {
        await setTelemetry(enabled);
    };

    const setSyncOnStartup = async (enabled: boolean) => {
        setSyncOnStartupEnabledState(enabled);
        try {
            unwrap(await commands.setSyncOnStartupEnabled(enabled));
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
        }
    };

    const updateSyncInterval = async (hours: number) => {
        setSyncIntervalHours(hours);
        try {
            unwrap(await commands.setSyncIntervalHours(hours));
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
        }
    };

    const toggleVerboseLogs = async (enabled: boolean) => {
        await setVerboseLogsEnabled(enabled);
    };

    const toggleCleanBuild = async (enabled: boolean) => {
        await setCleanBuildEnabled(enabled);
    };

    const toggleAutomaticHousekeeping = async (enabled: boolean) => {
        setAutomaticHousekeepingEnabled(enabled);
        try {
            unwrap(await commands.setAutomaticHousekeepingEnabled(enabled));
        } catch (e) {
            setAutomaticHousekeepingEnabled(!enabled);
            getErrorService()?.reportError(e as Error | string);
        }
    };

    const performHousekeeping = async () => {
        try {
            unwrap(await commands.performHousekeeping(null));
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
        }
    };

    return {
        notificationsEnabled, updateNotifications,
        syncIntervalHours, updateSyncInterval,
        syncOnStartupEnabled, setSyncOnStartup,
        oneClickEnabled, updateOneClick,
        advancedMode: reducePasswordPrompts, toggleAdvancedMode,
        telemetryEnabled, toggleTelemetry,
        isAurEnabled, toggleAur,
        isFlatpakEnabled, toggleFlatpak,
        isChaoticEnabled, toggleChaotic: setChaoticEnabled,
        verboseLogsEnabled, toggleVerboseLogs,
        cleanBuildEnabled, toggleCleanBuild,
        automaticHousekeepingEnabled, toggleAutomaticHousekeeping,
        performHousekeeping,
        parallelDownloads, setParallelDownloads,
        repos, toggleRepo, reorderRepos,
        isSyncing, triggerManualSync, repoCounts,
        infraStats,
        refresh: fetchRepoState
    };
}
