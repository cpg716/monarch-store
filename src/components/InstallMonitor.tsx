import { useState, useEffect, useRef } from 'react';
import { Terminal, CheckCircle2, XCircle, Loader2, Play, Minimize2, Maximize2, ShieldCheck, RefreshCw, ChevronUp, Trash2, Download, Package as PackageIcon, Sparkles, Unlock, Key, HardDrive, Wifi, Clock, Shield, GitBranch, Wrench, Box } from 'lucide-react';
import { commands } from '../services/bindings';
import { unwrap } from '../utils/specta';
import { listen } from '@tauri-apps/api/event';
import { clsx } from 'clsx';
import { friendlyError } from '../utils/friendlyError';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { useAppStore } from '../store/internal_store';
import { useSessionPassword } from '../context/useSessionPassword';
import { useErrorService } from '../context/ErrorContext';
import { useToast } from '../context/ToastContext';

import { PackageSource } from '../services/bindings';


interface AlpmProgressEvent {
    event_type: string;
    package?: string;
    percent?: number;
    message: string;
}

interface InstallMonitorProps {
    pkg: { name: string; source: PackageSource; repoName?: string; displayName?: string; } | null;
    onClose: () => void;
    mode?: 'install' | 'uninstall';
    onSuccess?: () => void;
}

