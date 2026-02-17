import { useState, useEffect, useCallback } from 'react';
import { RefreshCw, ArrowRight, CheckCircle2, Download, AlertCircle, Unlock, Loader2, Terminal, ShieldCheck, RotateCw } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import ConfirmationModal from '../components/ConfirmationModal';
import CriticalNewsBlockerModal from '../components/CriticalNewsBlockerModal';
import { getReadNewsIds, markNewsItemsAsRead } from '../components/NewsFeed';
import { clsx } from 'clsx';
import { listen } from '@tauri-apps/api/event';
import { useAppStore } from '../store/internal_store';
import { useErrorService } from '../context/ErrorContext';
import { useToast } from '../context/ToastContext';
import { useSessionPassword } from '../context/useSessionPassword';
import { friendlyError } from '../utils/friendlyError';
import { commands, UpdateItem, NewsItem, AppMetadata } from '../services/bindings';
import { unwrap } from '../utils/specta';

import RepoBadge from '../components/RepoBadge';


// Helper component for Icon
import archLogo from '../assets/arch-logo.png';
import { resolveIconUrl } from '../utils/iconHelper';

// Pure component - receives icon URL from parent (batch loaded)
const AppIcon = ({ pkgId, iconUrl }: { pkgId: string, iconUrl?: string | null }) => {
    const displayIcon = resolveIconUrl(iconUrl) || archLogo;
    return <img src={displayIcon} alt={pkgId} className={clsx("w-full h-full object-contain", !iconUrl && "opacity-50 grayscale")} />;
};

