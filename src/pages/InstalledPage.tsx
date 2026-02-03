import { useState, useEffect } from 'react';
import { Search, Trash2, Play, HardDrive, Calendar, Package as PackageIcon, Loader2 } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import ConfirmationModal from '../components/ConfirmationModal';
import { useToast } from '../context/ToastContext';
import { useErrorService } from '../context/ErrorContext';
import { useSessionPassword } from '../context/useSessionPassword';
import { useAppStore } from '../store/internal_store';
import { Package } from '../components/PackageCard';

interface InstalledApp {
    name: string;
    version: string;
    size: string | null;
    install_date: string | null;
    description: string;
    icon: string | null;
}

// Helper component for Icon
import archLogo from '../assets/arch-logo.png';

const AppIcon = ({ appName, appIcon }: { appName: string; appIcon: string | null }) => {
    const [icon, setIcon] = useState<string | null>(appIcon);

    useEffect(() => {
        // Optimizing "The Storm": Disable client-side fetch loop entirely. 
        // We rely on the backend batch fetch.
        if (appIcon) {
            setIcon(appIcon);
        } else {
            setIcon(null);
        }
    }, [appName, appIcon]);

    const displayIcon = icon || archLogo;

    return <img src={displayIcon} alt={appName} className={clsx("w-full h-full object-contain", !icon && "opacity-50 grayscale")} />;
};

