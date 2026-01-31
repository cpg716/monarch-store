import { useState, useEffect } from 'react';
import {
    CheckCircle2, Globe, Palette,
    Trash2, ShieldCheck, Package, RefreshCw, Lock, Sparkles, AlertTriangle, Rocket, Activity, ChevronDown, Terminal
} from 'lucide-react';
import ConfirmationModal from '../components/ConfirmationModal';
import SystemHealthSection from '../components/SystemHealthSection';
import RepositoriesTab from '../components/settings/RepositoriesTab';
import HardwareOptimization from '../components/settings/HardwareOptimization';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import { clsx } from 'clsx';

import { useTheme } from '../hooks/useTheme';
import { useToast } from '../context/ToastContext';
import { useErrorService } from '../context/ErrorContext';
import { useSessionPassword } from '../context/useSessionPassword';
import { useSettings } from '../hooks/useSettings';
import { useDistro } from '../hooks/useDistro';
import { useAppStore, type AppState } from '../store/internal_store';


interface SettingsPageProps {
    onRestartOnboarding?: () => void;
    onRepairComplete?: () => void | Promise<void>;
}

/** Matches backend CacheSizeResult */
interface CacheSizeResult {
    size_bytes: number;
    human_readable: string;
}

/** Matches backend OrphansWithSizeResult */
interface OrphansWithSizeResult {
    orphans: string[];
    total_size_bytes: number;
    human_readable: string;
}

