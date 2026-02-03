import { useState, useEffect, useRef } from 'react';
import { Terminal, CheckCircle2, XCircle, Loader2, Play, Minimize2, Maximize2, ShieldCheck, RefreshCw, ChevronUp, Trash2, Download, Package as PackageIcon, Sparkles, Unlock, Key, HardDrive, Wifi } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { clsx } from 'clsx';
import { friendlyError } from '../utils/friendlyError';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { useAppStore } from '../store/internal_store';
import { useSessionPassword } from '../context/useSessionPassword';
import { useErrorService } from '../context/ErrorContext';
import { useToast } from '../context/ToastContext';

import { PackageSource } from '../types/alpm';

interface InstallMonitorProps {
    pkg: { name: string; source: PackageSource; repoName?: string; } | null;
    onClose: () => void;
    mode?: 'install' | 'uninstall';
    onSuccess?: () => void;
}

// Matches the Rust AlpmClassifiedError (helper) and GUI error_classifier
interface ClassifiedError {
    kind: string;
    title: string;
    description: string;
    /** Helper sends string (e.g. "UnlockDatabase"); GUI classifier may send object */
    recovery_action?: string | { type: string; payload?: string };
    raw_message: string;
}

export default function InstallMonitor({ pkg, onClose, mode = 'install', onSuccess }: InstallMonitorProps) {
    const { requestSessionPassword } = useSessionPassword();
    const errorService = useErrorService();
    const reducePasswordPrompts = useAppStore((s) => s.reducePasswordPrompts);
    const { show: showToast } = useToast();

    const [status, setStatus] = useState<'idle' | 'running' | 'success' | 'error'>('idle');

    const [logs, setLogs] = useState<string[]>([]);
    const [visualProgress, setVisualProgress] = useState(0);
    const [targetProgress, setTargetProgress] = useState(0);
    const [minimized, setMinimized] = useState(false);
    // Compact by default; user can open "View log" if they want full output
    const [showLogs, setShowLogs] = useState(false);
    const logsEndRef = useRef<HTMLDivElement>(null);
    const actionStartedForRef = useRef<string | null>(null);
    const silentDbRepairAttemptedRef = useRef(false);
    const logsRef = useRef<string[]>([]);
    const autoUnlockAttemptedRef = useRef(false);
    const [commandPreview, setCommandPreview] = useState<string>('');
    // Throttle log updates to prevent freeze when hundreds of progress events arrive
    const logBufferRef = useRef<string[]>([]);
    const logFlushScheduledRef = useRef(false);
    const LOG_CAP = 2000;
    const flushLogBufferRef = useRef<() => void>(() => { });
    flushLogBufferRef.current = () => {
        if (logBufferRef.current.length === 0) {
            logFlushScheduledRef.current = false;
            return;
        }
        const toAdd = logBufferRef.current;
        logBufferRef.current = [];
        logFlushScheduledRef.current = false;
        setLogs((prev) => {
            const next = [...prev, ...toAdd];
            return next.length > LOG_CAP ? next.slice(-LOG_CAP) : next;
        });
    };
    const appendLogThrottled = (message: string) => {
        logBufferRef.current.push(message);
        if (!logFlushScheduledRef.current) {
            logFlushScheduledRef.current = true;
            setTimeout(() => flushLogBufferRef.current(), 180);
        }
    };

    // Throttle progress/status updates so we don't re-render hundreds of times per second (prevents freeze)
    const progressStatusRef = useRef<{ target: number; status: string }>({ target: 0, status: '' });
    const progressFlushScheduledRef = useRef(false);
    const PROGRESS_FLUSH_MS = 200;
    const flushProgressStatusRef = useRef<() => void>(() => { });
    flushProgressStatusRef.current = () => {
        progressFlushScheduledRef.current = false;
        const { target, status } = progressStatusRef.current;
        setTargetProgress(target);
        setDetailedStatus(status);
    };
    const setProgressStatusThrottled = (target: number, status: string) => {
        progressStatusRef.current = { target, status };
        if (!progressFlushScheduledRef.current) {
            progressFlushScheduledRef.current = true;
            setTimeout(() => flushProgressStatusRef.current(), PROGRESS_FLUSH_MS);
        }
    };

    // Structured error from backend classification
    const [classifiedError, setClassifiedError] = useState<ClassifiedError | null>(null);
    const [isRecovering, setIsRecovering] = useState(false);

    logsRef.current = logs;

    // Sync verbose preference to storage (for Settings "Show Detailed Transaction Logs")
    useEffect(() => {
        if (showLogs) {
            localStorage.setItem('monarch_verbose_logs', 'true');
            useAppStore.getState().verboseLogsEnabled !== true && useAppStore.getState().setVerboseLogsEnabled?.(true);
        } else {
            localStorage.removeItem('monarch_verbose_logs');
        }
        logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [logs, minimized, showLogs]);

    const [detailedStatus, setDetailedStatus] = useState<string>('');

    const copyLogsToClipboard = async () => {
        const content = logsRef.current.join('\n');
        if (!content.trim()) {
            showToast('No logs available to copy.', 'info');
            return;
        }
        if (typeof navigator === 'undefined' || !navigator.clipboard) {
            showToast('Clipboard is unavailable in this environment.', 'error');
            return;
        }
        try {
            await navigator.clipboard.writeText(content);
            showToast('Install logs copied to clipboard.', 'success');
        } catch (err) {
            showToast('Unable to copy logs. Check clipboard permissions.', 'error');
        }
    };

    // Listeners
    useEffect(() => {
        if (!pkg) return;

        // ✅ NEW: Listen for structured ALPM progress events
        const unlistenAlpmProgress = listen('alpm-progress', (event: { payload: any }) => {
            const evt = event.payload as import('../types/alpm').AlpmProgressEvent;

            // Don't flood logs with every download_progress tick (status line already shows %); throttle and cap to prevent freeze
            if (evt.event_type !== 'download_progress') {
                appendLogThrottled(evt.message);
            }

            // Handle different event types — throttle all progress/status to prevent freeze from hundreds of updates/sec
            switch (evt.event_type) {
                case 'download_progress':
                    if (evt.percent !== undefined) {
                        setProgressStatusThrottled(
                            40 + Math.floor((evt.percent * 50) / 100),
                            `Downloading ${evt.package || 'package'}... ${evt.percent}%`
                        );
                    }
                    break;
                case 'extract_start':
                    setProgressStatusThrottled(90, `Extracting ${evt.package || 'package'}...`);
                    break;
                case 'extract_progress':
                    if (evt.percent !== undefined) {
                        setProgressStatusThrottled(
                            90 + Math.floor((evt.percent * 5) / 100),
                            `Extracting ${evt.package || 'package'}... ${evt.percent}%`
                        );
                    }
                    break;
                case 'install_start':
                    setProgressStatusThrottled(95, `Installing ${evt.package || 'package'}...`);
                    break;
                case 'install_progress':
                    if (evt.percent !== undefined) {
                        setProgressStatusThrottled(
                            95 + Math.floor((evt.percent * 5) / 100),
                            `Installing ${evt.package || 'package'}... ${evt.percent}%`
                        );
                    }
                    break;
                case 'install_complete':
                    setProgressStatusThrottled(99, `Installed ${evt.package || 'package'}`);
                    break;
                case 'progress':
                    if (evt.percent !== undefined) {
                        setProgressStatusThrottled(evt.percent, evt.message);
                    }
                    break;
                default:
                    setProgressStatusThrottled(progressStatusRef.current.target, evt.message);
            }
        });

        const unlistenOutput = listen('install-output', (event: { payload: unknown }) => {
            if (typeof event.payload !== 'string') return;
            const line = event.payload;
            appendLogThrottled(line);

            // Enhanced Progress Heuristics (fallback for non-ALPM operations like AUR builds) — throttled
            if (line.includes('%')) {
                const match = line.match(/(\d+)%/);
                if (match) {
                    const p = parseInt(match[1], 10);
                    // Don't reset bar to 0 when backend sends "Downloading ... 0%" (alpm-progress drives real %)
                    if (p > 0 || !line.includes('Downloading')) setProgressStatusThrottled(p, progressStatusRef.current.status || line);
                }
            } else if (line.includes('Cloning')) {
                setProgressStatusThrottled(10, 'Downloading Source Code...');
            } else if (line.includes('Checking dependencies')) {
                setProgressStatusThrottled(5, 'Resolving Dependencies...');
            } else if (line.includes('Building') && line.includes('dependencies')) {
                setProgressStatusThrottled(progressStatusRef.current.target, 'Building Dependencies...');
            } else if (line.includes('makepkg')) {
                setProgressStatusThrottled(20, 'Compiling Source (This may take a while)...');
            } else if (line.includes('Auto-importing PGP key')) {
                setProgressStatusThrottled(progressStatusRef.current.target, 'Security: Importing Signing Keys...');
            } else if (line.includes('Retrying build')) {
                setProgressStatusThrottled(progressStatusRef.current.target, 'Retrying Build with New Keys...');
            } else if (line.toLowerCase().includes('compiling')) {
                const next = Math.min(progressStatusRef.current.target + 1, 90);
                setProgressStatusThrottled(next, progressStatusRef.current.status);
            }
        });

        const unlistenRepair = listen('repair-log', (event: { payload: unknown }) => {
            if (typeof event.payload !== 'string') return;
            setLogs((prev: string[]) => [...prev, event.payload as string]);
        });

        const unlistenComplete = listen('install-complete', async (event: { payload: string }) => {
            if (event.payload === 'success') {
                setStatus('success');
                setTargetProgress(100);
                setVisualProgress(100);
                setDetailedStatus(`${mode === 'uninstall' ? 'Uninstallation' : 'Installation'} Complete`);
                if (onSuccess) onSuccess();
                return;
            }
            // Failure: try silent self-heal (no error popup)
            const currentLogs = logsRef.current;
            const hasCorruptDb = currentLogs.some((l: string) =>
                l.includes('Unrecognized archive format') || l.includes('could not open database') || l.includes('Sync databases are corrupt')
            );
            const hasDbLocked = currentLogs.some((l: string) =>
                l.includes('db.lck') || l.includes('Database Locked') || l.includes('ALPM_ERR_DB_WRITE') || l.includes('unable to lock database') || l.includes('could not remove') && l.includes('db.lck')
            );
            if (event.payload !== 'success' && hasDbLocked && !autoUnlockAttemptedRef.current) {
                autoUnlockAttemptedRef.current = true;
                setDetailedStatus('Waiting for another update...');
                setLogs(prev => [...prev, '\n--- Auto-unlocking database ---']);
                try {
                    const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
                    await invoke('repair_unlock_pacman', { password: pwd });
                    setLogs(prev => [...prev, '✓ Database unlocked. Retrying...']);
                    setTargetProgress(5);
                    setStatus('running');
                    actionStartedForRef.current = null;
                    handleAction();
                } catch (e) {
                    setLogs(prev => [...prev, `Unlock failed: ${e}`]);
                    setStatus('error');
                }
                return;
            }
            if (event.payload !== 'success' && hasCorruptDb && !silentDbRepairAttemptedRef.current) {
                silentDbRepairAttemptedRef.current = true;
                setDetailedStatus('Repairing databases...');
                setLogs(prev => [...prev, '\n--- Self-healing: Refreshing package databases ---']);
                try {
                    const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
                    await invoke('force_refresh_databases', { password: pwd });
                    setLogs(prev => [...prev, '✓ Databases refreshed. Retrying...']);
                    setTargetProgress(5);
                    setStatus('running');
                    actionStartedForRef.current = null;
                    handleAction();
                } catch (e) {
                    errorService.reportError(e as Error | string);
                    setLogs(prev => [...prev, `Repair failed: ${e}`]);
                    setStatus('error');
                }
                return;
            }
            setStatus('error');
        });

        // Listen for structured error classification from backend
        const unlistenClassifiedError = listen<ClassifiedError>('install-error-classified', (event) => {
            setClassifiedError(event.payload);
            setStatus('error');
        });

        return () => {
            unlistenAlpmProgress.then((f: () => void) => f()).catch(() => { });
            unlistenOutput.then((f: () => void) => f()).catch(() => { });
            unlistenRepair.then((f: () => void) => f()).catch(() => { });
            unlistenComplete.then((f: () => void) => f()).catch(() => { });
            unlistenClassifiedError.then((f: () => void) => f()).catch(() => { });
        };
    }, [pkg, reducePasswordPrompts, requestSessionPassword]);

    // Recovery action handlers
    const handleRecoveryAction = async (action: string) => {
        setIsRecovering(true);
        setLogs(prev => [...prev, `\n--- RECOVERY: ${action.toUpperCase()} ---`]);

        try {
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            switch (action) {
                case 'UnlockDatabase':
                    setLogs(prev => [...prev, 'Checking for stale lock file...']);
                    await invoke('repair_unlock_pacman', { password: pwd });
                    setLogs(prev => [...prev, '✓ Database unlocked successfully']);
                    break;

                case 'RepairKeyring':
                    setLogs(prev => [...prev, 'Resetting security keyring...', 'This may take a moment...']);
                    await invoke('fix_keyring_issues', { password: pwd });
                    setLogs(prev => [...prev, '✓ Keyring repaired successfully']);
                    break;

                case 'ForceRefreshDb':
                case 'RefreshMirrors':
                    setLogs(prev => [...prev, 'Forcing database refresh...']);
                    await invoke('trigger_repo_sync', { forceRefresh: true });
                    setLogs(prev => [...prev, '✓ Databases refreshed']);
                    break;

                case 'CleanCache':
                    setLogs(prev => [...prev, 'Clearing package cache...']);
                    await invoke('clear_cache', { keepVersions: 1 });
                    setLogs(prev => [...prev, '✓ Cache cleared']);
                    break;

                default:
                    setLogs(prev => [...prev, 'Preparing to retry...']);
            }

            // Reset state and retry the operation
            setLogs(prev => [...prev, '\n--- RETRYING OPERATION ---']);
            setClassifiedError(null);
            setStatus('running');
            setTargetProgress(5);

            // Retry the original action
            await handleAction();

        } catch (e) {
            setLogs(prev => [...prev, `Recovery failed: ${e}`]);
            setStatus('error');
        } finally {
            setIsRecovering(false);
        }
    };

    // Get recovery button config based on error kind
    const getRecoveryConfig = (kind: string) => {
        switch (kind) {
            case 'DatabaseLocked':
                return { icon: Unlock, label: 'Unlock & Retry', color: 'bg-amber-500 hover:bg-amber-600' };
            case 'KeyringError':
                return { icon: Key, label: 'Repair Keys & Retry', color: 'bg-purple-500 hover:bg-purple-600' };
            case 'MirrorFailure':
                return { icon: Wifi, label: 'Retry Download', color: 'bg-blue-500 hover:bg-blue-600' };
            case 'DiskFull':
                return { icon: HardDrive, label: 'Clear Cache & Retry', color: 'bg-red-500 hover:bg-red-600' };
            case 'PackageNotFound':
                return { icon: RefreshCw, label: 'Refresh & Retry', color: 'bg-teal-500 hover:bg-teal-600' };
            default:
                return { icon: RefreshCw, label: 'Retry', color: 'bg-blue-500 hover:bg-blue-600' };
        }
    };

    // SMOTH PROGRESS ANIMATION & PSEUDO-PROGRESS
    useEffect(() => {
        if (status !== 'running') return;

        const interval = setInterval(() => {
            setVisualProgress((prev: number) => {
                // If visual is behind target, move towards it smoothly
                if (prev < targetProgress) {
                    const diff = targetProgress - prev;
                    if (diff > 5) return prev + 1; // Faster catchup
                    return prev + 0.2; // Smooth crawl
                }

                // PSEUDO-PROGRESS: If we are at target but still running, 
                // crawl forward slowly to show activity (up to 95%)
                if (prev < 95) {
                    return prev + 0.05; // Very slow tick (pseudo-life)
                }

                return prev;
            });
        }, 100);

        return () => clearInterval(interval);
    }, [status, targetProgress]);

    // Auto-Start (One-Click Experience). Guard so we only run once per pkg (avoids React Strict Mode double-invocation → double password prompt).
    useEffect(() => {
        if (!pkg) {
            actionStartedForRef.current = null;
            return;
        }
        if (status === 'idle' && actionStartedForRef.current !== pkg.name) {
            actionStartedForRef.current = pkg.name;
            handleAction();
        }
    }, [pkg, status]);

    const handleAction = async () => {
        if (!pkg) return;
        setStatus('running');
        setLogs([`Starting ${mode === 'uninstall' ? 'uninstallation' : 'installation'} engine...`, `Target: ${pkg.name} (${pkg.source.label || pkg.source.id})`]);
        setTargetProgress(5);
        setVisualProgress(0);

        try {
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            if (mode === 'uninstall') {
                await invoke('uninstall_package', {
                    name: pkg.name,
                    source: pkg.source,
                    password: pwd
                });

                if (pkg.source.source_type === 'flatpak') {
                    setCommandPreview(`$ flatpak uninstall ${pkg.name} -y`);
                } else {
                    setCommandPreview(`$ pacman -Rns --noconfirm ${pkg.name}`);
                }
            } else {
                await invoke('install_package', {
                    name: pkg.name,
                    source: pkg.source,
                    password: pwd,
                    repoName: pkg.repoName || null
                });

                // Set Command Preview
                if (pkg.source.source_type === 'aur') {
                    setCommandPreview(`$ git clone https://aur.archlinux.org/${pkg.name}.git && makepkg -si`);
                } else if (pkg.source.source_type === 'flatpak') {
                    setCommandPreview(`$ flatpak install flathub ${pkg.name} -y`);
                } else {
                    setCommandPreview(`$ pacman -S --noconfirm ${pkg.name}`);
                }
            }
            // The command is async spawned, completion comes via event
        } catch (e) {
            errorService.reportError(e as Error | string);
            setLogs((prev: string[]) => [...prev, `Error launching: ${e}`]);
            setStatus('error');
        }
    };

    useEscapeKey(onClose, !!pkg);
    const focusTrapRef = useFocusTrap(!!pkg && !minimized);

    // Hooks must run unconditionally (before any early return) to avoid "Rendered fewer hooks than expected"
    const [isRepairing, setIsRepairing] = useState(false);
    const [repairSuccess, setRepairSuccess] = useState(false);
    const [autoRetryAttempted, setAutoRetryAttempted] = useState(false);
    const [updateRequired, setUpdateRequired] = useState(false);

    // Error Interceptor
    useEffect(() => {
        if (status === 'error' && logs.some(l => l.includes('SystemUpdateRequired'))) {
            // handled by handleAction catch block primarily, but checking logs is backup
        }
    }, [status, logs]);

    // Listener for specific failed_update_required event
    useEffect(() => {
        const unlistenUpdateReq = listen('install-complete', (event: { payload: string }) => {
            if (event.payload === 'failed_update_required') {
                setStatus('error');
                setUpdateRequired(true);
                setDetailedStatus("System Update Required");
                setLogs(prev => [...prev, "STOP: Package not found in current database.", "This usually means your system is out of date."]);
            }
        });
        return () => { unlistenUpdateReq.then(f => f()); };
    }, []);

    // Retry after Repair (must be registered unconditionally; handleAction is defined earlier in this component)
    useEffect(() => {
        if (repairSuccess && autoRetryAttempted && status !== 'running' && status !== 'success') {
            setLogs(prev => [...prev, '✓ System repaired. Retrying operation automatically...']);
            handleAction();
        }
    }, [repairSuccess, autoRetryAttempted]);

    if (!pkg) return null;

    const errorDetails = status === 'error' && logs.length > 0 ? friendlyError(logs[logs.length - 1]) : null;

    // STEPPER LOGIC
    const steps = [
        { id: 1, label: 'Safety', icon: ShieldCheck },
        { id: 2, label: 'Downloading', icon: Download },
        { id: 3, label: 'Installing', icon: PackageIcon },
        { id: 4, label: 'Finalizing', icon: Sparkles }
    ];

    const currentStep = (() => {
        if (status === 'success') return 4;
        if (detailedStatus.includes('Safety') || detailedStatus.includes('Resolving') || detailedStatus.includes('Lock')) return 1;
        if (detailedStatus.includes('Downloading') || detailedStatus.includes('Syncing') || detailedStatus.includes('Cloning')) return 2;
        if (detailedStatus.includes('Installing') || detailedStatus.includes('Building') || detailedStatus.includes('Compiling')) return 3;
        return 1;
    })();

    const displayStatus = status === 'error' && errorDetails
        ? errorDetails.title
        : status === 'idle' ? `Ready to ${mode === 'uninstall' ? 'Uninstall' : 'Install'}`
            : status === 'success' ? `${mode === 'uninstall' ? 'Uninstallation' : 'Installation'} Complete`
                : detailedStatus || (pkg.source.source_type === 'aur' ? 'Building App (This may take a while)...' : `${mode === 'uninstall' ? 'Uninstalling' : 'Installing'}...`);

    // RENDER STEPPER
    const renderStepper = () => (
        <div className="flex items-center justify-between px-5 py-3 bg-app-bg/50 border-b border-app-border">
            {steps.map((step, idx) => {
                const isActive = currentStep === step.id;
                const isCompleted = currentStep > step.id || status === 'success';

                return (
                    <div key={step.id} className="flex flex-col items-center gap-2 relative z-10 w-20">
                        <div className={clsx(
                            "w-8 h-8 rounded-full flex items-center justify-center transition-all duration-500",
                            isCompleted ? "bg-green-500 text-white" :
                                isActive ? "bg-blue-500 text-white shadow-[0_0_15px_rgba(59,130,246,0.5)]" :
                                    "bg-app-fg/10 text-app-muted"
                        )}>
                            {isCompleted ? <CheckCircle2 size={16} /> : <step.icon size={14} />}
                        </div>
                        <span className={clsx(
                            "text-[10px] font-bold uppercase tracking-wider transition-colors duration-300",
                            (isActive || isCompleted) ? "text-app-fg" : "text-app-muted/50"
                        )}>
                            {step.label}
                        </span>

                        {/* Connector Line */}
                        {idx < steps.length - 1 && (
                            <div className="absolute top-4 left-[50%] w-[calc(100%+2rem)] h-[2px] bg-app-fg/5 -z-10">
                                <div
                                    className="h-full bg-green-500 transition-all duration-700"
                                    style={{ width: isCompleted ? '100%' : '0%' }}
                                />
                            </div>
                        )}
                    </div>
                );
            })}
        </div>
    );

    if (minimized) {
        return (
            <div className="fixed bottom-4 right-4 z-50 bg-app-card border border-app-border p-4 rounded-xl shadow-2xl flex items-center gap-4 w-80 animate-in slide-in-from-bottom-4 transition-colors">
                <div className="bg-blue-500/20 p-2 rounded-lg text-blue-500 dark:text-blue-400">
                    <Loader2 size={20} className="animate-spin" />
                </div>
                <div className="flex-1">
                    <div className="text-sm font-bold text-app-fg">{detailedStatus || `Installing ${pkg.name}`}</div>
                    <div className="w-full bg-app-fg/10 h-1.5 mt-2 rounded-full overflow-hidden">
                        <div className="h-full bg-blue-500 transition-all duration-300" style={{ width: `${visualProgress}%` }} />
                    </div>
                </div>
                <button onClick={() => setMinimized(false)} className="p-2 hover:bg-app-fg/10 rounded-lg text-app-muted" aria-label="Expand install window">
                    <Maximize2 size={16} />
                </button>
            </div>
        );
    }

    // Heuristic Scan for Keyring Issues
    const hasKeyringError = logs.some(l =>
        l.includes("GPGME error") ||
        l.includes("PGP signature") ||
        l.includes("corrupted database") ||
        l.includes("invalid or corrupted")
    );

    const hasLockError = logs.some(l => l.includes("database is locked"));

    // AUTO-HEAL LOGIC (DISABLED - Pillar 3: "Ask First" Rule)
    // We now rely on the UI button to trigger handleRepair, instead of doing it automatically.
    /*
    useEffect(() => {
        if (status === 'error' && !autoRetryAttempted) {
             // ...
        }
    }, ...);
    */

    const handleUnlock = async () => {
        setIsRepairing(true);
        setAutoRetryAttempted(true); // Enable auto-retry after fix
        try {
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            await invoke('repair_unlock_pacman', { password: pwd });
            setLogs(prev => [...prev, '✓ Database unlocked.', 'Please try installing again.']);
            setRepairSuccess(true);
        } catch (e) {
            setLogs(prev => [...prev, `Unlock Failed: ${e}`]);
        } finally {
            setIsRepairing(false);
        }
    };

    const handleRepair = async () => {
        setIsRepairing(true);
        setAutoRetryAttempted(true); // Enable auto-retry after fix
        setLogs(prev => [...prev, '\n--- AUTO-HEALING: FIXING KEYRING ISSUES ---', 'The app detected a security key error.', 'Attempting to automatically repair trust database...', 'This will take a moment...']);
        try {
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            await invoke('repair_reset_keyring', { password: pwd });
            setLogs(prev => [...prev, '✓ Keyring reset successfully.', '--- REPAIR COMPLETE ---']);
            setRepairSuccess(true);
        } catch (e) {
            errorService.reportError(e as Error | string);
            setLogs(prev => [...prev, `Repair Failed: ${e}`]);
        } finally {
            setIsRepairing(false);
        }
    };

    const handleUpdateAndInstall = async () => {
        if (!pkg) return;
        setUpdateRequired(false);
        setStatus('running');
        setDetailedStatus('Updating System & Installing...');
        setLogs([]); // Clear previous error logs
        setLogs(prev => [...prev, '\n--- STARTING SYSTEM UPDATE ---', 'Syncing databases...', 'Performing full system upgrade (-Syu)...', 'This may take a while. Do not turn off your computer.']);

        try {
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            await invoke('update_and_install_package', {
                name: pkg.name,
                repoName: pkg.repoName || null,
                password: pwd
            });
            // Completion handled by event listener above
        } catch (e) {
            setLogs(prev => [...prev, `Update Failed: ${e}`]);
            setStatus('error');
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 bg-app-bg/60 backdrop-blur-sm animate-in fade-in duration-200">
            <div ref={focusTrapRef} className={clsx("w-full bg-app-card border border-app-border rounded-2xl shadow-2xl overflow-hidden flex flex-col transition-colors", showLogs ? "max-w-2xl max-h-[85vh]" : "max-w-md max-h-[min(70vh,420px)]")} role="dialog" aria-modal="true" aria-labelledby="install-monitor-title">
                {/* Header */}
                <div className="px-5 py-4 border-b border-app-border flex items-center justify-between bg-app-fg/5">
                    <div className="flex items-center gap-3">
                        <div className={clsx("w-10 h-10 rounded-full flex items-center justify-center",
                            status === 'success' ? "bg-green-500/20 text-green-500" :
                                (status === 'error' || updateRequired) ? "bg-red-500/20 text-red-500" :
                                    "bg-blue-500/20 text-blue-500"
                        )}>
                            {status === 'success' ? <CheckCircle2 size={20} /> :
                                (status === 'error' || updateRequired) ? <XCircle size={20} /> :
                                    <Terminal size={20} />}
                        </div>
                        <div className="min-w-0">
                            <h2 id="install-monitor-title" className="text-lg font-bold text-app-fg truncate">
                                {updateRequired ? "System Update Required" : displayStatus}
                            </h2>
                            {status === 'error' && errorDetails && (
                                <>
                                    <p className="text-red-500 text-sm font-medium mt-1 animate-in fade-in">{errorDetails.description}</p>
                                    {errorDetails.expertMessage && showLogs && (
                                        <p className="text-app-muted text-[10px] font-mono mt-2 opacity-80" title="Expert / raw output">
                                            [ALPM] {errorDetails.expertMessage.slice(0, 120)}{errorDetails.expertMessage.length > 120 ? '…' : ''}
                                        </p>
                                    )}
                                </>
                            )}
                            {status !== 'error' && (
                                <p className="text-app-muted text-sm">{pkg.source.label} Source</p>
                            )}
                        </div>
                    </div>
                        <div className="flex items-center gap-2">
                        <button
                            onClick={() => setShowLogs(!showLogs)}
                            className={clsx(
                                "p-2 rounded-lg transition-colors border",
                                showLogs ? "bg-app-fg/10 text-app-fg border-app-fg/10" : "text-app-muted hover:bg-app-fg/5 border-transparent"
                            )}
                            title={showLogs ? "Hide Logs" : "Show Transaction Logs"}
                        >
                            <Terminal size={18} />
                        </button>
                            <button
                                onClick={copyLogsToClipboard}
                                disabled={logs.length === 0}
                                className={clsx(
                                    "px-2.5 py-1.5 rounded-lg text-xs font-semibold border transition-colors",
                                    logs.length === 0
                                        ? "text-app-muted border-app-border/40 cursor-not-allowed opacity-60"
                                        : "text-accent border-app-border hover:border-accent hover:opacity-80"
                                )}
                            >
                                Copy Logs
                            </button>
                        {status === 'running' && (
                            <>
                                <button
                                    onClick={async () => {
                                        try {
                                            await invoke('cancel_install');
                                            setLogs(prev => [...prev, 'Installation cancelled.']);
                                            setStatus('error');
                                            setTimeout(() => onClose(), 800);
                                        } catch (e) {
                                            errorService.reportError(e as Error | string);
                                        }
                                    }}
                                    className="px-3 py-1.5 bg-red-500/10 hover:bg-red-500/20 text-red-500 text-xs font-bold rounded-lg transition-colors border border-red-500/20 flex items-center gap-2"
                                    aria-label="Stop installation"
                                >
                                    <XCircle size={14} /> Cancel
                                </button>
                                <button onClick={() => setMinimized(true)} className="p-2 hover:bg-app-fg/10 rounded-lg text-app-muted transition-colors" aria-label="Minimize install window">
                                    <Minimize2 size={20} />
                                </button>
                            </>
                        )}
                        <button
                            onClick={async () => {
                                if (status === 'running') {
                                    const stop = window.confirm(
                                        'Closing this window will not stop the installation—it will continue in the background. Do you want to cancel the installation instead?'
                                    );
                                    if (stop) {
                                        try {
                                            await invoke('cancel_install');
                                            setLogs(prev => [...prev, 'Installation cancelled.']);
                                            setStatus('error');
                                            setTimeout(() => onClose(), 800);
                                        } catch (e) {
                                            errorService.reportError(e as Error | string);
                                        }
                                    }
                                    return;
                                }
                                onClose();
                            }}
                            className="p-2 hover:bg-red-500/10 hover:text-red-500 rounded-lg text-app-muted transition-colors"
                            aria-label="Close"
                        >
                            <XCircle size={20} />
                        </button>
                    </div>
                </div>

                {/* Body */}
                <div className="p-0 flex-1 overflow-hidden flex flex-col">
                    {!minimized && status !== 'idle' && !updateRequired && renderStepper()}
                    {updateRequired ? (
                        <div className="p-8 flex flex-col items-center justify-center space-y-6 animate-in slide-in-from-bottom-4">
                            <div className="w-16 h-16 bg-amber-500/20 rounded-full flex items-center justify-center mb-2">
                                <RefreshCw size={32} className="text-amber-500" />
                            </div>
                            <div className="text-center space-y-2 max-w-md">
                                <p className="text-app-fg font-bold text-lg">
                                    Your System is Out of Date
                                </p>
                                <p className="text-app-muted text-sm">
                                    This app requires libraries that are newer than what you have installed.
                                    To install it safely, we must update your system.
                                </p>
                            </div>

                            <div className="bg-app-fg/5 p-4 rounded-xl text-xs text-app-muted font-mono w-full max-w-md">
                                <div className="flex items-center gap-2 mb-2 font-bold text-app-fg">
                                    <Terminal size={14} /> Proposed Action:
                                </div>
                                <div className="opacity-70">$ pacman -Syu {pkg.name}</div>
                            </div>

                            <div className="flex gap-3 w-full max-w-md">
                                <button
                                    onClick={onClose}
                                    className="flex-1 bg-app-fg/5 hover:bg-app-fg/10 text-app-fg font-medium py-3 rounded-xl transition-colors"
                                >
                                    Cancel
                                </button>
                                <button
                                    onClick={handleUpdateAndInstall}
                                    className="flex-[2] bg-amber-600 hover:bg-amber-500 text-white font-bold py-3 rounded-xl flex items-center justify-center gap-2 shadow-lg shadow-amber-900/20 transition-all active:scale-95"
                                >
                                    <RefreshCw size={18} />
                                    Update & Install
                                </button>
                            </div>
                        </div>
                    ) : status === 'idle' ? (
                        <div className="p-8 flex flex-col items-center justify-center space-y-6">
                            <div className="text-center space-y-2">
                                <p className="text-app-fg font-bold text-lg">
                                    Authentication Required
                                </p>
                                <p className="text-app-muted text-sm max-w-sm">
                                    Installing system-wide applications requires administrative privileges.
                                </p>
                            </div>

                            <div className="w-full max-w-sm space-y-3">
                                {/* Informational Block for Polkit */}
                                <div className="bg-blue-500/10 border border-blue-500/20 p-5 rounded-2xl flex gap-4 items-start">
                                    <ShieldCheck className="text-blue-500 shrink-0 mt-1" size={24} />
                                    <div>
                                        <h4 className="font-bold text-blue-500 mb-1 text-sm">One-Click Install Ready</h4>
                                        <p className="text-xs text-app-muted">
                                            If authorized, this will proceed instantly. Otherwise, the system will prompt you for a single secure authorization.
                                        </p>
                                    </div>
                                </div>
                            </div>

                            <div className="w-full max-w-sm flex gap-3">
                                <button
                                    onClick={onClose}
                                    className="flex-1 bg-app-fg/5 hover:bg-app-fg/10 text-app-fg font-medium py-3 rounded-xl transition-colors"
                                >
                                    Cancel
                                </button>
                                <button
                                    onClick={handleAction}
                                    className={clsx(
                                        "flex-[2] text-white font-bold py-3 rounded-xl flex items-center justify-center gap-2 shadow-lg transition-all active:scale-95",
                                        mode === 'uninstall' ? "bg-red-600 hover:bg-red-500 shadow-red-900/20" : "bg-blue-600 hover:bg-blue-500 shadow-blue-900/20"
                                    )}
                                >
                                    {mode === 'uninstall' ? <Trash2 size={18} /> : <Play size={18} fill="currentColor" />}
                                    {mode === 'uninstall' ? 'Confirm Uninstall' : 'Authorize & Install'}
                                </button>
                            </div>
                        </div>
                    ) : (
                        <div className="flex-1 flex flex-col h-full bg-app-bg transition-colors">
                            {status === 'success' ? (
                                <div className="px-5 py-5 flex flex-col items-center justify-center space-y-4 animate-in zoom-in-95 duration-500">
                                    <div className="w-14 h-14 bg-green-500/20 rounded-full flex items-center justify-center shadow-lg shadow-green-500/10">
                                        <CheckCircle2 size={28} className="text-green-500" />
                                    </div>
                                    <div className="text-center space-y-1">
                                        <h3 className="text-xl font-bold text-app-fg">Success!</h3>
                                        <p className="text-app-muted text-sm max-w-xs">
                                            {pkg.name} has been successfully {mode === 'uninstall' ? 'removed' : 'installed'}.
                                        </p>
                                    </div>

                                    {mode !== 'uninstall' && (
                                        <div className="bg-blue-500/5 border border-blue-500/10 px-4 py-3 rounded-xl flex gap-3 items-center max-w-xs animate-in slide-in-from-bottom-2 delay-300">
                                            <div className="p-2 bg-blue-500/10 rounded-lg text-blue-500 shrink-0">
                                                <Play size={16} fill="currentColor" />
                                            </div>
                                            <div className="min-w-0">
                                                <h4 className="font-bold text-app-fg text-xs">Where is it?</h4>
                                                <p className="text-[11px] text-app-muted leading-snug">
                                                    The app is now in your <b>Application Launcher</b>.
                                                </p>
                                            </div>
                                        </div>
                                    )}

                                    <div className="w-full max-w-xs pt-2">
                                        <div className="flex gap-2 w-full">
                                            {mode !== 'uninstall' && (
                                                <button
                                                    onClick={() => {
                                                        invoke('launch_app', { pkgName: pkg.name }).catch((e) => errorService.reportError(e as Error | string));
                                                        onClose();
                                                    }}
                                                    className="flex-1 bg-green-500 hover:bg-green-600 text-white font-bold py-3 rounded-xl shadow-lg shadow-green-500/20 active:scale-95 transition-all flex items-center justify-center gap-1.5 text-sm"
                                                >
                                                    <Play size={18} fill="currentColor" /> Launch Now
                                                </button>
                                            )}
                                            <button
                                                onClick={onClose}
                                                className={clsx(
                                                    "font-bold py-3 rounded-xl transition-all active:scale-95 flex items-center justify-center gap-1.5 text-sm",
                                                    mode === 'uninstall'
                                                        ? "flex-1 bg-app-fg text-app-bg hover:brightness-110 shadow-lg"
                                                        : "px-5 text-app-muted hover:text-app-fg hover:bg-app-subtle"
                                                )}
                                            >
                                                {mode === 'uninstall' ? 'Done' : 'Close'}
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            ) : (
                                <>
                                    {/* Progress Bar Area */}
                                    <div className="bg-app-card px-5 py-4 border-b border-app-border">
                                        {/* ... existing logic for keyrings/locks ... */}
                                        {hasKeyringError && status === 'error' && !repairSuccess && !autoRetryAttempted && (
                                            <div className="mb-4 p-3 bg-amber-500/10 border border-amber-500/20 rounded-xl flex items-center justify-between animate-in slide-in-from-top-2">
                                                <div className="flex items-center gap-3">
                                                    <div className="p-2 bg-amber-500/20 rounded-lg text-amber-500">
                                                        <ShieldCheck size={18} />
                                                    </div>
                                                    <div>
                                                        <h4 className="font-bold text-amber-500 text-sm">Keyring Issue Detected</h4>
                                                        <p className="text-xs text-app-muted">Your system keys seem outdated or corrupted.</p>
                                                    </div>
                                                </div>
                                                <button
                                                    onClick={handleRepair}
                                                    disabled={isRepairing}
                                                    className="px-4 py-2 bg-amber-500 hover:bg-amber-600 text-white text-xs font-bold rounded-lg transition-colors flex items-center gap-2 shadow-lg shadow-amber-500/20"
                                                >
                                                    {isRepairing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}
                                                    {isRepairing ? "Fixing..." : "Fix & Retry"}
                                                </button>
                                            </div>
                                        )}

                                        {hasLockError && status === 'error' && !autoRetryAttempted && (
                                            <div className="mb-4 p-3 bg-red-500/10 border border-red-500/20 rounded-xl flex items-center justify-between animate-in slide-in-from-top-2">
                                                <div className="flex items-center gap-3">
                                                    <div className="p-2 bg-red-500/20 rounded-lg text-red-500">
                                                        <ShieldCheck size={18} />
                                                    </div>
                                                    <div>
                                                        <h4 className="font-bold text-red-500 text-sm">Database Locked</h4>
                                                        <p className="text-xs text-app-muted">Another process might be using pacman.</p>
                                                    </div>
                                                </div>
                                                <button
                                                    onClick={handleUnlock}
                                                    disabled={isRepairing}
                                                    className="px-4 py-2 bg-red-500 hover:bg-red-600 text-white text-xs font-bold rounded-lg transition-colors flex items-center gap-2 shadow-lg shadow-red-500/20"
                                                >
                                                    {isRepairing ? <Loader2 size={14} className="animate-spin" /> : <ShieldCheck size={14} />}
                                                    {isRepairing ? "Unlocking..." : "Unlock & Retry"}
                                                </button>
                                            </div>
                                        )}

                                        {autoRetryAttempted && isRepairing && (
                                            <div className="mb-4 p-3 bg-blue-500/10 border border-blue-500/20 rounded-xl flex items-center gap-3 animate-in slide-in-from-top-2">
                                                <Loader2 size={18} className="text-blue-500 animate-spin" />
                                                <div>
                                                    <h4 className="font-bold text-blue-500 text-sm">Auto-Healing System</h4>
                                                    <p className="text-xs text-app-muted">Resolving technical issues automatically...</p>
                                                </div>
                                            </div>
                                        )}

                                        <div className="flex justify-between text-sm text-app-muted mb-1">
                                            <span>Status: {status === 'running' ? 'Working...' : status.toUpperCase()}</span>
                                            <span>{Math.round(visualProgress)}%</span>
                                        </div>
                                        {pkg.source.source_type === 'aur' && status === 'running' && (detailedStatus.includes('Building') || detailedStatus.includes('Compiling') || detailedStatus.includes('Cloning') || detailedStatus.includes('Downloading Source') || (visualProgress >= 25 && visualProgress <= 85)) && (
                                            <>
                                                <div className="text-xs text-blue-400 font-bold animate-pulse mb-1">Building from source…</div>
                                                <div className="text-[10px] text-app-muted mb-2">Large packages can take several minutes. You can cancel to skip the rest.</div>
                                            </>
                                        )}
                                        <div className="w-full bg-app-fg/10 h-2 rounded-full overflow-hidden">
                                            {/* Progress Steps for AUR */}
                                            {pkg.source.source_type === 'aur' && status === 'running' && (
                                                <div className="flex justify-between text-[10px] text-app-muted mt-1 px-1">
                                                    <span className={clsx(visualProgress > 10 && "text-blue-500 font-bold")}>Download</span>
                                                    <span className={clsx(visualProgress > 30 && "text-blue-500 font-bold")}>Prepare</span>
                                                    <span className={clsx(visualProgress > 50 && "text-blue-500 font-bold")}>Build</span>
                                                    <span className={clsx(visualProgress > 90 && "text-blue-500 font-bold")}>Install</span>
                                                </div>
                                            )}
                                            <div
                                                className={clsx("h-full transition-all duration-150",
                                                    (status as any) === 'success' ? "bg-green-500" :
                                                        status === 'error' ? "bg-red-500" : "bg-blue-500 relative"
                                                )}
                                                style={{ width: `${visualProgress}%` }}
                                            >
                                                {status === 'running' && <div className="absolute inset-0 bg-white/20 animate-pulse" />}
                                            </div>
                                        </div>
                                    </div>

                                    {/* Logs: compact by default; expand to view full log */}
                                    <div className="flex justify-center mt-3 px-4">
                                        <button
                                            onClick={() => setShowLogs(!showLogs)}
                                            className="text-xs text-app-muted hover:text-app-fg flex items-center gap-1.5 transition-colors py-2 px-3 rounded-lg hover:bg-app-fg/5"
                                            aria-expanded={showLogs}
                                        >
                                            {showLogs ? <><ChevronUp size={14} /> Hide Logs</> : <><Terminal size={14} /> Show Details</>}
                                        </button>
                                    </div>

                                    {/* Logs Terminal — only when user expands; scrollable so you can copy full log */}
                                    {showLogs && (
                                        <div className="flex flex-col min-h-[200px] max-h-[min(50vh,400px)] mt-2 mx-4 mb-4 rounded-lg border border-white/10 bg-black/20 overflow-hidden min-w-0 shrink-0">
                                            <div className="flex items-center justify-between px-3 py-2 border-b border-white/10 bg-app-fg/5 shrink-0">
                                                <span className="text-xs font-medium text-app-muted">Transaction log</span>
                                                <button
                                                    type="button"
                                                    onClick={() => setShowLogs(false)}
                                                    className="text-app-muted hover:text-app-fg p-1.5 rounded hover:bg-app-fg/10 transition-colors"
                                                    aria-label="Hide log"
                                                >
                                                    <ChevronUp size={16} />
                                                </button>
                                            </div>
                                            <div className="flex-1 min-h-0 overflow-y-auto overflow-x-auto p-4 font-mono text-xs text-app-muted space-y-1 overscroll-contain">
                                                {commandPreview && (
                                                    <div className="mb-2 pb-2 border-b border-white/10 text-blue-400 font-bold">
                                                        {commandPreview}
                                                    </div>
                                                )}
                                                {logs.map((log, i) => (
                                                    <div key={i} className="break-all whitespace-pre-wrap">
                                                        <span className="text-app-muted opacity-50 mr-2">[{new Date().toLocaleTimeString()}]</span>
                                                        {log}
                                                    </div>
                                                ))}
                                                <div ref={logsEndRef} />
                                            </div>
                                        </div>
                                    )}
                                </>
                            )}
                        </div>
                    )}
                </div>

                {/* Footer Actions - Enhanced with Smart Recovery */}
                {(status === 'error' && !isRepairing && !updateRequired) && (
                    <div className="p-4 bg-app-fg/5 border-t border-app-border">
                        {/* Smart Recovery Card when we have a classified error */}
                        {classifiedError && (
                            <div className="mb-4 p-4 bg-app-card rounded-xl border border-app-border">
                                <div className="flex items-start gap-3 mb-3">
                                    <div className="p-2 bg-red-500/10 rounded-lg text-red-500">
                                        <XCircle size={20} />
                                    </div>
                                    <div className="flex-1">
                                        <h4 className="font-bold text-app-fg">{classifiedError.title}</h4>
                                        <p className="text-sm text-app-muted mt-1">{classifiedError.description}</p>
                                    </div>
                                </div>

                                {/* One-Click Recovery Button */}
                                {classifiedError.kind && (
                                    <div className="flex gap-2">
                                        {(() => {
                                            const config = getRecoveryConfig(classifiedError.kind);
                                            const RecoveryIcon = config.icon;
                                            // Use recovery_action (e.g. "UnlockDatabase") when backend sends it; else kind for retry
                                            const action = typeof classifiedError.recovery_action === 'string'
                                                ? classifiedError.recovery_action
                                                : classifiedError.kind;
                                            return (
                                                <button
                                                    onClick={() => handleRecoveryAction(action)}
                                                    disabled={isRecovering}
                                                    className={clsx(
                                                        "flex-1 text-white font-bold py-3 rounded-xl flex items-center justify-center gap-2 shadow-lg transition-all active:scale-95",
                                                        config.color,
                                                        isRecovering && "opacity-50 cursor-not-allowed"
                                                    )}
                                                >
                                                    {isRecovering ? (
                                                        <Loader2 size={18} className="animate-spin" />
                                                    ) : (
                                                        <RecoveryIcon size={18} />
                                                    )}
                                                    {isRecovering ? 'Recovering...' : config.label}
                                                </button>
                                            );
                                        })()}
                                        <button
                                            onClick={onClose}
                                            disabled={isRecovering}
                                            className="px-6 py-3 bg-app-fg/10 hover:bg-app-fg/20 text-app-fg rounded-xl font-medium transition-colors"
                                        >
                                            Cancel
                                        </button>
                                    </div>
                                )}
                            </div>
                        )}

                        {/* Fallback buttons when no classified error */}
                        {!classifiedError && (
                            <div className="flex justify-end gap-3">
                                <button
                                    onClick={handleAction}
                                    className="bg-app-accent hover:bg-app-accent/80 text-white px-6 py-2 rounded-lg font-medium transition-colors shadow-lg shadow-app-accent/20"
                                >
                                    Retry
                                </button>
                                <button
                                    onClick={onClose}
                                    className="bg-app-fg/10 hover:bg-app-fg/20 text-app-fg px-6 py-2 rounded-lg font-medium transition-colors"
                                >
                                    Close
                                </button>
                            </div>
                        )}
                    </div>
                )}
            </div>
        </div>
    );
}
