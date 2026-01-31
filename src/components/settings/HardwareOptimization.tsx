import { Rocket, Zap, Globe, RefreshCw } from 'lucide-react';
import { clsx } from 'clsx';
import type { Repository } from '../../hooks/useSettings';

export interface SystemInfo {
    kernel: string;
    distro: string;
    cpu_optimization: string;
    pacman_version: string;
}

export interface HardwareOptimizationProps {
    systemInfo: SystemInfo | null;
    repos: Repository[];
    prioritizeOptimized: boolean;
    onPrioritizeOptimized: (enabled: boolean) => void;
    parallelDownloads: number;
    onParallelDownloads: (value: number) => void;
    isRankingMirrors: boolean;
    onRankMirrors: () => void;
    /** Distro-aware: "pacman-mirrors" | "reflector" | "rate-mirrors" | null. Used for label so we never suggest reflector on Manjaro. */
    mirrorRankTool?: string | null;
}

export default function HardwareOptimization({
    systemInfo,
    repos,
    prioritizeOptimized,
    onPrioritizeOptimized,
    parallelDownloads,
    onParallelDownloads,
    isRankingMirrors,
    onRankMirrors,
    mirrorRankTool = null,
}: HardwareOptimizationProps) {
    return (
        <section>
            <h2 className="text-lg font-semibold tracking-tight text-app-fg mb-4 flex items-center gap-2">
                <Rocket size={20} className="text-app-muted" />
                Performance & Hardware
            </h2>
            <div className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-xl p-6 space-y-6 shadow-sm dark:shadow-none">
                {/* CPU Optimization card — only when optimization is detected (not "None") */}
                {systemInfo?.cpu_optimization && systemInfo.cpu_optimization !== 'None' && (
                    <div className="relative group">
                        <div className="absolute inset-0 bg-purple-500/20 blur-3xl opacity-20 group-hover:opacity-40 transition-opacity duration-500" />
                        <div className="relative bg-purple-500/5 dark:bg-purple-500/10 border border-purple-500/20 dark:border-purple-500/10 rounded-xl p-6">
                            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 mb-4">
                                <div className="flex items-center gap-4 min-w-0 flex-1 max-w-prose">
                                    <div className="p-3 bg-purple-500/10 rounded-lg text-purple-600 dark:text-purple-400 shrink-0 w-10 h-10 flex items-center justify-center">
                                        <Zap size={20} />
                                    </div>
                                    <div className="min-w-0">
                                        <h3 className="text-lg font-semibold tracking-tight text-app-fg">
                                            CPU Optimization: {systemInfo.cpu_optimization.toUpperCase()}
                                        </h3>
                                        <p className="text-sm text-app-muted leading-relaxed mt-1 break-words">
                                            Prioritize optimized binaries for {systemInfo.cpu_optimization} architecture
                                        </p>
                                    </div>
                                </div>
                                <button
                                    type="button"
                                    role="switch"
                                    aria-checked={prioritizeOptimized}
                                    aria-label={prioritizeOptimized ? "Disable optimized binaries priority" : "Enable optimized binaries priority"}
                                    onClick={() => onPrioritizeOptimized(!prioritizeOptimized)}
                                    className={clsx(
                                        "w-14 h-8 rounded-full p-1 transition-all shadow-lg focus:outline-none focus:ring-2 focus:ring-purple-500/50",
                                        prioritizeOptimized
                                            ? "bg-purple-600 shadow-purple-500/30"
                                            : "bg-slate-200 dark:bg-white/10"
                                    )}
                                >
                                    <div className={clsx(
                                        "w-6 h-6 bg-white shadow-xl rounded-full transition-transform duration-300",
                                        prioritizeOptimized ? "translate-x-6" : "translate-x-0"
                                    )} />
                                </button>
                            </div>
                            {prioritizeOptimized && (
                                <div className="mt-4 pt-4 border-t border-purple-500/20">
                                    <p className="text-xs font-bold uppercase tracking-widest text-slate-400 dark:text-white/40 mb-2">
                                        Optimized Repositories
                                    </p>
                                    <div className="flex flex-wrap gap-2">
                                        {repos
                                            .filter(r => {
                                                const opt = systemInfo.cpu_optimization.toLowerCase();
                                                return r.name.toLowerCase().includes(opt) ||
                                                    (opt === 'znver4' && r.name.toLowerCase().includes('znver4'));
                                            })
                                            .map(repo => (
                                                <span
                                                    key={repo.name}
                                                    className="px-2 py-1 bg-purple-500/10 dark:bg-purple-500/20 text-purple-600 dark:text-purple-400 text-[10px] font-bold rounded border border-purple-500/20"
                                                >
                                                    {repo.name}
                                                </span>
                                            ))}
                                        {repos.filter(r => {
                                            const opt = systemInfo.cpu_optimization.toLowerCase();
                                            return r.name.toLowerCase().includes(opt) ||
                                                (opt === 'znver4' && r.name.toLowerCase().includes('znver4'));
                                        }).length === 0 && (
                                            <span className="text-xs text-slate-500 dark:text-white/50 italic">
                                                No optimized repositories enabled. Enable CachyOS repos to see optimized packages.
                                            </span>
                                        )}
                                    </div>
                                </div>
                            )}
                        </div>
                    </div>
                )}

                {/* Parallel Downloads */}
                <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pt-6 border-t border-app-border">
                    <div className="min-w-0 flex-1 max-w-prose">
                        <h3 className="text-lg font-semibold tracking-tight text-app-fg">Parallel Downloads</h3>
                        <p className="text-sm text-app-muted leading-relaxed mt-1 break-words">
                            Configure /etc/pacman.conf download threads (1-10). Higher values speed up downloads but use more bandwidth.
                        </p>
                    </div>
                    <div className="flex items-center gap-4">
                        <input
                            type="range"
                            min="1"
                            max="10"
                            value={parallelDownloads}
                            onChange={(e) => onParallelDownloads(parseInt(e.target.value, 10))}
                            className="w-32 accent-blue-600"
                            aria-label="Parallel downloads slider"
                        />
                        <span className="text-lg font-bold text-slate-800 dark:text-white w-8 text-center">
                            {parallelDownloads}
                        </span>
                    </div>
                </div>

                {/* Mirror Ranking */}
                <div className="flex items-center justify-between pt-4 border-t border-slate-200 dark:border-white/5">
                    <div className="max-w-[60%]">
                        <h3 className="font-bold text-slate-800 dark:text-white text-lg flex items-center gap-2">
                            <Globe size={18} />
                            Mirror Speed Optimization
                        </h3>
                        <p className="text-sm text-slate-500 dark:text-white/50 mt-1">
                            {mirrorRankTool === 'pacman-mirrors'
                                ? 'Rank Manjaro mirrors by download speed. This operation takes ~30 seconds.'
                                : 'Rank mirrors by download speed (reflector/rate-mirrors). This operation takes ~30 seconds.'}
                        </p>
                    </div>
                    <button
                        onClick={onRankMirrors}
                        disabled={isRankingMirrors}
                        aria-label="Rank mirrors by speed"
                        className={clsx(
                            "px-6 py-3 rounded-xl font-bold shadow-lg disabled:opacity-50 flex items-center gap-2 transition-all",
                            isRankingMirrors
                                ? "bg-slate-100 dark:bg-white/5 text-slate-400 dark:text-white/50 cursor-not-allowed"
                                : "bg-blue-600 hover:bg-blue-500 text-white shadow-blue-500/20 active:scale-95"
                        )}
                    >
                        <RefreshCw size={18} className={isRankingMirrors ? "animate-spin" : ""} />
                        {isRankingMirrors ? "Ranking..." : mirrorRankTool ? `Rank Mirrors (${mirrorRankTool === 'pacman-mirrors' ? 'Manjaro' : mirrorRankTool})` : "Rank Mirrors"}
                    </button>
                </div>
            </div>
        </section>
    );
}
