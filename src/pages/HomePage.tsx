import { useEffect, useState } from 'react';
import { Zap, AlertTriangle } from 'lucide-react';
import TrendingSection from '../components/TrendingSection';
import CategoryGrid from '../components/CategoryGrid';
import { useSmartEssentials } from '../hooks/useSmartEssentials';
import { useOnlineStatus } from '../hooks/useOnlineStatus'; // Vector 4: Offline Resilience
import { useDistro } from '../hooks/useDistro';
import { Package } from '../components/PackageCard';
import { WifiOff } from 'lucide-react';

interface HomePageProps {
    onSelectPackage: (pkg: Package) => void;
    onSeeAll: (view: 'essentials' | 'trending') => void;
    onSelectCategory: (category: string) => void;
}

export default function HomePage({ onSelectPackage, onSeeAll, onSelectCategory }: HomePageProps) {
    const { distro } = useDistro();
    const { essentials, loading } = useSmartEssentials();
    const isOnline = useOnlineStatus();
    const [showAlphaNotice, setShowAlphaNotice] = useState(() => {
        if (typeof window === 'undefined') return true;
        return localStorage.getItem('monarch_alpha_notice_dismissed') !== 'true';
    });
    const [offlineDismissed, setOfflineDismissed] = useState(false);

    useEffect(() => {
        if (typeof window === 'undefined') return;
        if (!showAlphaNotice) {
            localStorage.setItem('monarch_alpha_notice_dismissed', 'true');
        }
    }, [showAlphaNotice]);

    useEffect(() => {
        if (isOnline) {
            setOfflineDismissed(false);
        }
    }, [isOnline]);

    return (
        <div className="space-y-12 mt-4 animate-in fade-in duration-500">
            <section>
                <div className="flex items-center justify-between mb-4 px-2">
                    <div className="flex items-center gap-3">
                        <div className="p-2 rounded-xl bg-violet-600/10 text-violet-600">
                            <Zap size={20} />
                        </div>
                        <div>
                            <h2 className="text-xl font-bold text-slate-900 dark:text-white">Recommended Essentials</h2>
                            <p className="text-xs text-slate-500 dark:text-app-muted">
                                {loading ? "Curating for you..." : `Optimized for your ${distro.pretty_name} system.`}
                            </p>
                        </div>
                    </div>
                    <button
                        onClick={() => onSeeAll('essentials')}
                        className="text-sm font-bold text-accent hover:opacity-80 transition-colors"
                    >
                        See All
                    </button>
                </div>

                {/* ALPHA WARNING BANNER */}
                {showAlphaNotice && (
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
                            onClick={() => setShowAlphaNotice(false)}
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

                <TrendingSection
                    title=""
                    filterIds={essentials}
                    onSelectPackage={onSelectPackage}
                    onSeeAll={() => onSeeAll('essentials')}
                    variant="scroll"
                />
            </section>

            <TrendingSection
                title="Trending Applications"
                onSelectPackage={onSelectPackage}
                limit={7}
                onSeeAll={() => onSeeAll('trending')}
                variant="scroll"
            />

            <CategoryGrid onSelectCategory={onSelectCategory} />
        </div>
    );
}
