import { Zap } from 'lucide-react';
import TrendingSection from '../components/TrendingSection';
import CategoryGrid from '../components/CategoryGrid';
import { ESSENTIAL_IDS } from '../constants';
import { Package } from '../components/PackageCard';

interface HomePageProps {
    onSelectPackage: (pkg: Package) => void;
    onSeeAll: (view: 'essentials' | 'trending') => void;
    onSelectCategory: (category: string) => void;
}

export default function HomePage({ onSelectPackage, onSeeAll, onSelectCategory }: HomePageProps) {
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
                            <p className="text-xs text-slate-500 dark:text-app-muted">Optimized for your system.</p>
                        </div>
                    </div>
                    <button
                        onClick={() => onSeeAll('essentials')}
                        className="text-sm font-bold text-blue-500 hover:text-blue-400 transition-colors"
                    >
                        See All
                    </button>
                </div>
                <TrendingSection
                    title=""
                    filterIds={ESSENTIAL_IDS}
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