export default function UpdatesPage() {
    const errorService = useErrorService();
    const { success: toastSuccess } = useToast();
    const { requestSessionPassword } = useSessionPassword();
    const reducePasswordPrompts = useAppStore((s) => s.reducePasswordPrompts);
    const {
        isUpdating,
        updateProgress: progress,
        updateStatus: statusMessage,
        updateLogs: logs,
        rebootRequired,
        pacnewWarnings,
        pendingServiceRestarts,
        setUpdating,
        setPacnewWarnings,
        clearUpdateLogs
    } = useAppStore();

    const [updates, setUpdates] = useState<UpdateItem[]>([]);
    const [metadataCache, setMetadataCache] = useState<Record<string, AppMetadata>>({});
    const [isChecking, setIsChecking] = useState(true);
    const [updateResult, setUpdateResult] = useState<string | null>(null);
    const [showConsole, setShowConsole] = useState(false);
    const [password, setPassword] = useState('');
    const [currentStep, setCurrentStep] = useState(0);
    const [fixingLock, setFixingLock] = useState(false);
    const [showAuthHint, setShowAuthHint] = useState(false);
    const [orphansAfterUpdate, setOrphansAfterUpdate] = useState<string[]>([]);
    const [removingOrphans, setRemovingOrphans] = useState(false);
    const [newsItems, setNewsItems] = useState<NewsItem[]>([]);
    const [showBlockerModal, setShowBlockerModal] = useState(false);
    const [unreadCriticalItems, setUnreadCriticalItems] = useState<NewsItem[]>([]);
    const [snapshotStatus, setSnapshotStatus] = useState<{ tool: string, is_configured: boolean, message: string } | null>(null);
    const [doSnapshot, setDoSnapshot] = useState(true);
    const [viewingPkgbuild, setViewingPkgbuild] = useState<string | null>(null);
    const [pkgbuildContent, setPkgbuildContent] = useState<string>('');
    const [isLoadingPkgbuild, setIsLoadingPkgbuild] = useState(false);

    // Batch fetch metadata for updates
    useEffect(() => {
        if (updates.length > 0) {
            commands.getSnapshotStatus().then(unwrap).then(setSnapshotStatus).catch(() => { });
        }
    }, [updates.length]);

    const handleViewPkgbuild = async (name: string) => {
        setViewingPkgbuild(name);
        setIsLoadingPkgbuild(true);
        setPkgbuildContent('');
        try {
            const content = unwrap(await commands.fetchPkgbuild(name));
            setPkgbuildContent(content);
        } catch (e) {
            setPkgbuildContent(`Failed to load PKGBUILD: ${e}`);
        } finally {
            setIsLoadingPkgbuild(false);
        }
    };

    // Batch fetch metadata for updates
    useEffect(() => {
        if (updates.length === 0) return;

        // Identify which updates are missing from our cache
        const missing = updates.filter(u => !metadataCache[u.name]).map(u => u.name);

        if (missing.length > 0) {
            commands.getMetadataBatch(missing).then(unwrap).then(newMeta => {
                setMetadataCache(prev => ({ ...prev, ...newMeta }));
            }).catch(e => console.error("Failed to batch load metadata:", e));
        }
    }, [updates]); // Run whenever updates list updates (e.g. initial load or post-update check)

    const isLockOrBusyError = updateResult != null && /lock|busy|database.*(locked|busy)/i.test(updateResult);

    // If update is "stuck" on auth/connectivity for 5s, show hint (password dialog may be hidden).
    useEffect(() => {
        if (!isUpdating) {
            setShowAuthHint(false);
            return;
        }
        const t = window.setTimeout(() => {
            setShowAuthHint(true);
        }, 5000);
        return () => window.clearTimeout(t);
    }, [isUpdating]);

    const steps = [
        "Synchronizing Databases",
        "Upgrading System",
        "Updating Community Apps",
        "Updating Flatpaks"
    ];

    const fetchNews = useCallback(async () => {
        try {
            const list = unwrap(await commands.fetchNews());
            setNewsItems(list ?? []);
        } catch {
            setNewsItems([]);
        }
    }, []);

    useEffect(() => {
        fetchNews();
    }, [fetchNews]);

    useEffect(() => {
        if (statusMessage?.toLowerCase().includes("database") || statusMessage?.toLowerCase().includes("sync")) {
            setCurrentStep(0);
        } else if (statusMessage?.toLowerCase().includes("upgrade") || statusMessage?.toLowerCase().includes("installing core")) {
            setCurrentStep(1);
        } else if (statusMessage?.toLowerCase().includes("aur") || statusMessage?.toLowerCase().includes("community")) {
            setCurrentStep(2);
        } else if (statusMessage?.toLowerCase().includes("flatpak")) {
            setCurrentStep(3);
        }
    }, [statusMessage]);

    const checkForUpdates = async () => {
        setIsChecking(true);
        setUpdateResult(null);
        try {
            // For updates, sources are never "off": repo (incl. Chaotic-AUR), AUR, and Flatpak always included.
            // Discovery toggles (Settings → Sources) only affect search/browse, not the Updates list.
            const pendingUpdates = unwrap(await commands.checkUpdates(true, true));
            // Deduplicate by name:source_type:id to prevent React key collisions
            const seen = new Set<string>();
            const deduped = pendingUpdates.filter(pkg => {
                const key = `${pkg.name}:${pkg.source.source_type}:${pkg.source.id}`;
                if (seen.has(key)) return false;
                seen.add(key);
                return true;
            });
            setUpdates(deduped);
        } catch (e) {
            errorService.reportError(e as Error | string);
        } finally {
            setIsChecking(false);
        }
    };

    const [showConfirm, setShowConfirm] = useState(false);

    // Fetch updates on mount (list always includes repo + AUR + Flatpak for installed packages)
    useEffect(() => {
        checkForUpdates();
    }, []);

    // Listen for update-complete so we don't block the UI waiting for the backend.
    useEffect(() => {
        const unlisten = listen<{ success: boolean; message: string }>('update-complete', async (event) => {
            setUpdating(false);
            setUpdateResult(event.payload.message);
            checkForUpdates();
            try {
                const warnings = unwrap(await commands.getPacnewWarnings());
                setPacnewWarnings(warnings);
            } catch {
                // ignore
            }
            if (event.payload.success) {
                try {
                    const orphans = unwrap(await commands.getOrphans());
                    setOrphansAfterUpdate(orphans || []);
                } catch {
                    setOrphansAfterUpdate([]);
                }
            } else {
                setOrphansAfterUpdate([]);
            }
        });
        return () => {
            unlisten.then((fn) => fn()).catch(() => { });
        };
    }, [setUpdating, setPacnewWarnings]);

    const handleUpdateAll = () => {
        const readIds = getReadNewsIds();
        const unreadCritical = newsItems.filter((i) => i.is_critical && !readIds.includes(i.id));
        if (unreadCritical.length > 0) {
            setUnreadCriticalItems(unreadCritical);
            setShowBlockerModal(true);
        } else {
            setShowConfirm(true);
        }
    };

    const performUpdate = async () => {
        setShowConfirm(false);
        setUpdating(true);
        setUpdateResult(null);
        clearUpdateLogs();
        setCurrentStep(0);

        // For updates, sources are never "off": always run AUR and Flatpak phases for installed packages.
        // Pass modal password when AUR updates require it (backend uses it for makepkg/sudo).
        const pwd = password?.trim() || null;
        setPassword('');

        if (doSnapshot && snapshotStatus?.is_configured) {
            try {
                // We don't block the WHOLE update if snapshot fails, but we try.
                await commands.createSystemSnapshot(snapshotStatus.tool as any, `Monarch Store Update: ${new Date().toISOString()}`).then(unwrap);
            } catch (e) {
                console.error("Snapshot failed:", e);
                // Continue with update anyway, just log it?
                clearUpdateLogs();
            }
        }

        commands.performSystemUpdate(pwd, true, true).catch((e) => {
            errorService.reportError(e as Error | string);
            setUpdateResult(`Update failed: ${e}`);
            setUpdating(false);
        });
    };

    const needsReboot = updates.some(u => u.name === 'linux' || u.name.startsWith('nvidia'));

    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            {/* Header */}
            <div className="p-8 pb-6 border-b border-black/5 dark:border-white/5 bg-app-bg/95 backdrop-blur-3xl z-10 transition-colors shadow-sm dark:shadow-2xl dark:shadow-black/20 sticky top-0">
                <div className="flex items-end justify-between">
                    <div>
                        <h1 className="text-4xl lg:text-5xl font-black flex items-center gap-4 text-slate-900 dark:text-white tracking-tight leading-none mb-2">
                            <span className={clsx("p-2 rounded-2xl bg-blue-500/10 text-blue-500", (isUpdating || isChecking) && "animate-butterfly")}>
                                <RefreshCw size={32} />
                            </span>
                            Updates
                        </h1>
                        <p className="text-lg text-slate-500 dark:text-app-muted font-medium ml-1">
                            {isChecking ? "Checking for updates..." :
                                updates.length === 0 ? "Your system is up to date" :
                                    `${updates.length} updates available`}
                        </p>
                    </div>

                    <div className="flex items-center gap-3 flex-wrap">
                        <button
                            onClick={checkForUpdates}
                            disabled={isChecking || isUpdating}
                            className="px-6 py-3 rounded-xl bg-black/5 dark:bg-white/5 hover:bg-black/10 dark:hover:bg-white/10 text-slate-900 dark:text-white font-bold text-sm border border-black/10 dark:border-white/10 transition-all disabled:opacity-50 flex items-center gap-2 active:scale-95"
                        >
                            <RefreshCw size={18} className={isChecking ? "animate-spin" : ""} />
                            Check Now
                        </button>
                        <button
                            onClick={async () => {
                                try {
                                    const { command } = await commands.getSystemUpdateCommand();
                                    await navigator.clipboard.writeText(command);
                                    toastSuccess('Command copied. Paste in your terminal to run.');
                                } catch (e) {
                                    errorService.reportError(e as Error | string);
                                }
                            }}
                            disabled={isUpdating}
                            className="px-6 py-3 rounded-xl bg-black/5 dark:bg-white/5 hover:bg-black/10 dark:hover:bg-white/10 text-slate-900 dark:text-white font-bold text-sm border border-black/10 dark:border-white/10 transition-all disabled:opacity-50 flex items-center gap-2 active:scale-95"
                            title="Copy full system upgrade command (sudo pacman -Syu) to run in your terminal"
                        >
                            <Terminal size={18} /> Update in terminal
                        </button>
                        {updates.length > 0 && !isUpdating && (
                            <button
                                onClick={handleUpdateAll}
                                className="bg-blue-600 hover:bg-blue-500 text-white px-8 py-3 rounded-xl font-bold text-sm shadow-lg shadow-blue-900/20 active:scale-95 transition-all flex items-center gap-2 border border-white/10 hover:shadow-blue-500/20"
                            >
                                <Download size={20} /> Update All
                            </button>
                        )}
                    </div>
                </div>

                {/* System Status Indicators (NEW: Phase 4 & 5) */}
                {(rebootRequired || pacnewWarnings.length > 0) && (
                    <div className="mt-6 flex flex-col gap-3">
                        {rebootRequired && (
                            <motion.div
                                initial={{ opacity: 0, y: -10 }}
                                animate={{ opacity: 1, y: 0 }}
                                className="bg-orange-500/10 border border-orange-500/20 rounded-2xl p-4 flex items-center justify-between gap-4"
                            >
                                <div className="flex items-center gap-4">
                                    <div className="p-2 bg-orange-500/20 rounded-xl text-orange-500">
                                        <AlertCircle size={24} />
                                    </div>
                                    <div>
                                        <h3 className="font-bold text-slate-900 dark:text-white">Reboot Required</h3>
                                        <p className="text-sm text-slate-500 dark:text-white/50">Your system kernel or core drivers have been updated. Please restart to apply changes.</p>
                                    </div>
                                </div>
                                <button
                                    onClick={() => commands.launchApp({ pkg_name: 'reboot' }).catch(() => { })}
                                    className="px-6 py-2 bg-orange-600 hover:bg-orange-500 text-white rounded-xl font-bold text-sm transition-all shadow-lg shadow-orange-900/20 whitespace-nowrap"
                                >
                                    Restart Now
                                </button>
                            </motion.div>
                        )}

                        {pacnewWarnings.length > 0 && (
                            <motion.div
                                initial={{ opacity: 0, y: -10 }}
                                animate={{ opacity: 1, y: 0 }}
                                className="bg-blue-500/10 border border-blue-500/20 rounded-2xl p-4 flex items-center justify-between gap-4"
                            >
                                <div className="flex items-center gap-4">
                                    <div className="p-2 bg-blue-500/20 rounded-xl text-blue-500">
                                        <ShieldCheck size={24} />
                                    </div>
                                    <div>
                                        <h3 className="font-bold text-slate-900 dark:text-white">Config Review Needed</h3>
                                        <p className="text-sm text-slate-500 dark:text-white/50">{pacnewWarnings.length} .pacnew files detected. Review these to ensure system stability.</p>
                                    </div>
                                </div>
                                <div className="flex gap-4 overflow-x-auto custom-scrollbar-hidden py-1">
                                    {pacnewWarnings.slice(0, 2).map((p, i) => (
                                        <span key={i} className="text-[10px] font-mono bg-blue-500/10 px-2 py-1 rounded-md text-blue-400 whitespace-nowrap">
                                            {p.split('/').pop()}
                                        </span>
                                    ))}
                                    {pacnewWarnings.length > 2 && <span className="text-[10px] text-blue-400/50">+{pacnewWarnings.length - 2} more</span>}
                                </div>
                            </motion.div>
                        )}

                        {pendingServiceRestarts.length > 0 && (
                            <motion.div
                                initial={{ opacity: 0, y: -10 }}
                                animate={{ opacity: 1, y: 0 }}
                                className="bg-purple-500/10 border border-purple-500/20 rounded-2xl p-4 flex items-center justify-between gap-4"
                            >
                                <div className="flex items-center gap-4">
                                    <div className="p-2 bg-purple-500/20 rounded-xl text-purple-500">
                                        <RotateCw size={24} />
                                    </div>
                                    <div className="flex-1 min-w-0">
                                        <h3 className="font-bold text-slate-900 dark:text-white">Service Restarts Recommended</h3>
                                        <div className="flex flex-wrap gap-2 mt-1">
                                            {pendingServiceRestarts.map((s, i) => (
                                                <span key={i} className="text-[10px] font-mono bg-purple-500/10 px-2 py-0.5 rounded-md text-purple-400">
                                                    {s}
                                                </span >
                                            ))}
                                        </div>
                                    </div>
                                </div>
                                <button
                                    onClick={async () => {
                                        try {
                                            const pwd = await requestSessionPassword();
                                            for (const s of pendingServiceRestarts) {
                                                await commands.restartService(pwd, s).then(unwrap);
                                            }
                                            toastSuccess('Services restarted successfully');
                                            useAppStore.getState().refreshPendingUpdates();
                                        } catch (e) {
                                            console.error(e);
                                        }
                                    }}
                                    className="px-6 py-2 bg-purple-600 hover:bg-purple-500 text-white rounded-xl font-bold text-sm transition-all shadow-lg shadow-purple-900/20 whitespace-nowrap"
                                >
                                    Restart Now
                                </button>
                            </motion.div>
                        )}

                        {snapshotStatus?.is_configured && !isUpdating && updates.length > 0 && (
                            <motion.div
                                initial={{ opacity: 0, y: -10 }}
                                animate={{ opacity: 1, y: 0 }}
                                className="bg-emerald-500/10 border border-emerald-500/20 rounded-2xl p-4 flex items-center justify-between gap-4"
                            >
                                <div className="flex items-center gap-4">
                                    <div className="p-2 bg-emerald-500/20 rounded-xl text-emerald-500">
                                        < ShieldCheck size={24} />
                                    </div>
                                    <div>
                                        <h3 className="font-bold text-slate-900 dark:text-white">Safety Snapshot Available</h3>
                                        <p className="text-sm text-slate-500 dark:text-white/50">{snapshotStatus.message}. Create a system restore point before proceeding?</p>
                                    </div>
                                </div>
                                <label className="flex items-center gap-3 cursor-pointer group bg-black/5 dark:bg-white/5 px-4 py-2 rounded-xl border border-black/10 dark:border-white/10 hover:border-emerald-500/50 transition-all">
                                    <span className="text-sm font-bold text-slate-700 dark:text-emerald-400">Snapshot Enabled</span>
                                    <input
                                        type="checkbox"
                                        checked={doSnapshot}
                                        onChange={(e) => setDoSnapshot(e.target.checked)}
                                        className="w-5 h-5 rounded-md border-white/10 bg-black/20 text-emerald-500 focus:ring-emerald-500"
                                    />
                                </label>
                            </motion.div>
                        )}
                    </div>
                )}

                {/* Visual Stepper */}
                <AnimatePresence>
                    {isUpdating && (
                        <motion.div
                            initial={{ height: 0, opacity: 0 }}
                            animate={{ height: 'auto', opacity: 1 }}
                            exit={{ height: 0, opacity: 0 }}
                            className="mt-8 bg-black/5 dark:bg-black/20 rounded-2xl p-6 border border-black/5 dark:border-white/10"
                        >
                            <div className="flex items-center justify-between mb-8">
                                {steps.map((step, idx) => (
                                    <div key={idx} className="flex flex-col items-center flex-1 relative">
                                        <div className={clsx(
                                            "w-10 h-10 rounded-full flex items-center justify-center font-bold text-sm transition-all duration-500 z-10",
                                            currentStep > idx ? "bg-green-500 text-white" :
                                                currentStep === idx ? "bg-blue-600 text-white ring-4 ring-blue-500/20" :
                                                    "bg-black/10 dark:bg-white/10 text-slate-400"
                                        )}>
                                            {currentStep > idx ? <CheckCircle2 size={20} /> : idx + 1}
                                        </div>
                                        <span className={clsx(
                                            "mt-3 text-[10px] font-black uppercase tracking-widest",
                                            currentStep === idx ? "text-blue-500" : "text-app-muted opacity-50"
                                        )}>
                                            {step}
                                        </span>
                                        {idx < steps.length - 1 && (
                                            <div className="absolute top-5 left-1/2 w-full h-[2px] bg-black/5 dark:bg-white/5 -z-0">
                                                <motion.div
                                                    className="h-full bg-blue-500"
                                                    initial={{ width: 0 }}
                                                    animate={{ width: currentStep > idx ? '100%' : '0%' }}
                                                />
                                            </div>
                                        )}
                                    </div>
                                ))}
                            </div>

                            <div className="flex justify-between text-xs font-bold text-slate-900 dark:text-white mb-2 uppercase tracking-wider">
                                <span>{statusMessage || 'Preparing update...'}</span>
                                <span>{Math.round(progress)}%</span>
                            </div>
                            {showAuthHint && (
                                <p className="text-amber-600 dark:text-amber-400 text-xs font-medium mt-2 mb-1">
                                    If a password dialog appeared behind other windows, bring it to the front and enter your password to continue.
                                </p>
                            )}
                            <div className="h-2 bg-black/10 dark:bg-black/40 rounded-full overflow-hidden border border-black/5 dark:border-white/5">
                                <motion.div
                                    className="h-full bg-gradient-to-r from-blue-500 to-purple-500 relative"
                                    initial={{ width: 0 }}
                                    animate={{ width: `${progress}%` }}
                                >
                                    <div className="absolute inset-0 bg-white/20 animate-pulse" />
                                </motion.div>
                            </div>

                            <div className="flex items-center justify-between mt-4">
                                <button
                                    onClick={() => setShowConsole(!showConsole)}
                                    className="text-xs font-bold text-blue-500 hover:text-blue-400 flex items-center gap-2 transition-colors"
                                >
                                    <Download size={14} className={showConsole ? "rotate-180 transition-transform" : ""} />
                                    {showConsole ? "Hide Process Details" : "Show Process Details (Advanced)"}
                                </button>
                                {needsReboot && (
                                    <span className="text-[10px] font-bold text-orange-500 animate-pulse flex items-center gap-1">
                                        <AlertCircle size={12} /> Reboot will be required
                                    </span>
                                )}
                            </div>

                            <AnimatePresence>
                                {showConsole && (
                                    <motion.div
                                        initial={{ height: 0, opacity: 0 }}
                                        animate={{ height: 200, opacity: 1 }}
                                        exit={{ height: 0, opacity: 0 }}
                                        className="mt-3 bg-black/40 rounded-xl overflow-hidden border border-white/5 font-mono text-[10px] flex flex-col"
                                    >
                                        <div className="flex-1 overflow-y-auto p-4 custom-scrollbar flex flex-col-reverse">
                                            <div className="flex flex-col">
                                                {logs.map((log: string, i: number) => (
                                                    <div key={i} className="py-0.5 border-l-2 border-blue-500/20 pl-3 hover:bg-white/5 transition-colors whitespace-pre-wrap">
                                                        <span className="text-white/40 mr-2">[{i}]</span>
                                                        <span className="text-white/80">{log}</span>
                                                    </div>
                                                ))}
                                                <div id="logs-end" />
                                            </div>
                                        </div>
                                    </motion.div>
                                )}
                            </AnimatePresence>
                        </motion.div>
                    )}
                </AnimatePresence>

                {/* System busy / lock error - friendly banner with Fix It */}
                <AnimatePresence>
                    {isLockOrBusyError && !isUpdating && (
                        <motion.div
                            initial={{ height: 0, opacity: 0 }}
                            animate={{ height: 'auto', opacity: 1 }}
                            className="mt-6 p-4 rounded-xl bg-amber-500/10 border border-amber-500/20 text-amber-700 dark:text-amber-300 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3"
                        >
                            <div className="flex items-center gap-3">
                                <Unlock size={20} className="text-amber-500 shrink-0" />
                                <div>
                                    <span className="font-bold text-sm block">System is busy</span>
                                    <span className="text-xs opacity-90">Another process may be using the package database. You can try unlocking it.</span>
                                </div>
                            </div>
                            <button
                                onClick={async () => {
                                    setFixingLock(true);
                                    try {
                                        const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
                                        await commands.repairUnlockPacman(pwd).then(unwrap);
                                        setUpdateResult(null);
                                        await checkForUpdates();
                                    } catch (e) {
                                        const raw = e instanceof Error ? (e as Error).message : String(e);
                                        setUpdateResult(friendlyError(raw).description);
                                    } finally {
                                        setFixingLock(false);
                                    }
                                }}
                                disabled={fixingLock}
                                className="px-4 py-2 rounded-lg bg-amber-500 hover:bg-amber-600 text-white text-sm font-bold flex items-center gap-2 disabled:opacity-50 shrink-0"
                            >
                                {fixingLock ? <Loader2 size={16} className="animate-spin" /> : <Unlock size={16} />}
                                {fixingLock ? 'Fixing...' : 'Fix It'}
                            </button>
                        </motion.div>
                    )}
                </AnimatePresence>

                {/* Orphan cleanup after successful update */}
                <AnimatePresence>
                    {orphansAfterUpdate.length > 0 && !isUpdating && (
                        <motion.div
                            initial={{ height: 0, opacity: 0 }}
                            animate={{ height: 'auto', opacity: 1 }}
                            exit={{ height: 0, opacity: 0 }}
                            className="mt-6 p-4 rounded-xl bg-slate-500/10 dark:bg-white/5 border border-slate-500/20 dark:border-white/10 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3"
                        >
                            <div className="flex items-center gap-3">
                                <CheckCircle2 size={20} className="text-green-500 shrink-0" />
                                <div>
                                    <span className="font-bold text-sm block">Update complete</span>
                                    <span className="text-xs text-app-muted">
                                        {orphansAfterUpdate.length} orphan package{orphansAfterUpdate.length !== 1 ? 's' : ''} found. Remove them to save space?
                                    </span>
                                </div>
                            </div>
                            <button
                                onClick={async () => {
                                    setRemovingOrphans(true);
                                    try {
                                        await commands.removeOrphans(orphansAfterUpdate).then(unwrap);
                                        setOrphansAfterUpdate([]);
                                        await checkForUpdates();
                                    } catch (e) {
                                        errorService.reportError(e as Error | string);
                                    } finally {
                                        setRemovingOrphans(false);
                                    }
                                }}
                                disabled={removingOrphans}
                                className="px-4 py-2 rounded-lg bg-slate-600 hover:bg-slate-500 text-white text-sm font-bold flex items-center gap-2 disabled:opacity-50 shrink-0"
                            >
                                {removingOrphans ? <Loader2 size={16} className="animate-spin" /> : null}
                                {removingOrphans ? 'Removing…' : 'Remove orphans'}
                            </button>
                        </motion.div>
                    )}
                </AnimatePresence>

                {/* Reboot & Pacnew Warnings */}
                <AnimatePresence>
                    {(rebootRequired || pacnewWarnings.length > 0 || (needsReboot && !isUpdating)) && (
                        <motion.div
                            initial={{ height: 0, opacity: 0 }}
                            animate={{ height: 'auto', opacity: 1 }}
                            className="mt-6 flex flex-col gap-3"
                        >
                            {(rebootRequired || (needsReboot && !isUpdating && updates.length > 0)) && (
                                <div className="p-4 rounded-xl bg-orange-500/10 border border-orange-500/20 text-orange-600 dark:text-orange-400 flex items-center gap-3 font-bold text-sm">
                                    <AlertCircle size={18} />
                                    <span>{rebootRequired ? "System Reboot is required to apply kernel/driver updates." : "Safety Banner: This update includes kernel or driver changes. A reboot is highly recommended after completion."}</span>
                                    {rebootRequired && (
                                        <button
                                            onClick={() => commands.launchApp({ pkg_name: 'reboot' }).catch(() => { })}
                                            className="ml-auto px-4 py-1.5 rounded-lg bg-orange-500 text-white hover:bg-orange-600 transition-colors"
                                        >
                                            Reboot Now
                                        </button>
                                    )}
                                </div>
                            )}
                            {pacnewWarnings.length > 0 && (
                                <div className="p-4 rounded-xl bg-blue-500/10 border border-blue-500/20 text-blue-600 dark:text-blue-400 flex flex-col gap-2 text-sm">
                                    <div className="flex items-center gap-3 font-bold">
                                        <AlertCircle size={18} />
                                        <span>Detected {pacnewWarnings.length} configuration updates (.pacnew).</span>
                                    </div>
                                    <p className="opacity-80 ml-7">Please merge these files to ensure system stability. Use 'pacdiff' or similar.</p>
                                </div>
                            )}
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto p-8 custom-scrollbar">
                {isChecking ? (
                    <div className="flex flex-col items-center justify-center h-full text-app-muted gap-6">
                        <div className="w-24 h-24 bg-blue-500/5 rounded-full flex items-center justify-center animate-butterfly">
                            <RefreshCw size={48} className="text-blue-500" />
                        </div>
                        <p className="text-xl font-medium">Scoping repositories for updates...</p>
                    </div>
                ) : updates.length === 0 && !isUpdating ? (
                    <div className="flex flex-col items-center justify-center p-20 bg-white dark:bg-app-card/30 rounded-3xl border border-black/5 dark:border-white/5 mt-10 max-w-2xl mx-auto backdrop-blur-sm shadow-sm dark:shadow-none">
                        <div className="w-24 h-24 bg-green-500/10 text-green-500 rounded-full flex items-center justify-center mb-6 ring-4 ring-green-500/5">
                            <CheckCircle2 size={48} />
                        </div>
                        <div className="text-center">
                            <h3 className="text-3xl font-black text-slate-900 dark:text-white mb-2">All Clear!</h3>
                            <p className="text-lg text-slate-500 dark:text-app-muted">Your system is optimally configured and up to date.</p>
                            {updateResult && (
                                <pre className="mt-8 text-left text-xs bg-slate-50 dark:bg-black/40 border border-black/10 dark:border-white/10 rounded-xl p-6 w-full max-w-lg mx-auto whitespace-pre-wrap font-mono text-green-600 dark:text-green-400 overflow-x-auto shadow-inner">
                                    {updateResult}
                                </pre>
                            )}
                        </div>
                    </div>
                ) : (
                    <div className="space-y-6 max-w-5xl mx-auto">
                        {(['repo', 'aur', 'flatpak'] as const).map((sourceType) => {
                            const sectionUpdates = updates.filter((u) => (u.source?.source_type ?? 'repo') === sourceType);
                            if (sectionUpdates.length === 0) return null;
                            const sectionLabel = sourceType === 'repo' ? 'System (repos)' : sourceType === 'aur' ? 'AUR (community)' : 'Flatpak';
                            return (
                                <div key={sourceType} className="space-y-3">
                                    <h3 className="text-xs font-bold uppercase tracking-wider text-slate-500 dark:text-app-muted px-1">
                                        {sectionLabel} — {sectionUpdates.length} update{sectionUpdates.length !== 1 ? 's' : ''}
                                    </h3>
                                    {sectionUpdates.map((pkg, idx) => (
                                        <div
                                            key={`${pkg.name}:${String(pkg.source?.source_type ?? 'repo')}:${String(pkg.source?.id ?? pkg.name)}:${idx}`}
                                            className="bg-white dark:bg-app-card border border-black/5 dark:border-white/5 rounded-2xl p-5 flex items-center justify-between hover:bg-white/80 dark:hover:bg-white/5 transition-all group hover:scale-[1.01] hover:shadow-xl hover:border-black/10 dark:hover:border-white/10"
                                        >
                                            <div className="flex items-center gap-6">
                                                <div className="w-14 h-14 rounded-xl bg-slate-50 dark:bg-black/20 flex items-center justify-center shrink-0 overflow-hidden relative p-2 border border-black/5 dark:border-white/5 shadow-inner">
                                                    <AppIcon pkgId={pkg.name} iconUrl={metadataCache[pkg.name]?.icon_url} />
                                                </div>
                                                <div>
                                                    <h3 className="font-bold flex items-center gap-3 text-xl text-slate-900 dark:text-white mb-1">
                                                        {pkg.name}
                                                        <RepoBadge source={pkg.source} />
                                                    </h3>
                                                    <div className="flex items-center gap-3 text-sm font-medium">
                                                        <span className="text-slate-400 dark:text-app-muted line-through opacity-50">{pkg.current_version}</span>
                                                        <ArrowRight size={14} className="text-slate-300 dark:text-white/20" />
                                                        <span className="text-emerald-600 dark:text-emerald-400">{pkg.new_version}</span>
                                                    </div>
                                                </div>
                                            </div>

                                            <div className="flex items-center gap-6">
                                                {pkg.source.source_type === 'aur' && (
                                                    <div className="flex items-center gap-2">
                                                        <button
                                                            onClick={() => handleViewPkgbuild(pkg.name)}
                                                            className="px-3 py-1.5 rounded-lg bg-black/5 dark:bg-white/5 hover:bg-black/10 dark:hover:bg-white/10 text-xs font-bold text-app-muted border border-black/10 dark:border-white/10 transition-all"
                                                        >
                                                            View PKGBUILD
                                                        </button>
                                                        <div title="AUR Package: May take longer to build" className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-amber-100 dark:bg-amber-500/10 border border-amber-200 dark:border-amber-500/20 text-amber-700 dark:text-amber-500 text-xs font-bold">
                                                            <AlertCircle size={14} />
                                                            <span>Built from Source</span>
                                                        </div>
                                                    </div>
                                                )}
                                            </div>
                                        </div>
                                    ))}
                                </div>
                            );
                        })}
                    </div>
                )}
            </div>

            <CriticalNewsBlockerModal
                isOpen={showBlockerModal}
                onClose={() => setShowBlockerModal(false)}
                onProceed={() => {
                    markNewsItemsAsRead(unreadCriticalItems.map((i) => i.id));
                    performUpdate();
                }}
                criticalItems={unreadCriticalItems}
            />

            <ConfirmationModal
                isOpen={showConfirm}
                onClose={() => {
                    setShowConfirm(false);
                    setPassword('');
                }}
                onConfirm={performUpdate}
                title="Update System"
                message={
                    <div className="space-y-4">
                        <p>
                            {updates.some(u => u.source.source_type === 'aur')
                                ? "This update includes AUR packages which require building from source. Please enter your administrator password to proceed."
                                : "This will update all system packages. Are you ready to proceed?"
                            }
                        </p>
                        {doSnapshot && snapshotStatus?.is_configured && (
                            <div className="p-3 bg-emerald-500/10 border border-emerald-500/20 rounded-xl flex items-center gap-3">
                                <ShieldCheck size={18} className="text-emerald-500" />
                                <span className="text-xs font-medium text-emerald-700 dark:text-emerald-300">A system snapshot will be created automatically before the update begins.</span>
                            </div>
                        )}
                    </div>
                }
                confirmLabel="Start Update"
                variant="info"
                showPasswordInput={updates.some(u => u.source.source_type === 'aur')}
                passwordValue={password}
                onPasswordChange={setPassword}
            />

            <AnimatePresence>
                {viewingPkgbuild && (
                    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm">
                        <motion.div
                            initial={{ opacity: 0, scale: 0.95 }}
                            animate={{ opacity: 1, scale: 1 }}
                            exit={{ opacity: 0, scale: 0.95 }}
                            className="bg-app-card border border-app-border rounded-3xl w-full max-w-4xl h-[80vh] flex flex-col shadow-2xl overflow-hidden"
                        >
                            <div className="p-6 border-b border-app-border flex items-center justify-between">
                                <div>
                                    <h3 className="text-xl font-bold flex items-center gap-3">
                                        <Terminal size={20} className="text-blue-500" />
                                        Reviewing PKGBUILD: <span className="text-blue-500">{viewingPkgbuild}</span>
                                    </h3>
                                    <p className="text-xs text-app-muted mt-1 underline decoration-dotted">Always inspect scripts from the AUR for safety.</p>
                                </div>
                                <button
                                    onClick={() => setViewingPkgbuild(null)}
                                    className="p-2 hover:bg-white/10 rounded-xl transition-all"
                                >
                                    <RefreshCw size={20} className="rotate-45" />
                                </button>
                            </div>
                            <div className="flex-1 p-6 overflow-y-auto custom-scrollbar bg-black/20">
                                {isLoadingPkgbuild ? (
                                    <div className="h-full flex flex-col items-center justify-center gap-4 text-app-muted">
                                        <RefreshCw size={32} className="animate-spin" />
                                        <p className="font-medium">Fetching from AUR...</p>
                                    </div>
                                ) : (
                                    <pre className="text-xs font-mono text-slate-300 whitespace-pre-wrap leading-relaxed">
                                        {pkgbuildContent}
                                    </pre>
                                )}
                            </div>
                            <div className="p-4 border-t border-app-border bg-app-card flex justify-end">
                                <button
                                    onClick={() => setViewingPkgbuild(null)}
                                    className="px-6 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-xl font-bold text-sm transition-all"
                                >
                                    Close Review
                                </button>
                            </div>
                        </motion.div>
                    </div>
                )}
            </AnimatePresence>
        </div>
    );
}
