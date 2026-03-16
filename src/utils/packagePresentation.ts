import type { InstalledPackage, Package, PackageSource } from '../services/bindings';

type SourceLike = PackageSource | string | null | undefined;

export function getPackageDisplayTitle(
    pkg: Pick<Package, 'display_name' | 'display_title' | 'name'> | null | undefined
): string {
    if (!pkg) return '';
    const title = typeof pkg.display_title === 'string' ? pkg.display_title.trim() : '';
    if (title) return title;
    const display = typeof pkg.display_name === 'string' ? pkg.display_name.trim() : '';
    if (display) return display;
    return pkg.name;
}

export function getInstalledDisplayTitle(pkg: Pick<InstalledPackage, 'display_name' | 'name'> | null | undefined): string {
    if (!pkg) return '';
    const display = typeof pkg.display_name === 'string' ? pkg.display_name.trim() : '';
    if (display) return display;
    return pkg.name;
}

export function getPackageSourceLabel(source: SourceLike): string {
    if (!source) return 'Unknown source';
    if (typeof source === 'string') return source;
    return source.label || source.id || source.source_type;
}

export function getPackageSourceSummary(
    pkg: Pick<Package, 'source_summary'> | null | undefined,
    fallback: string
): string {
    const summary = typeof pkg?.source_summary === 'string' ? pkg.source_summary.trim() : '';
    return summary || fallback;
}

export function getPackagePrimaryActionLabel(
    pkg: Pick<Package, 'installed' | 'primary_action_label'> | null | undefined,
    options?: { setupRequired?: boolean }
): 'Configure' | 'Open' | 'Install' {
    if (options?.setupRequired) return 'Configure';
    const label = typeof pkg?.primary_action_label === 'string' ? pkg.primary_action_label.trim() : '';
    if (label === 'Configure' || label === 'Open' || label === 'Install') return label;
    if (pkg?.installed) return 'Open';
    return 'Install';
}
