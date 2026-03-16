import React, { useCallback, useEffect, useRef, useState } from 'react';
import { HardDrive, Trash2, Loader2, Info } from 'lucide-react';
import { commands, type CacheStats } from '../../services/bindings';
import { unwrap } from '../../utils/specta';
import { clsx } from 'clsx';

import { useToast } from '../../context/ToastContext';
import { useSessionPassword } from '../../context/useSessionPassword';
import { getErrorService } from '../../context/getErrorService';
import ConfirmationModal from '../ConfirmationModal';



const StorageTab: React.FC = () => {
    const [stats, setStats] = useState<CacheStats | null>(null);
    const [loading, setLoading] = useState(true);
    const [cleaning, setCleaning] = useState(false);
    const [keepVersions, setKeepVersions] = useState(2);
    const [showConfirm, setShowConfirm] = useState(false);
    const { show: showToast } = useToast();
    const { requestSessionPassword } = useSessionPassword();
    const hasRequestedInitialStats = useRef(false);

    const fetchStats = useCallback(async () => {
        try {
            setLoading(true);
            const result = unwrap(await commands.getCacheStats());
            setStats(result);
        } catch (e) {
            getErrorService()?.reportWarning(e as Error | string);
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        if (hasRequestedInitialStats.current) {
            return;
        }
        hasRequestedInitialStats.current = true;
        void fetchStats();
    }, [fetchStats]);

    const formatSize = (bytes: number | string) => {
        const val = typeof bytes === 'string' ? parseInt(bytes, 10) : bytes;
        if (val === 0) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(val) / Math.log(k));
        return parseFloat((val / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    };


    const handleClean = async () => {
        setShowConfirm(false);
        setCleaning(true);

        try {
            const password = await requestSessionPassword();
            unwrap(await commands.cleanPackageCache(password, keepVersions));
            showToast('Package cache cleaned successfully', 'success');
            await fetchStats();
        } catch (e) {
            showToast(String(e), 'error');
        } finally {
            setCleaning(false);
        }
    };

    return (
        <div className="space-y-6">
            <div className="bg-app-card/50 dark:bg-white/5 border border-app-border rounded-2xl p-6">
                <div className="flex items-center gap-4 mb-6">
                    <div className="p-3 bg-purple-500/20 rounded-xl text-purple-500 dark:text-purple-400">
                        <HardDrive className="w-6 h-6" />
                    </div>
                    <div>
                        <h3 className="text-xl font-semibold text-slate-900 dark:text-white">Package Cache</h3>
                        <p className="text-sm text-slate-500 dark:text-zinc-400">Manage downloaded package archives in /var/cache/pacman/pkg</p>
                    </div>
                </div>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-8">
                    <div className="bg-slate-100 dark:bg-zinc-900/50 rounded-xl p-4 border border-slate-200 dark:border-white/5">
                        <p className="text-sm text-slate-500 dark:text-zinc-500 mb-1">Total Disk Usage</p>
                        <p className="text-2xl font-bold text-slate-900 dark:text-white">
                            {loading ? <Loader2 className="w-6 h-6 animate-spin text-slate-500 dark:text-zinc-600" /> : formatSize(stats?.total_size_bytes || 0)}
                        </p>
                    </div>
                    <div className="bg-slate-100 dark:bg-zinc-900/50 rounded-xl p-4 border border-slate-200 dark:border-white/5">
                        <p className="text-sm text-slate-500 dark:text-zinc-500 mb-1">Cached Packages</p>
                        <p className="text-2xl font-bold text-slate-900 dark:text-white">
                            {loading ? <Loader2 className="w-6 h-6 animate-spin text-slate-500 dark:text-zinc-600" /> : stats?.package_count || 0}
                        </p>
                    </div>
                </div>

                <div className="bg-yellow-500/10 border border-yellow-500/20 rounded-xl p-4 flex gap-4 mb-8">
                    <Info className="w-6 h-6 text-yellow-500 shrink-0" />
                    <p className="text-sm text-yellow-700 dark:text-yellow-200/80 leading-relaxed">
                        Keeping a few old versions allows you to downgrade packages if a new update breaks something. Downgrading is common practice in Arch Linux troubleshooting.
                    </p>
                </div>

                <div className="flex flex-col gap-6">
                    <div>
                        <label className="text-sm font-medium text-slate-700 dark:text-zinc-300 mb-3 block">Versions to keep</label>
                        <div className="flex items-center gap-4">
                            {[0, 1, 2, 3].map((v) => (
                                <button
                                    key={v}
                                    onClick={() => setKeepVersions(v)}
                                    className={clsx(
                                        "flex-1 py-3 px-4 rounded-xl border transition-all text-sm font-medium",
                                        keepVersions === v
                                            ? "bg-purple-600 border-purple-500 text-white shadow-lg shadow-purple-600/20"
                                            : "bg-slate-100 dark:bg-white/5 border-slate-200 dark:border-white/10 text-slate-600 dark:text-zinc-400 hover:bg-slate-200 dark:hover:bg-white/10"
                                    )}
                                >
                                    {v === 0 ? 'None' : v}
                                </button>
                            ))}
                        </div>
                    </div>

                    <button
                        onClick={() => setShowConfirm(true)}
                        disabled={cleaning || (stats?.package_count || 0) === 0}
                        className={clsx(
                            "w-full py-4 rounded-xl font-semibold flex items-center justify-center gap-2 transition-all",
                            cleaning || (stats?.package_count || 0) === 0
                                ? "bg-slate-200 dark:bg-zinc-800 text-slate-500 dark:text-zinc-500 cursor-not-allowed"
                                : "bg-red-500/20 border border-red-500/30 text-red-500 hover:bg-red-500/30"
                        )}
                    >
                        {cleaning ? (
                            <>
                                <Loader2 className="w-5 h-5 animate-spin" />
                                Cleaning Cache...
                            </>
                        ) : (
                            <>
                                <Trash2 className="w-5 h-5" />
                                Clean Cache
                            </>
                        )}
                    </button>
                </div>
            </div>

            <ConfirmationModal
                isOpen={showConfirm}
                onClose={() => setShowConfirm(false)}
                onConfirm={handleClean}
                title="Clean Package Cache?"
                message={`This will remove old package versions from your system, keeping only the ${keepVersions} most recent versions. You won't be able to downgrade further than that.`}
                confirmLabel="Clean Now"
                variant="danger"
            />
        </div>
    );
};

export default StorageTab;
