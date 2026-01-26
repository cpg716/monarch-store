import { useState, useEffect } from 'react';
import { Search, Trash2, Play, HardDrive, Calendar, Package as PackageIcon, Loader2 } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import ConfirmationModal from '../components/ConfirmationModal';
import { useToast } from '../context/ToastContext';

interface InstalledApp {
    name: string;
    version: string;
    size: string | null;
    install_date: string | null;
    description: string;
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

export default function InstalledPage() {
    const [searchQuery, setSearchQuery] = useState('');
    const [apps, setApps] = useState<InstalledApp[]>([]);
    const [loading, setLoading] = useState(true);
    const [totalSize, setTotalSize] = useState('Calculating...');

    const [confirmModal, setConfirmModal] = useState<{ isOpen: boolean; id: string; name: string } | null>(null);
    const { success, error } = useToast();

    // Fetch installed packages on mount
    useEffect(() => {
        const fetchInstalled = async () => {
            setLoading(true);
            try {
                const packages = await invoke<InstalledApp[]>('get_installed_packages');
                setApps(packages);

                // Calculate total size
                const sizeSum = packages.reduce((acc, pkg) => {
                    const match = (pkg.size || "").match(/(\d+\.?\d*)/);
                    return acc + (match ? parseFloat(match[1]) : 0);
                }, 0);
                setTotalSize(`${sizeSum.toFixed(1)} MiB used`);
            } catch (e) {
                console.error('Failed to fetch installed packages:', e);
                error('Failed to load packages');
            } finally {
                setLoading(false);
            }
        };

        fetchInstalled();
    }, []);

    const filteredApps = apps.filter(app =>
        app.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        app.description.toLowerCase().includes(searchQuery.toLowerCase())
    );

    const handleUninstall = (id: string, name: string) => {
        setConfirmModal({ isOpen: true, id, name });
    };

    const performUninstall = async () => {
        if (!confirmModal) return;
        const { id, name } = confirmModal;

        try {
            await invoke('uninstall_package', { name: id, password: null });
            setApps(apps.filter(a => a.name !== id));
            success(`${name} uninstalled successfully`);
        } catch (e) {
            console.error('Uninstall failed:', e);
            error(`Failed to uninstall ${name}: ${e}`);
        }
    };

    const handleLaunch = async (id: string, name: string) => {
        try {
            await invoke('launch_app', { pkgName: id });
        } catch (e) {
            console.error('Launch failed:', e);
            error(`Could not launch ${name}. Missing desktop entry.`);
        }
    };

    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            {/* Header */}
            <div className="p-8 pb-6 sticky top-0 bg-app-bg/95 backdrop-blur-3xl z-20 border-b border-black/5 dark:border-white/5 transition-colors shadow-sm dark:shadow-2xl dark:shadow-black/20">
                <div className="flex items-end justify-between mb-6">
                    <div>
                        <h1 className="text-4xl lg:text-5xl font-black flex items-center gap-3 text-slate-900 dark:text-white tracking-tight leading-none mb-2">
                            Installed
                        </h1>
                        <p className="text-lg text-slate-500 dark:text-app-muted font-medium">
                            {loading ? 'Thinking...' : `${apps.length} packages • ${totalSize}`}
                        </p>
                    </div>
                </div>

                <div className="relative group">
                    <Search className="absolute left-4 top-1/2 -translate-y-1/2 text-slate-400 dark:text-app-muted group-focus-within:text-blue-500 transition-colors" size={20} />
                    <input
                        type="text"
                        placeholder="Filter installed apps..."
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        className="w-full bg-white dark:bg-black/20 border border-black/5 dark:border-white/10 rounded-2xl py-4 pl-12 pr-4 text-slate-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500/50 transition-all placeholder:text-slate-400 dark:placeholder:text-white/20 text-lg shadow-inner"
                    />
                </div>
            </div>

            {/* Content List */}
            <div className="flex-1 overflow-y-auto p-8 custom-scrollbar">
                {loading ? (
                    <div className="flex flex-col items-center justify-center py-32 text-app-muted gap-6">
                        <Loader2 size={48} className="animate-spin text-blue-500" />
                        <p className="text-xl font-medium">Loading library...</p>
                    </div>
                ) : filteredApps.length === 0 ? (
                    <div className="text-center text-app-muted mt-32">
                        <PackageIcon size={64} className="mx-auto mb-6 opacity-20" />
                        <p className="text-2xl font-bold text-slate-900 dark:text-white mb-2">No applications found</p>
                        <p className="text-lg opacity-60">Try a different search term</p>
                    </div>
                ) : (
                    <div className="space-y-3 max-w-5xl mx-auto">
                        <AnimatePresence>
                            {filteredApps.map((app) => (
                                <motion.div
                                    key={app.name}
                                    initial={{ opacity: 0, y: 10 }}
                                    animate={{ opacity: 1, y: 0 }}
                                    exit={{ opacity: 0, height: 0 }}
                                    className="group bg-white dark:bg-app-card border border-black/5 dark:border-white/5 hover:border-black/10 dark:hover:border-white/20 rounded-2xl transition-all overflow-hidden relative shadow-sm dark:shadow-lg hover:shadow-xl dark:hover:shadow-2xl hover:-translate-y-1 backdrop-blur-sm p-4 flex items-center gap-6"
                                >
                                    {/* Icon */}
                                    <div className="w-16 h-16 rounded-2xl bg-slate-50 dark:bg-black/20 border border-black/5 dark:border-white/5 flex items-center justify-center shrink-0 overflow-hidden relative shadow-inner p-2.5">
                                        <AppIcon pkgId={app.name} />
                                    </div>

                                    {/* Info */}
                                    <div className="flex-1 min-w-0 flex flex-col justify-center gap-1">
                                        <div className="flex items-center gap-3">
                                            <h3 className="font-bold text-xl text-slate-900 dark:text-white group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors">
                                                {app.name}
                                            </h3>
                                            <span className="px-2 py-0.5 rounded-md bg-slate-100 dark:bg-white/10 text-xs font-mono text-slate-500 dark:text-white/60 border border-black/5 dark:border-white/5">
                                                {app.version}
                                            </span>
                                        </div>
                                        <p className="text-slate-500 dark:text-app-muted text-sm truncate font-medium max-w-[90%]">
                                            {app.description || "No description available"}
                                        </p>
                                    </div>

                                    {/* Meta Stats */}
                                    <div className="hidden lg:flex flex-col gap-2 items-end min-w-[140px]">
                                        <div className="flex items-center gap-2 text-xs font-bold text-slate-500 dark:text-app-muted/80 bg-slate-50 dark:bg-black/20 px-3 py-1.5 rounded-lg border border-black/5 dark:border-white/5 w-full justify-end">
                                            {app.size || "Unknown"} <HardDrive size={12} className="text-blue-500" />
                                        </div>
                                        <div className="flex items-center gap-2 text-xs font-bold text-slate-500 dark:text-app-muted/80 bg-slate-50 dark:bg-black/20 px-3 py-1.5 rounded-lg border border-black/5 dark:border-white/5 w-full justify-end">
                                            {(app.install_date || "N/A").split(' ')[0]} <Calendar size={12} className="text-purple-500" />
                                        </div>
                                    </div>

                                    {/* Actions */}
                                    <div className="flex items-center gap-3 pl-4 border-l border-black/5 dark:border-white/5">
                                        <button
                                            onClick={() => handleLaunch(app.name, app.name)}
                                            className="h-10 px-5 rounded-xl bg-blue-600 hover:bg-blue-500 text-white font-bold text-sm flex items-center justify-center gap-2 transition-all shadow-lg shadow-blue-900/20 active:scale-95 border border-white/10 hover:shadow-blue-500/20"
                                        >
                                            <Play size={16} fill="currentColor" /> Launch
                                        </button>
                                        <button
                                            onClick={() => handleUninstall(app.name, app.name)}
                                            className="h-10 w-10 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-500 dark:text-red-400 border border-red-500/10 hover:border-red-500/30 transition-all flex items-center justify-center active:scale-95"
                                            title="Uninstall"
                                        >
                                            <Trash2 size={18} />
                                        </button>
                                    </div>
                                </motion.div>
                            ))}
                        </AnimatePresence>
                    </div>
                )}
            </div>

            <ConfirmationModal
                isOpen={!!confirmModal?.isOpen}
                onClose={() => setConfirmModal(null)}
                onConfirm={performUninstall}
                title={`Uninstall ${confirmModal?.name}?`}
                message={`Are you sure you want to remove ${confirmModal?.name}? This action cannot be undone.`}
                confirmLabel="Uninstall"
                variant="danger"
            />
        </div>
    );
}
