import { useState, useEffect } from 'react';
import { Search, Grid, List, Trash2, Play, HardDrive, Calendar, Package as PackageIcon, Loader2 } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';

interface InstalledApp {
    id: string;
    name: string;
    version: string;
    size: string;
    install_date: string;
    description: string;
}

// Helper component for Icon - uses local icons first, then metadata
const AppIcon = ({ pkgId }: { pkgId: string }) => {
    const [icon, setIcon] = useState<string | null>(null);

    useEffect(() => {
        // First try local icon lookup
        invoke<string | null>('get_package_icon', { pkgName: pkgId })
            .then(localIcon => {
                if (localIcon) {
                    setIcon(localIcon);
                } else {
                    // Fallback to metadata
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

    // Show first letter as fallback
    return (
        <div className="w-full h-full flex items-center justify-center bg-gradient-to-br from-blue-500/20 to-purple-500/20 rounded-lg">
            <span className="text-lg font-bold text-app-fg/50">{pkgId[0]?.toUpperCase()}</span>
        </div>
    );
};

export default function InstalledPage() {
    const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');
    const [searchQuery, setSearchQuery] = useState('');
    const [apps, setApps] = useState<InstalledApp[]>([]);
    const [loading, setLoading] = useState(true);
    const [totalSize, setTotalSize] = useState('Calculating...');

    // Fetch installed packages on mount
    useEffect(() => {
        const fetchInstalled = async () => {
            setLoading(true);
            try {
                const packages = await invoke<InstalledApp[]>('get_installed_packages');
                setApps(packages);

                // Calculate total size (simple sum)
                const sizeSum = packages.reduce((acc, pkg) => {
                    const match = pkg.size.match(/(\d+\.?\d*)/);
                    return acc + (match ? parseFloat(match[1]) : 0);
                }, 0);
                setTotalSize(`${sizeSum.toFixed(1)} MiB used`);
            } catch (e) {
                console.error('Failed to fetch installed packages:', e);
            } finally {
                setLoading(false);
            }
        };

        fetchInstalled();
    }, []);

    const filteredApps = apps.filter(app =>
        app.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        app.description.toLowerCase().includes(searchQuery.toLowerCase()) ||
        app.id.toLowerCase().includes(searchQuery.toLowerCase())
    );

    const handleUninstall = async (id: string, name: string) => {
        if (confirm(`Are you sure you want to uninstall ${name}?`)) {
            try {
                // Call the uninstall_package Tauri command
                await invoke('uninstall_package', { name: id, password: null });
                // Optimistically update UI (event listener will confirm)
                setApps(apps.filter(a => a.id !== id));
            } catch (e) {
                console.error('Uninstall failed:', e);
                alert(`Failed to uninstall ${name}. Please try again.`);
            }
        }
    };

    const handleLaunch = async (id: string, name: string) => {
        try {
            await invoke('launch_app', { pkgName: id });
        } catch (e) {
            console.error('Launch failed:', e);
            alert(`Could not launch ${name}. The application may not have a desktop entry.`);
        }
    };

    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            {/* Header */}
            <div className="p-8 pb-4 border-b border-app-border bg-app-card/50 backdrop-blur-xl z-10 transition-colors">
                <div className="flex items-center justify-between mb-6">
                    <div>
                        <h1 className="text-2xl font-bold flex items-center gap-2 text-app-fg">
                            <HardDrive className="text-green-500" size={24} />
                            Installed Applications
                        </h1>
                        <p className="text-app-muted text-sm">
                            {loading ? 'Loading...' : `${apps.length} packages installed • ${totalSize}`}
                        </p>
                    </div>

                    <div className="flex items-center gap-2 bg-app-subtle p-1 rounded-lg border border-app-border">
                        <button
                            onClick={() => setViewMode('grid')}
                            className={clsx(
                                "p-2 rounded-md transition-all",
                                viewMode === 'grid' ? "bg-app-hover text-app-fg shadow-sm" : "text-app-muted hover:text-app-fg"
                            )}
                        >
                            <Grid size={18} />
                        </button>
                        <button
                            onClick={() => setViewMode('list')}
                            className={clsx(
                                "p-2 rounded-md transition-all",
                                viewMode === 'list' ? "bg-app-hover text-app-fg shadow-sm" : "text-app-muted hover:text-app-fg"
                            )}
                        >
                            <List size={18} />
                        </button>
                    </div>
                </div>

                <div className="relative">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-app-muted" size={18} />
                    <input
                        type="text"
                        placeholder="Filter installed apps..."
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        className="w-full bg-app-subtle border border-app-border rounded-xl py-3 pl-10 pr-4 text-app-fg focus:outline-none focus:ring-2 focus:ring-green-500/50 transition-all placeholder:text-app-muted/50"
                    />
                </div>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto p-8">
                {loading ? (
                    <div className="flex flex-col items-center justify-center py-20 text-app-muted gap-4">
                        <Loader2 size={32} className="animate-spin text-green-500" />
                        <p>Loading installed packages...</p>
                    </div>
                ) : filteredApps.length === 0 ? (
                    <div className="text-center text-app-muted mt-20">
                        <PackageIcon size={48} className="mx-auto mb-4 opacity-30" />
                        <p className="text-lg font-medium text-app-fg">No applications found</p>
                        <p className="text-sm">Try a different search term</p>
                    </div>
                ) : (
                    <div className={clsx(
                        viewMode === 'grid' ? "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4" : "space-y-2"
                    )}>
                        <AnimatePresence>
                            {filteredApps.map((app) => (
                                <motion.div
                                    key={app.id}
                                    layout
                                    initial={{ opacity: 0, scale: 0.9 }}
                                    animate={{ opacity: 1, scale: 1 }}
                                    exit={{ opacity: 0, scale: 0.9 }}
                                    className={clsx(
                                        "group bg-app-card/40 border border-app-border hover:border-app-fg/10 rounded-xl transition-all overflow-hidden relative",
                                        viewMode === 'grid' ? "p-6 flex flex-col gap-4" : "p-4 flex items-center justify-between gap-4"
                                    )}
                                >
                                    <div className={clsx("flex items-start gap-4", viewMode === 'list' && "flex-1")}>
                                        <div className={clsx(
                                            "rounded-lg bg-app-subtle flex items-center justify-center shrink-0 overflow-hidden relative",
                                            viewMode === 'grid' ? "w-16 h-16 p-2" : "w-12 h-12 p-1.5"
                                        )}>
                                            <AppIcon pkgId={app.id} />
                                        </div>

                                        <div className="flex-1 min-w-0">
                                            <h3 className="font-bold text-lg truncate text-app-fg">{app.name}</h3>
                                            <p className="text-app-muted text-sm truncate">{app.version}</p>

                                            {viewMode === 'grid' && (
                                                <div className="mt-4 grid grid-cols-2 gap-2 text-xs text-app-muted">
                                                    <div className="flex items-center gap-1.5 bg-app-subtle p-1.5 rounded-md">
                                                        <HardDrive size={12} /> {app.size}
                                                    </div>
                                                    <div className="flex items-center gap-1.5 bg-app-subtle p-1.5 rounded-md">
                                                        <Calendar size={12} /> {app.install_date.split(' ')[0]}
                                                    </div>
                                                </div>
                                            )}
                                        </div>
                                    </div>

                                    {/* Actions */}
                                    <div className={clsx("flex items-center gap-2", viewMode === 'grid' ? "mt-auto pt-4 border-t border-app-border" : "")}>
                                        <button
                                            onClick={() => handleLaunch(app.id, app.name)}
                                            className="flex-1 px-4 py-2 rounded-lg bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-700 font-medium text-sm flex items-center justify-center gap-2 transition-colors"
                                        >
                                            <Play size={16} /> <span className={viewMode === 'list' ? "hidden md:inline" : ""}>Launch</span>
                                        </button>
                                        <button
                                            onClick={() => handleUninstall(app.id, app.name)}
                                            className="px-3 py-2 rounded-lg bg-red-500/10 hover:bg-red-500/20 text-red-400 transition-colors"
                                            title="Uninstall"
                                        >
                                            <Trash2 size={16} />
                                        </button>
                                    </div>
                                </motion.div>
                            ))}
                        </AnimatePresence>
                    </div>
                )}
            </div>
        </div>
    );
}
