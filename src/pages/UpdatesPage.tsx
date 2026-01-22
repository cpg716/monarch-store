import { useState, useEffect } from 'react';
import { RefreshCw, ArrowRight, CheckCircle2, Download, AlertCircle, Loader2 } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface PendingUpdate {
    id: string;
    name: string;
    current_version: string;
    new_version: string;
    size: string;
    update_type: string;
}

interface UpdateProgress {
    phase: string;
    progress: number;
    message: string;
}

// Helper component for Icon
const AppIcon = ({ pkgId }: { pkgId: string }) => {
    const [icon, setIcon] = useState<string | null>(null);

    useEffect(() => {
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

    if (icon) return <img src={icon} alt={pkgId} className="w-full h-full object-contain" />;
    return (
        <div className="w-full h-full flex items-center justify-center bg-gradient-to-br from-blue-500/20 to-purple-500/20 rounded-lg">
            <span className="text-sm font-bold text-app-fg/50">{pkgId[0]?.toUpperCase()}</span>
        </div>
    );
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

    const handleUpdateAll = async () => {
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

    const calculateTotalSize = (updates: PendingUpdate[]): string => {
        const total = updates.reduce((acc, pkg) => {
            const match = pkg.size.match(/(\d+\.?\d*)/);
            return acc + (match ? parseFloat(match[1]) : 0);
        }, 0);
        return `${total.toFixed(0)} MiB`;
    };

    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            {/* Header */}
            <div className="p-8 pb-4 border-b border-app-border bg-app-card/50 backdrop-blur-xl z-10 transition-colors">
                <div className="flex items-center justify-between">
                    <div>
                        <h1 className="text-2xl font-bold flex items-center gap-2 text-app-fg">
                            <RefreshCw className={clsx("text-blue-500", (isUpdating || isChecking) && "animate-spin")} size={24} />
                            System Updates
                        </h1>
                        <p className="text-app-muted text-sm">
                            {isChecking ? "Checking for updates..." :
                                updates.length === 0 ? "Your system is up to date" :
                                    `${updates.length} updates available • ${calculateTotalSize(updates)} to download`}
                        </p>
                    </div>

                    <div className="flex items-center gap-3">
                        <button
                            onClick={checkForUpdates}
                            disabled={isChecking || isUpdating}
                            className="px-4 py-2 rounded-xl bg-app-subtle hover:bg-app-hover text-app-fg font-medium text-sm border border-app-border transition-all disabled:opacity-50 flex items-center gap-2"
                        >
                            <RefreshCw size={16} className={isChecking ? "animate-spin" : ""} />
                            Refresh
                        </button>

                        {updates.length > 0 && !isUpdating && (
                            <button
                                onClick={handleUpdateAll}
                                className="bg-blue-600 hover:bg-blue-500 text-white px-6 py-2 rounded-xl font-bold text-sm shadow-lg shadow-blue-900/20 active:scale-95 transition-all flex items-center gap-2"
                            >
                                <Download size={18} /> Update All
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
                            className="mt-6"
                        >
                            <div className="flex justify-between text-xs text-app-muted mb-1">
                                <span>{statusMessage || 'Updating system...'}</span>
                                <span>{Math.round(progress)}%</span>
                            </div>
                            <div className="h-2 bg-app-subtle rounded-full overflow-hidden">
                                <motion.div
                                    className="h-full bg-blue-500"
                                    initial={{ width: 0 }}
                                    animate={{ width: `${progress}%` }}
                                />
                            </div>
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto p-8">
                {isChecking ? (
                    <div className="flex flex-col items-center justify-center h-full text-app-muted gap-4">
                        <Loader2 size={32} className="animate-spin text-blue-500" />
                        <p>Checking for updates...</p>
                    </div>
                ) : updates.length === 0 && !isUpdating ? (
                    <div className="flex flex-col items-center justify-center h-full text-app-muted gap-4">
                        <div className="w-16 h-16 bg-green-500/10 text-green-500 rounded-full flex items-center justify-center">
                            <CheckCircle2 size={32} />
                        </div>
                        <div className="text-center">
                            <h3 className="text-lg font-bold text-app-fg">All Good!</h3>
                            <p>Your system is completely up to date.</p>
                            {updateResult && (
                                <pre className="mt-4 text-left text-xs bg-app-card/50 border border-app-border rounded-lg p-4 max-w-md mx-auto whitespace-pre-wrap">
                                    {updateResult}
                                </pre>
                            )}
                        </div>
                    </div>
                ) : (
                    <div className="space-y-3">
                        {updates.map((pkg) => (
                            <div
                                key={pkg.id}
                                className="bg-app-card/40 border border-app-border rounded-xl p-4 flex items-center justify-between hover:bg-app-card/60 transition-colors group"
                            >
                                <div className="flex items-center gap-4">
                                    <div className="w-10 h-10 rounded-lg bg-app-subtle flex items-center justify-center shrink-0 overflow-hidden relative p-1.5">
                                        <AppIcon pkgId={pkg.id} />
                                    </div>
                                    <div>
                                        <h3 className="font-bold flex items-center gap-2 text-app-fg">
                                            {pkg.name}
                                            <span className={clsx(
                                                "text-[10px] px-1.5 py-0.5 rounded uppercase font-bold tracking-wider",
                                                pkg.update_type === 'official' ? "bg-teal-600/20 text-teal-600" :
                                                    pkg.update_type === 'chaotic' ? "bg-violet-600/20 text-violet-600" :
                                                        "bg-amber-600/20 text-amber-600"
                                            )}>
                                                {pkg.update_type}
                                            </span>
                                        </h3>
                                        <div className="flex items-center gap-2 text-sm">
                                            <span className="text-app-muted">{pkg.current_version}</span>
                                            <ArrowRight size={12} className="text-app-muted opacity-50" />
                                            <span className="text-emerald-700 font-medium">{pkg.new_version}</span>
                                        </div>
                                    </div>
                                </div>

                                <div className="flex items-center gap-6">
                                    <div className="text-right">
                                        <p className="text-sm text-app-muted">{pkg.size}</p>
                                    </div>
                                    {pkg.update_type === 'aur' && (
                                        <div title="AUR Package: May take longer to build" className="text-amber-500">
                                            <AlertCircle size={16} />
                                        </div>
                                    )}
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </div>
        </div>
    );
}
