import { useCallback, useEffect, useMemo, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Search, Loader2, Package as PackageIcon } from 'lucide-react';
import { commands, type Package } from '../services/bindings';
import { unwrap } from '../utils/specta';
import { useErrorService } from '../context/ErrorContext';
import PackageCard from '../components/PackageCard';
import { formatBytes } from '../utils/display';
import { useAppStore } from '../store/internal_store';
import { getPackageListKey } from '../utils/packageKey';

function describeError(error: unknown): string {
    if (error instanceof Error) return `${error.name}: ${error.message}`;
    try {
        return JSON.stringify(error);
    } catch {
        return String(error);
    }
}

export default function InstalledPage({
    onSelectPackage,
    onUninstallPackage,
}: {
    onSelectPackage: (pkg: Package) => void;
    onUninstallPackage: (pkg: Package) => void;
}) {
    const [searchQuery, setSearchQuery] = useState('');
    const [packages, setPackages] = useState<Package[]>([]);
    const [loading, setLoading] = useState(true);
    const errorService = useErrorService();
    const upsertPackages = useAppStore((s) => s.upsertPackages);

    const fetchInstalled = useCallback(async () => {
        setLoading(true);
        try {
            const nextPackages = unwrap(await commands.getInstalledCatalog());
            setPackages(nextPackages);
            upsertPackages(nextPackages);
        } catch (error) {
            errorService.reportError(describeError(error));
        } finally {
            setLoading(false);
        }
    }, [errorService, upsertPackages]);

    useEffect(() => {
        void fetchInstalled();
    }, [fetchInstalled]);

    useEffect(() => {
        const unlisten = listen<string>('install-complete', (event) => {
            if (event.payload === 'success') {
                void fetchInstalled();
            }
        });

        return () => {
            unlisten.then((fn) => fn()).catch(() => undefined);
        };
    }, [fetchInstalled]);

    const filteredPackages = useMemo(() => {
        const query = searchQuery.trim().toLowerCase();
        if (!query) return packages;
        return packages.filter((pkg) => {
            const display = `${pkg.display_name || ''} ${pkg.display_title || ''} ${pkg.name} ${pkg.description}`.toLowerCase();
            return display.includes(query);
        });
    }, [packages, searchQuery]);

    const totalSizeBytes = useMemo(() => (
        packages.reduce((sum, pkg) => {
            const size = Number(pkg.installed_size_bytes ?? pkg.installed_size ?? 0);
            return sum + (Number.isFinite(size) ? size : 0);
        }, 0)
    ), [packages]);

    const handleLaunch = useCallback(async (pkg: Package) => {
        try {
            await commands.launchPackage({
                package_name: pkg.name,
                app_id: pkg.app_id ?? null,
                desktop_entry: null,
                launch_target: pkg.launch_target ?? null,
                source: pkg.source,
            }).then(unwrap);
        } catch (error) {
            errorService.reportError(describeError(error));
        }
    }, [errorService]);

    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            <div className="px-5 pt-5 pb-4 sticky top-0 bg-app-bg/95 backdrop-blur-3xl z-20 border-b border-black/5 dark:border-white/5 transition-colors shadow-sm dark:shadow-2xl dark:shadow-black/20">
                <div className="flex items-end justify-between mb-4">
                    <div className="min-w-0">
                        <h1 className="text-2xl lg:text-3xl font-black text-slate-900 dark:text-white tracking-tight leading-none mb-1">
                            Installed
                        </h1>
                        <p className="text-sm text-slate-500 dark:text-app-muted font-medium truncate">
                            {loading ? 'Scanning installed applications…' : `${packages.length} packages${packages.length ? ` • ${formatBytes(totalSizeBytes)}` : ''}`}
                        </p>
                    </div>
                </div>

                <div className="relative group mt-3">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400 dark:text-app-muted" size={18} />
                    <input
                        type="text"
                        placeholder="Search installed apps by name or description..."
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        className="w-full bg-white dark:bg-black/20 border border-black/5 dark:border-white/10 rounded-xl py-2.5 pl-10 pr-3 text-slate-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-blue-500/50 transition-all placeholder:text-slate-400 dark:placeholder:text-white/20 text-sm shadow-inner"
                    />
                </div>
            </div>

            <div className="flex-1 overflow-y-auto p-4 sm:p-5 custom-scrollbar min-h-0">
                {loading ? (
                    <div className="flex flex-col items-center justify-center py-20 text-app-muted gap-4">
                        <Loader2 size={36} className="animate-spin text-blue-500" />
                        <p className="text-base font-medium">Loading installed catalog...</p>
                    </div>
                ) : filteredPackages.length === 0 ? (
                    <div className="text-center text-app-muted mt-20">
                        <PackageIcon size={48} className="mx-auto mb-4 opacity-20" />
                        <p className="text-xl font-bold text-slate-900 dark:text-white mb-1">No installed applications found</p>
                        <p className="text-sm opacity-60">Try a different search term</p>
                    </div>
                ) : (
                    <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4 max-w-7xl mx-auto">
                        {filteredPackages.map((pkg) => (
                            <PackageCard
                                key={getPackageListKey(pkg)}
                                pkg={pkg}
                                viewMode="installed"
                                onClick={onSelectPackage}
                                onPrimaryAction={handleLaunch}
                                onSecondaryAction={onUninstallPackage}
                                secondaryActionLabel="Uninstall"
                            />
                        ))}
                    </div>
                )}
            </div>
        </div>
    );
}
