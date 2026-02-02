import { clsx } from 'clsx';
import { PackageSource } from '../types/alpm';
import { getRepoColor } from '../utils/repoHelper';

interface RepoBadgeProps {
    source: PackageSource | string;
    className?: string;
}

export default function RepoBadge({ source, className }: RepoBadgeProps) {
    const isLegacy = typeof source === 'string';
    const labelRaw = isLegacy ? (source as string) : (source as PackageSource).label || (source as PackageSource).id;

    const colorClass = getRepoColor(labelRaw);

    const displayText = isLegacy
        ? (source as string).toUpperCase()
        : (source as PackageSource).label;

    return (
        <span
            className={clsx(
                'inline-flex items-center px-2.5 py-0.5 rounded-full text-[10px] font-bold tracking-wider shrink-0 whitespace-nowrap border shadow-sm',
                colorClass,
                className
            )}
            title={isLegacy ? (source as string) : `${(source as PackageSource).source_type}/${(source as PackageSource).id}`}
        >
            {displayText}
        </span>
    );
}
