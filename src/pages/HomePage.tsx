import { Zap } from 'lucide-react';
import TrendingSection from '../components/TrendingSection';
import CategoryGrid from '../components/CategoryGrid';
import { useSmartEssentials } from '../hooks/useSmartEssentials';
import { useOnlineStatus } from '../hooks/useOnlineStatus'; // Vector 4: Offline Resilience
import { Package } from '../components/PackageCard';
import { WifiOff } from 'lucide-react';

interface HomePageProps {
    onSelectPackage: (pkg: Package) => void;
    onSeeAll: (view: 'essentials' | 'trending') => void;
    onSelectCategory: (category: string) => void;
}

export default function HomePage({ onSelectPackage, onSeeAll, onSelectCategory }: HomePageProps) {
    const { essentials, loading } = useSmartEssentials();
    const isOnline = useOnlineStatus();

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
                                {loading ? "Curating for you..." : "Optimized for your system."}
                            </p>
                        </div>
                    </div>
                    <button
                        onClick={() => onSeeAll('essentials')}
                        className="text-sm font-bold text-blue-500 hover:text-blue-400 transition-colors"
                    >
                        See All
                    </button>
                </div>

                {/* VECTOR 4: OFFLINE BANNER */}
                {!isOnline && (
                    <div className="mx-2 mb-6 p-4 rounded-xl bg-amber-500/10 border border-amber-500/20 flex items-center gap-4 animate-in slide-in-from-top-2">
                        <div className="p-2 bg-amber-500/20 rounded-full text-amber-500">
                            <WifiOff size={20} />
                        </div>
                        <div>
                            <h3 className="font-bold text-amber-500 text-sm">No Internet Connection</h3>
                            <p className="text-xs text-amber-500/80">You are browsing cached application data. Install/Update may fail.</p>
                        </div>
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
