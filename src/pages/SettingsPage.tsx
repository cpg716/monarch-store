import { useState, useEffect } from 'react';
import {
    CheckCircle2, Palette, Info,
    ShieldCheck, Package, ArrowUp, ArrowDown, RefreshCw, Lock, Clock, ChevronDown, Sparkles
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { clsx } from 'clsx';
import { motion } from 'framer-motion';

import { useTheme } from '../hooks/useTheme';
// import { useToast } from '../context/ToastContext';

interface SettingsPageProps {
    onRestartOnboarding?: () => void;
}

import { useSettings } from '../hooks/useSettings';

export default function SettingsPage({ onRestartOnboarding }: SettingsPageProps) {
    const { themeMode, setThemeMode, accentColor, setAccentColor } = useTheme();
    // const { success, error } = useToast();

    // Centralized Logic
    const {
        notificationsEnabled, updateNotifications,
        syncIntervalHours, updateSyncInterval,
        isAurEnabled, toggleAur,
        repos, toggleRepo, reorderRepos,
        isSyncing, triggerManualSync, repoCounts
    } = useSettings();

    const [pkgVersion, setPkgVersion] = useState("0.0.0");
    const [systemInfo, setSystemInfo] = useState<{ kernel: string, de: string, distro: string, has_avx2: boolean } | null>(null);

    // Initial Load
    useEffect(() => {
        getVersion().then(setPkgVersion).catch(console.error);
        invoke<any>('get_system_info').then(setSystemInfo).catch(console.error);
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
                {/* Dashboard removed - Moved to System Health page */}
                <div className="flex-1 overflow-y-auto p-8 space-y-12">

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
                                    onClick={triggerManualSync}
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
                                                updateSyncInterval(val);
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
                                Manage your software sources. Disabling a source here <strong className="text-app-fg">hides it from the Store</strong> but keeps it active in the system, so your installed apps <strong className="text-green-500">continue to update safely</strong>.
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
                                        onClick={async () => {
                                            if (repo.enabled) {
                                                // Prevent accidental breakage
                                                // setIsOptimizing(true); // Show busy state if needed, or just block UI
                                                try {
                                                    const installed = await invoke<{ name: string, repository: string }[]>('get_installed_packages');
                                                    const affected = installed.filter(p => p.repository === repo.name);

                                                    if (affected.length > 0) {
                                                        const names = affected.map(p => p.name).slice(0, 3).join(', ');
                                                        const more = affected.length > 3 ? ` and ${affected.length - 3} others` : '';

                                                        if (!confirm(`⚠️ CRITICAL WARNING:\n\nYou have ${affected.length} apps installed from "${repo.name}" (including ${names}${more}).\n\nIf you disable this repository, these apps will STOP UPDATING and might break.\n\nAre you absolutely sure?`)) {
                                                            return;
                                                        }
                                                    }
                                                } catch (e) {
                                                    console.error("Failed to check dependencies", e);
                                                } finally {
                                                    // setIsOptimizing(false);
                                                }
                                                toggleRepo(repo.id);
                                            } else {
                                                toggleRepo(repo.id);
                                            }
                                        }}
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
                                            toggleAur(newState);
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
                                            updateNotifications(!notificationsEnabled);
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
                    <Info size={14} className="opacity-50" /> MonARCH Store v{pkgVersion} • Licensed under MIT • Powered by Chaotic-AUR
                </div>
            </div >
        </div >
    );
}
