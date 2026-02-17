import React from 'react';
import { Terminal, Trash2, Cpu, PackageOpen, Info, ChevronDown } from 'lucide-react';
import { clsx } from 'clsx';
import { useAppStore } from '../../store/internal_store';
import { useToast } from '../../context/ToastContext';
import { commands } from '../../services/bindings';
import { unwrap } from '../../utils/specta';

/** Default AUR workspace path (XDG cache); backend uses dirs::cache_dir() + monarch/build. */
const AUR_WORKSPACE_PATH_DEFAULT = '~/.cache/monarch/build';

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
            unwrap(await commands.clearBuildCache());
            success("Build cache cleared successfully.");
        } catch (e) {
            error("Failed to clear build cache: " + String(e));
        } finally {
            setIsClearing(false);
        }
    };

    return (
        <div className="space-y-8 animate-in fade-in slide-in-from-bottom-2 duration-300">
            {/* Native AUR Engine: header + config */}
            <section className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-2xl p-6 shadow-sm dark:shadow-none ring-2 ring-blue-500/20">
                <div className="flex items-start gap-3 mb-6">
                    <div className="p-2 rounded-lg text-blue-600 dark:text-blue-400 shrink-0">
                        <PackageOpen size={24} aria-hidden />
                    </div>
                    <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-2">
                            <h2 className="text-xl font-bold text-slate-900 dark:text-white">Native AUR Engine</h2>
                            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md bg-slate-100 dark:bg-white/10 text-xs text-slate-600 dark:text-white/70 font-medium">
                                <Info size={12} aria-hidden />
                                Powered by libalpm &amp; git2
                            </span>
                        </div>
                        <p className="text-sm text-slate-500 dark:text-white/50 mt-0.5">
                            Configure the internal compilation core used to build and install AUR packages.
                        </p>
                    </div>
                </div>

                <div className="space-y-6">
                    {/* AUR Workspace Path (read-only) */}
                    <div className="flex flex-col gap-1">
                        <h3 className="font-bold text-slate-900 dark:text-white flex items-center gap-2">
                            <Terminal size={16} className="text-slate-400" aria-hidden />
                            AUR Workspace Path
                        </h3>
                        <p className="text-sm font-mono text-slate-600 dark:text-white/60 bg-slate-100 dark:bg-white/5 rounded-lg px-3 py-2 border border-slate-200 dark:border-white/10">
                            {AUR_WORKSPACE_PATH_DEFAULT}
                        </p>
                        <p className="text-xs text-slate-400 dark:text-white/30">
                            XDG cache directory where AUR source and build artifacts are stored.
                        </p>
                    </div>

                    <div className="h-px bg-slate-100 dark:bg-white/5 w-full" />

                    {/* Show Build Logs */}
                    <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                        <div className="space-y-1">
                            <h3 className="font-bold text-slate-900 dark:text-white flex items-center gap-2">
                                <Terminal size={16} className="text-slate-400" aria-hidden />
                                Show Build Logs
                            </h3>
                            <p className="text-sm text-slate-500 dark:text-white/50 max-w-md">
                                Enable real-time output during the compilation process.
                            </p>
                        </div>
                        <button
                            onClick={() => setVerboseLogsEnabled(!verboseLogsEnabled)}
                            className={clsx(
                                "relative w-14 h-8 rounded-full p-1 transition-all duration-300 focus:outline-none focus:ring-2 focus:ring-blue-500/50 shrink-0 ring-2 ring-blue-500/30",
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

                    {/* Flush Build Cache (toggle) */}
                    <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                        <div className="space-y-1">
                            <h3 className="font-bold text-slate-900 dark:text-white flex items-center gap-2">
                                <Trash2 size={16} className="text-slate-400" aria-hidden />
                                Flush Build Cache
                            </h3>
                            <p className="text-sm text-slate-500 dark:text-white/50 max-w-md">
                                Remove temporary build files after each successful AUR install.
                            </p>
                        </div>
                        <button
                            onClick={() => setCleanBuild(!cleanBuild)}
                            className={clsx(
                                "relative w-14 h-8 rounded-full p-1 transition-all duration-300 focus:outline-none focus:ring-2 focus:ring-blue-500/50 shrink-0 ring-2 ring-blue-500/30",
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

            {/* Compilation Cores & Build Artifacts */}
            <section className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-2xl p-6 shadow-sm dark:shadow-none space-y-4 ring-2 ring-blue-500/20">
                    <div className="flex items-center gap-2 text-slate-900 dark:text-white font-bold">
                        <Cpu size={20} className="text-blue-600 dark:text-blue-400" aria-hidden />
                        Compilation Cores
                    </div>
                    <div className="space-y-2">
                        <label className="text-sm text-slate-500 dark:text-white/50 block" htmlFor="compilation-cores">
                            Max parallel makepkg jobs
                        </label>
                        <div className="relative">
                            <select
                                id="compilation-cores"
                                value={parallelDownloads}
                                onChange={(e) => setParallelDownloads(parseInt(e.target.value, 10))}
                                className="w-full appearance-none bg-slate-100 dark:bg-white/5 border border-slate-200 dark:border-white/10 rounded-xl px-4 py-2.5 text-sm font-bold text-slate-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500/40 transition-all cursor-pointer"
                            >
                                <option value={1}>1 core (serial)</option>
                                <option value={3}>3 cores (balanced)</option>
                                <option value={5}>5 cores (fast)</option>
                                <option value={10}>10 cores (max)</option>
                            </select>
                            <div className="absolute right-4 top-1/2 -translate-y-1/2 pointer-events-none text-slate-400" aria-hidden>
                                <ChevronDown size={16} />
                            </div>
                        </div>
                    </div>
                    <p className="text-xs text-slate-400 dark:text-white/30 flex items-start gap-1.5 leading-relaxed">
                        <Info size={12} className="shrink-0 mt-0.5" aria-hidden />
                        More cores speed up AUR builds; higher values increase CPU and I/O load.
                    </p>
                </div>

                <div className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-2xl p-6 shadow-sm dark:shadow-none flex flex-col justify-between ring-2 ring-blue-500/20">
                    <div className="space-y-2">
                        <div className="flex items-center gap-2 text-slate-900 dark:text-white font-bold">
                            <Trash2 size={20} className="text-red-500" aria-hidden />
                            Build Artifacts
                        </div>
                        <p className="text-sm text-slate-500 dark:text-white/50 leading-relaxed">
                            Delete cached source and build output in the AUR workspace to free disk space.
                        </p>
                    </div>

                    <button
                        onClick={handleClearBuildCache}
                        disabled={isClearing}
                        className="w-full bg-red-500/10 hover:bg-red-500/20 border-2 border-red-500/40 text-red-600 dark:text-red-400 font-bold py-3 rounded-xl transition-all active:scale-95 disabled:opacity-50 mt-4 flex items-center justify-center gap-2 focus:outline-none focus:ring-2 focus:ring-red-500/50"
                    >
                        {isClearing ? (
                            <div className="w-4 h-4 border-2 border-red-600/30 border-t-red-600 rounded-full animate-spin" aria-hidden />
                        ) : (
                            <Trash2 size={16} aria-hidden />
                        )}
                        Flush Build Cache
                    </button>
                </div>
            </section>
        </div>
    );
}
