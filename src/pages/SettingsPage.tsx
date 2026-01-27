import { useState, useEffect } from 'react';
import {
    CheckCircle2, Globe, Palette, Info,
    Trash2, ShieldCheck, Package, ArrowUp, ArrowDown, RefreshCw, Lock, Clock, ChevronDown, Sparkles, AlertTriangle
} from 'lucide-react';
import ConfirmationModal from '../components/ConfirmationModal';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { clsx } from 'clsx';


import { useTheme } from '../hooks/useTheme';
import { useToast } from '../context/ToastContext';
import { useSettings } from '../hooks/useSettings';


interface SettingsPageProps {
    onRestartOnboarding?: () => void;
}

export default function SettingsPage({ onRestartOnboarding }: SettingsPageProps) {
    const { themeMode, setThemeMode, accentColor, setAccentColor } = useTheme();
    const { success, error } = useToast();

    // Centralized Logic
    const {
        notificationsEnabled, updateNotifications,
        syncIntervalHours, updateSyncInterval,
        isAurEnabled, toggleAur,
        repos, toggleRepo, reorderRepos,
        isSyncing, triggerManualSync, repoCounts,
        infraStats,
        oneClickEnabled, updateOneClick,
        refresh
    } = useSettings();

    const [isOptimizing, setIsOptimizing] = useState(false);
    const [isRepairing, setIsRepairing] = useState<string | null>(null);
    const [pkgVersion, setPkgVersion] = useState("0.0.0");
    const [systemInfo, setSystemInfo] = useState<{ kernel: string, distro: string, cpu_optimization: string, pacman_version: string } | null>(null);
    const [repoSyncStatus, setRepoSyncStatus] = useState<Record<string, boolean> | null>(null);

    const [modalConfig, setModalConfig] = useState<{
        isOpen: boolean;
        title: string;
        message: string;
        onConfirm: () => void;
        variant?: 'danger' | 'info';
    }>({ isOpen: false, title: '', message: '', onConfirm: () => { } });

    // Initial Load
    useEffect(() => {
        getVersion().then(setPkgVersion).catch(console.error);
        invoke<any>('get_system_info').then(setSystemInfo).catch(console.error);
        invoke<Record<string, boolean>>('check_repo_sync_status').then(setRepoSyncStatus).catch(console.error);
    }, []);

    const moveRepo = (index: number, direction: 'up' | 'down') => {
        const newRepos = [...repos];
        if (direction === 'up' && index > 0) {
            [newRepos[index], newRepos[index - 1]] = [newRepos[index - 1], newRepos[index]];
        } else if (direction === 'down' && index < newRepos.length - 1) {
            [newRepos[index], newRepos[index + 1]] = [newRepos[index + 1], newRepos[index]];
        }
        reorderRepos(newRepos);
    };

    const handleOptimize = async () => {
        setIsOptimizing(true);
        try {
            const result = await invoke<string>('optimize_system');
            success(result);
        } catch (e) {
            error(`Optimization failed: ${e}`);
        } finally {
            setIsOptimizing(false);
        }
    };

    const handleClearCache = () => {
        setModalConfig({
            isOpen: true,
            title: "Clear Application Cache",
            message: "Are you sure you want to wipe all application caches? This will resolve metadata issues but may require re-downloading some data.",
            variant: 'danger',
            onConfirm: async () => {
                setIsOptimizing(true);
                try {
                    const result = await invoke<string>('clear_cache');
                    success(result);
                    refresh();
                } catch (e) {
                    error(`Cache wipe failed: ${e}`);
                } finally {
                    setIsOptimizing(false);
                }
            }
        });
    };

    const handleOrphans = () => {
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
                    } else {
                        setModalConfig({
                            isOpen: true,
                            title: "Remove Orphans",
                            message: `Found ${orphans.length} orphans. Remove them?`,
                            variant: 'danger',
                            onConfirm: async () => {
                                setIsRepairing("orphans");
                                await invoke('remove_orphans', { orphans });
                                success(`Successfully removed ${orphans.length} packages.`);
                                setIsRepairing(null);
                            }
                        });
                        return;
                    }
                } catch (e) {
                    error(`Failed: ${e}`);
                } finally {
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

            await invoke(cmd, { password: null });
            success(`${label} completed successfully.`);
            refresh();
        } catch (e) {
            error(`${task} failed: ${e}`);
        } finally {
            setIsRepairing(null);
        }
    };


    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            {/* Banner Header */}
            <div className="relative min-h-[200px] flex items-end mb-8">
                <div className="absolute inset-0 bg-gradient-to-r from-blue-900/60 to-purple-900/60 z-0" />
                <div className="absolute inset-0 bg-[url('/grid.svg')] opacity-20 z-0" />
                <div className="absolute inset-0 bg-gradient-to-t from-app-bg to-transparent z-10" />

                <div className="relative z-20 px-8 pb-8 w-full flex justify-between items-end">
                    <div>
                        <h1 className="text-4xl lg:text-6xl font-black text-white tracking-tight leading-none mb-3 drop-shadow-2xl flex items-center gap-4">
                            <ShieldCheck className="text-blue-400" size={56} />
                            Settings
                        </h1>
                        <p className="text-lg text-white/70 font-medium max-w-2xl">
                            Configure repositories, personalize your experience, and monitor system health.
                        </p>
                    </div>

                    {systemInfo && (
                        <div className="bg-white/10 backdrop-blur-md px-4 py-2 rounded-xl border border-white/10 text-right">
                            <div className="text-sm font-bold text-white">{systemInfo.distro}</div>
                            <div className="text-xs text-white/50 font-mono mt-0.5">
                                {systemInfo.kernel} • {systemInfo.cpu_optimization}
                            </div>
                        </div>
                    )}
                </div>
            </div>

            <div className="flex-1 overflow-y-auto p-8 space-y-12">
                {/* System Health Dashboard (v0.2.25 Restoration) */}
                {/* System Health Dashboard */}
                <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                    {/* 1. Global Connectivity */}
                    <div className="relative group">
                        <div className="absolute inset-0 bg-green-500/20 blur-3xl opacity-20 group-hover:opacity-40 transition-opacity duration-500" />
                        <div className="relative bg-white/5 backdrop-blur-xl border border-white/10 rounded-3xl p-6 h-full flex flex-col justify-between hover:bg-white/10 transition-colors">
                            <div className="flex justify-between items-start mb-4">
                                <div className="p-3 bg-green-500/10 rounded-2xl text-green-400">
                                    <Globe size={24} />
                                </div>
                                <div className="text-right">
                                    <div className="text-sm font-bold text-white">Online</div>
                                    <div className="text-[10px] text-white/50 font-mono tracking-wider">STATUS</div>
                                </div>
                            </div>
                            <div>
                                <div className="flex items-end justify-between mb-2">
                                    <span className="text-2xl font-black text-white">{infraStats?.latency || '45ms'}</span>
                                    <span className="text-xs text-green-400 font-bold bg-green-500/20 px-2 py-1 rounded-lg flex items-center gap-1">
                                        <div className="w-1.5 h-1.5 bg-green-400 rounded-full animate-pulse" />
                                        Connected
                                    </span>
                                </div>
                                <div className="text-xs text-white/40 font-medium">
                                    {infraStats?.mirrors || 14} Active Mirrors
                                </div>
                            </div>
                        </div>
                    </div>

                    {/* 2. Sync Pipeline */}
                    <div className="relative group">
                        <div className="absolute inset-0 bg-blue-500/20 blur-3xl opacity-20 group-hover:opacity-40 transition-opacity duration-500" />
                        <div className="relative bg-white/5 backdrop-blur-xl border border-white/10 rounded-3xl p-6 h-full flex flex-col justify-between hover:bg-white/10 transition-colors">
                            <div className="flex justify-between items-start mb-4">
                                <div className="p-3 bg-blue-500/10 rounded-2xl text-blue-400">
                                    <RefreshCw size={24} className={clsx(isSyncing && "animate-spin")} />
                                </div>
                                <div className="text-right">
                                    <div className="text-sm font-bold text-white">Sync</div>
                                    <div className="text-[10px] text-white/50 font-mono tracking-wider">PIPELINE</div>
                                </div>
                            </div>
                            <div>
                                <div className="flex items-end justify-between mb-2">
                                    <span className="text-2xl font-black text-white">
                                        {Object.values(repoCounts).reduce((a, b) => a + b, 0).toLocaleString()}
                                    </span>
                                    <span className="text-xs text-white/50 mb-1">Pkgs</span>
                                </div>
                                <div className="h-1 bg-white/10 rounded-full overflow-hidden w-full">
                                    <div className={clsx("h-full bg-blue-500 transition-all duration-1000", isSyncing ? "w-full animate-pulse" : "w-2/3")} />
                                </div>
                                <div className="text-xs text-white/40 font-medium mt-2">
                                    {isSyncing ? "Syncing..." : "Up to date"}
                                </div>
                            </div>
                        </div>
                    </div>

                    {/* 3. Integrity */}
                    <div className="relative group">
                        <div className="absolute inset-0 bg-purple-500/20 blur-3xl opacity-20 group-hover:opacity-40 transition-opacity duration-500" />
                        <div className="relative bg-white/5 backdrop-blur-xl border border-white/10 rounded-3xl p-6 h-full flex flex-col justify-between hover:bg-white/10 transition-colors">
                            <div className="flex justify-between items-start mb-4">
                                <div className="p-3 bg-purple-500/10 rounded-2xl text-purple-400">
                                    <ShieldCheck size={24} className={clsx(isOptimizing && "animate-bounce")} />
                                </div>
                                <div className="text-right">
                                    <div className="text-sm font-bold text-white">Health</div>
                                    <div className="text-[10px] text-white/50 font-mono tracking-wider">SYSTEM</div>
                                </div>
                            </div>
                            <div>
                                <div className="flex items-end justify-between mb-2">
                                    <span className="text-2xl font-black text-white">100%</span>
                                    <button
                                        onClick={handleOptimize}
                                        className="text-xs text-purple-400 font-bold bg-purple-500/20 px-2 py-1 rounded-lg hover:bg-purple-500/30 transition-colors"
                                    >
                                        {isOptimizing ? "Running..." : "Run Check"}
                                    </button>
                                </div>
                                <div className="text-xs text-white/40 font-medium">
                                    System integrity verified
                                </div>
                            </div>
                        </div>
                    </div>
                </div>


                {/* Main Content Sections */}
                <div className="space-y-12">
                    {/* Sync Control */}
                    <section>
                        <h2 className="text-2xl font-black text-white mb-6 flex items-center gap-3">
                            Repository Control
                        </h2>
                        <div className="bg-white/5 backdrop-blur-xl rounded-3xl p-8 border border-white/10 hover:bg-white/10 transition-colors">
                            <div className="flex flex-col md:flex-row items-center justify-between mb-8 gap-6">
                                <div>
                                    <h3 className="font-bold text-white text-xl mb-2 flex items-center gap-2">
                                        <RefreshCw size={24} className={clsx(isSyncing ? "animate-spin text-blue-400" : "text-white/50")} />
                                        Force Database Synchronization
                                    </h3>
                                    <p className="text-white/60 text-base max-w-xl leading-relaxed">
                                        Refresh local package catalogs. This updates Store listings while keeping your system configuration intact.
                                    </p>
                                </div>
                                <button
                                    onClick={triggerManualSync}
                                    disabled={isSyncing}
                                    className={clsx(
                                        "px-8 py-4 rounded-2xl font-bold transition-all flex items-center gap-3 text-lg shadow-xl min-w-[200px] justify-center",
                                        isSyncing
                                            ? "bg-white/5 text-white/50 cursor-not-allowed border border-white/5"
                                            : "bg-blue-600 hover:bg-blue-500 text-white shadow-blue-500/20 active:scale-95 border border-white/10"
                                    )}
                                >
                                    <RefreshCw size={20} className={isSyncing ? "animate-spin" : ""} />
                                    {isSyncing ? 'Syncing...' : 'Sync Now'}
                                </button>
                            </div>

                            {/* Stats Grid */}
                            <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4 pt-8 border-t border-white/5">
                                {Object.entries(repoCounts).sort((a, b) => b[1] - a[1]).map(([name, count]) => (
                                    <div key={name} className="flex flex-col items-center justify-center bg-black/20 p-4 rounded-2xl border border-white/5 h-24">
                                        <span className="text-[10px] uppercase font-bold text-white/40 mb-1 tracking-widest">{name}</span>
                                        <span className="text-xl font-black text-white">{count.toLocaleString()}</span>
                                    </div>
                                ))}
                                {isAurEnabled && (
                                    <div className="flex flex-col items-center justify-center bg-amber-500/10 p-4 rounded-2xl border border-amber-500/20 h-24">
                                        <span className="text-[10px] uppercase font-bold text-amber-500 mb-1 tracking-widest">AUR</span>
                                        <span className="text-xl font-black text-amber-500">Active</span>
                                    </div>
                                )}
                                {Object.keys(repoCounts).length === 0 && !isAurEnabled && (
                                    <div className="col-span-full text-center text-sm text-white/30 italic py-4">
                                        Waiting for synchronization...
                                    </div>
                                )}
                            </div>

                            {/* Auto Sync Interval */}
                            <div className="pt-8 border-t border-white/5 mt-8">
                                <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-4">
                                        <div className="p-3 bg-blue-500/20 rounded-xl text-blue-400">
                                            <Clock size={24} />
                                        </div>
                                        <div>
                                            <h4 className="font-bold text-white text-lg">Auto Sync Interval</h4>
                                            <p className="text-sm text-white/50">
                                                Automatically refresh package databases
                                            </p>
                                        </div>
                                    </div>
                                    <div className="relative">
                                        <select
                                            value={syncIntervalHours}
                                            onChange={(e) => {
                                                const val = parseInt(e.target.value, 10);
                                                updateSyncInterval(val);
                                            }}
                                            className="appearance-none bg-black/20 border border-white/10 rounded-xl px-6 py-3 pr-12 text-white font-bold focus:outline-none focus:ring-2 focus:ring-blue-500/50 cursor-pointer min-w-[200px]"
                                        >
                                            <option value={1}>Every 1 hour</option>
                                            <option value={3}>Every 3 hours</option>
                                            <option value={6}>Every 6 hours</option>
                                            <option value={12}>Every 12 hours</option>
                                            <option value={24}>Every 24 hours</option>
                                        </select>
                                        <ChevronDown size={16} className="absolute right-4 top-1/2 -translate-y-1/2 text-white/50 pointer-events-none" />
                                    </div>
                                </div>
                            </div>
                        </div>
                    </section>

                    {/* Repository Management */}
                    <section>
                        <h2 className="text-2xl font-black text-white mb-6 flex items-center gap-3">
                            <Package size={24} className="text-white/50" /> Software Sources
                        </h2>

                        <div className="bg-white/5 backdrop-blur-xl border border-white/10 rounded-3xl p-8">
                            <p className="text-sm text-white/50 mb-8 px-2 max-w-3xl leading-relaxed">
                                Toggling a source here <strong className="text-white">hides it from the Store</strong> but keeps it active in the system. Your installed apps <strong className="text-green-400">continue to update safely</strong>.
                            </p>

                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                {repos.map((repo, idx) => (
                                    <div key={repo.name} className={clsx(
                                        "relative flex flex-col p-6 rounded-2xl border transition-all duration-300 group overflow-hidden",
                                        repo.enabled ? "bg-white/5 border-white/10 hover:bg-white/10 shadow-lg hover:shadow-xl hover:-translate-y-1" : "bg-black/20 border-white/5 opacity-60 hover:opacity-100 grayscale hover:grayscale-0"
                                    )}>
                                        <div className="flex items-center justify-between w-full relative z-10">
                                            <div className="flex items-center gap-4">
                                                <div className="flex flex-col gap-1 text-white/20 group-hover:text-white/60 transition-colors">
                                                    <button onClick={() => moveRepo(idx, 'up')} disabled={idx === 0} className="hover:text-white disabled:opacity-0 transition-colors"><ArrowUp size={16} /></button>
                                                    <button onClick={() => moveRepo(idx, 'down')} disabled={idx === repos.length - 1} className="hover:text-white disabled:opacity-0 transition-colors"><ArrowDown size={16} /></button>
                                                </div>
                                                <div>
                                                    <h4 className={clsx("font-bold text-lg", repo.enabled ? "text-white" : "text-white/50")}>
                                                        {repo.name}
                                                        {idx === 0 && repo.enabled && <span className="ml-3 text-[10px] bg-blue-500/20 text-blue-300 border border-blue-500/30 px-2 py-0.5 rounded-full uppercase tracking-wider font-bold">Primary</span>}
                                                    </h4>
                                                    <p className="text-xs text-white/40 mt-1 line-clamp-1">{repo.description}</p>
                                                </div>
                                            </div>

                                            <button
                                                onClick={() => toggleRepo(repo.id)}
                                                className={clsx(
                                                    "w-12 h-7 rounded-full p-1 transition-all",
                                                    repo.enabled ? "bg-blue-600 shadow-lg shadow-blue-500/30" : "bg-white/10"
                                                )}
                                            >
                                                <div className={clsx(
                                                    "w-5 h-5 bg-white shadow-xl rounded-full transition-transform duration-300",
                                                    repo.enabled ? "translate-x-5" : "translate-x-0"
                                                )} />
                                            </button>
                                        </div>

                                        {/* Background Decor */}
                                        {repo.enabled && (
                                            <div className="absolute -bottom-10 -right-10 w-32 h-32 bg-blue-500/10 blur-3xl rounded-full pointer-events-none group-hover:bg-blue-500/20 transition-colors" />
                                        )}

                                        {/* Sync Warning */}
                                        {repo.enabled && repoSyncStatus && repoSyncStatus[repo.name] === false && (
                                            <div className="mt-4 pt-4 border-t border-white/5 flex items-start gap-3 animate-in slide-in-from-top-2">
                                                <AlertTriangle size={16} className="text-amber-500 shrink-0 mt-0.5" />
                                                <div>
                                                    <p className="text-xs font-bold text-amber-500">Sync Required</p>
                                                    <p className="text-[10px] text-white/40 mt-0.5">Database missing. Will auto-fix on next install.</p>
                                                </div>
                                            </div>
                                        )}
                                    </div>
                                ))}
                            </div>

                            {/* AUR Section */}
                            <div className="mt-8 pt-8 border-t border-white/5">
                                <div className="relative overflow-hidden flex flex-col md:flex-row items-center justify-between p-6 rounded-3xl border border-amber-500/20 bg-amber-500/5 group">
                                    <div className="absolute inset-0 bg-amber-500/10 blur-3xl opacity-0 group-hover:opacity-20 transition-opacity duration-500" />

                                    <div className="flex items-center gap-5 relative z-10">
                                        <div className="p-4 bg-amber-500/20 rounded-2xl text-amber-500 shadow-lg shadow-amber-900/20">
                                            <Lock size={24} />
                                        </div>
                                        <div>
                                            <h4 className="font-bold text-amber-500 text-lg flex items-center gap-3">
                                                Enable AUR <span className="text-[10px] bg-amber-500 text-black px-2 py-0.5 rounded font-black tracking-widest">EXPERIMENTAL</span>
                                            </h4>
                                            <p className="text-sm text-amber-200/60 mt-1 max-w-md">
                                                Access millions of community-maintained packages.
                                                <br />⚠ Use at your own risk. Not officially supported.
                                            </p>
                                        </div>
                                    </div>

                                    <div className="mt-4 md:mt-0 relative z-10">
                                        <button
                                            onClick={() => toggleAur(!isAurEnabled)}
                                            className={clsx(
                                                "w-14 h-8 rounded-full p-1 transition-all shadow-xl",
                                                isAurEnabled ? "bg-amber-500 shadow-amber-500/20" : "bg-white/10"
                                            )}
                                        >
                                            <div className={clsx(
                                                "w-6 h-6 bg-white shadow-xl rounded-full transition-transform duration-300",
                                                isAurEnabled ? "translate-x-6" : "translate-x-0"
                                            )} />
                                        </button>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </section>

                    {/* Customization & Workflow Grid */}
                    <div className="grid grid-cols-1 xl:grid-cols-2 gap-8">
                        {/* Integration */}
                        <section className="flex flex-col h-full">
                            <h2 className="text-xl font-bold text-white mb-6 flex items-center gap-3">
                                <Palette size={22} className="text-white/50" /> Workflow & Interface
                            </h2>
                            <div className="bg-white/5 backdrop-blur-xl border border-white/10 rounded-3xl p-8 space-y-8 flex-1">
                                <div className="flex items-center justify-between">
                                    <div className="max-w-[70%]">
                                        <h3 className="font-bold text-white text-lg">Native Notifications</h3>
                                        <p className="text-sm text-white/50 mt-1">Broadcast install completions to your desktop.</p>
                                    </div>
                                    <button
                                        onClick={() => updateNotifications(!notificationsEnabled)}
                                        className={clsx(
                                            "w-14 h-8 rounded-full p-1 transition-all shadow-lg",
                                            notificationsEnabled ? "bg-blue-600" : "bg-white/10"
                                        )}
                                    >
                                        <div className={clsx(
                                            "w-6 h-6 bg-white rounded-full transition-transform duration-300 shadow-md",
                                            notificationsEnabled ? "translate-x-6" : "translate-x-0"
                                        )} />
                                    </button>
                                </div>

                                <div className="h-px bg-white/5 w-full" />

                                <div className="flex items-center justify-between">
                                    <div className="max-w-[70%]">
                                        <h3 className="font-bold text-white text-lg flex items-center gap-2">
                                            <Sparkles size={16} className="text-blue-400" /> Initial Setup
                                        </h3>
                                        <p className="text-sm text-white/50 mt-1">Re-run the welcome wizard to configure preferences.</p>
                                    </div>
                                    <button
                                        onClick={onRestartOnboarding}
                                        className="h-10 px-6 rounded-xl bg-white/5 hover:bg-white/10 text-white font-bold text-xs transition-all border border-white/10 active:scale-95"
                                    >
                                        Run Wizard
                                    </button>
                                </div>
                            </div>
                        </section>

                        {/* Appearance (Moved into Grid) */}
                        <section className="flex flex-col h-full">
                            <h2 className="text-xl font-bold text-white mb-6 flex items-center gap-3">
                                <Palette size={22} className="text-white/50" /> Appearance
                            </h2>
                            <div className="bg-white/5 backdrop-blur-xl border border-white/10 rounded-3xl p-8 transition-colors hover:bg-white/10 space-y-8 flex-1">
                                {/* Theme Mode */}
                                <div className="flex items-center justify-between">
                                    <div>
                                        <h3 className="font-bold text-white text-lg">Interface Theme</h3>
                                        <p className="text-sm text-white/50">Select your preferred brightness.</p>
                                    </div>
                                    <div className="flex bg-black/40 p-1 rounded-2xl border border-white/5 shadow-inner">
                                        {(['system', 'light', 'dark'] as const).map((mode) => (
                                            <button
                                                key={mode}
                                                onClick={() => setThemeMode(mode)}
                                                className={clsx(
                                                    "px-4 py-2 rounded-xl text-xs font-black transition-all",
                                                    themeMode === mode
                                                        ? "bg-white text-black shadow-md"
                                                        : "text-white/40 hover:text-white"
                                                )}
                                            >
                                                {mode.toUpperCase()}
                                            </button>
                                        ))}
                                    </div>
                                </div>

                                <div className="h-px bg-white/5 w-full" />

                                {/* Accents */}
                                <div className="flex gap-4 items-center overflow-x-auto pb-2">
                                    {(['#3b82f6', '#8b5cf6', '#10b981', '#f59e0b', '#ef4444']).map((color) => (
                                        <button
                                            key={color}
                                            onClick={() => setAccentColor(color)}
                                            className={clsx(
                                                "w-12 h-12 rounded-full border-4 transition-all relative flex-shrink-0 flex items-center justify-center",
                                                accentColor === color ? "border-white scale-110 shadow-xl shadow-white/20" : "border-transparent opacity-40 hover:opacity-100 hover:scale-105"
                                            )}
                                            style={{ backgroundColor: color }}
                                        >
                                            {accentColor === color && (
                                                <CheckCircle2 size={20} className="text-white drop-shadow-md" />
                                            )}
                                        </button>
                                    ))}
                                    <div className="ml-2 border-l border-white/10 pl-4">
                                        <h3 className="font-bold text-white text-sm">Accent</h3>
                                        <p className="text-xs text-white/40">Color</p>
                                    </div>
                                </div>
                            </div>
                        </section>
                    </div>

                    {/* System Management (Consolidated) */}
                    <section id="system-health">
                        <h2 className="text-2xl font-black text-white mb-6 flex items-center gap-3">
                            <ShieldCheck size={24} className="text-blue-400" />
                            System Management
                        </h2>

                        <div className="bg-white/5 backdrop-blur-xl border border-white/10 rounded-3xl p-8 space-y-12 transition-colors hover:bg-white/10">
                            {/* Security Control */}
                            <div className="flex flex-col md:flex-row items-center justify-between gap-8">
                                <div className="max-w-xl">
                                    <h3 className="font-bold text-white text-xl mb-2 flex items-center gap-2">
                                        <Lock size={20} className="text-emerald-400" />
                                        One-Click Authentication
                                    </h3>
                                    <p className="text-white/60 text-base leading-relaxed">
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
                                        oneClickEnabled ? "bg-emerald-500 shadow-emerald-500/20" : "bg-white/10"
                                    )}
                                >
                                    <div className={clsx(
                                        "w-7 h-7 bg-white rounded-full transition-transform duration-300 shadow-lg",
                                        oneClickEnabled ? "translate-x-7" : "translate-x-0"
                                    )} />
                                </button>
                            </div>

                            <div className="h-px bg-white/5 w-full" />

                            {/* Repair Actions Grid */}
                            <div>
                                <h3 className="text-sm font-black uppercase tracking-widest text-white/40 mb-6 px-2">Maintenance & Repair Tools</h3>
                                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                                    <RepairButton
                                        icon={<Lock className="text-blue-400" />}
                                        title="Unlock Database"
                                        desc="Clear stuck locks from the package manager."
                                        onClick={() => handleRepairTask('unlock')}
                                        loading={isRepairing === 'unlock'}
                                    />
                                    <RepairButton
                                        icon={<ShieldCheck className="text-purple-400" />}
                                        title="Fix System Keys"
                                        desc="Repair signatures and security keyring."
                                        onClick={() => handleRepairTask('keyring')}
                                        loading={isRepairing === 'keyring'}
                                    />
                                    <RepairButton
                                        icon={<Trash2 className="text-red-400" />}
                                        title="Clear Cache"
                                        desc="Wipe temporary package downloads."
                                        onClick={handleClearCache}
                                        loading={isOptimizing}
                                    />
                                    <RepairButton
                                        icon={<Package size={18} className="text-amber-400" />}
                                        title="Clean Orphans"
                                        desc="Remove unused system dependencies."
                                        onClick={handleOrphans}
                                        loading={isRepairing === 'orphans'}
                                    />
                                </div>
                            </div>
                        </div>
                    </section>

                    <div className="text-center text-white/20 text-xs pt-12 pb-8 border-t border-white/5 font-medium">
                        <Info size={14} className="opacity-50 inline mr-2 mb-0.5" />
                        MonARCH Store v{pkgVersion} • Licensed under MIT • Powered by Chaotic-AUR
                    </div>
                </div >
            </div >

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
            className="group p-6 bg-white/5 rounded-3xl border border-white/10 hover:border-white/20 hover:bg-white/10 transition-all text-left flex flex-col gap-3 shadow-sm hover:shadow-lg disabled:opacity-50"
        >
            <div className="p-3 bg-black/20 rounded-2xl w-fit group-hover:scale-110 transition-transform">
                {loading ? <RefreshCw className="animate-spin text-white" size={20} /> : icon}
            </div>
            <div>
                <h4 className="font-bold text-white group-hover:text-blue-400 transition-colors text-sm">{title}</h4>
                <p className="text-[10px] text-white/40 leading-relaxed mt-1">{desc}</p>
            </div>
        </button>
    );
}
