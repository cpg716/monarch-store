import React, { useState, useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronDown } from 'lucide-react';
import { clsx } from 'clsx';

import { PackageSource } from '../services/bindings';
import { useDistro } from '../hooks/useDistro';
import { getSourceTierForSort, isSameSource } from '../utils/repoHelper';
import { getSourceKey } from '../utils/packageKey';
import { getSourceBrand } from '../utils/sourceBrand';

interface RepoVariant {
    source: PackageSource | string;
    version: string;
    repo_name?: string;
    pkg_name?: string;
}

interface RepoSelectorProps {
    variants: RepoVariant[];
    selectedSource: PackageSource | string;
    onChange: (source: PackageSource | string) => void;
    disabled?: boolean;
}

const RepoSelector: React.FC<RepoSelectorProps> = ({ variants, selectedSource, onChange, disabled = false }) => {
    const { distro } = useDistro();
    const [isOpen, setIsOpen] = useState(false);
    const containerRef = useRef<HTMLDivElement>(null);
    const distroId = typeof distro.id === 'string' ? distro.id : (distro.id as any).Unknown ?? '';

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    const selectedVariant = variants.find((variant) => isSameSource(variant.source, selectedSource)) ?? variants[0];
    const selectedBrand = selectedVariant
        ? getSourceBrand(selectedVariant.source, distroId, selectedVariant.pkg_name)
        : null;

    return (
        <div className="relative w-full min-w-0" ref={containerRef}>
            <button
                type="button"
                onClick={() => {
                    if (!disabled) setIsOpen(!isOpen);
                }}
                title={selectedBrand?.hint}
                disabled={disabled}
                className={clsx(
                    'w-full min-w-0 flex items-center justify-between gap-2 px-3 md:px-4 py-3 rounded-xl border transition-all text-left shadow-sm dark:shadow-none',
                    selectedBrand?.bgClass ?? 'bg-app-card border-app-border',
                    isOpen ? 'ring-2 ring-blue-500/10 border-blue-400/50' : 'hover:brightness-105',
                    disabled && 'cursor-not-allowed opacity-75 hover:brightness-100'
                )}
            >
                <div className="flex items-center gap-3 min-w-0 flex-1">
                    {selectedBrand?.logoAsset && (
                        <img
                            src={selectedBrand.logoAsset}
                            alt={selectedBrand.altText}
                            className="h-9 w-9 rounded-lg object-contain border border-white/5 bg-black/10 p-1.5 shrink-0"
                            loading="lazy"
                        />
                    )}
                    <div className="flex flex-col leading-none min-w-0 flex-1">
                        <div className="flex items-center gap-2 min-w-0">
                            <span className={clsx('text-sm font-bold truncate', selectedBrand?.colorClass ?? 'text-app-fg')}>
                                {selectedBrand ? selectedBrand.label : 'Select Source'}
                            </span>
                            {selectedBrand?.recommended && (
                                <span className="bg-blue-500 text-white text-[10px] font-bold px-1.5 py-0.5 rounded shadow-sm shrink-0">
                                    RECOMMENDED
                                </span>
                            )}
                        </div>
                        {selectedVariant && (
                            <span className="text-[10px] text-app-muted font-mono mt-1 opacity-70 truncate">
                                {selectedBrand?.shortLabel ?? 'Source'} • v{selectedVariant.version}
                            </span>
                        )}
                    </div>
                </div>
                <motion.div
                    animate={{ rotate: isOpen ? 180 : 0 }}
                    transition={{ duration: 0.2 }}
                    className="shrink-0"
                >
                    <ChevronDown size={16} className={clsx('opacity-50', selectedBrand?.colorClass ?? 'text-app-muted')} />
                </motion.div>
            </button>

            <AnimatePresence>
                {isOpen && !disabled && (
                    <motion.div
                        initial={{ opacity: 0, y: 5, scale: 0.98 }}
                        animate={{ opacity: 1, y: 0, scale: 1 }}
                        exit={{ opacity: 0, y: 5, scale: 0.98 }}
                        transition={{ duration: 0.15 }}
                        className="absolute top-full left-0 mt-2 p-1 bg-[#121212] border border-white/10 rounded-xl shadow-[0_20px_50px_rgba(0,0,0,0.5)] z-[110] overflow-hidden w-full min-w-[280px] ring-1 ring-white/5 backdrop-blur-xl"
                    >
                        <div className="flex flex-col gap-1 max-h-[350px] overflow-y-auto custom-scrollbar">
                            {(() => {
                                const seen = new Set<string>();
                                const sorted = [...variants].sort((a, b) => getSourceTierForSort(b.source, distroId) - getSourceTierForSort(a.source, distroId));
                                return sorted.filter((variant) => {
                                    const key = `${getSourceKey(variant.source)}-${String(variant.pkg_name ?? '')}-${String(variant.version ?? '')}`;
                                    if (seen.has(key)) return false;
                                    seen.add(key);
                                    return true;
                                }).map((variant, idx) => {
                                    const brand = getSourceBrand(variant.source, distroId, variant.pkg_name);
                                    const isSelected = isSameSource(selectedSource, variant.source);
                                    return (
                                        <button
                                            key={`${getSourceKey(variant.source)}-${String(variant.pkg_name ?? variant.version ?? '')}-${idx}`}
                                            onClick={() => {
                                                onChange(variant.source);
                                                setIsOpen(false);
                                            }}
                                            className={clsx(
                                                'flex items-center justify-between px-3 py-3 rounded-lg transition-all duration-200 group text-left',
                                                isSelected ? 'bg-white/5 border border-white/10 shadow-inner' : 'hover:bg-white/5 hover:scale-[1.01]'
                                            )}
                                        >
                                            <div className="flex items-center gap-4 min-w-0">
                                                {brand.logoAsset && (
                                                    <div className={clsx('w-10 h-10 rounded-lg flex items-center justify-center shrink-0 border border-white/5', isSelected ? 'bg-white/10' : 'bg-white/[0.02]')}>
                                                        <img src={brand.logoAsset} alt={brand.altText} className="h-6 w-6 object-contain" loading="lazy" />
                                                    </div>
                                                )}
                                                <div className="flex flex-col min-w-0">
                                                    <div className="flex items-center gap-2">
                                                        <span className={clsx('text-sm font-black truncate', isSelected ? 'text-white' : 'text-white/70')}>
                                                            {brand.label}
                                                        </span>
                                                        {brand.recommended && (
                                                            <span className="text-[8px] bg-blue-500 text-white px-1.5 py-0.5 rounded font-black tracking-widest border border-blue-400/20">
                                                                TOP
                                                            </span>
                                                        )}
                                                    </div>
                                                    <div className="flex items-center gap-2 mt-1">
                                                        <span className="text-[10px] font-bold text-app-muted uppercase tracking-widest opacity-60">
                                                            {brand.shortLabel} · v{variant.version.split('-')[0]}
                                                        </span>
                                                        <span className={clsx('text-[8px] font-black uppercase tracking-tighter', typeof variant.source === 'string' ? 'text-emerald-400' : variant.source.source_type !== 'aur' ? 'text-emerald-400' : 'text-amber-500')}>
                                                            {typeof variant.source === 'string' ? 'Instant' : variant.source.source_type !== 'aur' ? 'Instant' : 'Compile'}
                                                        </span>
                                                    </div>
                                                    <div className="mt-1 text-[10px] text-app-muted line-clamp-1">{brand.hint}</div>
                                                </div>
                                            </div>
                                            {isSelected && <div className="w-2 h-2 rounded-full bg-blue-500 shadow-[0_0_8px_rgba(59,130,246,0.8)]" />}
                                        </button>
                                    );
                                });
                            })()}
                        </div>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
};

export default RepoSelector;
