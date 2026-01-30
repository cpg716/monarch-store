import { useState, useEffect } from 'react';
import { RefreshCw, ArrowRight, CheckCircle2, Download, AlertCircle, Unlock, Loader2 } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import ConfirmationModal from '../components/ConfirmationModal';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useAppStore } from '../store/internal_store';
import { useErrorService } from '../context/ErrorContext';

interface PendingUpdate {
    name: string;
    old_version: string;
    new_version: string;
    repo: string;
}


// Helper component for Icon
import archLogo from '../assets/arch-logo.png';

const AppIcon = ({ pkgId }: { pkgId: string }) => {
    const [icon, setIcon] = useState<string | null>(null);

    useEffect(() => {
        if (!pkgId) return;
        invoke<string | null>('get_package_icon', { pkgName: pkgId })
            .then(localIcon => {
                if (localIcon) {
                    setIcon(localIcon);
                } else {
                    invoke<any>('get_metadata', { pkgName: pkgId, upstreamUrl: null })
                        .then(meta => {
                            if (meta && meta.icon_url) setIcon(meta.icon_url);
                        })
                        .catch(() => { });
                }
            })
            .catch(() => { });
    }, [pkgId]);

    const displayIcon = icon || archLogo;

    return <img src={displayIcon} alt={pkgId} className={clsx("w-full h-full object-contain", !icon && "opacity-50 grayscale")} />;
};