/** Human-readable app name for header and success (e.g. "obs-studio" → "Obs Studio"). */
function appDisplayName(pkg: NonNullable<InstallMonitorProps['pkg']>): string {
    if (pkg.displayName?.trim()) return pkg.displayName.trim();
    return pkg.name
        .replace(/[-_]/g, ' ')
        .replace(/\b\w/g, (c) => c.toUpperCase());
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

// §7.3: Store timestamp when line is appended, not at render time
interface LogEntry {
    text: string;
    ts: number;
}
function logEntry(text: string): LogEntry {
    return { text, ts: Date.now() };
}

export default function InstallMonitor({ pkg, onClose, mode = 'install', onSuccess }: InstallMonitorProps) {
    const { requestSessionPassword } = useSessionPassword();
    const errorService = useErrorService();
    const reducePasswordPrompts = useAppStore((s) => s.reducePasswordPrompts);
    const { show: showToast } = useToast();

    const [status, setStatus] = useState<'idle' | 'running' | 'success' | 'error'>('idle');

    const [logs, setLogs] = useState<LogEntry[]>([]);
    const [visualProgress, setVisualProgress] = useState(0);
    const [targetProgress, setTargetProgress] = useState(0);
    const [minimized, setMinimized] = useState(false);
    // Compact by default; user can open "View log" if they want full output
    const [showLogs, setShowLogs] = useState(false);
    const logsEndRef = useRef<HTMLDivElement>(null);
    const actionStartedForRef = useRef<string | null>(null);
    const silentDbRepairAttemptedRef = useRef(false);
    const logsRef = useRef<LogEntry[]>([]);
    const autoUnlockAttemptedRef = useRef(false);
    const [commandPreview, setCommandPreview] = useState<string>('');

    // --- Phase 2 Feature State ---
    // 2.2: Flatpak progress from backend
    const [flatpakProgress, setFlatpakProgress] = useState<number | null>(null);
    // 2.3: Flatpak permissions preview
    const [flatpakPermissions, setFlatpakPermissions] = useState<string[] | null>(null);
    const [showPermissions, setShowPermissions] = useState(false);
    // 2.4: Download size
    const [downloadSize, setDownloadSize] = useState<string | null>(null);
    // 2.5: Elapsed time
    const [elapsedSeconds, setElapsedSeconds] = useState(0);
    const elapsedTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
    // Throttle log updates to prevent freeze when hundreds of progress events arrive
    const logBufferRef = useRef<LogEntry[]>([]);
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
        logBufferRef.current.push(logEntry(message));
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
            useAppStore.getState().verboseLogsEnabled !== true && useAppStore.getState().setVerboseLogsEnabled?.(true);
        }
        logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [logs, minimized, showLogs]);

    const [detailedStatus, setDetailedStatus] = useState<string>('');

    const copyLogsToClipboard = async () => {
        const content = logsRef.current.map((e) => `[${new Date(e.ts).toLocaleTimeString()}] ${e.text}`).join('\n');
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

        const isFlatpak = pkg.source && typeof pkg.source === 'object' && pkg.source.source_type === 'flatpak';

        // ✅ NEW: Listen for structured ALPM progress events
        const unlistenAlpmProgress = listen('alpm-progress', (event: { payload: any }) => {
            if (isFlatpak) return; // Ignore background ALPM logs during Flatpak installs
            const evt = event.payload as AlpmProgressEvent;

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
                case 'install-finalizing':
                    // Prevent hanging progress bar; indicate background housekeeping
                    setProgressStatusThrottled(99, evt.message || 'Finishing up housekeeping...');
                    break;
                case 'progress':
                    if (evt.percent !== undefined) {
                        setProgressStatusThrottled(evt.percent, evt.message);
                    }
                    break;
                case 'hook_start':
                case 'pkg_install_start':
                case 'pkg_remove_start':
                    setProgressStatusThrottled(evt.percent ?? progressStatusRef.current.target, evt.message);
                    break;
                default:
                    setProgressStatusThrottled(progressStatusRef.current.target, evt.message);
            }
        });

        const unlistenOutput = listen('install-output', (event: { payload: unknown }) => {
            if (typeof event.payload !== 'string') return;
            const line = event.payload;

            // Ignore background ALPM sync logs during Flatpak installs
            if (isFlatpak && !line.includes('[Flatpak') && !line.startsWith('Installing ') && !line.startsWith('Uninstalling ')) {
                return;
            }

            // Flatpak emits a new line for every progress frame, leading to thousands of logs.
            const isFlatpakProgressLine = isFlatpak && line.includes('%') && (line.includes('Installing') || line.includes('Updating') || line.includes('Uninstalling'));

            if (!isFlatpakProgressLine) {
                appendLogThrottled(line);
            }

            // Enhanced Progress Heuristics (fallback for non-ALPM operations like AUR builds) — throttled
            if (isFlatpak) {
                // Flatpaks are handled entirely by `flatpak-progress`, do not overwrite the UI status label with raw log text here.
            } else if (line.includes('%')) {
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

            // 2.4: Extract download size from pacman/flatpak output
            const sizeMatch = line.match(/(?:Total Download Size|Download size|Installed Size)[:\s]+(\d+(?:\.\d+)?\s*(?:MiB|KiB|GiB|MB|KB|GB|B))/i);
            if (sizeMatch) {
                setDownloadSize(sizeMatch[1]);
            }

            // 2.3: Extract Flatpak permissions from metadata lines
            if (line.includes('permissions:') || line.includes('--filesystem=') || line.includes('--socket=') || line.includes('--device=')) {
                const permMatch = line.match(/(--\w+=\S+)/g);
                if (permMatch && permMatch.length > 0) {
                    setFlatpakPermissions(prev => {
                        const existing = prev || [];
                        const newPerms = permMatch.filter(p => !existing.includes(p));
                        return newPerms.length > 0 ? [...existing, ...newPerms] : existing;
                    });
                }
            }
        });

        const unlistenRepair = listen('repair-log', (event: { payload: unknown }) => {
            if (typeof event.payload !== 'string') return;
            setLogs((prev) => [...prev, logEntry(event.payload as string)]);
        });

        const unlistenComplete = listen('install-complete', async (event: { payload: string }) => {
            // §7.4: Handle failed_update_required in main listener (single place)
            if (event.payload === 'failed_update_required') {
                setStatus('error');
                setUpdateRequired(true);
                setDetailedStatus('System Update Required');
                setLogs((prev) => [
                    ...prev,
                    logEntry('STOP: Package not found in current database.'),
                    logEntry('This usually means your system is out of date.'),
                ]);
                return;
            }
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
            const hasCorruptDb = currentLogs.some((l) =>
                l.text.includes('Unrecognized archive format') || l.text.includes('could not open database') || l.text.includes('Sync databases are corrupt')
            );
            const hasDbLocked = currentLogs.some((l) =>
                l.text.includes('db.lck') || l.text.includes('Database Locked') || l.text.includes('ALPM_ERR_DB_WRITE') || l.text.includes('unable to lock database') || (l.text.includes('could not remove') && l.text.includes('db.lck'))
            );
            if (event.payload !== 'success' && hasDbLocked && !autoUnlockAttemptedRef.current) {
                autoUnlockAttemptedRef.current = true;
                setDetailedStatus('Waiting for another update...');
                setLogs(prev => [...prev, logEntry('\n--- Auto-unlocking database ---')]);
                try {
                    const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
                    unwrap(await commands.repairUnlockPacman(pwd ?? null));
                    setLogs(prev => [...prev, logEntry('✓ Database unlocked. Retrying...')]);
                    setTargetProgress(5);
                    setStatus('running');
                    actionStartedForRef.current = null;
                    handleAction();
                } catch (e) {
                    setLogs(prev => [...prev, logEntry(`Unlock failed: ${e}`)]);
                    setStatus('error');
                }
                return;
            }
            if (event.payload !== 'success' && hasCorruptDb && !silentDbRepairAttemptedRef.current) {
                silentDbRepairAttemptedRef.current = true;
                setDetailedStatus('Repairing databases...');
                setLogs(prev => [...prev, logEntry('\n--- Self-healing: Refreshing package databases ---')]);
                try {
                    const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
                    unwrap(await commands.forceRefreshDatabases(pwd ?? null));
                    setLogs(prev => [...prev, logEntry('✓ Databases refreshed. Retrying...')]);
                    setTargetProgress(5);
                    setStatus('running');
                    actionStartedForRef.current = null;
                    handleAction();
                } catch (e) {
                    errorService.reportError(e as Error | string);
                    setLogs(prev => [...prev, logEntry(`Repair failed: ${e}`)]);
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

        // 2.2: Listen for real-time Flatpak progress events
        const unlistenFlatpakProgress = listen<number>('flatpak-progress', (event) => {
            const pct = event.payload;
            if (typeof pct === 'number' && pct >= 0 && pct <= 100) {
                setFlatpakProgress(pct);
                setProgressStatusThrottled(
                    Math.floor(10 + (pct * 85) / 100),
                    `Installing Flatpak... ${pct}%`
                );
            }
        });

        return () => {
            unlistenAlpmProgress.then((f: () => void) => f()).catch(() => { });
            unlistenOutput.then((f: () => void) => f()).catch(() => { });
            unlistenRepair.then((f: () => void) => f()).catch(() => { });
            unlistenComplete.then((f: () => void) => f()).catch(() => { });
            unlistenClassifiedError.then((f: () => void) => f()).catch(() => { });
            unlistenFlatpakProgress.then((f: () => void) => f()).catch(() => { });
        };
    }, [pkg, reducePasswordPrompts, requestSessionPassword]);

    // 2.5: Elapsed time counter
    useEffect(() => {
        if (status === 'running') {
            setElapsedSeconds(0);
            elapsedTimerRef.current = setInterval(() => {
                setElapsedSeconds(prev => prev + 1);
            }, 1000);
        } else {
            if (elapsedTimerRef.current) {
                clearInterval(elapsedTimerRef.current);
                elapsedTimerRef.current = null;
            }
        }
        return () => {
            if (elapsedTimerRef.current) clearInterval(elapsedTimerRef.current);
        };
    }, [status]);

    // Recovery action handlers
    const handleRecoveryAction = async (action: string) => {
        setIsRecovering(true);
        setLogs(prev => [...prev, logEntry(`\n--- RECOVERY: ${action.toUpperCase()} ---`)]);

        try {
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            switch (action) {
                case 'UnlockDatabase':
                    setLogs(prev => [...prev, logEntry('Checking for stale lock file...')]);
                    unwrap(await commands.repairUnlockPacman(pwd ?? null));
                    setLogs(prev => [...prev, logEntry('✓ Database unlocked successfully')]);
                    break;

                case 'RepairKeyring':
                    setLogs(prev => [...prev, logEntry('Resetting security keyring...'), logEntry('This may take a moment...')]);
                    unwrap(await commands.fixKeyringIssues(pwd ?? null));
                    setLogs(prev => [...prev, logEntry('✓ Keyring repaired successfully')]);
                    break;

                case 'ForceRefreshDb':
                case 'RefreshMirrors':
                    setLogs(prev => [...prev, logEntry('Forcing database refresh...')]);
                    unwrap(await commands.triggerRepoSync(null));
                    setLogs(prev => [...prev, logEntry('✓ Databases refreshed')]);
                    break;

                case 'CleanCache':
                    setLogs(prev => [...prev, logEntry('Clearing package cache...')]);
                    unwrap(await commands.clearCache());
                    setLogs(prev => [...prev, logEntry('✓ Cache cleared')]);
                    break;

                case 'FlatpakReinstall':
                    if (pkg) {
                        setLogs(prev => [...prev, logEntry('Reinstalling Flatpak app...'), logEntry('Removing corrupted install...')]);
                        try {
                            unwrap(await commands.uninstallPackage(pkg.name, pkg.source as any, pwd ?? null));
                            setLogs(prev => [...prev, logEntry('✓ Old install removed. Reinstalling...')]);
                        } catch {
                            setLogs(prev => [...prev, logEntry('Note: Could not remove old install, attempting fresh install...')]);
                        }
                    }
                    break;

                default:
                    setLogs(prev => [...prev, logEntry('Preparing to retry...')]);
            }

            // Reset state and retry the operation
            setLogs(prev => [...prev, logEntry('\n--- RETRYING OPERATION ---')]);
            setClassifiedError(null);
            setStatus('running');
            setTargetProgress(5);

            // Retry the original action
            await handleAction();

        } catch (e) {
            setLogs(prev => [...prev, logEntry(`Recovery failed: ${e}`)]);
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
            case 'CorruptedPackage':
                return { icon: RefreshCw, label: 'Reinstall & Retry', color: 'bg-orange-500 hover:bg-orange-600' };
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
        setLogs([logEntry(`Starting ${mode === 'uninstall' ? 'uninstallation' : 'installation'} engine...`), logEntry(`Target: ${pkg.name} (${pkg.source.label || pkg.source.id})`)]);
        setTargetProgress(5);
        setVisualProgress(0);

        // §7.2: Set command preview before invoke so user sees what will run immediately
        if (mode === 'uninstall') {
            if (pkg.source.source_type === 'flatpak') {
                setCommandPreview(`$ flatpak uninstall ${pkg.name} -y`);
            } else {
                setCommandPreview(`$ pacman -Rns --noconfirm ${pkg.name}`);
            }
        } else {
            if (pkg.source.source_type === 'aur') {
                setCommandPreview(`$ git clone https://aur.archlinux.org/${pkg.name}.git && makepkg -si`);
            } else if (pkg.source.source_type === 'flatpak') {
                setCommandPreview(`$ flatpak install flathub ${pkg.name} -y`);
            } else {
                setCommandPreview(`$ pacman -S --noconfirm ${pkg.name}`);
            }
        }

        try {
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            if (mode === 'uninstall') {
                unwrap(await commands.uninstallPackage(pkg.name, pkg.source as any, pwd ?? null));
            } else {
                unwrap(await commands.installPackage(pkg.name, pkg.source as any, pwd ?? null, pkg.repoName || null));
            }
            // The command is async spawned, completion comes via event
        } catch (e) {
            errorService.reportError(e as Error | string);
            setLogs((prev) => [...prev, logEntry(`Error launching: ${e}`)]);
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
        if (status === 'error' && logs.some((l) => l.text.includes('SystemUpdateRequired'))) {
            // handled by main install-complete listener (failed_update_required)
        }
    }, [status, logs]);


    // Retry after Repair (must be registered unconditionally; handleAction is defined earlier in this component)
    useEffect(() => {
        if (repairSuccess && autoRetryAttempted && status !== 'running' && status !== 'success') {
            setLogs(prev => [...prev, logEntry('✓ System repaired. Retrying operation automatically...')]);
            handleAction();
        }
    }, [repairSuccess, autoRetryAttempted]);

    if (!pkg) return null;

    const errorDetails = status === 'error' && logs.length > 0 ? friendlyError(logs[logs.length - 1].text) : null;

    // 2.1: SOURCE-ADAPTIVE STEPPER LOGIC
    const sourceType = pkg.source.source_type;
    const steps = sourceType === 'aur'
        ? [
            { id: 1, label: 'Clone', icon: GitBranch },
            { id: 2, label: 'Build', icon: Wrench },
            { id: 3, label: 'Install', icon: PackageIcon },
            { id: 4, label: 'Done', icon: Sparkles }
        ]
        : sourceType === 'flatpak'
            ? [
                { id: 1, label: 'Fetch', icon: ShieldCheck },
                { id: 2, label: 'Download', icon: Download },
                { id: 3, label: 'Install', icon: Box },
                { id: 4, label: 'Done', icon: Sparkles }
            ]
            : [
                { id: 1, label: 'Resolve', icon: ShieldCheck },
                { id: 2, label: 'Download', icon: Download },
                { id: 3, label: 'Install', icon: PackageIcon },
                { id: 4, label: 'Done', icon: Sparkles }
            ];

    const currentStep = (() => {
        if (status === 'success') return 4;

        let step = 1;
        if (sourceType === 'aur') {
            if (detailedStatus.includes('Cloning') || detailedStatus.includes('Source')) step = 1;
            else if (detailedStatus.includes('Building') || detailedStatus.includes('Compiling') || detailedStatus.includes('makepkg')) step = 2;
            else if (detailedStatus.includes('Installing') || detailedStatus.includes('Copying')) step = 3;
        } else if (sourceType === 'flatpak') {
            if (detailedStatus.includes('Fetch') || detailedStatus.includes('Resolving') || detailedStatus.includes('Safety')) step = 1;
            else if (detailedStatus.includes('Downloading') || detailedStatus.includes('Flatpak')) step = 2;
            else if (detailedStatus.includes('Installing') || detailedStatus.includes('Deploying')) step = 3;
        } else {
            if (detailedStatus.includes('Safety') || detailedStatus.includes('Resolving') || detailedStatus.includes('Lock') || detailedStatus.includes('Syncing')) step = 1;
            else if (detailedStatus.includes('Downloading')) step = 2;
            else if (detailedStatus.includes('Installing') || detailedStatus.includes('Extracting')) step = 3;
        }

        // Fallback progress tracking if exact keywords don't match
        if (step === 1 && visualProgress > 0 && visualProgress < 90) return 2;
        if (visualProgress >= 90 && status === 'running') return 3;

        return step;
    })();

    const displayStatus = status === 'error' && errorDetails
        ? errorDetails.title
        : status === 'idle' ? `Ready to ${mode === 'uninstall' ? 'Uninstall' : 'Install'}`
            : status === 'success' ? `${mode === 'uninstall' ? 'Uninstallation' : 'Installation'} Complete`
                : detailedStatus || (pkg.source.source_type === 'aur' ? 'Building App (This may take a while)...' : `${mode === 'uninstall' ? 'Uninstalling' : 'Installing'}...`);

    // RENDER STEPPER - clear progress for users, matches app semantic colors
    const renderStepper = () => (
        <div className="flex items-center justify-between px-5 py-4 bg-app-bg/40 border-b border-app-border">
            {steps.map((step, idx) => {
                const isActive = currentStep === step.id;
                const isCompleted = currentStep > step.id || status === 'success';

                return (
                    <div key={step.id} className="flex flex-col items-center gap-2 relative z-10 w-20">
                        <div className={clsx(
                            'w-9 h-9 rounded-full flex items-center justify-center transition-all duration-300',
                            isCompleted ? 'bg-green-500/90 text-white' :
                                isActive ? 'bg-app-accent text-white shadow-lg shadow-app-accent/25' :
                                    'bg-app-fg/10 text-app-muted'
                        )}>
                            {isCompleted ? <CheckCircle2 size={18} /> : <step.icon size={16} />}
                        </div>
                        <span className={clsx(
                            'text-[10px] font-semibold uppercase tracking-wider transition-colors duration-300',
                            (isActive || isCompleted) ? 'text-app-fg' : 'text-app-muted/60'
                        )}>
                            {step.label}
                        </span>

                        {idx < steps.length - 1 && (
                            <div className="absolute top-[18px] left-[50%] w-[calc(100%+2rem)] h-0.5 bg-app-border -z-10">
                                <div
                                    className="h-full bg-green-500/80 transition-all duration-500 rounded-full"
                                    style={{ width: isCompleted ? '100%' : '0%' }}
                                />
                            </div>
                        )}
                    </div>
                );
            })}
        </div>
    );

    // 2.3: Flatpak Permissions Preview Panel
    const renderFlatpakPermissions = () => {
        if (sourceType !== 'flatpak' || mode === 'uninstall' || !flatpakPermissions || flatpakPermissions.length === 0) return null;

        const friendlyPerm = (perm: string) => {
            if (perm.includes('filesystem=home')) return '📁 Home Directory Access';
            if (perm.includes('filesystem=host')) return '📁 Full Filesystem Access';
            if (perm.includes('filesystem=')) return `📁 ${perm.replace('--filesystem=', '')}`;
            if (perm.includes('socket=x11')) return '🖥️ X11 Display';
            if (perm.includes('socket=wayland')) return '🖥️ Wayland Display';
            if (perm.includes('socket=pulseaudio')) return '🔊 Audio Access';
            if (perm.includes('socket=')) return `🔌 ${perm.replace('--socket=', '')}`;
            if (perm.includes('device=dri')) return '🎮 GPU Access';
            if (perm.includes('device=all')) return '⚠️ All Devices';
            if (perm.includes('device=')) return `🔧 ${perm.replace('--device=', '')}`;
            if (perm.includes('share=network')) return '🌐 Network Access';
            if (perm.includes('share=ipc')) return '🔗 IPC Access';
            return perm;
        };

        return (
            <div className="px-5 py-3 bg-amber-500/5 border-b border-amber-500/20">
                <button
                    onClick={() => setShowPermissions(!showPermissions)}
                    className="flex items-center gap-2 text-xs font-semibold text-amber-400 hover:text-amber-300 transition-colors w-full"
                >
                    <Shield size={13} />
                    <span>Flatpak Permissions ({flatpakPermissions.length})</span>
                    <ChevronUp size={12} className={clsx('ml-auto transition-transform', !showPermissions && 'rotate-180')} />
                </button>
                {showPermissions && (
                    <div className="mt-2 grid grid-cols-2 gap-1">
                        {flatpakPermissions.map((perm, i) => (
                            <span key={i} className="text-[10px] text-app-muted/80 font-mono bg-app-bg/50 rounded px-2 py-0.5 truncate">
                                {friendlyPerm(perm)}
                            </span>
                        ))}
                    </div>
                )}
            </div>
        );
    };

    if (minimized) {
        return (
            <div className="fixed bottom-4 right-4 z-50 bg-app-card/95 backdrop-blur-xl border border-app-border p-4 rounded-2xl shadow-2xl flex items-center gap-4 w-[22rem] animate-in slide-in-from-bottom-4 transition-all">
                <div className="bg-app-accent/20 p-2.5 rounded-xl text-app-accent flex-shrink-0">
                    <Loader2 size={20} className="animate-spin" />
                </div>
                <div className="flex-1 min-w-0">
                    <div className="text-sm font-semibold text-app-fg leading-tight break-words">
                        {detailedStatus || (mode === 'uninstall' ? `Uninstalling ${appDisplayName(pkg)}` : `Installing ${appDisplayName(pkg)}`)}
                    </div>
                    <div className="w-full bg-app-fg/10 h-1.5 mt-2 rounded-full overflow-hidden">
                        <div className="h-full bg-app-accent transition-all duration-300 rounded-full" style={{ width: `${visualProgress}%` }} />
                    </div>
                </div>
                <button onClick={() => setMinimized(false)} className="p-2 hover:bg-app-hover rounded-xl text-app-muted transition-colors" aria-label="Expand install window">
                    <Maximize2 size={16} />
                </button>
            </div>
        );
    }

    // Heuristic Scan for Keyring Issues
    const hasKeyringError = logs.some((l) =>
        l.text.includes('GPGME error') ||
        l.text.includes('PGP signature') ||
        l.text.includes('corrupted database') ||
        l.text.includes('invalid or corrupted')
    );

    const hasLockError = logs.some((l) => l.text.includes('database is locked'));

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
            unwrap(await commands.repairUnlockPacman(pwd ?? null));
            setLogs(prev => [...prev, logEntry('✓ Database unlocked.'), logEntry('Please try installing again.')]);
            setRepairSuccess(true);
        } catch (e) {
            setLogs(prev => [...prev, logEntry(`Unlock Failed: ${e}`)]);
        } finally {
            setIsRepairing(false);
        }
    };

    const handleRepair = async () => {
        setIsRepairing(true);
        setAutoRetryAttempted(true); // Enable auto-retry after fix
        setLogs(prev => [...prev, logEntry('\n--- AUTO-HEALING: FIXING KEYRING ISSUES ---'), logEntry('The app detected a security key error.'), logEntry('Attempting to automatically repair trust database...'), logEntry('This will take a moment...')]);
        try {
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            unwrap(await commands.repairResetKeyring(pwd ?? null));
            setLogs(prev => [...prev, logEntry('✓ Keyring reset successfully.'), logEntry('--- REPAIR COMPLETE ---')]);
            setRepairSuccess(true);
        } catch (e) {
            errorService.reportError(e as Error | string);
            setLogs(prev => [...prev, logEntry(`Repair Failed: ${e}`)]);
        } finally {
            setIsRepairing(false);
        }
    };

    const handleUpdateAndInstall = async () => {
        if (!pkg) return;
        setUpdateRequired(false);
        setStatus('running');
        setDetailedStatus('Updating System & Installing...');
        setLogs([
            logEntry('\n--- STARTING SYSTEM UPDATE ---'),
            logEntry('Syncing databases...'),
            logEntry('Performing full system upgrade (-Syu)...'),
            logEntry('This may take a while. Do not turn off your computer.'),
        ]);

        try {
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            unwrap(await commands.updateAndInstallPackage(pkg.name, pkg.repoName || null, pwd ?? null));
            // Completion handled by event listener above
        } catch (e) {
            setLogs(prev => [...prev, logEntry(`Update Failed: ${e}`)]);
            setStatus('error');
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 bg-app-bg/70 backdrop-blur-md animate-in fade-in duration-200">
            <div ref={focusTrapRef} className={clsx("w-full bg-app-card/95 backdrop-blur-xl border border-app-border rounded-2xl shadow-2xl overflow-hidden flex flex-col transition-all duration-200", showLogs ? "max-w-2xl max-h-[85vh]" : "max-w-md max-h-[min(88vh,580px)] min-h-[320px]")} role="dialog" aria-modal="true" aria-labelledby="install-monitor-title">
                {/* Header - matches app card/sidebar styling */}
                <div className="px-5 py-4 border-b border-app-border flex items-center justify-between bg-app-subtle/80">
                    <div className="flex items-center gap-3">
                        <div className={clsx('w-11 h-11 rounded-xl flex items-center justify-center transition-colors',
                            status === 'success' ? 'bg-green-500/20 text-green-500' :
                                (status === 'error' || updateRequired) ? 'bg-red-500/20 text-red-500' :
                                    'bg-app-accent/20 text-app-accent'
                        )}>
                            {status === 'success' ? <CheckCircle2 size={22} /> :
                                (status === 'error' || updateRequired) ? <XCircle size={22} /> :
                                    <Terminal size={22} />}
                        </div>
                        <div className="min-w-0 flex-1">
                            <h2 id="install-monitor-title" className="text-lg font-bold text-app-fg leading-tight break-words pr-4">
                                {updateRequired
                                    ? 'System Update Required'
                                    : status === 'idle'
                                        ? mode === 'uninstall'
                                            ? `Uninstall ${appDisplayName(pkg)}`
                                            : `Install ${appDisplayName(pkg)}`
                                        : status === 'running'
                                            ? mode === 'uninstall'
                                                ? `Uninstalling ${appDisplayName(pkg)}`
                                                : `Installing ${appDisplayName(pkg)}`
                                            : status === 'success'
                                                ? mode === 'uninstall'
                                                    ? `${appDisplayName(pkg)} removed`
                                                    : `${appDisplayName(pkg)} installed`
                                                : status === 'error'
                                                    ? 'Installation failed'
                                                    : displayStatus}
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
                                <p className="text-app-muted text-sm flex flex-wrap items-center gap-x-3 gap-y-1 mt-1">
                                    <span className="font-medium text-app-fg/80">{pkg.source.label} · {appDisplayName(pkg)}</span>
                                    {status === 'running' && (
                                        <span className="inline-flex items-center gap-1 text-app-accent/80 font-mono text-xs shrink-0">
                                            <Clock size={11} />
                                            {Math.floor(elapsedSeconds / 60).toString().padStart(2, '0')}:{(elapsedSeconds % 60).toString().padStart(2, '0')}
                                        </span>
                                    )}
                                    {downloadSize && (
                                        <span className="text-app-muted/70 font-mono text-[10px] shrink-0">({downloadSize})</span>
                                    )}
                                </p>
                            )}
                        </div>
                    </div>
                </div>
                {/* Hide top-right log toggle to deduplicate */}
                <div className="flex items-center gap-2">
                    {status === 'running' && (
                        <>
                            <button
                                onClick={async () => {
                                    try {
                                        unwrap(await commands.abortInstallation());
                                        unwrap(await commands.cancelInstall());
                                        setLogs(prev => [...prev, logEntry('Installation cancelled.')]);
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
                                        unwrap(await commands.abortInstallation());
                                        unwrap(await commands.cancelInstall());
                                        setLogs(prev => [...prev, logEntry('Installation cancelled.')]);
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
                        className="p-2.5 hover:bg-app-hover hover:text-red-500 rounded-xl text-app-muted transition-colors"
                        aria-label="Close"
                    >
                        <XCircle size={20} />
                    </button>
                </div>

                {/* Body: scrollable so success/Done and info are never cut off */}
                <div className="p-0 flex-1 min-h-0 flex flex-col overflow-hidden">
                    {!minimized && sourceType === 'flatpak' && mode !== 'uninstall' && renderFlatpakPermissions()}
                    {!minimized && status !== 'idle' && !updateRequired && renderStepper()}
                    {updateRequired ? (
                        <div className="p-8 flex-1 min-h-0 overflow-y-auto flex flex-col items-center justify-center space-y-6 animate-in slide-in-from-bottom-4">
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
                        <div className="p-8 flex-1 min-h-0 overflow-y-auto flex flex-col items-center justify-center space-y-6">
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
                        <div className="flex-1 min-h-0 flex flex-col overflow-y-auto bg-app-bg transition-colors">
                            {status === 'success' ? (
                                <div className="px-5 py-6 flex flex-col items-center justify-center space-y-5 animate-in zoom-in-95 duration-500 overflow-y-auto w-full">
                                    <div className="w-16 h-16 bg-green-500/20 rounded-full flex items-center justify-center shadow-lg shadow-green-500/10 shrink-0 mt-4">
                                        <CheckCircle2 size={32} className="text-green-500" />
                                    </div>
                                    <div className="text-center space-y-2">
                                        <p className="text-app-muted text-sm">
                                            {mode === 'uninstall' ? 'Successfully removed' : 'Successfully installed'}
                                        </p>
                                        <div className="inline-flex items-center justify-center px-4 py-2 rounded-xl bg-app-fg/10 border border-app-border">
                                            <span className="text-lg font-bold text-app-fg">{appDisplayName(pkg)}</span>
                                        </div>
                                    </div>

                                    {mode !== 'uninstall' && (
                                        <div className="bg-blue-500/5 border border-blue-500/10 px-4 py-3 rounded-xl flex gap-3 items-center w-full max-w-sm animate-in slide-in-from-bottom-2 delay-300">
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

                                    <div className="w-full max-w-sm space-y-3 pt-4 pb-2">
                                        <button
                                            onClick={onClose}
                                            className={clsx(
                                                "w-full font-bold py-3.5 rounded-xl transition-all active:scale-[0.98] flex items-center justify-center gap-2 text-sm shadow-lg",
                                                mode === 'uninstall'
                                                    ? "bg-app-fg text-app-bg hover:brightness-110"
                                                    : "bg-green-500 hover:bg-green-600 text-white shadow-green-500/20"
                                            )}
                                        >
                                            <CheckCircle2 size={18} />
                                            Done
                                        </button>
                                        {mode !== 'uninstall' && (
                                            <button
                                                onClick={() => {
                                                    commands.launchApp({ pkg_name: pkg.name }).then(unwrap).catch((e) => errorService.reportError(e as Error | string));
                                                    onClose();
                                                }}
                                                className="w-full py-3 rounded-xl text-sm font-semibold text-app-fg hover:bg-app-hover border border-app-border transition-colors flex items-center justify-center gap-1.5"
                                            >
                                                <Play size={16} fill="currentColor" />
                                                Launch {appDisplayName(pkg)}
                                            </button>
                                        )}
                                    </div>
                                </div>
                            ) : (
                                <>
                                    {/* Progress Bar Area - clear status for users */}
                                    <div className="bg-app-bg/30 px-5 py-5 border-b border-app-border">
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

                                        <div className="flex justify-between items-start text-sm mb-2 gap-4">
                                            <span className="font-medium text-app-fg leading-tight break-words flex-1">
                                                {displayStatus}
                                            </span>
                                            <span className="text-app-muted tabular-nums shrink-0 font-bold">{Math.round(visualProgress)}%</span>
                                        </div>

                                        {/* NON-TECHNICAL EXPLAINER */}
                                        {status === 'running' && (
                                            <div className="text-xs text-app-muted mb-3 italic">
                                                {(() => {
                                                    const s = displayStatus.toLowerCase();
                                                    if (s.includes('resolving dependencies')) return "Figuring out what extra files this app needs to work properly...";
                                                    if (s.includes('downloading')) return "Fetching the application files securely from the servers...";
                                                    if (s.includes('verifying') || s.includes('integrity') || s.includes('keyring')) return "Checking the digital locks to ensure the download is safe and authentic...";
                                                    if (s.includes('building') || s.includes('compiling') || s.includes('cloning')) return "Building the app specifically for your computer (this can take a few minutes)...";
                                                    if (s.includes('installing') && pkg.source.source_type === 'aur') return "Putting the newly built files in exactly the right places...";
                                                    if (s.includes('installing')) return "Putting the app files in exactly the right places...";
                                                    if (s.includes('removing') || s.includes('uninstalling')) return "Carefully removing the app and its leftover files...";
                                                    if (s.includes('housekeeping')) return "Cleaning up the temporary files we don't need anymore...";
                                                    if (s.includes('preparing transaction')) return "Getting everything ready for the installation...";
                                                    return "Please wait while the system processes your request.";
                                                })()}
                                            </div>
                                        )}

                                        <div className="w-full bg-app-fg/10 h-3 rounded-full overflow-hidden mb-1">
                                            <div
                                                className={clsx('h-full transition-all duration-300 rounded-full',
                                                    status === 'error' ? 'bg-red-500' : 'bg-app-accent relative'
                                                )}
                                                style={{ width: `${visualProgress}%` }}
                                            >
                                                {status === 'running' && <div className="absolute inset-0 bg-white/20 animate-pulse rounded-full" />}
                                            </div>
                                        </div>
                                    </div>

                                    {/* Advanced: expandable transaction log — out of the way by default */}
                                    <div className="flex justify-between items-center px-5 pt-3 pb-2 border-t border-app-border/50">
                                        <span className="text-[10px] text-app-muted uppercase tracking-wider font-semibold">Technical Details</span>
                                        <button
                                            onClick={() => setShowLogs(!showLogs)}
                                            className="text-xs font-semibold text-app-muted hover:text-app-accent flex items-center gap-1.5 transition-colors py-1.5 px-3 rounded-lg hover:bg-app-hover border border-transparent hover:border-app-border"
                                            aria-expanded={showLogs}
                                        >
                                            {showLogs ? 'Hide Raw Output' : 'Show Raw Output'}
                                        </button>
                                        <button
                                            onClick={copyLogsToClipboard}
                                            disabled={logs.length === 0}
                                            className={clsx(
                                                'px-3 py-1.5 rounded-lg text-xs font-semibold border transition-colors',
                                                logs.length === 0
                                                    ? 'text-app-muted border-app-border cursor-not-allowed opacity-60'
                                                    : 'text-app-accent border-app-border hover:bg-app-accent/10 hover:border-app-accent/40'
                                            )}
                                        >
                                            Copy All Logs
                                        </button>
                                    </div>

                                    {showLogs && (
                                        <div className="flex flex-col flex-1 min-h-[150px] mx-5 mb-4 rounded-xl border border-app-border bg-black/40 overflow-hidden shadow-inner">
                                            <div className="flex-1 overflow-y-auto p-4 font-mono text-[10px] text-app-muted space-y-1.5 overscroll-contain">
                                                {commandPreview && (
                                                    <div className="mb-2 pb-2 border-b border-app-border/30 text-app-accent font-semibold flex items-center gap-2">
                                                        <Terminal size={12} /> {commandPreview}
                                                    </div>
                                                )}
                                                {logs.map((log, i) => (
                                                    <div key={i} className="break-all whitespace-pre-wrap leading-relaxed flex gap-2">
                                                        <span className="text-app-muted/50 tabular-nums select-none shrink-0 w-16 opacity-70">[{new Date(log.ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })}]</span>
                                                        <span className="text-app-fg/80">{log.text}</span>
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

                {/* Footer Actions - Smart Recovery */}
                {(status === 'error' && !isRepairing && !updateRequired) && (
                    <div className="p-4 bg-app-subtle/80 border-t border-app-border">
                        {classifiedError && (
                            <div className="mb-4 p-4 bg-app-bg/50 rounded-xl border border-app-border">
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
                                    className="btn-accent px-6 py-2.5 rounded-xl font-semibold shadow-lg shadow-app-accent/20"
                                >
                                    Retry
                                </button>
                                <button
                                    onClick={onClose}
                                    className="bg-app-fg/10 hover:bg-app-hover text-app-fg px-6 py-2.5 rounded-xl font-medium transition-colors"
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
