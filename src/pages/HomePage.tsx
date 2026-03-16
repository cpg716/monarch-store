import { useEffect, useState } from 'react';
import { Sparkles, Flame, AlertTriangle } from 'lucide-react';
import EssentialsSection from '../components/EssentialsSection';
import TrendingSection from '../components/TrendingSection';
import CategoryGrid from '../components/CategoryGrid';
import { useOnlineStatus } from '../hooks/useOnlineStatus';
import type { Package } from '../services/bindings';
import { WifiOff } from 'lucide-react';
import { useAppStore } from '../store/internal_store';
import { useDistro } from '../hooks/useDistro';
import type { DiscoveryIntent } from '../services/bindings';

interface HomePageProps {
    onSelectPackage: (pkg: Package) => void;
    onSeeAll: (view: 'essentials' | 'trending') => void;
    onSelectCategory: (category: string) => void;
    quickStarts: DiscoveryIntent[];
    essentialsPackages: Package[];
    trendingPackages: Package[];
    homeDiscoveryLoading: boolean;
    homeDiscoveryError: string | null;
    onQuickStart: (intent: DiscoveryIntent) => void;
    onOpenSettings?: () => void;
}

export default function HomePage({ onSelectPackage, onSeeAll, onSelectCategory, quickStarts, essentialsPackages, trendingPackages, homeDiscoveryLoading, homeDiscoveryError, onQuickStart, onOpenSettings }: HomePageProps) {
    const essentialsIds = useAppStore((s) => s.essentialsIds);
    const trendingIds = useAppStore((s) => s.trendingIds);
    const hasHomeContent = essentialsIds.length > 0
        || trendingIds.length > 0
        || essentialsPackages.length > 0
        || trendingPackages.length > 0;
    const loading = homeDiscoveryLoading && !hasHomeContent;
    const isOnline = useOnlineStatus();

    const alphaNoticeDismissed = useAppStore(s => s.alphaNoticeDismissed);
    const setAlphaNoticeDismissed = useAppStore(s => s.setAlphaNoticeDismissed);
    const isAurEnabled = useAppStore((s) => s.isAurEnabled);
    const isFlatpakEnabled = useAppStore((s) => s.isFlatpakEnabled);
    const isChaoticEnabled = useAppStore((s) => s.isChaoticEnabled);
    const { distro } = useDistro();

    const [offlineDismissed, setOfflineDismissed] = useState(false);

    useEffect(() => {
        if (isOnline) {
            setOfflineDismissed(false);
        }
    }, [isOnline]);

    return (
        <div className="space-y-12 mt-4 animate-in fade-in duration-500">
            <section className="rounded-xl border border-app-border bg-app-card p-5">
                <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
                    <div>
                        <h1 className="text-2xl font-black tracking-tight text-white">Software for your Arch-based system</h1>
                        <p className="mt-2 text-sm text-slate-300">
                            Discover trusted apps, compare sources, and install without touching the terminal.
                        </p>
                    </div>
                    <div className="grid gap-3 sm:grid-cols-3">
                        <div className="rounded-lg border border-white/5 bg-black/20 px-3 py-3">
                            <div className="text-[11px] font-semibold text-app-muted">Detected distro</div>
                            <div className="mt-1 text-sm font-bold text-white">{distro.pretty_name || 'Arch Linux'}</div>
                        </div>
                        <div className="rounded-lg border border-white/5 bg-black/20 px-3 py-3">
                            <div className="text-[11px] font-semibold text-app-muted">Active sources</div>
                            <div className="mt-1 text-sm font-bold text-white">
                                {['Official', isChaoticEnabled && 'Chaotic-AUR', isAurEnabled && 'AUR', isFlatpakEnabled && 'Flatpak']
                                    .filter(Boolean)
                                    .join(' • ')}
                            </div>
                        </div>
                        <div className="rounded-lg border border-white/5 bg-black/20 px-3 py-3">
                            <div className="text-[11px] font-semibold text-app-muted">Status</div>
                            <div className="mt-1 text-sm font-bold text-white">{isOnline ? 'Online' : 'Offline cache mode'}</div>
                        </div>
                    </div>
                </div>
            </section>

            {quickStarts.length > 0 && (
                <section className="rounded-xl border border-app-border bg-app-card p-5">
                    <div className="flex items-center justify-between gap-3">
                        <div>
                            <h2 className="text-lg font-black tracking-tight text-white">Quick Starts</h2>
                            <p className="mt-1 text-xs text-slate-300">
                                Jump into common software tasks without guessing what to search for.
                            </p>
                        </div>
                    </div>
                    <div className="mt-4 flex flex-wrap gap-2">
                        {quickStarts.map((intent) => (
                            <button
                                key={intent.id}
                                type="button"
                                onClick={() => onQuickStart(intent)}
                                className="rounded-full border border-app-border bg-black/20 px-3 py-2 text-xs font-bold text-white transition-colors hover:border-blue-500/40 hover:bg-blue-500/10"
                                title={intent.description}
                            >
                                {intent.label}
                            </button>
                        ))}
                    </div>
                </section>
            )}

            <section>
                <div className="flex items-center justify-between mb-4 px-2">
                    <div className="flex items-center gap-3">
                        <div className="p-2 rounded-xl bg-violet-600/10 text-violet-600">
                            <Sparkles size={20} />
                        </div>
                        <div>
                            <h2 className="text-xl font-bold text-slate-900 dark:text-white">Recommended Essentials</h2>
                            <p className="text-xs text-slate-500 dark:text-app-muted">
                                {loading ? "Curating for you..." : "Curated apps to get started."}
                            </p>
                        </div>
                    </div>
                    <button
                        onClick={() => onSeeAll('essentials')}
                        className="text-sm font-bold text-accent hover:opacity-80 transition-colors"
                    >
                        See All →
                    </button>
                </div>

                {/* ALPHA WARNING BANNER */}
                {!alphaNoticeDismissed && (
                    <div className="mx-2 mb-6 p-4 rounded-xl bg-violet-500/10 border border-violet-500/20 flex items-start gap-4 animate-in slide-in-from-top-2">
                        <div className="p-2 bg-violet-500/20 rounded-full text-violet-500">
                            <AlertTriangle size={20} />
                        </div>
                        <div className="flex-1 space-y-1">
                            <h3 className="font-bold text-violet-500 text-sm">Experimental Alpha Release</h3>
                            <p className="text-xs text-violet-500/80">
                                MonARCH Store is in early Alpha. Package installations and updates are still experimental—proceed carefully on production systems.
                            </p>
                        </div>
                        <button
                            type="button"
                            onClick={() => setAlphaNoticeDismissed(true)}
                            className="text-xs font-bold text-violet-500/80 hover:text-violet-500 transition-colors"
                            aria-label="Dismiss alpha warning"
                        >
                            Dismiss
                        </button>
                    </div>
                )}

                {/* VECTOR 4: OFFLINE BANNER */}
                {!isOnline && !offlineDismissed && (
                    <div className="mx-2 mb-6 p-4 rounded-xl bg-amber-500/10 border border-amber-500/20 flex items-start gap-4 animate-in slide-in-from-top-2">
                        <div className="p-2 bg-amber-500/20 rounded-full text-amber-500">
                            <WifiOff size={20} />
                        </div>
                        <div className="flex-1 space-y-1">
                            <h3 className="font-bold text-amber-500 text-sm">No Internet Connection</h3>
                            <p className="text-xs text-amber-500/80">You are browsing cached application data. Install and update actions may fail until connectivity returns.</p>
                        </div>
                        <button
                            type="button"
                            onClick={() => setOfflineDismissed(true)}
                            className="text-xs font-bold text-amber-600/80 hover:text-amber-600 transition-colors"
                            aria-label="Dismiss offline warning"
                        >
                            Dismiss
                        </button>
                    </div>
                )}

                {homeDiscoveryError && !hasHomeContent && (
                    <div className="mx-2 mb-6 p-4 rounded-xl bg-rose-500/10 border border-rose-500/20 flex items-start gap-4 animate-in slide-in-from-top-2">
                        <div className="p-2 bg-rose-500/20 rounded-full text-rose-400">
                            <AlertTriangle size={20} />
                        </div>
                        <div className="flex-1 space-y-1">
                            <h3 className="font-bold text-rose-300 text-sm">Discovery Unavailable</h3>
                            <p className="text-xs text-rose-200/80">
                                MonARCH could not build a fresh discovery snapshot right now. Search and Installed are still available while discovery data catches up.
                            </p>
                            <p className="text-[11px] text-rose-200/60 break-all">{homeDiscoveryError}</p>
                        </div>
                    </div>
                )}

                <EssentialsSection
                    title=""
                    filterIds={essentialsIds}
                    preloadedPackages={essentialsPackages}
                    limit={7}
                    onSelectPackage={onSelectPackage}
                    onSeeAll={() => onSeeAll('essentials')}
                    variant="scroll"
                    onOpenSettings={onOpenSettings}
                    hideHeader
                    loading={loading && essentialsIds.length === 0 && essentialsPackages.length === 0}
                />
            </section>

            <section>
                <div className="flex items-center justify-between mb-4 px-2">
                    <div className="flex items-center gap-3">
                        <div className="p-2 rounded-xl bg-amber-500/10 text-amber-500">
                            <Flame size={20} />
                        </div>
                        <div>
                            <h2 className="text-xl font-bold text-slate-900 dark:text-white">Trending Applications</h2>
                            <p className="text-xs text-slate-500 dark:text-app-muted">
                                Popular apps right now.
                            </p>
                        </div>
                    </div>
                    <button
                        onClick={() => onSeeAll('trending')}
                        className="text-sm font-bold text-accent hover:opacity-80 transition-colors"
                    >
                        See All →
                    </button>
                </div>
                <TrendingSection
                    title=""
                    listKind="trending"
                    filterIds={trendingIds}
                    preloadedPackages={trendingPackages}
                    onSelectPackage={onSelectPackage}
                    limit={7}
                    onSeeAll={() => onSeeAll('trending')}
                    variant="scroll"
                    onOpenSettings={onOpenSettings}
                    hideHeader
                    preloadInProgress={loading && trendingIds.length === 0 && trendingPackages.length === 0}
                />
            </section>

            <CategoryGrid onSelectCategory={onSelectCategory} />
        </div>
    );
}