export default function SettingsPage({ onRestartOnboarding, onRepairComplete }: SettingsPageProps) {
    const { themeMode, setThemeMode, accentColor, setAccentColor } = useTheme();
    const { success } = useToast();
    const errorService = useErrorService();
    const { requestSessionPassword } = useSessionPassword();
    const { distro } = useDistro(); // <-- Identity Matrix

    // Centralized Logic
    const {
        notificationsEnabled, updateNotifications,
        syncIntervalHours, updateSyncInterval,
        syncOnStartupEnabled, setSyncOnStartup,
        isAurEnabled, toggleAur,
        repos, toggleRepo, reorderRepos,
        isSyncing, triggerManualSync, repoCounts,
        infraStats,
        oneClickEnabled, updateOneClick,
        advancedMode, toggleAdvancedMode,
        refresh
    } = useSettings();

    // Use store directly for critical reactivity
    const telemetryEnabled = useAppStore((state: AppState) => state.telemetryEnabled);
    const setTelemetry = useAppStore((state: AppState) => state.setTelemetry);
    const verboseLogsEnabled = useAppStore((state: AppState) => state.verboseLogsEnabled);
    const setVerboseLogsEnabled = useAppStore((state: AppState) => state.setVerboseLogsEnabled);
    const reducePasswordPrompts = useAppStore((state: AppState) => state.reducePasswordPrompts);
    const setReducePasswordPrompts = useAppStore((state: AppState) => state.setReducePasswordPrompts);

    // Atomic local state for ZERO LANCY UI
    const [localToggle, setLocalToggle] = useState(telemetryEnabled);
    useEffect(() => { setLocalToggle(telemetryEnabled); }, [telemetryEnabled]);

    const handleToggle = async () => {
        const target = !localToggle;
        setLocalToggle(target); // Immediate visual flip
        try {
            await setTelemetry(target);
            if (target) success("Telemetry Enabled. Thank you!");
            else success("Telemetry Disabled.");
        } catch (e) {
            errorService.reportError(e as Error | string);
            setLocalToggle(telemetryEnabled); // Rollback visual on error
        }
    };

    const [isOptimizing, setIsOptimizing] = useState(false);
    const [isRepairing, setIsRepairing] = useState<string | null>(null);
    const [advancedRepairOpen, setAdvancedRepairOpen] = useState(false);
    const [pkgVersion, setPkgVersion] = useState<string>('');
    const [installMode, setInstallMode] = useState<'system' | 'portable'>('portable');
    const [systemInfo, setSystemInfo] = useState<{ kernel: string, distro: string, cpu_optimization: string, pacman_version: string } | null>(null);
    const [repoSyncStatus, setRepoSyncStatus] = useState<Record<string, boolean> | null>(null);
    const [syncProgressMessage, setSyncProgressMessage] = useState<string | null>(null);
    const [prioritizeOptimized, setPrioritizeOptimized] = useState(false);
    const [parallelDownloads, setParallelDownloads] = useState(5);
    const [isRankingMirrors, setIsRankingMirrors] = useState(false);
    const [mirrorRankTool, setMirrorRankTool] = useState<string | null>(null);

    const [modalConfig, setModalConfig] = useState<{
        isOpen: boolean;
        title: string;
        message: string;
        onConfirm: () => void;
        variant?: 'danger' | 'info';
    }>({ isOpen: false, title: '', message: '', onConfirm: () => { } });

    // Initial Load & Scroll Reset
    useEffect(() => {
        window.scrollTo(0, 0);
        getVersion().then(setPkgVersion).catch((e) => errorService.reportError(e as Error | string));
        invoke<string>('get_install_mode_command').then(mode => setInstallMode(mode as 'system' | 'portable')).catch((e) => errorService.reportError(e as Error | string));
        invoke<{ kernel: string; distro: string; cpu_optimization: string; pacman_version: string }>('get_system_info').then(setSystemInfo).catch((e) => errorService.reportError(e as Error | string));
        invoke<Record<string, boolean>>('check_repo_sync_status').then(setRepoSyncStatus).catch((e) => errorService.reportError(e as Error | string));
        invoke<string | null>('get_mirror_rank_tool').then(setMirrorRankTool).catch(() => setMirrorRankTool(null));
        // Load hardware optimization preference
        const saved = localStorage.getItem('prioritize-optimized-binaries');
        if (saved !== null) setPrioritizeOptimized(saved === 'true');
        // Load parallel downloads (default 5, as set in helper)
        const savedParallel = localStorage.getItem('parallel-downloads');
        if (savedParallel) setParallelDownloads(parseInt(savedParallel, 10));
    }, []);

    // Detailed sync progress (GPG/db steps) for Repository Control
    useEffect(() => {
        const unlisten = listen<string>('sync-progress', (event) => setSyncProgressMessage(event.payload));
        return () => { unlisten.then((f) => f()).catch(() => {}); };
    }, []);
    useEffect(() => {
        if (!isSyncing) setSyncProgressMessage(null);
    }, [isSyncing]);

    // --- LOCKING LOGIC ---
    const isRepoLocked = (name: string): boolean => {
        // GOD MODE: If Advanced Mode is ON, NOTHING is locked.
        if (advancedMode) return false;

        if (name === 'chaotic-aur') {
            // Manjaro: Blocked
            if ((distro.capabilities.chaotic_aur_support as string) === 'blocked') return true;
            // Garuda: Native (effectively locked ON usually, but we might allow disable? No, let's keep it user choice unless it breaks things)
            // Actually, 'Native' usually means installed by system. We won't strict lock it OFF, but we might warn.
            // For this specific 'isRepoLocked' helper, we strictly care about "CANNOT CHANGE".
            return (distro.capabilities.chaotic_aur_support as string) === 'blocked';
        }
        return false;
    };

    const handleOptimize = async () => {
        setIsOptimizing(true);
        try {
            const result = await invoke<string>('optimize_system');
            success(result);
        } catch (e) {
            errorService.reportError(e as Error | string);
        } finally {
            setIsOptimizing(false);
        }
    };

    const handleClearCache = async () => {
        // Query cache size before showing confirmation
        let cacheInfo = "all cached data";
        try {
            const size = await invoke<CacheSizeResult>('get_cache_size');
            cacheInfo = size.human_readable || "all cached data";
        } catch (e) {
            // Fallback if command doesn't exist yet
            console.warn("get_cache_size not available:", e);
        }
        setModalConfig({
            isOpen: true,
            title: "Clear Application & Package Cache",
            message: `Clear ${cacheInfo} of cached packages? This will clear in-app caches and the pacman package cache on disk, freeing space. You may need to re-download packages on next install.`,
            variant: 'danger',
            onConfirm: async () => {
                setIsOptimizing(true);
                try {
                    await invoke('clear_cache');
                    await invoke('clear_pacman_package_cache', { keep: 0 });
                    success('Application and package cache cleared.');
                    refresh();
                } catch (e) {
                    errorService.reportError(e as Error | string);
                } finally {
                    setIsOptimizing(false);
                }
            }
        });
    };

    const handleOrphans = async () => {
        setModalConfig({
            isOpen: true,
            title: "Scan for Orphans",
            message: "Scan for and remove unused orphan packages?",
            variant: 'info',
            onConfirm: async () => {
                setIsRepairing("orphans");
                try {
                    const orphans = await invoke<string[]>('get_orphans');
                    if (orphans.length === 0) {
                        success("No orphan packages found.");
                        setIsRepairing(null);
                    } else {
                        // Query total size of orphans
                        let sizeInfo = "";
                        try {
                            const size = await invoke<OrphansWithSizeResult>('get_orphans_with_size');
                            sizeInfo = size.human_readable ? ` (~${size.human_readable})` : "";
                        } catch (e) {
                            // Fallback if command doesn't exist yet
                            console.warn("get_orphans_with_size not available:", e);
                        }
                        setModalConfig({
                            isOpen: true,
                            title: "Remove Orphans",
                            message: `Found ${orphans.length} orphan package${orphans.length > 1 ? 's' : ''}${sizeInfo}. Remove them?`,
                            variant: 'danger',
                            onConfirm: async () => {
                                setIsRepairing("orphans");
                                await invoke('remove_orphans', { orphans });
                                success(`Successfully removed ${orphans.length} package${orphans.length > 1 ? 's' : ''}.`);
                                setIsRepairing(null);
                            }
                        });
                        return;
                    }
                } catch (e) {
                    errorService.reportError(e as Error | string);
                    setIsRepairing(null);
                }
            }
        });
    };

    const handleRepairTask = async (task: string) => {
        setIsRepairing(task);
        try {
            let cmd = '';
            let label = '';
            if (task === 'keyring') { cmd = 'repair_reset_keyring'; label = 'Keyring Repair'; }
            if (task === 'unlock') { cmd = 'repair_unlock_pacman'; label = 'Database Unlock'; }

            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            await invoke(cmd, { password: pwd });
            success(`${label} completed successfully.`);
            await onRepairComplete?.();
            refresh();
        } catch (e) {
            errorService.reportError(e as Error | string);
        } finally {
            setIsRepairing(null);
        }
    };

    const handleForceRefreshDatabases = async () => {
        setIsRepairing("refresh_db");
        try {
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            await invoke("force_refresh_databases", { password: pwd });
            success("Databases refreshed successfully.");
            await onRepairComplete?.();
        } catch (e) {
            errorService.reportError(e as Error | string);
        } finally {
            setIsRepairing(null);
        }
    };

    const handlePrioritizeOptimized = async (enabled: boolean) => {
        setPrioritizeOptimized(enabled);
        localStorage.setItem('prioritize-optimized-binaries', String(enabled));
        // Backend will use this preference when building priority order in transactions.rs
        success(enabled ? "Optimized binaries prioritized" : "Using standard repository priority");
    };

    const handleParallelDownloads = async (value: number) => {
        setParallelDownloads(value);
        localStorage.setItem('parallel-downloads', value.toString());
        try {
            // Backend command to update /etc/pacman.conf
            await invoke('set_parallel_downloads', { count: value });
            success(`Parallel downloads set to ${value}. Restart MonARCH for full effect.`);
        } catch (e) {
            errorService.reportError(e as Error | string);
        }
    };

    const handleMirrorRanking = async () => {
        setIsRankingMirrors(true);
        try {
            await invoke('rank_mirrors');
            success("Mirrors ranked successfully. Fastest mirrors are now prioritized.");
        } catch (e) {
            errorService.reportError(e as Error | string);
        } finally {
            setIsRankingMirrors(false);
        }
    };

    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            {/* Banner Header */}
            <div className="relative min-h-[200px] flex items-end mb-8 bg-slate-100 dark:bg-black transition-colors overflow-hidden">
                {/* Dark Mode Gradients: Hidden in light mode via dark:block */}
                <div className="absolute inset-0 bg-gradient-to-r from-blue-900/60 to-purple-900/60 z-0 hidden dark:block" />
                <div className="absolute inset-0 bg-[url('/grid.svg')] opacity-20 z-0" />
                <div className="absolute inset-0 bg-gradient-to-t from-app-bg to-transparent z-10 hidden dark:block" />

                {/* Light Mode Gradient: Subtle fade */}
                <div className="absolute inset-0 bg-gradient-to-b from-white/0 to-slate-200/50 z-0 dark:hidden" />

                <div className="relative z-20 px-6 sm:px-8 pb-8 w-full max-w-4xl mx-auto flex flex-col sm:flex-row sm:justify-between sm:items-end gap-4">
                    <div className="min-w-0">
                        <h1 className="text-4xl lg:text-5xl font-black text-slate-900 dark:text-white tracking-tight leading-none mb-3 drop-shadow-sm dark:drop-shadow-2xl flex items-center gap-4 transition-colors">
                            <ShieldCheck className="text-blue-600 dark:text-blue-400 shrink-0" size={56} />
                            Settings
                        </h1>
                        <p className="text-lg text-slate-600 dark:text-white/70 font-medium max-w-prose break-words transition-colors">
                            Configure repositories, personalize your experience, and monitor system health.
                        </p>
                    </div>

                    {systemInfo && (
                        <div className="bg-white/50 dark:bg-white/10 backdrop-blur-md px-4 py-2 rounded-xl border border-slate-200 dark:border-white/10 text-right shadow-sm dark:shadow-none shrink-0">
                            <div className="text-sm font-bold text-slate-900 dark:text-white">{systemInfo.distro}</div>
                            <div className="text-xs text-slate-500 dark:text-white/50 font-mono mt-0.5">
                                {systemInfo.kernel} • {systemInfo.cpu_optimization}
                            </div>
                        </div>
                    )}
                </div>
            </div>

            <div className="flex-1 overflow-y-auto p-6 space-y-8 max-w-4xl mx-auto w-full">
                {/* System Health Dashboard */}
                <div className="grid grid-cols-1 sm:grid-cols-3 gap-6">
                    {/* 1. Global Connectivity */}
                    <div className="relative group">
                        <div className="absolute inset-0 bg-green-500/20 blur-3xl opacity-20 group-hover:opacity-40 transition-opacity duration-500" />
                        <div className="relative bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-xl p-6 h-full flex flex-col justify-between hover:bg-app-card/80 dark:hover:bg-white/10 transition-colors shadow-sm dark:shadow-none">
                            <div className="flex justify-between items-start mb-4">
                                <div className="p-3 bg-green-500/10 rounded-lg text-green-600 dark:text-green-400 w-10 h-10 flex items-center justify-center shrink-0">
                                    <Globe size={24} />
                                </div>
                                <div className="text-right">
                                    <div className="text-sm font-bold text-slate-900 dark:text-white">Online</div>
                                    <div className="text-[10px] text-slate-500 dark:text-white/50 font-mono tracking-wider">STATUS</div>
                                </div>
                            </div>
                            <div>
                                <div className="flex items-end justify-between mb-2">
                                    <span className="text-2xl font-black text-slate-900 dark:text-white">{infraStats?.latency || '45ms'}</span>
                                    <span className="text-xs text-green-600 dark:text-green-400 font-bold bg-green-500/10 dark:bg-green-500/20 px-2 py-1 rounded-lg flex items-center gap-1">
                                        <div className="w-1.5 h-1.5 bg-green-500 dark:bg-green-400 rounded-full animate-pulse" />
                                        Connected
                                    </span>
                                </div>
                                <div className="text-xs text-slate-500 dark:text-white/40 font-medium">
                                    {infraStats?.mirrors || 14} Active Mirrors
                                </div>
                            </div>
                        </div>
                    </div>

                    {/* 2. Sync Pipeline */}
                    <div className="relative group">
                        <div className="absolute inset-0 bg-blue-500/20 blur-3xl opacity-20 group-hover:opacity-40 transition-opacity duration-500" />
                        <div className="relative bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-xl p-6 h-full flex flex-col justify-between hover:bg-app-card/80 dark:hover:bg-white/10 transition-colors shadow-sm dark:shadow-none">
                            <div className="flex justify-between items-start mb-4">
                                <div className="p-3 bg-blue-500/10 rounded-lg text-blue-600 dark:text-blue-400 w-10 h-10 flex items-center justify-center shrink-0">
                                    <RefreshCw size={24} className={clsx(isSyncing && "animate-spin")} />
                                </div>
                                <div className="text-right">
                                    <div className="text-sm font-bold text-slate-900 dark:text-white">Sync</div>
                                    <div className="text-[10px] text-slate-500 dark:text-white/50 font-mono tracking-wider">PIPELINE</div>
                                </div>
                            </div>
                            <div>
                                <div className="flex items-end justify-between mb-2">
                                    <span className="text-2xl font-black text-slate-900 dark:text-white">
                                        {Object.values(repoCounts).reduce((a, b) => a + b, 0).toLocaleString()}
                                    </span>
                                    <span className="text-xs text-slate-500 dark:text-white/50 mb-1">Pkgs</span>
                                </div>
                                <div className="h-1 bg-slate-200 dark:bg-white/10 rounded-full overflow-hidden w-full">
                                    <div className={clsx("h-full bg-blue-500 transition-all duration-1000", isSyncing ? "w-full animate-pulse" : "w-2/3")} />
                                </div>
                                <div className="text-xs text-slate-500 dark:text-white/40 font-medium mt-2">
                                    {isSyncing ? "Syncing..." : "Up to date"}
                                </div>
                            </div>
                        </div>
                    </div>

                    {/* 3. Integrity */}
                    <div className="relative group">
                        <div className="absolute inset-0 bg-purple-500/20 blur-3xl opacity-20 group-hover:opacity-40 transition-opacity duration-500" />
                        <div className="relative bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-xl p-6 h-full flex flex-col justify-between hover:bg-app-card/80 dark:hover:bg-white/10 transition-colors shadow-sm dark:shadow-none">
                            <div className="flex justify-between items-start mb-4">
                                <div className="p-3 bg-purple-500/10 rounded-lg text-purple-600 dark:text-purple-400 w-10 h-10 flex items-center justify-center shrink-0">
                                    <ShieldCheck size={24} className={clsx(isOptimizing && "animate-bounce")} />
                                </div>
                                <div className="text-right">
                                    <div className="text-sm font-bold text-slate-900 dark:text-white">Health</div>
                                    <div className="text-[10px] text-slate-500 dark:text-white/50 font-mono tracking-wider">SYSTEM</div>
                                </div>
                            </div>
                            <div>
                                <div className="flex items-end justify-between mb-2">
                                    <span className="text-2xl font-black text-slate-900 dark:text-white">100%</span>
                                    <button
                                        onClick={handleOptimize}
                                        aria-label={isOptimizing ? "Running system health check" : "Run system health check"}
                                        className="text-xs text-purple-600 dark:text-purple-400 font-bold bg-purple-500/10 dark:bg-purple-500/20 px-2 py-1 rounded-lg hover:bg-purple-500/20 dark:hover:bg-purple-500/30 transition-colors focus:outline-none focus:ring-2 focus:ring-purple-500/50"
                                    >
                                        {isOptimizing ? "Running..." : "Run Check"}
                                    </button>
                                </div>
                                <div className="text-xs text-slate-500 dark:text-white/40 font-medium">
                                    System integrity verified
                                </div>
                            </div>
                        </div>
                    </div>
                </div>


                {/* Single-column layout at all sizes (medium-window look) */}
                <div className="flex flex-col gap-10">
                    <div className="space-y-10">
                        <RepositoriesTab
                            isSyncing={isSyncing}
                            syncProgressMessage={syncProgressMessage}
                            triggerManualSync={triggerManualSync}
                            repoCounts={repoCounts}
                            isAurEnabled={isAurEnabled}
                            toggleAur={toggleAur}
                            syncOnStartupEnabled={syncOnStartupEnabled}
                            setSyncOnStartup={setSyncOnStartup}
                            syncIntervalHours={syncIntervalHours}
                            updateSyncInterval={updateSyncInterval}
                            repos={repos}
                            toggleRepo={toggleRepo}
                            reorderRepos={reorderRepos}
                            repoSyncStatus={repoSyncStatus}
                            distro={distro}
                            isRepoLocked={isRepoLocked}
                            reportWarning={(msg) => errorService.reportWarning(msg)}
                            reportError={(msg) => errorService.reportError(msg)}
                        />
                        <HardwareOptimization
                            systemInfo={systemInfo}
                            repos={repos}
                            prioritizeOptimized={prioritizeOptimized}
                            onPrioritizeOptimized={handlePrioritizeOptimized}
                            parallelDownloads={parallelDownloads}
                            onParallelDownloads={handleParallelDownloads}
                            isRankingMirrors={isRankingMirrors}
                            onRankMirrors={handleMirrorRanking}
                            mirrorRankTool={mirrorRankTool}
                        />
                    </div>
                    <div className="space-y-10">
                        <SystemHealthSection />

                        {/* Workflow & Appearance: single column */}
                        <div className="space-y-10">
                        {/* Integration */}
                        <section className="flex flex-col h-full">
                            <h2 className="text-lg font-semibold tracking-tight text-app-fg mb-4 flex items-center gap-2">
                                <Palette size={20} className="text-app-muted" /> Workflow & Interface
                            </h2>
                            <div className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-xl p-6 space-y-6 flex-1 shadow-sm dark:shadow-none">
                                <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                                    <div className="min-w-0 flex-1 max-w-prose">
                                        <h3 className="text-lg font-semibold tracking-tight text-app-fg flex items-center gap-2">
                                            <Terminal size={16} className="text-app-muted shrink-0" /> Show Detailed Transaction Logs
                                        </h3>
                                        <p className="text-sm text-app-muted leading-relaxed mt-1 break-words">Expand the install modal to show real-time pacman/makepkg output (Glass Cockpit).</p>
                                    </div>
                                    <button
                                        type="button"
                                        role="switch"
                                        aria-checked={verboseLogsEnabled}
                                        aria-label="Toggle detailed transaction logs"
                                        onClick={() => setVerboseLogsEnabled(!verboseLogsEnabled)}
                                        className={clsx(
                                            "w-14 h-8 rounded-full p-1 transition-all shadow-lg focus:outline-none focus:ring-2 focus:ring-blue-500/50 shrink-0",
                                            verboseLogsEnabled ? "bg-blue-600" : "bg-slate-200 dark:bg-white/10"
                                        )}
                                    >
                                        <div className={clsx(
                                            "w-6 h-6 bg-white rounded-full transition-transform duration-300 shadow-md",
                                            verboseLogsEnabled ? "translate-x-6" : "translate-x-0"
                                        )} />
                                    </button>
                                </div>

                                <div className="h-px bg-slate-100 dark:bg-white/5 w-full" />

                                <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                                    <div className="min-w-0 flex-1 max-w-prose">
                                        <h3 className="font-bold text-slate-800 dark:text-white text-lg flex items-center gap-2">
                                            <Lock size={16} className="text-slate-500 dark:text-white/50 shrink-0" /> Reduce password prompts
                                        </h3>
                                        <p className="text-sm text-slate-500 dark:text-white/50 mt-1 break-words leading-relaxed">Use one password in MonARCH for this session (about 15 min). Not stored. Less secure than using the system prompt each time.</p>
                                    </div>
                                    <button
                                        type="button"
                                        role="switch"
                                        aria-checked={reducePasswordPrompts}
                                        aria-label={reducePasswordPrompts ? "Use system prompt each time" : "Reduce password prompts"}
                                        onClick={() => setReducePasswordPrompts(!reducePasswordPrompts)}
                                        className={clsx(
                                            "w-14 h-8 rounded-full p-1 transition-all shadow-lg focus:outline-none focus:ring-2 focus:ring-blue-500/50 shrink-0",
                                            reducePasswordPrompts ? "bg-amber-500" : "bg-slate-200 dark:bg-white/10"
                                        )}
                                    >
                                        <div className={clsx(
                                            "w-6 h-6 bg-white rounded-full transition-transform duration-300 shadow-md",
                                            reducePasswordPrompts ? "translate-x-6" : "translate-x-0"
                                        )} />
                                    </button>
                                </div>

                                <div className="h-px bg-slate-100 dark:bg-white/5 w-full" />

                                <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                                    <div className="min-w-0 flex-1 max-w-prose">
                                        <h3 className="text-lg font-semibold tracking-tight text-app-fg">Native Notifications</h3>
                                        <p className="text-sm text-app-muted leading-relaxed mt-1 break-words">Broadcast install completions to your desktop.</p>
                                    </div>
                                    <button
                                        type="button"
                                        role="switch"
                                        aria-checked={notificationsEnabled}
                                        aria-label={notificationsEnabled ? "Disable notifications" : "Enable notifications"}
                                        onClick={() => updateNotifications(!notificationsEnabled)}
                                        className={clsx(
                                            "w-14 h-8 rounded-full p-1 transition-all shadow-lg focus:outline-none focus:ring-2 focus:ring-blue-500/50 shrink-0",
                                            notificationsEnabled ? "bg-blue-600" : "bg-slate-200 dark:bg-white/10"
                                        )}
                                    >
                                        <div className={clsx(
                                            "w-6 h-6 bg-white rounded-full transition-transform duration-300 shadow-md",
                                            notificationsEnabled ? "translate-x-6" : "translate-x-0"
                                        )} />
                                    </button>
                                </div>

                                <div className="h-px bg-slate-100 dark:bg-white/5 w-full" />

                                <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                                    <div className="min-w-0 flex-1 max-w-prose">
                                        <h3 className="text-lg font-semibold tracking-tight text-app-fg flex items-center gap-2">
                                            <Sparkles size={16} className="text-blue-500 dark:text-blue-400 shrink-0" /> Initial Setup
                                        </h3>
                                        <p className="text-sm text-app-muted leading-relaxed mt-1 break-words">Re-run the welcome wizard to configure preferences.</p>
                                    </div>
                                    <button
                                        onClick={onRestartOnboarding}
                                        aria-label="Re-run onboarding wizard"
                                        className="h-10 px-6 rounded-xl bg-slate-100 hover:bg-slate-200 dark:bg-white/5 dark:hover:bg-white/10 text-slate-700 dark:text-white font-bold text-xs transition-all border border-slate-200 dark:border-white/10 active:scale-95 focus:outline-none focus:ring-2 focus:ring-blue-500/50"
                                    >
                                        Run Wizard
                                    </button>
                                </div>
                            </div>
                        </section>

                        {/* Appearance */}
                        <section className="flex flex-col h-full">
                            <h2 className="text-lg font-semibold tracking-tight text-app-fg mb-4 flex items-center gap-2">
                                <Palette size={20} className="text-app-muted" /> Appearance
                            </h2>
                            <div className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-xl p-6 transition-colors hover:bg-app-card/80 dark:hover:bg-white/10 space-y-6 flex-1 shadow-sm dark:shadow-none">
                                {/* Theme Mode */}
                                <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                                    <div className="min-w-0 max-w-prose">
                                        <h3 className="text-lg font-semibold tracking-tight text-app-fg">Interface Theme</h3>
                                        <p className="text-sm text-app-muted leading-relaxed break-words">Select your preferred brightness.</p>
                                    </div>
                                    <div className="flex bg-app-fg/5 p-1 rounded-lg border border-app-border shadow-inner min-w-[120px]">
                                        {(['system', 'light', 'dark'] as const).map((mode) => (
                                            <button
                                                key={mode}
                                                onClick={() => setThemeMode(mode)}
                                                aria-label={`Set theme to ${mode}`}
                                                aria-pressed={themeMode === mode}
                                                className={clsx(
                                                    "px-4 py-2 rounded-xl text-xs font-black transition-all focus:outline-none focus:ring-2 focus:ring-blue-500/50",
                                                    themeMode === mode
                                                        ? "bg-white text-black shadow-md dark:bg-white dark:text-black"
                                                        : "text-slate-400 hover:text-slate-900 dark:text-white/40 dark:hover:text-white"
                                                )}
                                            >
                                                {mode.toUpperCase()}
                                            </button>
                                        ))}
                                    </div>
                                </div>

                                <div className="h-px bg-slate-100 dark:bg-white/5 w-full" />

                                {/* Accents */}
                                <div className="flex gap-4 items-center overflow-x-auto pb-2">
                                    {(['#3b82f6', '#8b5cf6', '#10b981', '#f59e0b', '#ef4444']).map((color) => (
                                        <button
                                            key={color}
                                            onClick={() => setAccentColor(color)}
                                            aria-label={`Set accent color to ${color}`}
                                            aria-pressed={accentColor === color}
                                            className={clsx(
                                                "w-12 h-12 rounded-full border-4 transition-all relative flex-shrink-0 flex items-center justify-center focus:outline-none focus:ring-2 focus:ring-blue-500/50",
                                                accentColor === color ? "border-slate-200 dark:border-white scale-110 shadow-xl shadow-slate-400/20 dark:shadow-white/20" : "border-transparent opacity-40 hover:opacity-100 hover:scale-105"
                                            )}
                                            style={{ backgroundColor: color }}
                                        >
                                            {accentColor === color && (
                                                <CheckCircle2 size={20} className="text-white drop-shadow-md" />
                                            )}
                                        </button>
                                    ))}
                                    <div className="ml-2 border-l border-slate-200 dark:border-white/10 pl-4">
                                        <h3 className="font-bold text-slate-800 dark:text-white text-sm">Accent</h3>
                                        <p className="text-xs text-slate-400 dark:text-white/40">Color</p>
                                    </div>
                                </div>
                            </div>
                        </section>
                    </div>

                    {/* System Management (Consolidated) */}
                    <section id="system-health">
                        <h2 className="text-lg font-semibold tracking-tight text-app-fg mb-4 flex items-center gap-2">
                            <ShieldCheck size={20} className="text-app-muted" />
                            System Management
                        </h2>

                        <div className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-xl p-6 space-y-6 transition-colors hover:bg-app-card/80 dark:hover:bg-white/10 shadow-sm dark:shadow-none">
                            {/* Security Control */}
                            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-6">
                                <div className="min-w-0 flex-1 max-w-prose">
                                    <h3 className="font-bold text-slate-800 dark:text-white text-xl mb-2 flex items-center gap-2">
                                        <Lock size={20} className="text-emerald-500 dark:text-emerald-400 shrink-0" />
                                        One-Click Authentication
                                    </h3>
                                    <p className="text-slate-500 dark:text-white/60 text-base leading-relaxed break-words">
                                        Allow MonARCH to perform system maintenance and app installs without asking for a password every time. This modifies Polkit security policies.
                                    </p>
                                </div>
                                <button
                                    onClick={() => {
                                        const newVal = !oneClickEnabled;
                                        updateOneClick(newVal).then(() => {
                                            if (newVal) success("One-Click Control Enabled");
                                            else success("One-Click Control Disabled");
                                        });
                                    }}
                                    className={clsx(
                                        "w-16 h-9 rounded-full p-1 transition-all shadow-xl shrink-0",
                                        oneClickEnabled ? "bg-emerald-500 shadow-emerald-500/20" : "bg-slate-200 dark:bg-white/10"
                                    )}
                                >
                                    <div className={clsx(
                                        "w-7 h-7 bg-white rounded-full transition-transform duration-300 shadow-lg",
                                        oneClickEnabled ? "translate-x-7" : "translate-x-0"
                                    )} />
                                </button>
                            </div>

                            <div className="h-px bg-slate-200 dark:bg-white/5 w-full" />

                            {/* Fix My System (one-click) + Advanced Repair dropdown */}
                            <div>
                                <h3 className="text-sm font-black uppercase tracking-widest text-slate-400 dark:text-white/40 mb-4 px-2">Maintenance & Repair Tools</h3>
                                <div className="flex flex-col gap-4">
                                    <div className="flex flex-wrap items-center gap-3">
                                        <button
                                            onClick={handleForceRefreshDatabases}
                                            disabled={isRepairing === 'refresh_db'}
                                            className="inline-flex items-center gap-2 px-6 py-3 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-white font-bold text-sm shadow-lg disabled:opacity-50 transition-all focus:outline-none focus:ring-2 focus:ring-emerald-500/50"
                                        >
                                            <ShieldCheck size={18} />
                                            Fix My System
                                        </button>
                                        <button
                                            type="button"
                                            onClick={() => setAdvancedRepairOpen(!advancedRepairOpen)}
                                            className="inline-flex items-center gap-2 px-4 py-2 rounded-xl border border-slate-200 dark:border-white/10 bg-white/50 dark:bg-white/5 text-slate-700 dark:text-white font-medium text-sm hover:bg-slate-100 dark:hover:bg-white/10 transition-all focus:outline-none focus:ring-2 focus:ring-blue-500/50"
                                        >
                                            Advanced Repair
                                            <ChevronDown size={16} className={clsx(advancedRepairOpen && "rotate-180 transition-transform")} />
                                        </button>
                                    </div>
                                    {advancedRepairOpen && (
                                        <div className="relative z-30 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-4 pt-2 border-t border-app-border">
                                    <RepairButton
                                        icon={<Lock className="text-blue-600 dark:text-blue-400" />}
                                        title="Unlock Database"
                                        desc="Clear stuck locks from the package manager."
                                        onClick={() => handleRepairTask('unlock')}
                                        loading={isRepairing === 'unlock'}
                                    />
                                    <RepairButton
                                        icon={<ShieldCheck className="text-purple-600 dark:text-purple-400" />}
                                        title="Fix System Keys"
                                        desc="Repair signatures and security keyring."
                                        onClick={() => handleRepairTask('keyring')}
                                        loading={isRepairing === 'keyring'}
                                    />
                                    <RepairButton
                                        icon={<RefreshCw className="text-sky-600 dark:text-sky-400" />}
                                        title="Refresh Databases"
                                        desc="Force re-download of pacman sync databases."
                                        onClick={handleForceRefreshDatabases}
                                        loading={isRepairing === 'refresh_db'}
                                    />
                                    <RepairButton
                                        icon={<Trash2 className="text-red-600 dark:text-red-400" />}
                                        title="Clear Cache"
                                        desc="Clear app caches and pacman package cache on disk."
                                        onClick={handleClearCache}
                                        loading={isOptimizing}
                                    />
                                    <RepairButton
                                        icon={<Package size={18} className="text-amber-600 dark:text-amber-400" />}
                                        title="Clean Orphans"
                                        desc="Remove unused system dependencies."
                                        onClick={handleOrphans}
                                        loading={isRepairing === 'orphans'}
                                    />
                                        </div>
                                    )}
                                </div>
                            </div>
                        </div>
                    </section>


                    {/* Privacy Control */}
                    <section className="pt-8 border-t border-app-border">
                        <h2 className="text-lg font-semibold tracking-tight text-app-fg mb-4 flex items-center gap-2">
                            <Activity size={20} className="text-app-muted" />
                            Privacy & Data
                        </h2>
                        <div className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-xl p-6 flex flex-col md:flex-row items-center justify-between gap-6 transition-colors hover:bg-app-card/80 dark:hover:bg-white/10 shadow-sm dark:shadow-none">
                            <div className="min-w-0 flex-1 max-w-prose">
                                <h3 className="text-lg font-semibold tracking-tight text-app-fg mb-2 flex items-center gap-2">
                                    Anonymous Telemetry
                                </h3>
                                <p className="text-sm text-app-muted leading-relaxed break-words">
                                    Help us improve MonARCH by sharing anonymous usage statistics (search trends, install success rates).
                                    <br />
                                    <span className="text-xs font-bold uppercase tracking-wider text-slate-400 dark:text-white/40 mt-1 block">We never collect personal data.</span>
                                </p>
                            </div>
                            <button
                                onClick={handleToggle}
                                className={clsx(
                                    "w-16 h-9 rounded-full p-1 transition-all shadow-xl shrink-0 flex items-center justify-start relative",
                                    localToggle ? "bg-teal-500 shadow-teal-500/20" : "bg-slate-400 dark:bg-white/20"
                                )}
                            >
                                <div
                                    className={clsx(
                                        "z-10 w-7 h-7 bg-white rounded-full transition-transform duration-300 shadow-lg flex items-center justify-center pointer-events-none"
                                    )}
                                    style={{ transform: localToggle ? 'translateX(28px)' : 'translateX(0px)' }}
                                >
                                    {/* Subtle indicator dot */}
                                    <div className={clsx("w-1.5 h-1.5 rounded-full", localToggle ? "bg-teal-500" : "bg-slate-400")} />
                                </div>
                                <span className={clsx(
                                    "absolute text-[9px] font-black tracking-tighter transition-opacity duration-300 pointer-events-none",
                                    localToggle ? "left-2 opacity-100 text-white" : "left-2 opacity-0"
                                )}>ON</span>
                                <span className={clsx(
                                    "absolute text-[9px] font-black tracking-tighter transition-opacity duration-300 pointer-events-none",
                                    localToggle ? "right-2 opacity-0" : "right-2 opacity-100 text-white/80"
                                )}>OFF</span>
                            </button>
                        </div>
                    </section>


                    {/* DANGER ZONE: Advanced Configuration */}
                    <section className="pt-8 border-t border-app-border">
                        <div className="bg-red-50 dark:bg-red-500/5 border border-red-500/20 rounded-xl p-6 relative overflow-hidden">
                            {/* Background Warning Stripes */}
                            <div className="absolute inset-0 bg-[repeating-linear-gradient(45deg,transparent,transparent_10px,rgba(239,68,68,0.05)_10px,rgba(239,68,68,0.05)_20px)] pointer-events-none" />

                            <div className="relative z-10 flex flex-col md:flex-row items-center justify-between gap-8">
                                <div>
                                    <h3 className="font-bold text-red-600 dark:text-red-400 text-xl mb-2 flex items-center gap-2">
                                        <AlertTriangle size={24} />
                                        Advanced Configuration
                                    </h3>
                                    <p className="text-slate-600 dark:text-white/60 text-base max-w-2xl leading-relaxed">
                                        Enable "God Mode" to bypass repository safety checks.
                                        <br />
                                        <span className="text-red-600 dark:text-red-400 font-bold">WARNING:</span> This allows installing incompatible packages (e.g., Manjaro repos on Arch) which can result in <strong className="text-slate-900 dark:text-white">partial upgrades and system breakage</strong>.
                                    </p>
                                </div>
                                <div className="flex items-center gap-4">
                                    <span className={clsx("text-xs font-bold uppercase tracking-wider", advancedMode ? "text-red-600 dark:text-red-400" : "text-slate-400 dark:text-white/30")}>
                                        {advancedMode ? "UNLOCKED" : "SAFE MODE"}
                                    </span>
                                    <button
                                        onClick={() => {
                                            if (!advancedMode) {
                                                setModalConfig({
                                                    isOpen: true,
                                                    title: "Enable Advanced Repository Mode?",
                                                    message: "⚠ CRITICAL WARNING ⚠\n\nYou are about to disable Distro-Safety Locks.\n\n• Manjaro Users: Enabling Arch/Chaotic repos will cause glibc mismatches.\n• Arch Users: Manjaro repos may downgrade critical system packages.\n\nOnly proceed if you are an expert capable of recovering a broken system.",
                                                    variant: 'danger',
                                                    onConfirm: () => {
                                                        toggleAdvancedMode(true);
                                                        success("Advanced Mode Enabled. Safety locks removed.");
                                                    }
                                                });
                                            } else {
                                                toggleAdvancedMode(false);
                                                success("Safety locks re-engaged.");
                                            }
                                        }}
                                        className={clsx(
                                            "w-16 h-9 rounded-full p-1 transition-all shadow-xl shrink-0 border",
                                            advancedMode
                                                ? "bg-red-600 border-red-500 shadow-red-600/20"
                                                : "bg-slate-200 dark:bg-black/40 border-slate-300 dark:border-white/10"
                                        )}
                                    >
                                        <div className={clsx(
                                            "w-7 h-7 bg-white rounded-full transition-transform duration-300 shadow-lg flex items-center justify-center text-red-600",
                                            advancedMode ? "translate-x-7" : "translate-x-0"
                                        )}>
                                            {advancedMode && <Lock size={12} className="text-red-600" />}
                                        </div>
                                    </button>
                                </div>
                            </div>
                        </div>
                    </section>

                    {/* About MonARCH & Updates */}
                    <section className="pt-12 border-t border-slate-200 dark:border-white/5">
                        <div className="bg-white dark:bg-white/5 backdrop-blur-xl border border-slate-200 dark:border-white/10 rounded-3xl p-8 flex flex-col md:flex-row items-center justify-between gap-8">
                            <div className="flex items-center gap-6">
                                <div className="w-16 h-16 bg-blue-600 dark:bg-blue-500 rounded-2xl flex items-center justify-center shadow-lg shadow-blue-500/20">
                                    <div className="w-10 h-10 border-4 border-white rounded-full flex items-center justify-center font-black text-white text-xl">M</div>
                                </div>
                                <div>
                                    <h3 className="font-bold text-slate-800 dark:text-white text-xl">MonARCH Store</h3>
                                    <div className="flex items-center gap-2 mt-1">
                                        <span className="text-xs font-mono text-slate-500 dark:text-white/40">v{pkgVersion}</span>
                                        <div className="w-1 h-1 bg-slate-300 dark:bg-white/20 rounded-full" />
                                        {installMode === 'system' ? (
                                            <span className="flex items-center gap-1.5 px-2 py-0.5 bg-blue-100 dark:bg-blue-500/20 text-blue-700 dark:text-blue-400 text-[10px] font-bold rounded-md uppercase tracking-wider border border-blue-200 dark:border-blue-500/30">
                                                <Package size={10} />
                                                Managed by Pacman
                                            </span>
                                        ) : (
                                            <span className="flex items-center gap-1.5 px-2 py-0.5 bg-purple-100 dark:bg-purple-500/20 text-purple-700 dark:text-purple-400 text-[10px] font-bold rounded-md uppercase tracking-wider border border-purple-200 dark:border-purple-500/30">
                                                <Rocket size={10} />
                                                Portable AppImage
                                            </span>
                                        )}
                                    </div>
                                </div>
                            </div>

                            <button
                                onClick={async () => {
                                    if (installMode === 'system') {
                                        // System mode: Point towards system update or just inform
                                        setModalConfig({
                                            isOpen: true,
                                            title: "System Update Check",
                                            message: "MonARCH is installed as a system package. To update, please use the global 'Perform System Update' action or run 'pacman -Syu' in a terminal.",
                                            variant: 'info',
                                            onConfirm: () => {
                                                // Could potentially trigger perform_system_update here
                                            }
                                        });
                                    } else {
                                        // Portable mode: Handle via Tauri's updater
                                        success("Checking for application updates...");
                                        // This would normally call tauri-plugin-updater logic
                                    }
                                }}
                                className="px-6 py-2.5 bg-slate-900 dark:bg-white text-white dark:text-slate-900 rounded-lg font-bold text-sm hover:scale-105 transition-transform flex items-center gap-2 "
                            >
                                <RefreshCw size={16} />
                                {installMode === 'system' ? 'Check for System Updates' : 'Check for App Updates'}
                            </button>
                        </div>
                    </section>

                    <div className="text-center text-slate-400 dark:text-white/20 text-[10px] pt-12 pb-8 opacity-50 font-medium">
                        MonARCH Store • Licensed under MIT • Powered by Chaotic-AUR
                    </div>
                    </div>
                </div>
            </div>

            <ConfirmationModal
                isOpen={modalConfig.isOpen}
                onClose={() => setModalConfig({ ...modalConfig, isOpen: false })}
                onConfirm={modalConfig.onConfirm}
                title={modalConfig.title}
                message={modalConfig.message}
                variant={modalConfig.variant}
            />
        </div >
    );
}

function RepairButton({ icon, title, desc, onClick, loading }: { icon: React.ReactNode, title: string, desc: string, onClick: () => void, loading: boolean }) {
    return (
        <button
            onClick={onClick}
            disabled={loading}
            className="group p-6 bg-app-card/50 dark:bg-white/5 rounded-xl border border-app-border hover:border-app-fg/20 hover:bg-app-card/80 dark:hover:bg-white/10 transition-all text-left flex flex-col gap-3 shadow-sm hover:shadow-lg disabled:opacity-50"
        >
            <div className="p-3 bg-app-fg/5 rounded-lg w-10 h-10 flex items-center justify-center group-hover:scale-110 transition-transform shrink-0 overflow-hidden">
                {loading ? <RefreshCw className="animate-spin text-slate-500 dark:text-white" size={20} /> : icon}
            </div>
            <div>
                <h4 className="font-bold text-slate-800 dark:text-white group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors text-sm">{title}</h4>
                <p className="text-[10px] text-slate-500 dark:text-white/40 leading-relaxed mt-1">{desc}</p>
            </div>
        </button>
    );
}