export default function InstalledPage({ onSelectPackage }: { onSelectPackage: (pkg: Package) => void }) {
    const [searchQuery, setSearchQuery] = useState('');
    const [apps, setApps] = useState<InstalledApp[]>([]);
    const [loading, setLoading] = useState(true);
    const [totalSize, setTotalSize] = useState('Calculating...');

    const [confirmModal, setConfirmModal] = useState<{ isOpen: boolean; id: string; name: string } | null>(null);
    const { success } = useToast();
    const errorService = useErrorService();
    const { requestSessionPassword } = useSessionPassword();
    const reducePasswordPrompts = useAppStore((s) => s.reducePasswordPrompts);

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
                errorService.reportError(e as Error | string);
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
            const pwd = reducePasswordPrompts ? await requestSessionPassword() : null;
            await invoke('uninstall_package', { name: id, password: pwd });
            setApps(apps.filter(a => a.name !== id));
            success(`${name} uninstalled successfully`);
        } catch (e) {
            errorService.reportError(e as Error | string);
        }
    };

    const handleLaunch = async (id: string) => {
        try {
            await invoke('launch_app', { pkgName: id });
        } catch (e) {
            errorService.reportError(e as Error | string);
        }
    };

    const handleNavigation = async (app: InstalledApp) => {
        try {
            // Try to get proper package info
            const results = await invoke<Package[]>('get_packages_by_names', { names: [app.name] });
            if (results && results.length > 0) {
                onSelectPackage(results[0]);
            } else {
                // Search as fallback
                const searchResults = await invoke<Package[]>('search_packages', { query: app.name });
                const exactMatch = searchResults.find(p => p.name.toLowerCase() === app.name.toLowerCase());
                if (exactMatch) {
                    onSelectPackage(exactMatch);
                } else if (searchResults.length > 0) {
                    onSelectPackage(searchResults[0]);
                }
            }
        } catch (e) {
            errorService.reportError(e as Error | string);
        }
    };

    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            {/* Header — ~30% tighter */}
            <div className="px-5 pt-5 pb-4 sticky top-0 bg-app-bg/95 backdrop-blur-3xl z-20 border-b border-black/5 dark:border-white/5 transition-colors shadow-sm dark:shadow-2xl dark:shadow-black/20">
                <div className="flex items-end justify-between mb-4">
                    <div className="min-w-0">
                        <h1 className="text-2xl lg:text-3xl font-black flex items-center gap-2 text-slate-900 dark:text-white tracking-tight leading-none mb-1">
                            Installed
                        </h1>
                        <p className="text-sm text-slate-500 dark:text-app-muted font-medium truncate">
                            {loading ? 'Thinking...' : `${apps.length} packages • ${totalSize}`}
                        </p>
                    </div>
                </div>

                <div className="relative group mt-3">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400 dark:text-app-muted group-focus-within:text-accent transition-colors" size={18} />
                    <input
                        type="text"
                        placeholder="Filter installed apps..."
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        className="w-full bg-white dark:bg-black/20 border border-black/5 dark:border-white/10 rounded-xl py-2.5 pl-10 pr-3 text-slate-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500/50 transition-all placeholder:text-slate-400 dark:placeholder:text-white/20 text-sm shadow-inner"
                    />
                </div>
            </div>

            {/* Content List */}
            <div className="flex-1 overflow-y-auto p-4 sm:p-5 custom-scrollbar min-h-0">
                {loading ? (
                    <div className="flex flex-col items-center justify-center py-20 text-app-muted gap-4">
                        <Loader2 size={36} className="animate-spin text-blue-500" />
                        <p className="text-base font-medium">Loading library...</p>
                    </div>
                ) : filteredApps.length === 0 ? (
                    <div className="text-center text-app-muted mt-20">
                        <PackageIcon size={48} className="mx-auto mb-4 opacity-20" />
                        <p className="text-xl font-bold text-slate-900 dark:text-white mb-1">No applications found</p>
                        <p className="text-sm opacity-60">Try a different search term</p>
                    </div>
                ) : (
                    <div className="space-y-2 max-w-4xl mx-auto">
                        <AnimatePresence>
                            {filteredApps.map((app) => (
                                <motion.div
                                    key={app.name}
                                    initial={{ opacity: 0, y: 8 }}
                                    animate={{ opacity: 1, y: 0 }}
                                    exit={{ opacity: 0, height: 0 }}
                                    onClick={() => handleNavigation(app)}
                                    className="group bg-white dark:bg-app-card border border-black/5 dark:border-white/5 hover:border-black/10 dark:hover:border-white/20 rounded-xl transition-all overflow-hidden relative shadow-sm dark:shadow-md hover:shadow-lg hover:-translate-y-0.5 backdrop-blur-sm p-3 flex items-center gap-3 md:gap-4 cursor-pointer min-w-0"
                                >
                                    {/* Icon */}
                                    <div className="w-11 h-11 rounded-xl bg-slate-50 dark:bg-black/20 border border-black/5 dark:border-white/5 flex items-center justify-center shrink-0 overflow-hidden relative shadow-inner p-1.5">
                                        <AppIcon appName={app.name} appIcon={app.icon} />
                                    </div>

                                    {/* Info — min-w-0 so text truncates instead of overflowing */}
                                    <div className="flex-1 min-w-0 flex flex-col justify-center gap-0.5">
                                        <div className="flex items-center gap-2 min-w-0">
                                            <h3 className="font-bold text-base text-slate-900 dark:text-white group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors truncate">
                                                {app.name}
                                            </h3>
                                            <span className="px-1.5 py-0.5 rounded bg-slate-100 dark:bg-white/10 text-[10px] font-mono text-slate-500 dark:text-white/60 border border-black/5 dark:border-white/5 shrink-0">
                                                {app.version}
                                            </span>
                                        </div>
                                        <p className="text-slate-500 dark:text-app-muted text-xs font-medium line-clamp-2 min-w-0">
                                            {app.description || "No description available"}
                                        </p>
                                    </div>

                                    {/* Meta Stats — compact */}
                                    <div className="hidden sm:flex flex-col gap-1 items-end min-w-[100px] shrink-0">
                                        <div className="flex items-center gap-1.5 text-[10px] font-bold text-slate-500 dark:text-app-muted/80 bg-slate-50 dark:bg-black/20 px-2 py-1 rounded border border-black/5 dark:border-white/5">
                                            {app.size || "—"} <HardDrive size={10} className="text-blue-500 shrink-0" />
                                        </div>
                                        <div className="flex items-center gap-1.5 text-[10px] font-bold text-slate-500 dark:text-app-muted/80 bg-slate-50 dark:bg-black/20 px-2 py-1 rounded border border-black/5 dark:border-white/5">
                                            {(app.install_date || "N/A").split(' ')[0]} <Calendar size={10} className="text-purple-500 shrink-0" />
                                        </div>
                                    </div>

                                    {/* Actions */}
                                    <div className="flex items-center gap-1.5 pl-3 border-l border-black/5 dark:border-white/5 shrink-0">
                                        <button
                                            onClick={(e) => { e.stopPropagation(); handleLaunch(app.name); }}
                                            className="h-8 px-3 rounded-lg btn-accent hover:opacity-90 font-bold text-xs flex items-center justify-center gap-1.5 transition-all shadow-md active:scale-95 border border-white/10"
                                        >
                                            <Play size={14} fill="currentColor" /> Launch
                                        </button>
                                        <button
                                            onClick={(e) => { e.stopPropagation(); handleUninstall(app.name, app.name); }}
                                            className="h-8 w-8 rounded-lg bg-red-500/10 hover:bg-red-500/20 text-red-500 dark:text-red-400 border border-red-500/10 hover:border-red-500/30 transition-all flex items-center justify-center active:scale-95 shrink-0"
                                            title="Uninstall"
                                        >
                                            <Trash2 size={14} />
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
