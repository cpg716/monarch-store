import { clsx } from 'clsx';
import { PackageSource } from '../services/bindings';
import { useDistro } from '../hooks/useDistro';
import { getSourceBrand } from '../utils/sourceBrand';

interface RepoBadgeProps {
    source: PackageSource | string;
    className?: string;
    compact?: boolean;
    distroId?: string;
    showLogo?: boolean;
    logoSize?: 'sm' | 'md';
}

export default function RepoBadge({
    source,
    className,
    compact,
    distroId: distroIdProp,
    showLogo = true,
    logoSize = 'sm',
}: RepoBadgeProps) {
    const { distro } = useDistro();
    const distroId = distroIdProp ?? (typeof distro.id === 'string' ? distro.id : (distro.id as any).Unknown ?? '');
    const brand = getSourceBrand(source, distroId);
    const sizeClass = logoSize === 'md' ? 'h-4 w-4' : 'h-3 w-3';

    return (
        <span
            className={clsx(
                'inline-flex items-center gap-1.5 shrink-0 whitespace-nowrap border leading-none font-black',
                compact
                    ? 'px-2 py-1 rounded-md text-[8px]'
                    : 'px-3 py-1 rounded-lg text-[9px] shadow-sm shadow-black/20',
                brand.bgClass,
                brand.colorClass,
                className
            )}
            title={brand.hint}
        >
            {showLogo && brand.logoAsset && (
                <img
                    src={brand.logoAsset}
                    alt={brand.altText}
                    className={clsx(sizeClass, 'rounded-[3px] object-contain')}
                    loading="lazy"
                />
            )}
            <span className="uppercase tracking-widest">{compact ? brand.shortLabel : brand.label}</span>
        </span>
    );
}
