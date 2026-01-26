import { useState, useEffect } from 'react';
import { RefreshCw, ArrowRight, CheckCircle2, Download, AlertCircle, Loader2 } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import ConfirmationModal from '../components/ConfirmationModal';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface PendingUpdate {
    name: string;
    old_version: string;
    new_version: string;
    repo: string;
}

interface UpdateProgress {
    phase: string;
    progress: number;
    message: string;
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
    const [updates, setUpdates] = useState<PendingUpdate[]>([]);
    const [isUpdating, setIsUpdating] = useState(false);
    const [isChecking, setIsChecking] = useState(true);
    const [progress, setProgress] = useState(0);
    const [statusMessage, setStatusMessage] = useState('');
    const [updateResult, setUpdateResult] = useState<string | null>(null);

    // Listen for update progress events
    useEffect(() => {
        const unlisten = listen<UpdateProgress>('update-progress', (event) => {
            setProgress(event.payload.progress);
            setStatusMessage(event.payload.message);

            if (event.payload.phase === 'complete' || event.payload.phase === 'error') {
                setTimeout(() => {
                    setIsUpdating(false);
                    if (event.payload.phase === 'complete') {
                        setUpdates([]);
                    }
                    setProgress(0);
                }, 1500);
            }
        });

        return () => {
            unlisten.then(fn => fn());
        };
    }, []);

    // Fetch updates on mount
    useEffect(() => {
        checkForUpdates();
    }, []);

    const checkForUpdates = async () => {
        setIsChecking(true);
        setUpdateResult(null);
        try {
            const pendingUpdates = await invoke<PendingUpdate[]>('check_for_updates');
            setUpdates(pendingUpdates);
        } catch (e) {
            console.error('Failed to check for updates:', e);
        } finally {
            setIsChecking(false);
        }
    };

    const [showConfirm, setShowConfirm] = useState(false);

    const handleUpdateAll = () => {
        setShowConfirm(true);
    };

    const performUpdate = async () => {
        setIsUpdating(true);
        setProgress(0);
        setStatusMessage('Initializing update...');
        setUpdateResult(null);

        try {
            const result = await invoke<string>('perform_system_update', { password: null });
            setUpdateResult(result);
            // Refresh updates list after successful update
            await checkForUpdates();
        } catch (e) {
            console.error('Update failed:', e);
            setUpdateResult(`Update failed: ${e}`);
            setIsUpdating(false);
        }
    };



    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            {/* Header */}
            <div className="p-8 pb-6 border-b border-black/5 dark:border-white/5 bg-app-bg/95 backdrop-blur-3xl z-10 transition-colors shadow-sm dark:shadow-2xl dark:shadow-black/20 sticky top-0">
                <div className="flex items-end justify-between">
                    <div>
                        <h1 className="text-4xl lg:text-5xl font-black flex items-center gap-4 text-slate-900 dark:text-white tracking-tight leading-none mb-2">
                            <span className={clsx("p-2 rounded-2xl bg-blue-500/10 text-blue-500", (isUpdating || isChecking) && "animate-spin")}>
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

                {/* Progress Bar */}
                <AnimatePresence>
                    {isUpdating && (
                        <motion.div
                            initial={{ height: 0, opacity: 0 }}
                            animate={{ height: 'auto', opacity: 1 }}
                            exit={{ height: 0, opacity: 0 }}
                            className="mt-8 bg-black/5 dark:bg-black/20 rounded-2xl p-6 border border-black/5 dark:border-white/10"
                        >
                            <div className="flex justify-between text-xs font-bold text-slate-900 dark:text-white mb-2 uppercase tracking-wider">
                                <span>{statusMessage || 'Updating system...'}</span>
                                <span>{Math.round(progress)}%</span>
                            </div>
                            <div className="h-4 bg-black/10 dark:bg-black/40 rounded-full overflow-hidden border border-black/5 dark:border-white/5">
                                <motion.div
                                    className="h-full bg-gradient-to-r from-blue-500 to-purple-500 relative"
                                    initial={{ width: 0 }}
                                    animate={{ width: `${progress}%` }}
                                >
                                    <div className="absolute inset-0 bg-white/20 animate-pulse" />
                                </motion.div>
                            </div>
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto p-8 custom-scrollbar">
                {isChecking ? (
                    <div className="flex flex-col items-center justify-center h-full text-app-muted gap-6">
                        <Loader2 size={48} className="animate-spin text-blue-500" />
                        <p className="text-xl font-medium">Checking repositories...</p>
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
                onClose={() => setShowConfirm(false)}
                onConfirm={performUpdate}
                title="Update System"
                message="This will update all system packages. This process cannot be interrupted. Are you ready to proceed?"
                confirmLabel="Start Update"
                variant="info"
            />
        </div>
    );
}
