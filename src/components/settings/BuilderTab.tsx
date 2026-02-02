import React from 'react';
import { Terminal, Trash2, Cpu, Zap, Info, ChevronDown } from 'lucide-react';
import { clsx } from 'clsx';
import { useAppStore } from '../../store/internal_store';
import { useToast } from '../../context/ToastContext';
import { invoke } from '@tauri-apps/api/core';

export default function BuilderTab() {
    const {
        verboseLogsEnabled, setVerboseLogsEnabled,
        cleanBuild, setCleanBuild,
        parallelDownloads, setParallelDownloads
    } = useAppStore();
    const { success, error } = useToast();
    const [isClearing, setIsClearing] = React.useState(false);

    const handleClearBuildCache = async () => {
        setIsClearing(true);
        try {
            await invoke('clear_build_cache');
            success("Build cache cleared successfully.");
        } catch (e) {
            error("Failed to clear build cache: " + String(e));
        } finally {
            setIsClearing(false);
        }
    };

    return (
        <div className="space-y-8 animate-in fade-in slide-in-from-bottom-2 duration-300">
            {/* Section 1: Compilation Strategy */}
            <section className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-2xl p-6 shadow-sm dark:shadow-none">
                <div className="flex items-center gap-3 mb-6">
                    <div className="p-2 bg-amber-500/10 rounded-lg text-amber-600 dark:text-amber-400">
                        <Cpu size={24} />
                    </div>
                    <div>
                        <h2 className="text-xl font-bold text-slate-900 dark:text-white">Native Builder</h2>
                        <p className="text-sm text-slate-500 dark:text-white/50">Configure how MonARCH compiles packages from source (AUR).</p>
                    </div>
                </div>

                <div className="space-y-6">
                    {/* Show Build Logs */}
                    <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                        <div className="space-y-1">
                            <h3 className="font-bold text-slate-900 dark:text-white flex items-center gap-2">
                                <Terminal size={16} className="text-slate-400" />
                                Show Build Logs
                            </h3>
                            <p className="text-sm text-slate-500 dark:text-white/50 max-w-md">
                                Enable real-time output during the compilation process (Glass Cockpit).
                            </p>
                        </div>
                        <button
                            onClick={() => setVerboseLogsEnabled(!verboseLogsEnabled)}
                            className={clsx(
                                "relative w-14 h-8 rounded-full p-1 transition-all duration-300 focus:outline-none focus:ring-2 focus:ring-blue-500/50 shrink-0",
                                verboseLogsEnabled ? "bg-blue-600 shadow-lg shadow-blue-600/20" : "bg-slate-200 dark:bg-white/10"
                            )}
                        >
                            <div className={clsx(
                                "w-6 h-6 bg-white rounded-full transition-transform duration-300 shadow-sm",
                                verboseLogsEnabled ? "translate-x-6" : "translate-x-0"
                            )} />
                        </button>
                    </div>

                    <div className="h-px bg-slate-100 dark:bg-white/5 w-full" />

                    {/* Clean Build */}
                    <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                        <div className="space-y-1">
                            <h3 className="font-bold text-slate-900 dark:text-white flex items-center gap-2">
                                <Trash2 size={16} className="text-slate-400" />
                                Clean Build
                            </h3>
                            <p className="text-sm text-slate-500 dark:text-white/50 max-w-md">
                                Always remove temporary build files after a successful installation.
                            </p>
                        </div>
                        <button
                            onClick={() => setCleanBuild(!cleanBuild)}
                            className={clsx(
                                "relative w-14 h-8 rounded-full p-1 transition-all duration-300 focus:outline-none focus:ring-2 focus:ring-amber-500/50 shrink-0",
                                cleanBuild ? "bg-amber-500 shadow-lg shadow-amber-500/20" : "bg-slate-200 dark:bg-white/10"
                            )}
                        >
                            <div className={clsx(
                                "w-6 h-6 bg-white rounded-full transition-transform duration-300 shadow-sm",
                                cleanBuild ? "translate-x-6" : "translate-x-0"
                            )} />
                        </button>
                    </div>
                </div>
            </section>

            {/* Section 2: Performance & Cache */}
            <section className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-2xl p-6 shadow-sm dark:shadow-none space-y-4">
                    <div className="flex items-center gap-2 text-slate-900 dark:text-white font-bold">
                        <Zap size={20} className="text-sky-500" />
                        Parallelism
                    </div>
                    <div className="space-y-2">
                        <label className="text-sm text-slate-500 dark:text-white/50 block">Max Parallel Downloads</label>
                        <div className="relative">
                            <select
                                value={parallelDownloads}
                                onChange={(e) => setParallelDownloads(parseInt(e.target.value, 10))}
                                className="w-full appearance-none bg-slate-100 dark:bg-white/5 border border-slate-200 dark:border-white/10 rounded-xl px-4 py-2.5 text-sm font-bold text-slate-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500/40 transition-all cursor-pointer"
                            >
                                <option value={1}>1 Session (Serial)</option>
                                <option value={3}>3 Sessions (Balanced)</option>
                                <option value={5}>5 Sessions (Fast)</option>
                                <option value={10}>10 Sessions (Extreme)</option>
                            </select>
                            <div className="absolute right-4 top-1/2 -translate-y-1/2 pointer-events-none text-slate-400">
                                <ChevronDown size={16} />
                            </div>
                        </div>
                    </div>
                    <p className="text-xs text-slate-400 dark:text-white/30 flex items-start gap-1.5 leading-relaxed">
                        <Info size={12} className="shrink-0 mt-0.5" />
                        Higher values speed up downloads but increase load on mirrors.
                    </p>
                </div>

                <div className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-2xl p-6 shadow-sm dark:shadow-none flex flex-col justify-between">
                    <div className="space-y-2">
                        <div className="flex items-center gap-2 text-slate-900 dark:text-white font-bold">
                            <Trash2 size={20} className="text-red-500" />
                            Build Artifacts
                        </div>
                        <p className="text-sm text-slate-500 dark:text-white/50 leading-relaxed">
                            Delete cached source files and compiled objects to free up system space.
                        </p>
                    </div>

                    <button
                        onClick={handleClearBuildCache}
                        disabled={isClearing}
                        className="w-full bg-red-500/10 hover:bg-red-500/20 border border-red-500/20 text-red-600 dark:text-red-400 font-bold py-3 rounded-xl transition-all active:scale-95 disabled:opacity-50 mt-4 flex items-center justify-center gap-2"
                    >
                        {isClearing ? (
                            <div className="w-4 h-4 border-2 border-red-600/30 border-t-red-600 rounded-full animate-spin" />
                        ) : (
                            <Trash2 size={16} />
                        )}
                        Clear Build Cache
                    </button>
                </div>
            </section>
        </div>
    );
}
