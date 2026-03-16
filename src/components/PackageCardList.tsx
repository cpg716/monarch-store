import type { Package } from '../services/bindings';
import PackageCard from './PackageCard';
import { useAppStore } from '../store/internal_store';
import { usePackageCardList, type PackageCardListSource } from '../hooks/usePackageCardList';
import { getPackageListKey } from '../utils/packageKey';
import { debugWarn } from '../utils/debugLog';

type PackageCardListVariant = 'scroll' | 'grid';

interface PackageCardListProps {
    source: PackageCardListSource;
    onSelectPackage: (pkg: Package) => void;
    variant?: PackageCardListVariant;
    onSeeAll?: () => void;
    showViewAllCard?: boolean;
    setupRequiredResolver?: (pkg: Package) => boolean;
    onConfigureSource?: () => void;
    className?: string;
    emptyState?: React.ReactNode;
    surfaceName?: string;
}

export default function PackageCardList({
    source,
    onSelectPackage,
    variant = 'grid',
    onSeeAll,
    showViewAllCard = false,
    setupRequiredResolver,
    onConfigureSource,
    className = '',
    emptyState = null,
    surfaceName = 'PackageCardList',
}: PackageCardListProps) {
    const packageRegistry = useAppStore((s) => s.packageRegistry);
    const { ids, packages } = usePackageCardList({
        source,
        packageRegistry,
        sort: 'preserve',
    });
    const unresolvedIds = source.mode === 'ids'
        ? source.ids.filter((id) => !packageRegistry[id])
        : [];

    if (source.mode === 'ids' && unresolvedIds.length > 0) {
        debugWarn(`[${surfaceName}] Registry-backed list has unresolved package ids`, {
            requested: source.ids.length,
            resolved: ids.length,
            unresolved: unresolvedIds.length,
            sample_unresolved: unresolvedIds.slice(0, 5),
        });
    }

    if (source.mode === 'packages' && source.packages.length > 0 && packages.length === 0) {
        debugWarn(`[${surfaceName}] Direct backend payload could not resolve visible cards`, {
            requested: source.packages.length,
        });
    }

    if (packages.length === 0) {
        return emptyState ? <>{emptyState}</> : null;
    }

    if (variant === 'scroll') {
        return (
            <div className={`relative group/scroll max-w-7xl mx-auto ${className}`.trim()}>
                <div className="flex gap-6 overflow-x-auto pb-6 scrollbar-hide snap-x relative z-0">
                    {packages.map((pkg) => {
                        const id = getPackageListKey(pkg) || pkg.name;
                        return (
                            <div key={id} className="snap-start flex-shrink-0 w-[280px]">
                                <PackageCard
                                    pkg={pkg}
                                    pkgId={source.mode === 'ids' ? id : undefined}
                                    onClick={onSelectPackage}
                                    setupRequired={setupRequiredResolver?.(pkg) ?? false}
                                    onConfigureSource={onConfigureSource}
                                    skipMetadataFetch={!!pkg.icon}
                                />
                            </div>
                        );
                    })}
                    {showViewAllCard && onSeeAll && (
                        <div className="snap-start flex-shrink-0 w-[280px] flex">
                            <button
                                onClick={onSeeAll}
                                className="w-full h-full bg-app-card/30 border-2 border-dashed border-app-border rounded-2xl flex flex-col items-center justify-center gap-4 group transition-all min-h-[200px] accent-hover-outline"
                            >
                                <div className="w-12 h-12 rounded-full bg-app-subtle flex items-center justify-center transition-opacity group-hover:opacity-90">
                                    <span className="text-2xl">→</span>
                                </div>
                                <span className="font-bold text-app-fg transition-opacity group-hover:opacity-90">View All</span>
                            </button>
                        </div>
                    )}
                </div>
            </div>
        );
    }

    return (
        <div className={`grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 max-w-7xl mx-auto w-full ${className}`.trim()}>
            {packages.map((pkg) => {
                const id = getPackageListKey(pkg) || pkg.name;
                return (
                    <PackageCard
                        key={id}
                        pkg={pkg}
                        pkgId={source.mode === 'ids' ? id : undefined}
                        onClick={onSelectPackage}
                        setupRequired={setupRequiredResolver?.(pkg) ?? false}
                        onConfigureSource={onConfigureSource}
                        skipMetadataFetch={!!pkg.icon}
                    />
                );
            })}
            {showViewAllCard && onSeeAll && (
                <button
                    onClick={onSeeAll}
                    className="bg-app-card/30 border-2 border-dashed border-app-border rounded-2xl flex flex-col items-center justify-center gap-4 group transition-all p-8 h-full min-h-[220px] accent-hover-outline"
                >
                    <div className="w-12 h-12 rounded-full bg-app-fg/5 flex items-center justify-center transition-opacity group-hover:opacity-90">
                        <span className="text-2xl">→</span>
                    </div>
                    <span className="font-bold text-app-fg transition-opacity group-hover:opacity-90">View All</span>
                </button>
            )}
        </div>
    );
}
