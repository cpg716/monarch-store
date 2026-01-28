import React, { useState, useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronDown, Check, Zap, Globe, ShieldCheck, Hammer, Server } from 'lucide-react';
import { clsx } from 'clsx';

interface RepoVariant {
    source: string;
    version: string;
    repo_name?: string;
}

interface RepoSelectorProps {
    variants: RepoVariant[];
    selectedSource: string;
    onChange: (source: string) => void;
}

const RepoSelector: React.FC<RepoSelectorProps> = ({ variants, selectedSource, onChange }) => {
    const [isOpen, setIsOpen] = useState(false);
    const containerRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    const selectedVariant = variants.find(v => v.source === selectedSource);

    const getSourceInfo = (variant?: RepoVariant) => {
        if (!variant) return { label: 'Select Source', icon: Globe, color: 'text-app-muted', bg: 'bg-app-card' };

        const { source, repo_name } = variant;
        const isOptimized = repo_name?.includes('v3') || repo_name?.includes('v4') || repo_name?.includes('znver4');

        switch (source) {
            case 'chaotic':
                return { label: 'Chaotic-AUR', badge: 'CHAOTIC', icon: ShieldCheck, color: 'text-green-500', bg: 'bg-green-500/10 border-green-500/20', recommended: true };
            case 'cachyos':
                if (isOptimized) {
                    return { label: 'CachyOS (Optimized)', badge: 'OPTIMIZED', icon: Zap, color: 'text-purple-500', bg: 'bg-purple-500/10 border-purple-500/20' };
                }
                return { label: 'CachyOS', badge: 'CACHYOS', icon: Zap, color: 'text-purple-400', bg: 'bg-purple-500/10 border-purple-500/20' };
            case 'manjaro':
                return { label: 'Manjaro', badge: 'MANJARO', icon: ShieldCheck, color: 'text-teal-500', bg: 'bg-teal-500/10 border-teal-500/20' };
            case 'garuda':
                return { label: 'Garuda', badge: 'GARUDA', icon: Zap, color: 'text-orange-500', bg: 'bg-orange-500/10 border-orange-500/20' };
            case 'endeavour':
                return { label: 'EndeavourOS', badge: 'ENDEAVOUR', icon: Zap, color: 'text-purple-500', bg: 'bg-purple-500/10 border-purple-500/20' };
            case 'official':
                return { label: 'Official Arch', badge: 'OFFICIAL', icon: Server, color: 'text-blue-500', bg: 'bg-blue-500/10 border-blue-500/20', recommended: true };
            case 'aur':
                return { label: 'AUR (Source Build)', badge: 'AUR', icon: Hammer, color: 'text-amber-500', bg: 'bg-amber-500/10 border-amber-500/20' };
            default:
                return { label: source, badge: source.toUpperCase(), icon: Server, color: 'text-slate-600 dark:text-app-muted', bg: 'bg-slate-100 dark:bg-app-subtle border-slate-200 dark:border-app-border' };
        }
    };

    const info = getSourceInfo(selectedVariant);
    const Icon = info.icon;

    return (
        <div className="relative w-full" ref={containerRef}>
            <button
                onClick={() => setIsOpen(!isOpen)}
                className={clsx(
                    "w-full flex items-center justify-between px-4 py-3 rounded-xl border transition-all text-left shadow-sm dark:shadow-none",
                    info.bg,
                    isOpen ? "ring-2 ring-blue-500/10 border-blue-400/50" : "hover:brightness-105 border-slate-200 dark:border-white/5"
                )}
            >
                <div className="flex items-center gap-3">
                    <Icon size={18} className={info.color} />
                    <div className="flex flex-col leading-none">
                        <div className="flex items-center gap-2">
                            <span className={clsx("text-sm font-bold whitespace-nowrap", info.color)}>
                                {info.label}
                            </span>
                            {(info as any).recommended && (
                                <span className="bg-blue-500 text-white text-[10px] font-bold px-1.5 py-0.5 rounded shadow-sm">
                                    RECOMMENDED
                                </span>
                            )}
                        </div>
                        {selectedVariant && (
                            <span className="text-[10px] text-app-muted font-mono mt-1 opacity-70">
                                {info.badge} • v{selectedVariant.version}
                            </span>
                        )}
                    </div>
                </div>
                <motion.div
                    animate={{ rotate: isOpen ? 180 : 0 }}
                    transition={{ duration: 0.2 }}
                >
                    <ChevronDown size={16} className={clsx("opacity-50", info.color)} />
                </motion.div>
            </button>

            <AnimatePresence>
                {isOpen && (
                    <motion.div
                        initial={{ opacity: 0, y: 5, scale: 0.98 }}
                        animate={{ opacity: 1, y: 0, scale: 1 }}
                        exit={{ opacity: 0, y: 5, scale: 0.98 }}
                        transition={{ duration: 0.15 }}
                        className="absolute top-full left-0 mt-2 p-1 bg-app-card border border-app-border rounded-xl shadow-xl z-50 overflow-hidden min-w-full w-max max-w-[calc(100vw-2rem)]"
                    >
                        <div className="flex flex-col gap-1 max-h-[300px] overflow-y-auto custom-scrollbar">
                            {variants.map(v => {
                                const vInfo = getSourceInfo(v);
                                const VIcon = vInfo.icon;
                                const isSelected = selectedSource === v.source;
                                return (
                                    <button
                                        key={`${v.source}-${v.version}`}
                                        onClick={() => {
                                            onChange(v.source);
                                            setIsOpen(false);
                                        }}
                                        className={clsx(
                                            "flex items-center justify-between px-3 py-2.5 rounded-lg transition-colors group",
                                            isSelected ? "bg-blue-500/10 dark:bg-app-accent/10" : "hover:bg-slate-100 dark:hover:bg-white/5"
                                        )}
                                    >
                                        <div className="flex items-center gap-3">
                                            <VIcon size={16} className={vInfo.color} />
                                            <div className="flex flex-col items-start leading-none">
                                                <span className={clsx("text-sm font-medium whitespace-nowrap", isSelected ? "text-app-fg" : "text-app-muted")}>
                                                    {vInfo.label}
                                                </span>
                                                <div className="flex items-center gap-2 mt-1">
                                                    <span className="text-[10px] text-app-muted font-mono opacity-60">
                                                        {vInfo.badge} • v{v.version}
                                                    </span>
                                                    {(vInfo as any).recommended && (
                                                        <span className="text-[9px] bg-blue-500/10 text-blue-500 px-1.5 py-0.5 rounded font-bold border border-blue-500/20">
                                                            RECOMMENDED
                                                        </span>
                                                    )}
                                                    {v.source !== 'aur' ? (
                                                        <span className="text-[8px] bg-green-500/10 text-green-500 px-1 rounded font-bold">INSTANT</span>
                                                    ) : (
                                                        <span className="text-[8px] bg-amber-500/10 text-amber-500 px-1 rounded font-bold">SLOW BUILD</span>
                                                    )}
                                                </div>
                                            </div>
                                        </div>
                                        {isSelected && <Check size={14} className="text-app-accent" />}
                                    </button>
                                );
                            })}
                        </div>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
};

export default RepoSelector;
