import { useState, useEffect, useCallback, useMemo } from 'react';
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
import { commands, UpdateItem, NewsItem, DistroContext, UpdateSnapshot, UpdateSourceStatus } from '../services/bindings';
import { unwrap } from '../utils/specta';
import { notifyUpdateComplete } from '../services/notificationService';

function describeError(error: unknown): string {
    if (error instanceof Error) return `${error.name}: ${error.message}`;
    try {
        return JSON.stringify(error);
    } catch {
        return String(error);
    }
}

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
    const oneClickEnabled = useAppStore((s) => s.oneClickEnabled);
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
    const [sourceStatuses, setSourceStatuses] = useState<UpdateSourceStatus[]>([]);
    const [isChecking, setIsChecking] = useState(true);
    const [updateResult, setUpdateResult] = useState<string | null>(null);
    const [showConsole, setShowConsole] = useState(false);
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

    // Distro context for safety banners
    const [distroContext, setDistroContext] = useState<DistroContext | null>(null);
    useEffect(() => {
        commands.getDistroContext().then(setDistroContext).catch(() => { });
    }, []);

    // 3.1: Transaction manifest from backend
    interface UpdateManifest {
        total: number;
        repo_count: number;
        aur_count: number;
        flatpak_count: number;
        repo_packages: string[];
        aur_packages: string[];
        flatpak_packages: string[];
    }
    const [manifest, setManifest] = useState<UpdateManifest | null>(null);

    interface UpdateFailedPackage {
        name: string;
        source: string;
        reason: string;
    }
    interface UpdateRunSummary {
        repo: 'success' | 'failed' | 'skipped';
        aur: 'success' | 'partial' | 'failed' | 'skipped';
        flatpak: 'success' | 'partial' | 'failed' | 'skipped';
        succeeded_packages: string[];
        failed_packages: UpdateFailedPackage[];
        warnings: string[];
        duration_ms: number;
    }
    interface UpdateCompleteEvent {
        overall: 'success' | 'partial' | 'failed';
        summary: UpdateRunSummary;
        message: string;
    }
    interface UpdateSourceProgressEvent {
        source: 'repo' | 'aur' | 'flatpak';
        stage: string;
        current: number;
        total: number;
        package?: string;
    }

    // 3.2: Per-source progress indicators
    const [sourceProgress, setSourceProgress] = useState<{ repo: 'idle' | 'active' | 'done' | 'error'; aur: 'idle' | 'active' | 'done' | 'error'; flatpak: 'idle' | 'active' | 'done' | 'error' }>({ repo: 'idle', aur: 'idle', flatpak: 'idle' });
    const [sourceProgressDetail, setSourceProgressDetail] = useState<Record<'repo' | 'aur' | 'flatpak', { stage: string; current: number; total: number; package?: string }>>({
        repo: { stage: 'idle', current: 0, total: 0 },
        aur: { stage: 'idle', current: 0, total: 0 },
        flatpak: { stage: 'idle', current: 0, total: 0 },
    });
    const [lastUpdateSummary, setLastUpdateSummary] = useState<UpdateCompleteEvent | null>(null);
    const [showAdvancedControls, setShowAdvancedControls] = useState(false);
    const [advancedScope, setAdvancedScope] = useState({ repo: true, aur: true, flatpak: true });
    const [excludedUpdateKeys, setExcludedUpdateKeys] = useState<Record<string, boolean>>({});
    const [retryTargets, setRetryTargets] = useState<UpdateItem[]>([]);
    const sourceSections = useMemo(() => {
        const grouped = new Map<string, UpdateItem[]>();
        for (const u of updates) {
            const key = u.source?.source_type || 'unknown';
            const list = grouped.get(key) ?? [];
            list.push(u);
            grouped.set(key, list);
        }
        return Array.from(grouped.entries());
    }, [updates]);

    const sourceLabel = useCallback((sourceType: string) => {
        if (sourceType === 'repo') return 'System (repos)';
        if (sourceType === 'aur') return 'AUR (community)';
        if (sourceType === 'flatpak') return 'Flatpak';
        return `${sourceType} updates`;
    }, []);

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
        "Preparing update",
        "Updating system packages",
        "Building community packages (AUR)",
        "Updating Flatpak apps"
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
            const snapshot: UpdateSnapshot = unwrap(await commands.getUpdateSnapshot(true, true));
            setSourceStatuses(snapshot.sources);
            const pendingUpdates: UpdateItem[] = snapshot.items.map((item) => ({
                name: item.package.name,
                display_name: item.package.display_name,
                current_version: item.current_version,
                new_version: item.new_version,
                source: item.package.source,
                size: item.package.download_size_bytes ?? item.package.download_size ?? null,
                icon: item.package.icon,
            }));
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
            errorService.reportError(describeError(e));
        } finally {
            setIsChecking(false);
        }
    };

    const [showConfirm, setShowConfirm] = useState(false);

    const getUpdateKey = useCallback((pkg: UpdateItem) => `${pkg.name}:${pkg.source.source_type}:${pkg.source.id}`, []);
    const selectedUpdates = useMemo(
        () => updates.filter((u) => {
            const src = u.source.source_type;
            if (src === 'repo' && !advancedScope.repo) return false;
            if (src === 'aur' && !advancedScope.aur) return false;
            if (src === 'flatpak' && !advancedScope.flatpak) return false;
            return !excludedUpdateKeys[getUpdateKey(u)];
        }),
        [updates, advancedScope, excludedUpdateKeys, getUpdateKey]
    );

    // Fetch updates on mount (list always includes repo + AUR + Flatpak for installed packages)
    useEffect(() => {
        checkForUpdates();
    }, []);

    // Listen for update-complete so we don't block the UI waiting for the backend.
    useEffect(() => {
        const unlisten = listen<UpdateCompleteEvent>('update-complete', async (event) => {
            setUpdating(false);
            setUpdateResult(event.payload.message);
            setLastUpdateSummary(event.payload);
            // Mark all sources as done when update completes
            setSourceProgress(prev => ({
                repo: event.payload.summary.repo === 'failed' ? 'error' : prev.repo === 'active' ? 'done' : prev.repo,
                aur: (event.payload.summary.aur === 'failed' || event.payload.summary.aur === 'partial') ? 'error' : prev.aur === 'active' ? 'done' : prev.aur,
                flatpak: (event.payload.summary.flatpak === 'failed' || event.payload.summary.flatpak === 'partial') ? 'error' : prev.flatpak === 'active' ? 'done' : prev.flatpak,
            }));
            // Desktop notification on update completion
            notifyUpdateComplete(event.payload.overall !== 'failed', event.payload.message).catch(() => { });
            checkForUpdates();
            try {
                const warnings = unwrap(await commands.getPacnewWarnings());
                setPacnewWarnings(warnings);
            } catch {
                // ignore
            }
            if (event.payload.overall !== 'failed') {
                try {
                    const orphans = unwrap(await commands.getOrphans());
                    setOrphansAfterUpdate(orphans || []);
                } catch {
                    setOrphansAfterUpdate([]);
                }
            } else {
                setOrphansAfterUpdate([]);
            }

            if (event.payload.summary.failed_packages.length > 0) {
                const failedSet = new Set(event.payload.summary.failed_packages.map((f) => `${f.name}:${f.source}`));
                setRetryTargets(
                    updates.filter((u) => failedSet.has(`${u.name}:${u.source.source_type}`))
                );
            } else {
                setRetryTargets([]);
            }
        });
        return () => {
            unlisten.then((fn) => fn()).catch(() => { });
        };
    }, [setUpdating, setPacnewWarnings, updates]);

    // 3.1: Listen for transaction manifest from backend
    useEffect(() => {
        const unlisten = listen<UpdateManifest>('update-manifest', (event) => {
            setManifest(event.payload);
        });
        const unlistenSourceProgress = listen<UpdateSourceProgressEvent>('update-source-progress', (event) => {
            const { source, stage, current, total, package: pkg } = event.payload;
            setSourceProgressDetail((prev) => ({
                ...prev,
                [source]: { stage, current, total, package: pkg },
            }));
            setSourceProgress((prev) => ({
                ...prev,
                [source]:
                    stage === 'failed'
                        ? 'error'
                        : stage === 'complete' || stage === 'skipped'
                            ? 'done'
                            : stage === 'idle'
                                ? 'idle'
                                : 'active',
            }));
        });

        return () => {
            unlisten.then((fn) => fn()).catch(() => { });
            unlistenSourceProgress.then((fn) => fn()).catch(() => { });
        };
    }, []);

    const handleUpdateAll = () => {
        if (selectedUpdates.length === 0) {
            setUpdateResult('No updates selected. Adjust Advanced Controls or select updates to continue.');
            return;
        }
        setManifest({
            total: selectedUpdates.length,
            repo_count: selectedUpdates.filter((u) => u.source.source_type === 'repo').length,
            aur_count: selectedUpdates.filter((u) => u.source.source_type === 'aur').length,
            flatpak_count: selectedUpdates.filter((u) => u.source.source_type === 'flatpak').length,
            repo_packages: selectedUpdates.filter((u) => u.source.source_type === 'repo').map((u) => u.name),
            aur_packages: selectedUpdates.filter((u) => u.source.source_type === 'aur').map((u) => u.name),
            flatpak_packages: selectedUpdates.filter((u) => u.source.source_type === 'flatpak').map((u) => u.name),
        });
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
        setLastUpdateSummary(null);
        clearUpdateLogs();
        setCurrentStep(0);
        setSourceProgress({ repo: 'idle', aur: 'idle', flatpak: 'idle' });
        setSourceProgressDetail({
            repo: { stage: 'idle', current: 0, total: 0 },
            aur: { stage: 'idle', current: 0, total: 0 },
            flatpak: { stage: 'idle', current: 0, total: 0 },
        });

        const repoSelected = selectedUpdates.some((u) => u.source.source_type === 'repo');
        const aurSelected = selectedUpdates.some((u) => u.source.source_type === 'aur');
        const flatpakSelected = selectedUpdates.some((u) => u.source.source_type === 'flatpak');
        const usingAdvancedSelection =
            showAdvancedControls ||
            Object.keys(excludedUpdateKeys).length > 0 ||
            !advancedScope.repo ||
            !advancedScope.aur ||
            !advancedScope.flatpak;

        let pwd: string | null = null;
        if (oneClickEnabled || reducePasswordPrompts) {
            // Branded one-click auth is requested upfront; user can choose system prompt fallback.
            pwd = await requestSessionPassword();
        }

        if (doSnapshot && snapshotStatus?.is_configured) {
            try {
                // We don't block the WHOLE update if snapshot fails, but we try.
                await commands.createSystemSnapshot(snapshotStatus.tool as any, `Monarch Store Update: ${new Date().toISOString()}`).then(unwrap);
            } catch (e) {
                errorService.reportWarning(e as Error | string);
                clearUpdateLogs();
            }
        }

        const runPromise = usingAdvancedSelection
            ? commands.applyUpdates(selectedUpdates, pwd)
            : commands.performSystemUpdate(pwd, aurSelected, flatpakSelected);

        runPromise.catch((e) => {
            errorService.reportError(e as Error | string);
            setUpdateResult(`Update failed: ${e}`);
            setUpdating(false);
        });
    };

    const retryFailedOnly = async () => {
        if (retryTargets.length === 0 || isUpdating) return;
        setUpdating(true);
        setUpdateResult(null);
        clearUpdateLogs();
        setSourceProgress({ repo: 'idle', aur: 'idle', flatpak: 'idle' });
        let pwd: string | null = null;
        if (oneClickEnabled || reducePasswordPrompts) {
            pwd = await requestSessionPassword();
        }
        commands.applyUpdates(retryTargets, pwd).catch((e) => {
            errorService.reportError(e as Error | string);
            setUpdateResult(`Retry failed: ${e}`);
            setUpdating(false);
        });
    };

    const needsReboot = updates.some(u => u.name === 'linux' || u.name.startsWith('nvidia'));

    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            {/* Header */}
            <div className="sticky top-0 z-10 border-b border-black/5 bg-app-bg/95 p-6 pb-4 backdrop-blur-3xl transition-colors dark:border-white/5">
                <div className="flex items-end justify-between">
                    <div>
                        <h1 className="mb-2 flex items-center gap-3 text-2xl lg:text-3xl font-black tracking-tight leading-none text-slate-900 dark:text-white">
                            <span className={clsx("p-2 rounded-xl bg-blue-500/10 text-blue-500", (isUpdating || isChecking) && "animate-butterfly")}>
                                <RefreshCw size={24} />
                            </span>
                            Updates
                        </h1>
                        <p className="ml-1 text-sm font-medium text-slate-500 dark:text-app-muted">
                            {isChecking ? "Checking for updates..." :
                                updates.length === 0 ? "Your system is up to date" :
                                    `${updates.length} updates available`}
                        </p>
                        {!isChecking && (
                            <p className="ml-1 mt-1 text-xs text-slate-500 dark:text-app-muted/80">
                                Review update scope, blockers, and source progress before applying changes.
                            </p>
                        )}
                    </div>

                    <div className="flex items-center gap-3 flex-wrap">
                        <button
                            onClick={checkForUpdates}
                            disabled={isChecking || isUpdating}
                            className="px-4 py-2.5 rounded-lg bg-black/5 dark:bg-white/5 hover:bg-black/10 dark:hover:bg-white/10 text-slate-900 dark:text-white font-bold text-sm border border-black/10 dark:border-white/10 transition-all disabled:opacity-50 flex items-center gap-2 active:scale-95"
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
                            className="px-4 py-2.5 rounded-lg bg-black/5 dark:bg-white/5 hover:bg-black/10 dark:hover:bg-white/10 text-slate-900 dark:text-white font-bold text-sm border border-black/10 dark:border-white/10 transition-all disabled:opacity-50 flex items-center gap-2 active:scale-95"
                            title="Copy the full system upgrade command (sudo pacman -Syu) to run in your terminal"
                        >
                            <Terminal size={18} /> Copy terminal command
                        </button>
                        {updates.length > 0 && !isUpdating && (
                            <button
                                onClick={() => setShowAdvancedControls((v) => !v)}
                                className="px-4 py-2.5 rounded-lg bg-black/5 dark:bg-white/5 hover:bg-black/10 dark:hover:bg-white/10 text-slate-900 dark:text-white font-bold text-sm border border-black/10 dark:border-white/10 transition-all flex items-center gap-2"
                            >
                                <Terminal size={18} /> {showAdvancedControls ? 'Hide Advanced Scope' : 'Advanced Scope'}
                            </button>
                        )}
                        {updates.length > 0 && !isUpdating && (
                            <button
                                onClick={handleUpdateAll}
                                className="bg-blue-600 hover:bg-blue-500 text-white px-5 py-2.5 rounded-lg font-bold text-sm active:scale-95 transition-all flex items-center gap-2 border border-white/10"
                            >
                                <Download size={20} /> Update All
                            </button>
                        )}
                    </div>
                </div>

                {/* Distro-Aware Safety Banner */}
                {distroContext && distroContext.id !== 'arch' && (
                    <motion.div
                        initial={{ opacity: 0, y: -5 }}
                        animate={{ opacity: 1, y: 0 }}
                        className={clsx(
                            'mt-4 rounded-lg px-4 py-2.5 flex items-center gap-3 text-xs font-bold border',
                            distroContext.id === 'manjaro'
                                ? 'bg-amber-500/10 border-amber-500/20 text-amber-600 dark:text-amber-400'
                                : 'bg-app-accent/5 border-app-accent/15 text-app-accent/80'
                        )}
                    >
                        <ShieldCheck size={16} className="shrink-0" />
                        <span>
                            {distroContext.id === 'manjaro'
                                ? 'Manjaro Stability Guard: Chaotic-AUR is blocked. Updates follow Manjaro\'s delayed release cycle.'
                                : distroContext.id === 'cachyos'
                                    ? `Powered by CachyOS — ${distroContext.cpu_tier.toUpperCase()} optimized binaries included`
                                    : distroContext.id === 'garuda'
                                        ? 'Garuda detected — Chaotic-AUR pre-installed, included in updates'
                                        : distroContext.id === 'endeavouros'
                                            ? 'EndeavourOS detected — close-to-Arch experience'
                                            : `${distroContext.pretty_name} — Arch-compatible update pipeline`
                            }
                        </span>
                    </motion.div>
                )}

                {showAdvancedControls && updates.length > 0 && !isUpdating && (
                    <motion.div
                        initial={{ opacity: 0, y: -8 }}
                        animate={{ opacity: 1, y: 0 }}
                        className="mt-4 rounded-xl border border-black/10 dark:border-white/10 bg-black/5 dark:bg-white/5 p-4 space-y-4"
                    >
                        <div className="flex items-center justify-between">
                            <h3 className="text-sm font-bold text-slate-900 dark:text-white">Advanced Run Scope</h3>
                            <span className="text-xs text-slate-500 dark:text-app-muted">
                                Selected: {selectedUpdates.length}/{updates.length}
                            </span>
                        </div>
                        <p className="text-xs text-slate-500 dark:text-app-muted/80">
                            Use this only when you intentionally want to limit which update sources run. The default full update remains the safest path.
                        </p>
                        <div className="flex flex-wrap gap-2">
                            {(['repo', 'aur', 'flatpak'] as const).map((source) => (
                                <button
                                    key={source}
                                    onClick={() => setAdvancedScope((prev) => ({ ...prev, [source]: !prev[source] }))}
                                    className={clsx(
                                        'px-3 py-1.5 rounded-lg text-xs font-bold uppercase tracking-wide border transition-colors',
                                        advancedScope[source]
                                            ? 'bg-blue-500/15 text-blue-500 border-blue-500/30'
                                            : 'bg-black/5 dark:bg-white/5 text-slate-500 dark:text-app-muted border-black/10 dark:border-white/10'
                                    )}
                                >
                                    {source}
                                </button>
                            ))}
                        </div>
                        {retryTargets.length > 0 && (
                            <div className="flex items-center justify-between gap-4 rounded-xl border border-amber-500/30 bg-amber-500/10 p-3">
                                <p className="text-xs text-amber-700 dark:text-amber-300">
                                    {retryTargets.length} failed update{retryTargets.length !== 1 ? 's' : ''} ready for retry.
                                </p>
                                <button
                                    onClick={retryFailedOnly}
                                    disabled={isUpdating}
                                    className="px-3 py-1.5 rounded-lg bg-amber-500 hover:bg-amber-600 text-white text-xs font-bold disabled:opacity-60"
                                >
                                    Retry failed only
                                </button>
                            </div>
                        )}
                    </motion.div>
                )}

                {/* System Status Indicators (NEW: Phase 4 & 5) */}
                {(rebootRequired || pacnewWarnings.length > 0) && (
                    <div className="mt-6 flex flex-col gap-3">
                        {rebootRequired && (
                            <motion.div
                                initial={{ opacity: 0, y: -10 }}
                                animate={{ opacity: 1, y: 0 }}
                                className="bg-orange-500/10 border border-orange-500/20 rounded-xl p-4 flex items-center justify-between gap-4"
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
                                    onClick={() => commands.launchPackage({ package_name: 'reboot', app_id: null, desktop_entry: null, launch_target: null, source: null }).catch(() => { })}
                                    className="px-4 py-2 bg-orange-600 hover:bg-orange-500 text-white rounded-lg font-bold text-sm transition-all whitespace-nowrap"
                                >
                                    Restart Now
                                </button>
                            </motion.div>
                        )}

                        {pacnewWarnings.length > 0 && (
                            <motion.div
                                initial={{ opacity: 0, y: -10 }}
                                animate={{ opacity: 1, y: 0 }}
                                className="bg-blue-500/10 border border-blue-500/20 rounded-xl p-4 flex items-center justify-between gap-4"
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
                                className="bg-purple-500/10 border border-purple-500/20 rounded-xl p-4 flex items-center justify-between gap-4"
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
                                            errorService.reportError(e as Error | string);
                                        }
                                    }}
                                    className="px-4 py-2 bg-purple-600 hover:bg-purple-500 text-white rounded-lg font-bold text-sm transition-all whitespace-nowrap"
                                >
                                    Restart Now
                                </button>
                            </motion.div>
                        )}

                        {snapshotStatus?.is_configured && !isUpdating && updates.length > 0 && (
                            <motion.div
                                initial={{ opacity: 0, y: -10 }}
                                animate={{ opacity: 1, y: 0 }}
                                className="bg-emerald-500/10 border border-emerald-500/20 rounded-xl p-4 flex items-center justify-between gap-4"
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
                            className="mt-8 bg-black/5 dark:bg-black/20 rounded-xl p-5 border border-black/5 dark:border-white/10"
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

                            {/* 3.2: Per-source progress badges */}
                            <div className="flex items-center gap-3 mt-3">
                                {sourceSections.some(([t]) => t === 'repo') && (
                                    <div className={clsx(
                                        'flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[10px] font-bold uppercase tracking-wider border transition-all duration-300',
                                        sourceProgress.repo === 'active' ? 'bg-blue-500/15 text-blue-400 border-blue-500/30 animate-pulse' :
                                            sourceProgress.repo === 'done' ? 'bg-green-500/15 text-green-400 border-green-500/30' :
                                                sourceProgress.repo === 'error' ? 'bg-red-500/15 text-red-400 border-red-500/30' :
                                                    'bg-app-fg/5 text-app-muted/50 border-app-border/50'
                                    )}>
                                        {sourceProgress.repo === 'done' ? <CheckCircle2 size={10} /> :
                                            sourceProgress.repo === 'active' ? <Loader2 size={10} className="animate-spin" /> :
                                                <span className="w-1.5 h-1.5 rounded-full bg-current" />}
                                        Repo
                                    </div>
                                )}
                                {sourceSections.some(([t]) => t === 'aur') && (
                                    <div className={clsx(
                                        'flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[10px] font-bold uppercase tracking-wider border transition-all duration-300',
                                        sourceProgress.aur === 'active' ? 'bg-amber-500/15 text-amber-400 border-amber-500/30 animate-pulse' :
                                            sourceProgress.aur === 'done' ? 'bg-green-500/15 text-green-400 border-green-500/30' :
                                                sourceProgress.aur === 'error' ? 'bg-red-500/15 text-red-400 border-red-500/30' :
                                                    'bg-app-fg/5 text-app-muted/50 border-app-border/50'
                                    )}>
                                        {sourceProgress.aur === 'done' ? <CheckCircle2 size={10} /> :
                                            sourceProgress.aur === 'active' ? <Loader2 size={10} className="animate-spin" /> :
                                                <span className="w-1.5 h-1.5 rounded-full bg-current" />}
                                        AUR
                                    </div>
                                )}
                                {sourceSections.some(([t]) => t === 'flatpak') && (
                                    <div className={clsx(
                                        'flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[10px] font-bold uppercase tracking-wider border transition-all duration-300',
                                        sourceProgress.flatpak === 'active' ? 'bg-purple-500/15 text-purple-400 border-purple-500/30 animate-pulse' :
                                            sourceProgress.flatpak === 'done' ? 'bg-green-500/15 text-green-400 border-green-500/30' :
                                                sourceProgress.flatpak === 'error' ? 'bg-red-500/15 text-red-400 border-red-500/30' :
                                                    'bg-app-fg/5 text-app-muted/50 border-app-border/50'
                                    )}>
                                        {sourceProgress.flatpak === 'done' ? <CheckCircle2 size={10} /> :
                                            sourceProgress.flatpak === 'active' ? <Loader2 size={10} className="animate-spin" /> :
                                                <span className="w-1.5 h-1.5 rounded-full bg-current" />}
                                        Flatpak
                                    </div>
                                )}
                                {sourceSections
                                    .filter(([t]) => !['repo', 'aur', 'flatpak'].includes(t))
                                    .map(([t]) => (
                                        <div
                                            key={`badge-${t}`}
                                            className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[10px] font-bold uppercase tracking-wider border transition-all duration-300 bg-app-fg/5 text-app-muted/70 border-app-border/50"
                                        >
                                            <span className="w-1.5 h-1.5 rounded-full bg-current" />
                                            {t}
                                        </div>
                                    ))}
                            </div>
                            <p className="mt-2 text-[11px] text-slate-500 dark:text-app-muted">
                                {sourceProgressDetail.repo.stage !== 'idle' && `Repo: ${sourceProgressDetail.repo.stage} ${sourceProgressDetail.repo.total > 0 ? `(${sourceProgressDetail.repo.current}/${sourceProgressDetail.repo.total})` : ''} `}
                                {sourceProgressDetail.aur.stage !== 'idle' && `• AUR: ${sourceProgressDetail.aur.stage} ${sourceProgressDetail.aur.total > 0 ? `(${sourceProgressDetail.aur.current}/${sourceProgressDetail.aur.total})` : ''} `}
                                {sourceProgressDetail.flatpak.stage !== 'idle' && `• Flatpak: ${sourceProgressDetail.flatpak.stage} ${sourceProgressDetail.flatpak.total > 0 ? `(${sourceProgressDetail.flatpak.current}/${sourceProgressDetail.flatpak.total})` : ''}`}
                            </p>

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

                <AnimatePresence>
                    {!isUpdating && lastUpdateSummary && (
                        <motion.div
                            initial={{ opacity: 0, y: -8 }}
                            animate={{ opacity: 1, y: 0 }}
                            className={clsx(
                                'mt-6 rounded-2xl border p-4',
                                lastUpdateSummary.overall === 'success'
                                    ? 'bg-emerald-500/10 border-emerald-500/25'
                                    : lastUpdateSummary.overall === 'partial'
                                        ? 'bg-amber-500/10 border-amber-500/25'
                                        : 'bg-red-500/10 border-red-500/25'
                            )}
                        >
                            <div className="flex items-center justify-between gap-4">
                                <div>
                                    <h3 className="text-sm font-bold text-slate-900 dark:text-white">Update Summary</h3>
                                    <p className="text-xs text-slate-600 dark:text-app-muted mt-1">{lastUpdateSummary.message}</p>
                                </div>
                                <div className="text-right text-xs">
                                    <div className="font-bold text-slate-900 dark:text-white">{lastUpdateSummary.summary.succeeded_packages.length} updated</div>
                                    <div className="text-red-500">{lastUpdateSummary.summary.failed_packages.length} failed</div>
                                </div>
                            </div>
                            {lastUpdateSummary.summary.failed_packages.length > 0 && (
                                <div className="mt-3 space-y-1">
                                    {lastUpdateSummary.summary.failed_packages.slice(0, 5).map((item, idx) => (
                                        <p key={`${item.name}:${idx}`} className="text-xs text-slate-700 dark:text-app-muted">
                                            {item.name} ({item.source}): {item.reason}
                                        </p>
                                    ))}
                                    {lastUpdateSummary.summary.failed_packages.length > 5 && (
                                        <p className="text-xs text-slate-500 dark:text-app-muted">+{lastUpdateSummary.summary.failed_packages.length - 5} more failures</p>
                                    )}
                                    {retryTargets.length > 0 && (
                                        <button
                                            onClick={retryFailedOnly}
                                            className="mt-2 px-3 py-1.5 rounded-lg bg-amber-500 hover:bg-amber-600 text-white text-xs font-bold"
                                        >
                                            Retry failed only
                                        </button>
                                    )}
                                </div>
                            )}
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
                                            onClick={() => commands.launchPackage({ package_name: 'reboot', app_id: null, desktop_entry: null, launch_target: null, source: null }).catch(() => { })}
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
                        {sourceStatuses.filter((status) => status.status === 'timeout' || status.status === 'error').map((status) => (
                            <div
                                key={status.source}
                                className="rounded-2xl border border-amber-500/20 bg-amber-500/10 px-4 py-3 text-sm text-amber-800 dark:text-amber-200"
                            >
                                {status.source.toUpperCase()} check {status.status}: {status.error || 'Unknown error'}
                            </div>
                        ))}
                        {sourceSections.map(([sourceType, sectionUpdates]) => {
                            if (sourceType === 'repo' && !advancedScope.repo) return null;
                            if (sourceType === 'aur' && !advancedScope.aur) return null;
                            if (sourceType === 'flatpak' && !advancedScope.flatpak) return null;
                            if (sectionUpdates.length === 0) return null;
                            const sectionLabel = sourceLabel(sourceType);
                            return (
                                <div key={sourceType} className="space-y-3">
                                    <h3 className="text-xs font-bold uppercase tracking-wider text-slate-500 dark:text-app-muted px-1">
                                        {sectionLabel} — {sectionUpdates.length} update{sectionUpdates.length !== 1 ? 's' : ''}
                                    </h3>
                                    {sectionUpdates.map((pkg, idx) => (
                                        (() => {
                                            const pkgKey = getUpdateKey(pkg);
                                            const excluded = Boolean(excludedUpdateKeys[pkgKey]);
                                            return (
                                        <div
                                            key={`${pkg.name}:${String(pkg.source?.source_type ?? 'repo')}:${String(pkg.source?.id ?? pkg.name)}:${idx}`}
                                            className={clsx(
                                                "bg-white dark:bg-app-card border border-black/5 dark:border-white/5 rounded-2xl p-5 flex items-center justify-between hover:bg-white/80 dark:hover:bg-white/5 transition-all group hover:scale-[1.01] hover:shadow-xl hover:border-black/10 dark:hover:border-white/10",
                                                excluded && "opacity-50"
                                            )}
                                        >
                                            <div className="flex items-center gap-6">
                                                <div className="w-14 h-14 rounded-xl bg-slate-50 dark:bg-black/20 flex items-center justify-center shrink-0 overflow-hidden relative p-2 border border-black/5 dark:border-white/5 shadow-inner">
                                                    <AppIcon pkgId={pkg.name} iconUrl={pkg.icon} />
                                                </div>
                                                <div>
                                                    <h3 className="font-bold flex items-center gap-3 text-xl text-slate-900 dark:text-white mb-1">
                                                        {pkg.display_name || pkg.name}
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
                                                {showAdvancedControls && (
                                                    <label className="flex items-center gap-2 text-xs text-slate-500 dark:text-app-muted cursor-pointer">
                                                        <input
                                                            type="checkbox"
                                                            checked={!excluded}
                                                            onChange={() => {
                                                                setExcludedUpdateKeys((prev) => ({
                                                                    ...prev,
                                                                    [pkgKey]: !prev[pkgKey],
                                                                }));
                                                            }}
                                                        />
                                                        Include
                                                    </label>
                                                )}
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
                                            );
                                        })()
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
                    setManifest(null);
                }}
                onConfirm={performUpdate}
                title="Update System"
                message={
                    <div className="space-y-4">
                        <p>
                            This will apply selected updates across system repos, AUR, and Flatpak. We will report partial success if any source has failures.
                        </p>

                        {/* 3.1: Transaction Manifest Details */}
                        <div className="space-y-2 bg-app-bg/50 rounded-xl p-3 border border-app-border">
                            <p className="text-xs font-semibold text-app-fg/80 uppercase tracking-wider">Transaction Summary</p>
                            {(manifest?.repo_count ?? selectedUpdates.filter(u => u.source.source_type === 'repo').length) > 0 && (
                                <div className="flex items-center gap-2">
                                    <span className="w-2 h-2 rounded-full bg-blue-500" />
                                    <span className="text-xs text-app-muted">
                                        <strong className="text-app-fg">{manifest?.repo_count ?? selectedUpdates.filter(u => u.source.source_type === 'repo').length}</strong> Official packages (full system upgrade)
                                    </span>
                                </div>
                            )}
                            {(manifest?.aur_count ?? selectedUpdates.filter(u => u.source.source_type === 'aur').length) > 0 && (
                                <div className="flex items-center gap-2">
                                    <span className="w-2 h-2 rounded-full bg-amber-500" />
                                    <span className="text-xs text-app-muted">
                                        <strong className="text-app-fg">{manifest?.aur_count ?? selectedUpdates.filter(u => u.source.source_type === 'aur').length}</strong> AUR packages
                                        <span className="text-[10px] opacity-60 ml-1">(build from source)</span>
                                    </span>
                                </div>
                            )}
                            {(manifest?.flatpak_count ?? selectedUpdates.filter(u => u.source.source_type === 'flatpak').length) > 0 && (
                                <div className="flex items-center gap-2">
                                    <span className="w-2 h-2 rounded-full bg-purple-500" />
                                    <span className="text-xs text-app-muted">
                                        <strong className="text-app-fg">{manifest?.flatpak_count ?? selectedUpdates.filter(u => u.source.source_type === 'flatpak').length}</strong> Flatpak apps
                                    </span>
                                </div>
                            )}
                            <hr className="border-app-border/50" />
                            <p className="text-[10px] text-app-muted/60 font-mono">
                                Total: {manifest?.total ?? selectedUpdates.length} package{(manifest?.total ?? selectedUpdates.length) !== 1 ? 's' : ''}
                            </p>
                        </div>

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
                showPasswordInput={false}
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
