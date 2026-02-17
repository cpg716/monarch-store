import { clsx } from 'clsx';
import { PackageSource } from '../services/bindings';
import { useDistro } from '../hooks/useDistro';

interface RepoBadgeProps {
    source: PackageSource | string;
    className?: string;
    /** When true, smaller font and padding for use in card badge rows. */
    compact?: boolean;
    /** Optional: pass distroId from parent to avoid per-instance useDistro() hook call. */
    distroId?: string;
}

/** Distro-aware badge: CachyOS → "CachyOS", Manjaro official → "Manjaro", Arch official → "Arch", etc. */
function getDistroAwareBadge(source: PackageSource | string, distroId: string): { label: string; colorClass: string } {
    const id = typeof source === 'string' ? source.toLowerCase() : (source as PackageSource).id.toLowerCase();
    const sourceType = typeof source === 'string' ? 'repo' : (source as PackageSource).source_type;

    // TIER 1: CachyOS (repo starts with cachyos)
    if (id.startsWith('cachyos')) {
        return {
            label: 'CachyOS',
            colorClass: 'bg-cyan-100 text-cyan-800 border-cyan-200 dark:bg-cyan-900/30 dark:text-cyan-300 dark:border-cyan-700/50',
        };
    }

    // TIER 2: Manjaro system (core/extra/multilib AND distro is Manjaro)
    const isArchOfficialRepo = ['core', 'extra', 'community', 'multilib', 'official'].includes(id);
    if (isArchOfficialRepo && distroId === 'manjaro') {
        return {
            label: 'Manjaro',
            colorClass: 'bg-teal-100 text-teal-800 border-teal-200 dark:bg-teal-900/30 dark:text-teal-300 dark:border-teal-700/50',
        };
    }

    // TIER 3: Chaotic-AUR
    if (id === 'chaotic-aur') {
        return {
            label: 'Chaotic',
            colorClass: 'bg-purple-100 text-purple-800 border-purple-200 dark:bg-purple-900/30 dark:text-purple-300 dark:border-purple-700/50',
        };
    }

    // TIER 4: Arch official (core, extra, multilib) — "Arch" so it's clear it's not CachyOS
    if (isArchOfficialRepo) {
        return {
            label: 'Arch',
            colorClass: 'bg-emerald-100 text-emerald-800 border-emerald-200 dark:bg-emerald-900/30 dark:text-emerald-300 dark:border-emerald-700/50',
        };
    }

    // TIER 5: Flatpak (card badge always "Flatpak", not "Universal")
    if (sourceType === 'flatpak' || id === 'flathub' || id === 'flatpak') {
        return {
            label: 'Flatpak',
            colorClass: 'bg-blue-100 text-blue-800 border-blue-200 dark:bg-blue-900/30 dark:text-blue-300 dark:border-blue-700/50',
        };
    }

    // TIER 6: AUR
    if (sourceType === 'aur' || id === 'aur') {
        return {
            label: 'AUR',
            colorClass: 'bg-orange-100 text-orange-800 border-orange-200 dark:bg-orange-900/30 dark:text-orange-300 dark:border-orange-700/50',
        };
    }

    // Other distro repos (Garuda, Endeavour, Manjaro repo id, etc.) — use id for label, neutral style
    if (sourceType === 'repo') {
        if (id.includes('manjaro')) return { label: 'Manjaro', colorClass: 'bg-teal-100 text-teal-800 border-teal-200 dark:bg-teal-900/30 dark:text-teal-300 dark:border-teal-700/50' };
        if (id.includes('garuda')) return { label: 'Garuda', colorClass: 'bg-orange-100 text-orange-800 border-orange-200 dark:bg-orange-900/30 dark:text-orange-300 dark:border-orange-700/50' };
        if (id.includes('endeavour')) return { label: 'EndeavourOS', colorClass: 'bg-violet-100 text-violet-800 border-violet-200 dark:bg-violet-900/30 dark:text-violet-300 dark:border-violet-700/50' };
        const lbl = typeof source === 'object' ? (source as PackageSource).label || '' : '';
        if (lbl === 'Official' || lbl === 'Official Repository' || lbl === 'Arch Official') {
            return { label: 'Arch', colorClass: 'bg-emerald-100 text-emerald-800 border-emerald-200 dark:bg-emerald-900/30 dark:text-emerald-300 dark:border-emerald-700/50' };
        }
        if (lbl.includes('Chaotic')) return { label: 'Chaotic', colorClass: 'bg-purple-100 text-purple-800 border-purple-200 dark:bg-purple-900/30 dark:text-purple-300 dark:border-purple-700/50' };
        if (lbl.includes('Manjaro')) return { label: 'Manjaro', colorClass: 'bg-teal-100 text-teal-800 border-teal-200 dark:bg-teal-900/30 dark:text-teal-300 dark:border-teal-700/50' };
        if (lbl.includes('Garuda')) return { label: 'Garuda', colorClass: 'bg-orange-100 text-orange-800 border-orange-200 dark:bg-orange-900/30 dark:text-orange-300 dark:border-orange-700/50' };
        if (lbl.includes('CachyOS')) return { label: 'CachyOS', colorClass: 'bg-cyan-100 text-cyan-800 border-cyan-200 dark:bg-cyan-900/30 dark:text-cyan-300 dark:border-cyan-700/50' };
        if (lbl.includes('Endeavour')) return { label: 'EndeavourOS', colorClass: 'bg-violet-100 text-violet-800 border-violet-200 dark:bg-violet-900/30 dark:text-violet-300 dark:border-violet-700/50' };
    }

    // Fallback: use backend label or id; never show "Unknown"
    const backendLabel = typeof source === 'object' ? (source as PackageSource).label || (source as PackageSource).id : source;
    const raw = typeof backendLabel === 'string' ? backendLabel : 'Repo';
    const label = (raw === 'Unknown' || raw.toLowerCase() === 'unknown') ? 'Other repository' : raw;
    return {
        label,
        colorClass: 'bg-slate-100 text-slate-800 border-slate-200 dark:bg-slate-700/30 dark:text-slate-300 dark:border-slate-600/50',
    };
}

export default function RepoBadge({ source, className, compact, distroId: distroIdProp }: RepoBadgeProps) {
    const { distro } = useDistro();
    const distroId = distroIdProp ?? (typeof distro.id === 'string' ? distro.id : (distro.id as any).Unknown ?? '');
    const { label, colorClass } = getDistroAwareBadge(source, distroId);

    return (
        <span
            className={clsx(
                'inline-flex items-center shrink-0 whitespace-nowrap border uppercase tracking-widest leading-none font-black',
                compact
                    ? 'px-2 py-1 rounded-md text-[8px]'
                    : 'px-3 py-1 rounded-lg text-[9px] shadow-sm shadow-black/20',
                colorClass,
                className
            )}
            title={typeof source === 'string' ? source : `${(source as PackageSource).source_type}/${(source as PackageSource).id}`}
        >
            {label}
        </span>
    );
}