export default function UpdatesPage() {
    const errorService = useErrorService();
    const {
        isUpdating,
        updateProgress: progress,
        updateStatus: statusMessage,
        updateLogs: logs,
        rebootRequired,
        pacnewWarnings,
        setUpdating,
        clearUpdateLogs
    } = useAppStore();

    const [updates, setUpdates] = useState<PendingUpdate[]>([]);
    const [isChecking, setIsChecking] = useState(true);
    const [updateResult, setUpdateResult] = useState<string | null>(null);
    const [showConsole, setShowConsole] = useState(false);
    const [password, setPassword] = useState('');
    const [currentStep, setCurrentStep] = useState(0);
    const [fixingLock, setFixingLock] = useState(false);
    const [showAuthHint, setShowAuthHint] = useState(false);

    const isLockOrBusyError = updateResult != null && /lock|busy|database.*(locked|busy)/i.test(updateResult);

    // If update is "stuck" on auth/connectivity for 12s, show hint (password dialog may be hidden).
    useEffect(() => {
        if (!isUpdating) {
            setShowAuthHint(false);
            return;
        }
        const t = window.setTimeout(() => {
            setShowAuthHint(true);
        }, 12000);
        return () => window.clearTimeout(t);
    }, [isUpdating]);

    const steps = [
        "Synchronizing Databases",
        "Upgrading System",
        "Updating Community Apps"
    ];

    // Fetch updates on mount
    useEffect(() => {
        checkForUpdates();
    }, []);

    useEffect(() => {
        if (statusMessage?.toLowerCase().includes("database") || statusMessage?.toLowerCase().includes("sync")) {
            setCurrentStep(0);
        } else if (statusMessage?.toLowerCase().includes("upgrade") || statusMessage?.toLowerCase().includes("installing core")) {
            setCurrentStep(1);
        } else if (statusMessage?.toLowerCase().includes("aur") || statusMessage?.toLowerCase().includes("community")) {
            setCurrentStep(2);
        }
    }, [statusMessage]);

    const checkForUpdates = async () => {
        setIsChecking(true);
        setUpdateResult(null);
        try {
            const pendingUpdates = await invoke<PendingUpdate[]>('check_for_updates');
            setUpdates(pendingUpdates);
        } catch (e) {
            errorService.reportError(e as Error | string);
        } finally {
            setIsChecking(false);
        }
    };

    const [showConfirm, setShowConfirm] = useState(false);

    // Listen for update-complete so we don't block the UI waiting for the backend.
    useEffect(() => {
        const unlisten = listen<{ success: boolean; message: string }>('update-complete', (event) => {
            setUpdating(false);
            setUpdateResult(event.payload.message);
            checkForUpdates();
        });
        return () => {
            unlisten.then((fn) => fn()).catch(() => {});
        };
    }, [setUpdating]);

    const handleUpdateAll = () => {
        setShowConfirm(true);
    };

    const performUpdate = () => {
        setShowConfirm(false);
        setUpdating(true);
        setUpdateResult(null);
        clearUpdateLogs();
        setCurrentStep(0);

        // Fire-and-forget: never await so the UI never blocks. Backend returns "started" and runs update in background.
        invoke<string>('perform_system_update', { password: password || null }).catch((e) => {
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
                                    `${updates.length} updates available (${(updates.length * 1.5).toFixed(1)} MB)`}
                        </p>
                    </div>

                    <div className="flex items-center gap-3">
                        <button
                            onClick={checkForUpdates}
                            disabled={isChecking || isUpdating}
                            className="px-6 py-3 rounded-xl bg-black/5 dark:bg-white/5 hover:bg-black/10 dark:hover:bg-white/10 text-slate-900 dark:text-white font-bold text-sm border border-black/10 dark:border-white/10 transition-all disabled:opacity-50 flex items-center gap-2 active:scale-95"
                        >
                            <RefreshCw size={18} className={isChecking ? "animate-spin" : ""} />
                            Check Now
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
                                        await invoke('repair_unlock_pacman', { password: null });
                                        setUpdateResult(null);
                                        await checkForUpdates();
                                    } catch (e) {
                                        setUpdateResult(String(e));
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
                                            onClick={() => invoke('launch_app', { pkgName: 'reboot' })}
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
                    <div className="space-y-3 max-w-5xl mx-auto">
                        {updates.map((pkg) => (
                            <div
                                key={pkg.name}
                                className="bg-white dark:bg-app-card border border-black/5 dark:border-white/5 rounded-2xl p-5 flex items-center justify-between hover:bg-white/80 dark:hover:bg-white/5 transition-all group hover:scale-[1.01] hover:shadow-xl hover:border-black/10 dark:hover:border-white/10"
                            >
                                <div className="flex items-center gap-6">
                                    <div className="w-14 h-14 rounded-xl bg-slate-50 dark:bg-black/20 flex items-center justify-center shrink-0 overflow-hidden relative p-2 border border-black/5 dark:border-white/5 shadow-inner">
                                        <AppIcon pkgId={pkg.name} />
                                    </div>
                                    <div>
                                        <h3 className="font-bold flex items-center gap-3 text-xl text-slate-900 dark:text-white mb-1">
                                            {pkg.name}
                                            <span className={clsx(
                                                "text-[10px] px-2 py-0.5 rounded-full uppercase font-black tracking-widest border",
                                                pkg.repo === 'official' ? "bg-teal-100 dark:bg-teal-500/10 text-teal-700 dark:text-teal-400 border-teal-200 dark:border-teal-500/20" :
                                                    pkg.repo === 'chaotic' ? "bg-violet-100 dark:bg-violet-500/10 text-violet-700 dark:text-violet-400 border-violet-200 dark:border-violet-500/20" :
                                                        "bg-amber-100 dark:bg-amber-500/10 text-amber-700 dark:text-amber-400 border-amber-200 dark:border-amber-500/20"
                                            )}>
                                                {pkg.repo}
                                            </span>
                                        </h3>
                                        <div className="flex items-center gap-3 text-sm font-medium">
                                            <span className="text-slate-400 dark:text-app-muted line-through opacity-50">{pkg.old_version}</span>
                                            <ArrowRight size={14} className="text-slate-300 dark:text-white/20" />
                                            <span className="text-emerald-600 dark:text-emerald-400">{pkg.new_version}</span>
                                        </div>
                                    </div>
                                </div>

                                <div className="flex items-center gap-6">
                                    {pkg.repo === 'aur' && (
                                        <div title="AUR Package: May take longer to build" className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-amber-100 dark:bg-amber-500/10 border border-amber-200 dark:border-amber-500/20 text-amber-700 dark:text-amber-500 text-xs font-bold">
                                            <AlertCircle size={14} />
                                            <span>Compassion Needed</span>
                                        </div>
                                    )}
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </div>

            <ConfirmationModal
                isOpen={showConfirm}
                onClose={() => {
                    setShowConfirm(false);
                    setPassword('');
                }}
                onConfirm={performUpdate}
                title="Update System"
                message={updates.some(u => u.repo === 'aur')
                    ? "This update includes AUR packages which require building from source. Please enter your administrator password to proceed."
                    : "This will update all system packages. Are you ready to proceed?"
                }
                confirmLabel="Start Update"
                variant="info"
                showPasswordInput={updates.some(u => u.repo === 'aur')}
                passwordValue={password}
                onPasswordChange={setPassword}
            />
        </div>
    );
}
