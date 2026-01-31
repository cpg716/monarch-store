import { useState } from 'react';
import {
    RefreshCw, Package, ArrowUp, ArrowDown, Lock, Clock, ChevronDown,
    Eye, EyeOff, HelpCircle, AlertTriangle, Gauge,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import type { Repository } from '../../hooks/useSettings';
import type { DistroContext } from '../../hooks/useDistro';

export interface MirrorTestResult {
    url: string;
    latency_ms: number | null;
}

export interface RepositoriesTabProps {
    // Sync
    isSyncing: boolean;
    syncProgressMessage: string | null;
    triggerManualSync: () => void;
    repoCounts: Record<string, number>;
    isAurEnabled: boolean;
    toggleAur: (enabled: boolean) => void;
    syncOnStartupEnabled: boolean;
    setSyncOnStartup: (enabled: boolean) => void;
    syncIntervalHours: number;
    updateSyncInterval: (hours: number) => void;
    // Repos
    repos: Repository[];
    toggleRepo: (id: string) => void;
    reorderRepos: (repos: Repository[]) => void;
    repoSyncStatus: Record<string, boolean> | null;
    distro: DistroContext;
    isRepoLocked: (name: string) => boolean;
    reportWarning: (msg: string) => void;
    reportError?: (msg: string) => void;
}

export default function RepositoriesTab({
    isSyncing,
    syncProgressMessage,
    triggerManualSync,
    repoCounts,
    isAurEnabled,
    toggleAur,
    syncOnStartupEnabled,
    setSyncOnStartup,
    syncIntervalHours,
    updateSyncInterval,
    repos,
    toggleRepo,
    reorderRepos,
    repoSyncStatus,
    distro,
    isRepoLocked,
    reportWarning,
    reportError,
}: RepositoriesTabProps) {
    const [testingRepo, setTestingRepo] = useState<string | null>(null);
    const [mirrorResults, setMirrorResults] = useState<Record<string, MirrorTestResult[]>>({});

    const handleTestMirrors = async (repo: Repository) => {
        const key = repo.name;
        setTestingRepo(key);
        setMirrorResults((prev) => ({ ...prev, [key]: [] }));
        try {
            const result = await invoke<MirrorTestResult[]>('test_mirrors', { repoKey: key });
            setMirrorResults((prev) => ({ ...prev, [key]: result ?? [] }));
        } catch (e) {
            const msg = e instanceof Error ? e.message : String(e);
            if (reportError) reportError(msg);
            setMirrorResults((prev) => ({
                ...prev,
                [key]: [{ url: msg, latency_ms: null }],
            }));
        } finally {
            setTestingRepo(null);
        }
    };

    const moveRepo = (index: number, direction: 'up' | 'down') => {
        const newRepos = [...repos];
        if (direction === 'up' && index > 0) {
            [newRepos[index], newRepos[index - 1]] = [newRepos[index - 1], newRepos[index]];
        } else if (direction === 'down' && index < newRepos.length - 1) {
            [newRepos[index], newRepos[index + 1]] = [newRepos[index + 1], newRepos[index]];
        }
        reorderRepos(newRepos);
    };

    return (
        <div className="space-y-8">
            {/* Repository Control */}
            <section>
                <h2 className="text-lg font-semibold tracking-tight text-app-fg mb-4">
                    Repository Control
                </h2>
                <div className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-xl p-6 hover:bg-app-card/80 dark:hover:bg-white/10 transition-colors shadow-sm dark:shadow-none">
                    <div className="flex flex-col md:flex-row items-center justify-between mb-6 gap-6">
                        <div className="min-w-0 flex-1 max-w-prose">
                            <h3 className="text-lg font-semibold tracking-tight text-app-fg mb-2 flex items-center gap-2">
                                <RefreshCw size={20} className={clsx(isSyncing ? "animate-spin text-blue-500" : "text-app-muted")} />
                                Force Database Synchronization
                            </h3>
                            <p className="text-sm text-app-muted leading-relaxed break-words">
                                Refresh local package catalogs. This updates Store listings while keeping your system configuration intact.
                            </p>
                        </div>
                        <button
                            onClick={triggerManualSync}
                            disabled={isSyncing}
                            aria-label="Sync repositories now"
                            aria-busy={isSyncing}
                            className={clsx(
                                "px-6 py-3 rounded-lg font-bold transition-all flex items-center gap-2 text-sm shadow-lg min-w-[120px] justify-center shrink-0",
                                isSyncing
                                    ? "bg-slate-100 dark:bg-white/5 text-slate-400 dark:text-white/50 cursor-not-allowed border border-slate-200 dark:border-white/5"
                                    : "bg-blue-600 hover:bg-blue-500 text-white shadow-blue-500/20 active:scale-95 border border-white/10"
                            )}
                        >
                            <RefreshCw size={20} className={isSyncing ? "animate-spin" : ""} />
                            {isSyncing ? 'Syncing...' : 'Sync Now'}
                        </button>
                    </div>
                    {isSyncing && syncProgressMessage && (
                        <p className="text-sm text-slate-600 dark:text-white/70 font-medium mt-2" aria-live="polite">
                            {syncProgressMessage}
                        </p>
                    )}

                    <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4 pt-8 border-t border-slate-200 dark:border-white/5">
                        {Object.entries(repoCounts).sort((a, b) => b[1] - a[1]).map(([name, count]) => (
                            <div key={name} className="flex flex-col items-center justify-center bg-app-fg/5 p-4 rounded-lg border border-app-border h-24">
                                <span className="text-[10px] uppercase font-bold text-slate-500 dark:text-white/40 mb-1 tracking-widest">{name}</span>
                                <span className="text-xl font-black text-slate-800 dark:text-white">{count.toLocaleString()}</span>
                            </div>
                        ))}
                        {isAurEnabled && (
                            <div className="flex flex-col items-center justify-center bg-amber-500/10 p-4 rounded-lg border border-amber-500/20 h-24">
                                <span className="text-[10px] uppercase font-bold text-amber-600 dark:text-amber-500 mb-1 tracking-widest">AUR</span>
                                <span className="text-xl font-black text-amber-600 dark:text-amber-500">Active</span>
                            </div>
                        )}
                        {Object.keys(repoCounts).length === 0 && !isAurEnabled && (
                            <div className="col-span-full text-center text-sm text-slate-400 dark:text-white/30 italic py-4">
                                Waiting for synchronization...
                            </div>
                        )}
                    </div>

                    <div className="pt-6 border-t border-slate-200 dark:border-white/5 mt-6">
                        <div className="flex items-center justify-between gap-4">
                            <div className="flex items-center gap-4">
                                <div className="p-3 bg-slate-100 dark:bg-white/10 rounded-xl text-slate-600 dark:text-white/70">
                                    <RefreshCw size={24} />
                                </div>
                                <div>
                                    <h4 className="font-bold text-slate-800 dark:text-white text-lg">Sync repositories when app starts</h4>
                                    <p className="text-sm text-slate-500 dark:text-white/50">
                                        Refresh package databases on launch (or use Sync Now manually)
                                    </p>
                                </div>
                            </div>
                            <button
                                type="button"
                                role="switch"
                                aria-checked={syncOnStartupEnabled}
                                aria-label={syncOnStartupEnabled ? 'Sync on startup is on' : 'Sync on startup is off'}
                                onClick={() => setSyncOnStartup(!syncOnStartupEnabled)}
                                className={clsx(
                                    'relative inline-flex h-8 w-14 shrink-0 rounded-full border-2 transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500/50',
                                    syncOnStartupEnabled ? 'border-blue-500 bg-blue-500' : 'border-slate-300 dark:border-white/20 bg-slate-200 dark:bg-white/10'
                                )}
                            >
                                <span
                                    className={clsx(
                                        'pointer-events-none inline-block h-7 w-7 transform rounded-full bg-white shadow ring-0 transition',
                                        syncOnStartupEnabled ? 'translate-x-6' : 'translate-x-1'
                                    )}
                                />
                            </button>
                        </div>
                    </div>

                    <div className="pt-6 border-t border-app-border mt-6">
                        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                            <div className="flex items-center gap-4 min-w-0 flex-1 max-w-prose">
                                <div className="p-3 bg-blue-500/10 dark:bg-blue-500/20 rounded-lg text-blue-600 dark:text-blue-400 shrink-0 w-10 h-10 flex items-center justify-center">
                                    <Clock size={20} />
                                </div>
                                <div className="min-w-0">
                                    <h4 className="text-lg font-semibold tracking-tight text-app-fg">Auto Sync Interval</h4>
                                    <p className="text-sm text-app-muted leading-relaxed break-words">
                                        Automatically refresh package databases
                                    </p>
                                </div>
                            </div>
                            <div className="relative shrink-0">
                                <select
                                    value={syncIntervalHours}
                                    onChange={(e) => updateSyncInterval(parseInt(e.target.value, 10))}
                                    className="appearance-none bg-app-fg/5 border border-app-border rounded-lg px-4 py-2 pr-10 text-app-fg font-bold focus:outline-none focus:ring-2 focus:ring-blue-500/50 cursor-pointer min-w-[120px]"
                                    aria-label="Auto sync interval"
                                >
                                    <option value={1}>Every 1 hour</option>
                                    <option value={3}>Every 3 hours</option>
                                    <option value={6}>Every 6 hours</option>
                                    <option value={12}>Every 12 hours</option>
                                    <option value={24}>Every 24 hours</option>
                                </select>
                                <ChevronDown size={16} className="absolute right-4 top-1/2 -translate-y-1/2 text-slate-500 dark:text-white/50 pointer-events-none" />
                            </div>
                        </div>
                    </div>
                </div>
            </section>

            {/* Software Sources */}
            <section>
                <h2 className="text-lg font-semibold tracking-tight text-app-fg mb-4 flex items-center gap-2">
                    <Package size={20} className="text-app-muted" /> Software Sources
                </h2>
                <div className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-xl p-6 shadow-sm dark:shadow-none">
                    <p className="text-sm text-app-muted mb-6 max-w-prose leading-relaxed break-words">
                        Toggling a source here <strong className="text-app-fg">hides it from the Store</strong> but keeps it active in the system. Your installed apps <strong className="text-green-600 dark:text-green-400">continue to update safely</strong>.
                    </p>

                    <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                        {repos.map((repo, idx) => {
                            const locked = isRepoLocked(repo.name);
                            return (
                                <div
                                    key={repo.name}
                                    tabIndex={0}
                                    onKeyDown={(e) => {
                                        if ((e.key === 'Enter' || e.key === ' ') && !locked) {
                                            e.preventDefault();
                                            toggleRepo(repo.id);
                                        }
                                    }}
                                    className={clsx(
                                        "relative flex flex-col p-6 rounded-xl border border-app-border transition-all duration-300 group overflow-hidden",
                                        "focus:outline-none focus:ring-2 focus:ring-blue-500/50",
                                        repo.enabled ? "bg-app-card/80 dark:bg-white/5 hover:shadow-lg hover:-translate-y-0.5" : "bg-app-fg/5 dark:bg-black/20 opacity-80 hover:opacity-100"
                                    )}
                                >
                                    <div className="flex items-center justify-between w-full relative z-10">
                                        <div className="flex items-center gap-4">
                                            <div className="flex flex-col gap-1 text-slate-300 dark:text-white/20 group-hover:text-slate-500 dark:group-hover:text-white/60 transition-colors" role="group" aria-label={`Reorder ${repo.name}`}>
                                                <button
                                                    type="button"
                                                    onClick={() => moveRepo(idx, 'up')}
                                                    onKeyDown={(e) => {
                                                        if ((e.key === 'Enter' || e.key === ' ') && idx > 0) {
                                                            e.preventDefault();
                                                            moveRepo(idx, 'up');
                                                        }
                                                    }}
                                                    disabled={idx === 0}
                                                    aria-label={`Move ${repo.name} up in priority`}
                                                    className="hover:text-slate-800 dark:hover:text-white disabled:opacity-0 transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500/50 rounded"
                                                >
                                                    <ArrowUp size={16} />
                                                </button>
                                                <button
                                                    type="button"
                                                    onClick={() => moveRepo(idx, 'down')}
                                                    onKeyDown={(e) => {
                                                        if ((e.key === 'Enter' || e.key === ' ') && idx < repos.length - 1) {
                                                            e.preventDefault();
                                                            moveRepo(idx, 'down');
                                                        }
                                                    }}
                                                    disabled={idx === repos.length - 1}
                                                    aria-label={`Move ${repo.name} down in priority`}
                                                    className="hover:text-slate-800 dark:hover:text-white disabled:opacity-0 transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500/50 rounded"
                                                >
                                                    <ArrowDown size={16} />
                                                </button>
                                            </div>
                                            <div>
                                                <h4 className={clsx("font-bold text-lg", repo.enabled ? "text-slate-800 dark:text-white" : "text-slate-500 dark:text-white/50")}>
                                                    {repo.name}
                                                    {idx === 0 && repo.enabled && <span className="ml-3 text-[10px] bg-blue-500/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-300 border border-blue-500/20 dark:border-blue-500/30 px-2 py-0.5 rounded-full uppercase tracking-wider font-bold">Primary</span>}
                                                    {locked && (
                                                        <span className="ml-3 text-[10px] bg-red-500/10 dark:bg-red-500/20 text-red-600 dark:text-red-300 border border-red-500/20 dark:border-red-500/30 px-2 py-0.5 rounded-full uppercase tracking-wider font-bold flex items-center gap-1 inline-flex">
                                                            <Lock size={8} /> Blocked by {distro.pretty_name}
                                                        </span>
                                                    )}
                                                </h4>
                                                <p className="text-xs text-slate-600 dark:text-white/60 mt-1 line-clamp-1">{repo.description}</p>
                                                <p className="text-[10px] font-medium mt-2 flex items-center gap-1.5 text-slate-600 dark:text-white/70">
                                                    {locked ? (
                                                        <><Lock size={10} className="shrink-0" /> Incompatible with your system</>
                                                    ) : repo.enabled ? (
                                                        <><Eye size={10} className="shrink-0 text-blue-500 dark:text-blue-400" /> Visible in Store</>
                                                    ) : (
                                                        <><EyeOff size={10} className="shrink-0" /> Hidden from Store · Still receives updates</>
                                                    )}
                                                </p>
                                                {locked && (repo.id === 'chaotic-aur' || repo.name === 'Chaotic-AUR') && (
                                                    <details className="mt-3 group/why">
                                                        <summary className="text-[10px] font-bold text-slate-600 dark:text-white/70 cursor-pointer hover:text-slate-800 dark:hover:text-white/90 inline-flex items-center gap-1.5 list-none [&::-webkit-details-marker]:hidden focus:outline-none focus:ring-2 focus:ring-blue-500/50 rounded">
                                                            <HelpCircle size={10} className="shrink-0" /> Why is this blocked?
                                                        </summary>
                                                        <p className="mt-2 text-[10px] text-slate-700 dark:text-white/80 leading-relaxed pl-4 border-l-2 border-red-500/30">
                                                            Chaotic-AUR builds are tied to Arch's glibc and kernel ABI. On {distro.pretty_name} those differ, so packages from this repo can cause library conflicts and partial upgrades. Keeping it disabled avoids breakage.
                                                        </p>
                                                    </details>
                                                )}
                                            </div>
                                        </div>
                                        <div className="flex items-center gap-3">
                                            <button
                                                type="button"
                                                onClick={() => handleTestMirrors(repo)}
                                                disabled={testingRepo !== null}
                                                aria-label={`Test mirrors for ${repo.name}`}
                                                className={clsx(
                                                    "flex items-center gap-2 px-3 py-2 rounded-xl text-sm font-medium transition-colors",
                                                    testingRepo !== null
                                                        ? "bg-slate-100 dark:bg-white/5 text-slate-400 dark:text-white/40 cursor-not-allowed"
                                                        : "bg-slate-100 dark:bg-black/20 text-slate-700 dark:text-white/80 hover:bg-slate-200 dark:hover:bg-white/10 border border-slate-200 dark:border-white/10"
                                                )}
                                            >
                                                <Gauge size={16} className={testingRepo === repo.name ? 'animate-pulse' : ''} />
                                                {testingRepo === repo.name ? 'Testing…' : 'Test Mirrors'}
                                            </button>
                                            <button
                                                type="button"
                                                role="switch"
                                                aria-checked={repo.enabled}
                                                aria-disabled={locked}
                                                aria-label={locked ? `${repo.name} is blocked by ${distro.pretty_name}` : repo.enabled ? `Hide ${repo.name} from Store` : `Show ${repo.name} in Store`}
                                                onClick={() => {
                                                    if (locked) {
                                                        reportWarning(`This repository is incompatible with ${distro.pretty_name}.`);
                                                        return;
                                                    }
                                                    toggleRepo(repo.id);
                                                }}
                                                disabled={locked}
                                                className={clsx(
                                                    "w-12 h-7 rounded-full p-1 transition-all",
                                                    locked ? "bg-red-500/10 cursor-not-allowed border border-red-500/20" :
                                                        repo.enabled ? "bg-blue-600 shadow-lg shadow-blue-500/30" : "bg-slate-300 dark:bg-white/10"
                                                )}
                                            >
                                                <div className={clsx(
                                                    "w-5 h-5 shadow-xl rounded-full transition-transform duration-300",
                                                    locked ? "bg-red-500/50 translate-x-0" :
                                                        repo.enabled ? "translate-x-5 bg-white" : "translate-x-0 bg-white"
                                                )} />
                                            </button>
                                        </div>
                                    </div>
                                    {mirrorResults[repo.name]?.length > 0 && (
                                        <div className="mt-4 pt-4 border-t border-slate-200 dark:border-white/5">
                                            <p className="text-[10px] uppercase font-bold text-slate-500 dark:text-white/40 mb-2 tracking-widest">Top mirrors (latency)</p>
                                            <ul className="space-y-1.5 text-xs text-slate-700 dark:text-white/80 font-mono">
                                                {mirrorResults[repo.name].slice(0, 3).map((m) => (
                                                    <li key={m.url} className="truncate break-all">
                                                        {m.url.startsWith('http') ? (
                                                            <span>{m.url.replace(/^https?:\/\//, '').split('/')[0]}</span>
                                                        ) : (
                                                            <span className="text-amber-600 dark:text-amber-400">{m.url}</span>
                                                        )}
                                                        {m.latency_ms != null && (
                                                            <span className="ml-2 text-slate-500 dark:text-white/50 font-sans">({m.latency_ms} ms)</span>
                                                        )}
                                                    </li>
                                                ))}
                                            </ul>
                                        </div>
                                    )}
                                    {repo.enabled && (
                                        <div className="absolute -bottom-10 -right-10 w-32 h-32 bg-blue-500/5 dark:bg-blue-500/10 blur-3xl rounded-full pointer-events-none group-hover:bg-blue-500/10 dark:group-hover:bg-blue-500/20 transition-colors" />
                                    )}
                                    {repo.enabled && repoSyncStatus && repoSyncStatus[repo.name] === false && (
                                        <div className="mt-4 pt-4 border-t border-slate-200 dark:border-white/5 flex items-start gap-3 animate-in slide-in-from-top-2">
                                            <AlertTriangle size={16} className="text-amber-500 shrink-0 mt-0.5" />
                                            <div>
                                                <p className="text-xs font-bold text-amber-500">Sync Required</p>
                                                <p className="text-[10px] text-slate-400 dark:text-white/40 mt-0.5">Database missing. Will auto-fix on next install.</p>
                                            </div>
                                        </div>
                                    )}
                                </div>
                            );
                        })}
                    </div>

                    <div className="mt-6 pt-6 border-t border-app-border">
                        <div className="relative overflow-hidden flex flex-col md:flex-row items-center justify-between p-6 rounded-xl border border-amber-500/20 bg-amber-500/5 group">
                            <div className="absolute inset-0 bg-amber-500/10 blur-3xl opacity-0 group-hover:opacity-20 transition-opacity duration-500" />
                            <div className="flex items-center gap-4 relative z-10">
                                <div className="p-4 bg-amber-500/20 rounded-lg text-amber-600 dark:text-amber-500 shadow-lg shadow-amber-900/10 dark:shadow-amber-900/20 w-10 h-10 flex items-center justify-center shrink-0">
                                    <Lock size={24} />
                                </div>
                                <div>
                                    <h4 className="font-bold text-amber-700 dark:text-amber-500 text-lg flex items-center gap-3">
                                        Enable AUR <span className="text-[10px] bg-amber-500 text-white dark:text-black px-2 py-0.5 rounded font-black tracking-widest">EXPERIMENTAL</span>
                                    </h4>
                                    <p className="text-sm text-amber-800/60 dark:text-amber-200/60 mt-1 max-w-md">
                                        Access millions of community-maintained packages.
                                        <br />⚠ Use at your own risk. Not officially supported.
                                    </p>
                                </div>
                            </div>
                            <div className="mt-4 md:mt-0 relative z-10">
                                <button
                                    type="button"
                                    role="switch"
                                    aria-checked={isAurEnabled}
                                    aria-label={isAurEnabled ? 'Disable AUR' : 'Enable AUR'}
                                    onClick={() => toggleAur(!isAurEnabled)}
                                    className={clsx(
                                        "w-14 h-8 rounded-full p-1 transition-all shadow-xl",
                                        isAurEnabled ? "bg-amber-500 shadow-amber-500/20" : "bg-slate-200 dark:bg-white/10"
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
        </div>
    );
}
