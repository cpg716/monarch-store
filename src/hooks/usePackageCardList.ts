import { useMemo } from 'react';
import type { Package } from '../services/bindings';
import { getPackageDisplayTitle } from '../utils/packagePresentation';
import { getPackageListKey } from '../utils/packageKey';
import { getSourceFamilyId } from '../utils/repoHelper';

export type PackageCardListSource =
    | { mode: 'ids'; ids: string[] }
    | { mode: 'packages'; packages: Package[] };

export type PackageCardListSort = 'preserve' | 'name' | 'updated';

export interface UsePackageCardListOptions {
    source: PackageCardListSource;
    packageRegistry: Record<string, Package>;
    limit?: number;
    sourceFamilyFilter?: string | null;
    sort?: PackageCardListSort;
    query?: string;
}

export interface UsePackageCardListResult {
    packages: Package[];
    ids: string[];
    totalBeforeDedupe: number;
    totalVisible: number;
}

type ResolvedEntry = {
    id: string;
    pkg: Package;
};

function resolveDirectEntryId(pkg: Package, index: number): string {
    return getPackageListKey(pkg)
        || pkg.canonical_id
        || pkg.name
        || pkg.app_id
        || `direct-${index}`;
}

function buildVisibleDedupeKey(pkg: Package): string {
    return getPackageListKey(pkg) || pkg.canonical_id || pkg.name || pkg.app_id || '__unknown__';
}

function matchesSourceFamily(pkg: Package, sourceFamilyFilter: string | null | undefined): boolean {
    if (!sourceFamilyFilter || sourceFamilyFilter === 'all') return true;

    const primaryFamily = getSourceFamilyId(typeof pkg.source === 'string' ? pkg.source : pkg.source);
    if (primaryFamily === sourceFamilyFilter) return true;

    return pkg.available_sources?.some((source) => getSourceFamilyId(source) === sourceFamilyFilter) ?? false;
}

function updatedSortValue(pkg: Package): number {
    const value = pkg.last_modified_unix ?? pkg.last_modified;
    if (typeof value === 'string') {
        const parsed = parseInt(value, 10);
        return Number.isFinite(parsed) ? parsed : 0;
    }
    return value || 0;
}

export function usePackageCardList({
    source,
    packageRegistry,
    limit,
    sourceFamilyFilter,
    sort = 'preserve',
}: UsePackageCardListOptions): UsePackageCardListResult {
    return useMemo(() => {
        const resolved: ResolvedEntry[] = source.mode === 'ids'
            ? source.ids
                .map((id) => {
                    const pkg = packageRegistry[id];
                    return pkg ? { id, pkg } : null;
                })
                .filter((entry): entry is ResolvedEntry => entry != null)
            : source.packages
                .map((pkg, index) => ({
                    id: resolveDirectEntryId(pkg, index),
                    pkg,
                }));

        const filtered = resolved.filter(({ pkg }) => matchesSourceFamily(pkg, sourceFamilyFilter));
        const totalBeforeDedupe = filtered.length;

        const seen = new Set<string>();
        const deduped = filtered.filter(({ pkg }) => {
            const key = buildVisibleDedupeKey(pkg);
            if (seen.has(key)) return false;
            seen.add(key);
            return true;
        });

        const sorted = sort === 'preserve'
            ? deduped
            : [...deduped].sort((a, b) => {
                if (sort === 'name') {
                    return getPackageDisplayTitle(a.pkg).localeCompare(getPackageDisplayTitle(b.pkg));
                }
                return updatedSortValue(b.pkg) - updatedSortValue(a.pkg);
            });

        const limited = typeof limit === 'number' ? sorted.slice(0, limit) : sorted;

        return {
            packages: limited.map((entry) => entry.pkg),
            ids: limited.map((entry) => entry.id),
            totalBeforeDedupe,
            totalVisible: limited.length,
        };
    }, [source, packageRegistry, limit, sourceFamilyFilter, sort]);
}
