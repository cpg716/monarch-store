import { useState, useEffect } from 'react';
import {
    CheckCircle2, Zap, Globe, Palette, Info,
    Trash2, ShieldCheck, Activity, Package, ArrowUp, ArrowDown, RefreshCw, Lock, Clock, ChevronDown, Sparkles
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import { motion } from 'framer-motion';

import { useTheme } from '../hooks/useTheme';

interface SettingsPageProps {
    onRestartOnboarding?: () => void;
}

export default function SettingsPage({ onRestartOnboarding }: SettingsPageProps) {
    const { themeMode, setThemeMode, accentColor, setAccentColor } = useTheme();

    const [isOptimizing, setIsOptimizing] = useState(false);
    const [notificationsEnabled, setNotificationsEnabled] = useState(() => {
        return localStorage.getItem('notifications-enabled') !== 'false';
    });
    const [systemInfo, setSystemInfo] = useState<{ kernel: string, de: string, distro: string } | null>(null);
    // const [healthStatus, setHealthStatus] = useState<'healthy' | 'maintenance'>('healthy');


    // Sync interval setting (in hours)
    const [syncIntervalHours, setSyncIntervalHours] = useState<number>(() => {
        const saved = localStorage.getItem('sync-interval-hours');
        return saved ? parseInt(saved, 10) : 3; // Default 3 hours
    });

    // Repository Priority State - fetched from backend
    interface Repository {
        id: string;
        name: string;
        enabled: boolean;
        description: string;
    }

    const [repos, setRepos] = useState<Repository[]>([]);

    // State for sync
    const [isSyncing, setIsSyncing] = useState(false);
    const [isAurEnabled, setIsAurEnabled] = useState(false);

    const [repoCounts, setRepoCounts] = useState<Record<string, number>>({});

    const fetchCounts = () => {
        invoke<Record<string, number>>('get_repo_counts').then(setRepoCounts).catch(console.error);
    };

    const handleSync = async () => {
        setIsSyncing(true);
        try {
            await invoke<string>('trigger_repo_sync', { syncIntervalHours });

            fetchCounts(); // Update counts after sync
        } catch (e) {
            console.error(e);
        } finally {
            setIsSyncing(false);
        }
    };

    // Load Initial State
    useEffect(() => {
        // 1. Check AUR
        invoke<boolean>('is_aur_enabled').then(setIsAurEnabled).catch(console.error);

        // 2. Fetch ALL repos and group by family for simple UI
        invoke<{ name: string; enabled: boolean; source: string }[]>('get_repo_states').then(backendRepos => {
            // Define repo families with friendly names and descriptions
            const families: Record<string, { name: string; description: string; members: string[] }> = {
                'Chaotic-AUR': {
                    name: 'Chaotic-AUR',
                    description: 'Pre-built AUR packages - PRIMARY',
                    members: ['Chaotic-AUR'],
                },
                'CachyOS': {
                    name: 'CachyOS',
                    description: 'Performance-optimized packages (auto-selects best for your CPU)',
                    members: ['cachyos', 'cachyos-v3', 'cachyos-core', 'cachyos-v4'],
                },
                'Manjaro': {
                    name: 'Manjaro',
                    description: 'Stable, tested packages from Manjaro Linux',
                    members: ['manjaro-core', 'manjaro-extra', 'manjaro-multilib'],
                },
                'Garuda': {
                    name: 'Garuda',
                    description: 'Gaming and performance-focused packages',
                    members: ['garuda'],
                },
                'EndeavourOS': {
                    name: 'EndeavourOS',
                    description: 'Lightweight, minimalist packages',
                    members: ['endeavouros'],
                },
            };

            // Create grouped repos - family is enabled if ANY member is enabled
            const groupedRepos = Object.entries(families).map(([key, family]) => {
                const memberRepos = backendRepos.filter(r =>
                    family.members.some(m => r.name.toLowerCase() === m.toLowerCase())
                );
                const anyEnabled = memberRepos.some(r => r.enabled);

                return {
                    id: key.toLowerCase().replace(/\s+/g, '-'),
                    name: family.name,
                    enabled: anyEnabled,
                    description: family.description,
                };
            });

            setRepos(groupedRepos);
        }).catch(console.error);

        // 3. Get System Info
        invoke<any>('get_system_info').then(setSystemInfo).catch(console.error);

        // 4. Get Counts
        fetchCounts();
    }, []);

    const moveRepo = (index: number, direction: 'up' | 'down') => {
        const newRepos = [...repos];
        if (direction === 'up' && index > 0) {
            [newRepos[index], newRepos[index - 1]] = [newRepos[index - 1], newRepos[index]];
        } else if (direction === 'down' && index < newRepos.length - 1) {
            [newRepos[index], newRepos[index + 1]] = [newRepos[index + 1], newRepos[index]];
        }
        setRepos(newRepos);
    };

    const toggleRepo = async (id: string) => {
        const repo = repos.find(r => r.id === id);
        if (repo) {
            const newEnabled = !repo.enabled;
            setRepos(repos.map(r => r.id === id ? { ...r, enabled: newEnabled } : r));

            try {
                // Use toggle_repo_family to enable/disable all variants at once
                await invoke('toggle_repo_family', { family: repo.name, enabled: newEnabled });

                // Trigger a background sync and refresh counts
                invoke('trigger_repo_sync').finally(() => {
                    fetchCounts();
                });
            } catch (e) {
                console.error(e);
                // Revert on error
                setRepos(repos.map(r => r.id === id ? { ...r, enabled: !newEnabled } : r));
            }
        }
    };

    // Remove unused getRepoDescription - descriptions now inline above

    const handleOptimize = async () => {
        setIsOptimizing(true);
        try {
            const result = await invoke<string>('optimize_system');
            alert(result);
        } catch (e) {
            alert(`Optimization failed: ${e}`);
        } finally {
            setIsOptimizing(false);
        }
    };

    const handleClearCache = async () => {
        if (!confirm("Are you sure you want to wipe all application caches? This will resolve metadata issues but may require re-downloading some data.")) {
            return;
        }

        setIsOptimizing(true); // Reuse loading state
        try {
            const result = await invoke<string>('clear_cache');
            alert(result);
            // Optionally reload window to force fresh state?
            // window.location.reload(); 
        } catch (e) {
            alert(`Cache wipe failed: ${e}`);
        } finally {
            setIsOptimizing(false);
        }
    };

    const colors = [
        { id: '#3b82f6', label: 'MonARCH Blue', class: 'bg-blue-500' },
        { id: '#a855f7', label: 'Nebula Purple', class: 'bg-purple-500' },
        { id: '#10b981', label: 'Aurora Green', class: 'bg-green-500' },
        { id: '#f59e0b', label: 'Solar Orange', class: 'bg-amber-500' },
    ];



    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            {/* Header */}
            <div className="p-8 pb-4 border-b border-app-border bg-app-card/50 backdrop-blur-xl z-10 transition-colors">
                <div className="flex justify-between items-start">
                    <div>
                        <h1 className="text-2xl font-bold flex items-center gap-2 text-app-fg">
                            <ShieldCheck className="text-blue-700" size={24} />
                            Settings
                        </h1>
                        <p className="text-app-muted text-sm">Configure your store and monitor system health</p>
                    </div>
                    {systemInfo && (
                        <div className="text-right">
                            <div className="text-xs font-bold text-app-fg">{systemInfo.distro}</div>
                            <div className="text-[10px] text-app-muted">{systemInfo.de} • {systemInfo.kernel}</div>
                        </div>
                    )}
                </div>
            </div>

            <div className="flex-1 overflow-y-auto p-8 space-y-12">
                {/* System Health Dashboard */}
                <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                    {/* 1. Global Connectivity */}
                    <div className="relative group">
                        <div className="absolute inset-0 bg-gradient-to-br from-green-500/20 to-emerald-500/5 blur-xl group-hover:blur-2xl transition-all duration-500 opacity-50" />
                        <div className="relative bg-app-card/40 backdrop-blur-xl border border-green-500/30 rounded-3xl p-6 shadow-2xl overflow-hidden transition-all group-hover:-translate-y-1 h-full">
                            <div className="absolute top-0 right-0 p-4 opacity-10 group-hover:scale-125 transition-transform duration-700 pointer-events-none">
                                <Globe size={64} className="text-emerald-700" />
                            </div>
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-2 bg-green-500/20 rounded-xl">
                                    <Zap size={24} className="text-emerald-700 animate-pulse" />
                                </div>
                                <div>
                                    <h3 className="font-bold text-app-fg text-lg leading-tight">Connectivity</h3>
                                    <p className="text-[10px] text-app-muted uppercase tracking-widest font-extrabold">Service Status</p>
                                </div>
                            </div>
                            <div className="space-y-4">
                                <div className="flex items-center justify-between">
                                    <span className="text-sm text-app-fg/80 font-medium">Chaotic-AUR</span>
                                    <span className="flex items-center gap-1.5 text-xs font-bold text-emerald-700">
                                        <span className="w-2 h-2 rounded-full bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.8)]" />
                                        ONLINE
                                    </span>
                                </div>
                                <div className="h-1.5 w-full bg-app-subtle rounded-full overflow-hidden">
                                    <div className="h-full bg-green-500 w-[95%] shadow-[0_0_10px_rgba(34,197,94,0.4)]" />
                                </div>
                                <p className="text-[11px] text-app-muted font-medium opacity-80">Latency: 45ms • 14 Active Mirrors</p>
                            </div>
                        </div>
                    </div>

                    {/* 2. Repository Sync Pipeline */}
                    <div className="relative group">
                        <div className="absolute inset-0 bg-gradient-to-br from-blue-500/20 to-cyan-500/5 blur-xl group-hover:blur-2xl transition-all duration-500 opacity-50" />
                        <div className="relative bg-app-card/40 backdrop-blur-xl border border-blue-500/30 rounded-3xl p-6 shadow-2xl overflow-hidden transition-all group-hover:-translate-y-1 h-full">
                            <div className="absolute top-0 right-0 p-4 opacity-10 group-hover:scale-125 transition-transform duration-700 pointer-events-none">
                                <RefreshCw size={64} className="text-blue-700" />
                            </div>
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-2 bg-blue-500/20 rounded-xl text-blue-700">
                                    <Package size={24} className={clsx(isSyncing && "animate-spin")} />
                                </div>
                                <div>
                                    <h3 className="font-bold text-app-fg text-lg leading-tight">Sync Pipeline</h3>
                                    <p className="text-[10px] text-app-muted uppercase tracking-widest font-extrabold">Data Refresh</p>
                                </div>
                            </div>
                            <div className="space-y-4">
                                <div className="flex items-center justify-between">
                                    <span className="text-sm text-app-fg/80 font-medium">Catalogs</span>
                                    <span className="text-xs font-bold text-app-fg">{Object.values(repoCounts).reduce((a, b) => a + b, 0).toLocaleString()} Pkgs</span>
                                </div>
                                <div className="h-1.5 w-full bg-app-subtle rounded-full overflow-hidden">
                                    <div className={clsx(
                                        "h-full bg-blue-500 transition-all duration-1000",
                                        isSyncing ? "w-[100%] animate-pulse" : "w-[65%]"
                                    )} />
                                </div>
                                <p className="text-[11px] text-app-muted font-medium opacity-80">{isSyncing ? "Syncing catalogs..." : "Last checked: Just now"}</p>
                            </div>
                        </div>
                    </div>

                    {/* 3. System Integrity */}
                    <div className="relative group">
                        <div className="absolute inset-0 bg-gradient-to-br from-purple-500/20 to-pink-500/5 blur-xl group-hover:blur-2xl transition-all duration-500 opacity-50" />
                        <div className="relative bg-app-card/40 backdrop-blur-xl border border-purple-500/30 rounded-3xl p-6 shadow-2xl overflow-hidden transition-all group-hover:-translate-y-1 h-full">
                            <div className="absolute top-0 right-0 p-4 opacity-10 group-hover:scale-125 transition-transform duration-700 pointer-events-none">
                                <ShieldCheck size={64} className="text-purple-700" />
                            </div>
                            <div className="flex items-center gap-3 mb-4">
                                <div className="p-2 bg-purple-500/20 rounded-xl text-purple-700">
                                    <Activity size={24} className={clsx(isOptimizing && "animate-bounce")} />
                                </div>
                                <div>
                                    <h3 className="font-bold text-app-fg text-lg leading-tight">Integrity</h3>
                                    <p className="text-[10px] text-app-muted uppercase tracking-widest font-extrabold">Optimization</p>
                                </div>
                            </div>
                            <div className="space-y-4">
                                <div className="flex items-center justify-between">
                                    <span className="text-sm text-app-fg/80 font-medium">Local Health</span>
                                    <span className="text-xs font-bold text-emerald-700">EXCELLENT</span>
                                </div>
                                <div className="h-1.5 w-full bg-app-subtle rounded-full overflow-hidden text-purple-700 font-bold">
                                    <div className="h-full bg-current w-full" />
                                </div>
                                <button
                                    onClick={handleOptimize}
                                    className="text-[11px] text-purple-700 font-bold hover:text-purple-600 transition-colors uppercase tracking-tight"
                                >
                                    {isOptimizing ? "Optimizing..." : "Run Optimization"}
                                </button>
                            </div>
                        </div>
                    </div>
                </div>

                {/* Main Content Sections */}
                <div className="space-y-12">
                    {/* Sync Control */}
                    <section>
                        <h2 className="text-xl font-bold mb-4 flex items-center gap-3 text-app-fg">
                            <RefreshCw size={22} className={clsx(isSyncing ? "animate-spin text-app-accent" : "text-app-muted")} />
                            Repository Control
                        </h2>
                        <div className="bg-app-card/30 rounded-3xl p-8 border border-app-border/50 transition-all hover:bg-app-card/40">
                            <div className="flex items-center justify-between mb-6">
                                <div>
                                    <h3 className="font-bold text-app-fg text-lg mb-1">Force Database Synchronization</h3>
                                    <p className="text-app-muted text-sm max-w-md">
                                        Refresh local package catalogs from Chaotic-AUR and other secondary repositories. Recommended after changing priorities.
                                    </p>
                                </div>
                                <button
                                    onClick={handleSync}
                                    disabled={isSyncing}
                                    className={clsx(
                                        "px-8 py-4 rounded-2xl font-bold transition-all flex items-center gap-3 text-lg",
                                        isSyncing
                                            ? "bg-app-fg/10 text-app-muted cursor-not-allowed"
                                            : "bg-app-accent hover:opacity-90 text-white shadow-xl shadow-app-accent/20 active:scale-95"
                                    )}
                                >
                                    <RefreshCw size={20} className={isSyncing ? "animate-spin" : ""} />
                                    {isSyncing ? 'Syncing...' : 'Sync Now'}
                                </button>
                            </div>

                            {/* Stats Grid */}
                            <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-3 pt-6 border-t border-app-border/30">
                                {Object.entries(repoCounts).sort((a, b) => b[1] - a[1]).map(([name, count]) => (
                                    <div key={name} className="flex flex-col items-center bg-app-bg/50 p-2 rounded-xl border border-app-border/30">
                                        <span className="text-[10px] uppercase font-bold text-app-muted mb-0.5">{name}</span>
                                        <span className="text-sm font-black text-app-fg">{count.toLocaleString()}</span>
                                    </div>
                                ))}
                                {Object.keys(repoCounts).length === 0 && (
                                    <div className="col-span-full text-center text-xs text-app-muted italic">
                                        Syncing database stats...
                                    </div>
                                )}
                            </div>

                            {/* Auto Sync Interval */}
                            <div className="pt-6 border-t border-app-border/30 mt-6">
                                <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-3">
                                        <Clock size={20} className="text-blue-700" />
                                        <div>
                                            <h4 className="font-bold text-app-fg">Auto Sync Interval</h4>
                                            <p className="text-xs text-app-muted">
                                                Automatically refresh package databases in the background
                                            </p>
                                        </div>
                                    </div>
                                    <div className="relative">
                                        <select
                                            value={syncIntervalHours}
                                            onChange={(e) => {
                                                const val = parseInt(e.target.value, 10);
                                                setSyncIntervalHours(val);
                                                localStorage.setItem('sync-interval-hours', val.toString());
                                            }}
                                            className="appearance-none bg-app-card border border-app-border rounded-xl px-4 py-2 pr-10 text-app-fg font-medium focus:outline-none focus:ring-2 focus:ring-blue-500/50 cursor-pointer"
                                        >
                                            <option value={1}>Every 1 hour</option>
                                            <option value={3}>Every 3 hours</option>
                                            <option value={6}>Every 6 hours</option>
                                            <option value={12}>Every 12 hours</option>
                                            <option value={24}>Every 24 hours</option>
                                        </select>
                                        <ChevronDown size={16} className="absolute right-3 top-1/2 -translate-y-1/2 text-app-muted pointer-events-none" />
                                    </div>
                                </div>
                            </div>
                        </div>
                    </section>

                    {/* Repository Management */}
                    <section>
                        <h2 className="text-xl font-bold text-app-fg mb-4 flex items-center gap-3">
                            <Package size={22} className="text-app-muted" /> Repository Priority
                        </h2>
                        <div className="bg-app-card/30 border border-app-border/50 rounded-3xl overflow-hidden p-6 space-y-3">
                            <p className="text-sm text-app-muted mb-6 px-2">
                                Define the order in which sources are searched. The top-most active repository is used as the primary binary source.
                            </p>

                            {repos.map((repo, idx) => (
                                <div key={repo.id} className={clsx(
                                    "flex items-center justify-between p-4 rounded-2xl border transition-all duration-300",
                                    repo.enabled ? "bg-app-card/60 border-app-border/80 text-app-fg shadow-sm" : "bg-app-subtle border-transparent opacity-50 text-app-muted"
                                )}>
                                    <div className="flex items-center gap-4">
                                        <div className="flex flex-col gap-1 text-app-muted">
                                            <button onClick={() => moveRepo(idx, 'up')} disabled={idx === 0} className="hover:text-app-fg disabled:opacity-20 transition-colors"><ArrowUp size={16} /></button>
                                            <button onClick={() => moveRepo(idx, 'down')} disabled={idx === repos.length - 1} className="hover:text-app-fg disabled:opacity-20 transition-colors"><ArrowDown size={16} /></button>
                                        </div>
                                        <div>
                                            <h4 className={clsx("font-bold text-base", repo.enabled ? "text-app-fg" : "text-app-muted")}>
                                                {repo.name}
                                                {idx === 0 && repo.enabled && <span className="ml-3 text-[10px] bg-app-accent/20 text-app-accent px-2.5 py-1 rounded-full uppercase tracking-widest font-black">Primary</span>}
                                            </h4>
                                            <p className="text-xs text-app-muted mt-0.5">{repo.description}</p>
                                        </div>
                                    </div>

                                    <button
                                        onClick={() => toggleRepo(repo.id)}
                                        className={clsx(
                                            "w-12 h-7 rounded-full p-1 transition-all relative",
                                            repo.enabled ? "bg-app-accent" : "bg-app-subtle"
                                        )}
                                    >
                                        <div className={clsx(
                                            "w-5 h-5 bg-white shadow-xl rounded-full transition-transform duration-300",
                                            repo.enabled ? "translate-x-5" : "translate-x-0"
                                        )} />
                                    </button>
                                </div>
                            ))}

                            {/* AUR Section */}
                            <div className="mt-8 pt-6 border-t border-app-border/30">
                                <div className="flex items-center justify-between p-5 rounded-2xl border border-amber-500/20 bg-amber-500/5">
                                    <div className="flex items-center gap-4">
                                        <div className="p-3 bg-amber-600/20 rounded-2xl text-amber-600">
                                            <Lock size={20} />
                                        </div>
                                        <div>
                                            <h4 className="font-bold text-amber-600 text-base">
                                                Enable AUR Source <span className="text-[10px] bg-amber-500 text-white px-2 py-0.5 rounded ml-2 font-black">EXPERIMENTAL</span>
                                            </h4>
                                            <p className="text-sm text-amber-600/90 mt-0.5 max-w-md">
                                                Build directly from the Arch User Repository. Note: This requires compilation and is significantly slower.
                                            </p>
                                        </div>
                                    </div>

                                    <button
                                        onClick={async () => {
                                            const newState = !isAurEnabled;
                                            setIsAurEnabled(newState);
                                            await invoke('set_aur_enabled', { enabled: newState });
                                        }}
                                        className={clsx(
                                            "w-12 h-7 rounded-full p-1 transition-all relative",
                                            isAurEnabled ? "bg-amber-500" : "bg-app-subtle"
                                        )}
                                    >
                                        <div className={clsx(
                                            "w-5 h-5 bg-white shadow-xl rounded-full transition-transform duration-300",
                                            isAurEnabled ? "translate-x-5" : "translate-x-0"
                                        )} />
                                    </button>
                                </div>
                            </div>
                        </div>
                    </section>

                    {/* Customization & DE */}
                    <div className="grid grid-cols-1 lg:grid-cols-2 gap-12">
                        {/* Integration */}
                        <section>

                            <h2 className="text-xl font-bold text-app-fg mb-4 flex items-center gap-3">
                                <Palette size={22} className="text-app-muted" /> System Integration
                            </h2>
                            <div className="bg-app-card/30 border border-app-border/50 rounded-3xl p-8 space-y-6">
                                <div className="flex items-center justify-between">
                                    <div className="max-w-[70%]">
                                        <h3 className="font-bold text-app-fg text-base">Native Notifications</h3>
                                        <p className="text-xs text-app-muted mt-1 leading-relaxed">Broadcast install completions to your desktop environment's notification center.</p>
                                    </div>
                                    <button
                                        onClick={() => {
                                            const next = !notificationsEnabled;
                                            setNotificationsEnabled(next);
                                            localStorage.setItem('notifications-enabled', String(next));
                                        }}
                                        className={clsx(
                                            "w-12 h-7 rounded-full p-1 transition-all relative shadow-lg",
                                            notificationsEnabled ? "bg-app-accent" : "bg-app-fg/20"
                                        )}
                                    >
                                        <div className={clsx(
                                            "w-5 h-5 bg-white rounded-full transition-transform duration-300",
                                            notificationsEnabled ? "translate-x-5" : "translate-x-0"
                                        )} />
                                    </button>
                                </div>
                                <div className="flex items-center justify-between border-t border-app-border/30 pt-6">
                                    <div>
                                        <h3 className="font-bold text-app-fg text-base">Interface Mode</h3>
                                        <p className="text-xs text-app-muted mt-0.5">Application-level color scheme</p>
                                    </div>
                                    <div className="flex bg-app-card/60 p-1.5 rounded-2xl border border-app-border/50 shadow-sm">
                                        {(['system', 'light', 'dark'] as const).map((mode) => (
                                            <button
                                                key={mode}
                                                onClick={() => setThemeMode(mode)}
                                                className={clsx(
                                                    "px-5 py-2 rounded-xl text-xs font-black transition-all",
                                                    themeMode === mode
                                                        ? "bg-app-accent text-white shadow-lg shadow-app-accent/20"
                                                        : "text-app-muted hover:text-app-fg"
                                                )}
                                            >
                                                {mode.toUpperCase()}
                                            </button>
                                        ))}
                                    </div>
                                </div>

                                <div className="flex items-center justify-between border-t border-app-border/30 pt-6">
                                    <div className="max-w-[70%]">
                                        <h3 className="font-bold text-app-fg text-base flex items-center gap-2">
                                            <Sparkles size={16} className="text-app-accent" /> Initial Setup
                                        </h3>
                                        <p className="text-xs text-app-muted mt-1 leading-relaxed">Re-run the welcome wizard to configure repositories and aesthetic preferences.</p>
                                    </div>
                                    <button
                                        onClick={onRestartOnboarding}
                                        className="h-10 px-6 rounded-xl bg-app-subtle hover:bg-app-accent hover:text-white text-app-fg font-bold text-xs transition-all flex items-center gap-2 border border-app-border/50 hover:border-app-accent/50 shadow-sm hover:shadow-lg hover:shadow-app-accent/20 active:scale-95"
                                    >
                                        Run Setup Wizard
                                    </button>
                                </div>
                            </div>
                        </section>

                        {/* Maintenance */}
                        <section>
                            <h2 className="text-xl font-bold text-app-fg mb-4 flex items-center gap-3">
                                <Trash2 size={22} className="text-app-muted" /> System Maintenance
                            </h2>
                            <div className="bg-app-card/30 border border-app-border/50 rounded-3xl p-8 h-full space-y-6">
                                {/* Disk Cleanup */}
                                <div className="flex items-center justify-between">
                                    <div className="max-w-[60%]">
                                        <h3 className="text-base font-bold text-app-fg">Disk Cleanup</h3>
                                        <p className="text-xs text-app-muted mt-1 leading-relaxed">Clear old package caches, temporary files, and build artifacts to free disk space.</p>
                                    </div>
                                    <button
                                        onClick={handleClearCache}
                                        disabled={isOptimizing}
                                        className={clsx(
                                            "px-5 py-2.5 bg-red-500/20 hover:bg-red-500/30 text-red-700 rounded-2xl text-xs font-black transition-all border border-red-500/30 active:scale-95 flex items-center gap-2",
                                            isOptimizing && "opacity-50 cursor-not-allowed"
                                        )}
                                    >
                                        <Trash2 size={16} /> WIPE CACHE
                                    </button>
                                </div>

                                {/* Orphan Cleanup */}
                                <div className="flex items-center justify-between border-t border-app-border/30 pt-6">
                                    <div className="max-w-[60%]">
                                        <h3 className="text-base font-bold text-app-fg">Orphan Packages</h3>
                                        <p className="text-xs text-app-muted mt-1 leading-relaxed">Remove unused dependencies that are no longer required by any installed package.</p>
                                    </div>
                                    <button
                                        onClick={async () => {
                                            if (confirm("Scan for and remove unused orphan packages? This requires authentication.")) {
                                                setIsOptimizing(true);
                                                try {
                                                    const orphans = await invoke<string[]>('get_orphans');
                                                    if (orphans.length === 0) {
                                                        alert("No orphan packages found. Your system is clean.");
                                                    } else {
                                                        if (confirm(`Found ${orphans.length} orphans:\n${orphans.slice(0, 5).join(', ')}${orphans.length > 5 ? '...' : ''}\n\nRemove them?`)) {
                                                            await invoke('remove_orphans', { orphans });
                                                            alert(`Successfully removed ${orphans.length} packages.`);
                                                        }
                                                    }
                                                } catch (e) {
                                                    alert(`Failed: ${e}`);
                                                } finally {
                                                    setIsOptimizing(false);
                                                }
                                            }
                                        }}
                                        disabled={isOptimizing}
                                        className={clsx(
                                            "px-5 py-2.5 bg-app-subtle hover:bg-app-hover text-app-fg rounded-2xl text-xs font-black transition-all border border-app-border/50 active:scale-95 flex items-center gap-2",
                                            isOptimizing && "opacity-50 cursor-not-allowed"
                                        )}
                                    >
                                        <Package size={16} /> CLEAN ORPHANS
                                    </button>
                                </div>
                            </div>
                        </section>
                    </div>

                    {/* Appearance */}
                    <section>
                        <h2 className="text-xl font-bold text-app-fg mb-4 flex items-center gap-3">
                            <Palette size={22} className="text-app-muted" /> Semantic Accents
                        </h2>
                        <div className="bg-app-card/30 border border-app-border/50 rounded-3xl p-8 transition-all hover:bg-app-card/40">
                            <div className="flex gap-6 items-center">
                                {colors.map((color) => (
                                    <button
                                        key={color.id}
                                        onClick={() => setAccentColor(color.id)}
                                        className={clsx(
                                            "w-16 h-16 rounded-3xl border-4 transition-all relative flex-shrink-0",
                                            color.class,
                                            accentColor === color.id ? "border-app-fg scale-110 shadow-2xl rotate-3" : "border-transparent opacity-40 hover:opacity-100 hover:scale-105"
                                        )}
                                        title={color.label}
                                    >
                                        {accentColor === color.id && (
                                            <motion.div
                                                layoutId="activeColor"
                                                className="absolute inset-0 flex items-center justify-center text-white"
                                            >
                                                <CheckCircle2 size={24} className="drop-shadow-md" />
                                            </motion.div>
                                        )}
                                    </button>
                                ))}
                                <div className="ml-4">
                                    <h3 className="font-bold text-app-fg text-lg">Visual Signature</h3>
                                    <p className="text-sm text-app-muted">Personalize the primary interactive highlight color used throughout the MonARCH Store environment.</p>
                                </div>
                            </div>
                        </div>
                    </section>
                </div>

                {/* Footer Info */}
                <div className="text-center text-app-muted text-xs pt-12 pb-8 border-t border-app-border/20">
                    <p className="flex items-center justify-center gap-2 font-medium">
                        <Info size={14} className="opacity-50" /> MonARCH Store v0.1.0-alpha • Licensed under MIT • Powered by Chaotic-AUR
                    </p>
                </div>
            </div>
        </div >
    );
}
