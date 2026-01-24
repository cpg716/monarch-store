import React, { useState, useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronDown, Check, Zap, Package, Layers, Globe } from 'lucide-react';
import { clsx } from 'clsx';

interface RepoVariant {
    source: string;
    version: string;
}

interface RepoSelectorProps {
    variants: RepoVariant[];
    selectedSource: string;
    onChange: (source: string) => void;
}

const RepoSelector: React.FC<RepoSelectorProps> = ({ variants, selectedSource, onChange }) => {
    const [isOpen, setIsOpen] = useState(false);
    const containerRef = useRef<HTMLDivElement>(null);

    // Close on click outside
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

    const getIcon = (source: string) => {
        switch (source) {
            case 'chaotic': return <Zap size={14} className="text-yellow-500" />;
            case 'official': return <Package size={14} className="text-blue-500" />;
            case 'aur': return <Layers size={14} className="text-purple-500" />;
            case 'cachyos': return <Zap size={14} className="text-green-500" />;
            default: return <Globe size={14} className="text-app-muted" />;
        }
    };

    const getLabel = (source: string) => {
        const labels: Record<string, string> = {
            'chaotic': 'Chaotic-AUR (Prebuilt)',
            'official': 'Official Repository',
            'aur': 'AUR (Source Code)',
            'cachyos': 'CachyOS Optimized',
            'garuda': 'Garuda Linux',
            'endeavour': 'EndeavourOS',
            'manjaro': 'Manjaro Stable'
        };
        return labels[source] || source.charAt(0).toUpperCase() + source.slice(1);
    };

    return (
        <div className="relative w-full" ref={containerRef}>
            <button
                onClick={() => setIsOpen(!isOpen)}
                className={clsx(
                    "w-full flex items-center justify-between px-4 py-3 rounded-xl border transition-all text-left",
                    isOpen
                        ? "bg-app-card border-blue-500/50 ring-2 ring-blue-500/10"
                        : "bg-app-card/50 border-app-border hover:bg-app-card hover:border-app-fg/20"
                )}
            >
                <div className="flex items-center gap-3">
                    {selectedVariant && getIcon(selectedVariant.source)}
                    <div className="flex flex-col leading-none">
                        <span className="text-sm font-bold text-app-fg">
                            {selectedVariant ? getLabel(selectedVariant.source) : 'Select Source'}
                        </span>
                        {selectedVariant && (
                            <span className="text-[10px] text-app-muted font-mono mt-1">
                                v{selectedVariant.version}
                            </span>
                        )}
                    </div>
                </div>
                <motion.div
                    animate={{ rotate: isOpen ? 180 : 0 }}
                    transition={{ duration: 0.2 }}
                >
                    <ChevronDown size={16} className="text-app-muted" />
                </motion.div>
            </button>

            <AnimatePresence>
                {isOpen && (
                    <motion.div
                        initial={{ opacity: 0, y: 5, scale: 0.98 }}
                        animate={{ opacity: 1, y: 0, scale: 1 }}
                        exit={{ opacity: 0, y: 5, scale: 0.98 }}
                        transition={{ duration: 0.15 }}
                        className="absolute top-full left-0 right-0 mt-2 p-1 bg-app-card border border-app-border rounded-xl shadow-xl z-50 overflow-hidden"
                    >
                        <div className="flex flex-col gap-1 max-h-[300px] overflow-y-auto custom-scrollbar">
                            {variants.map(v => (
                                <button
                                    key={v.source}
                                    onClick={() => {
                                        onChange(v.source);
                                        setIsOpen(false);
                                    }}
                                    className={clsx(
                                        "flex items-center justify-between px-3 py-2.5 rounded-lg transition-colors group",
                                        selectedSource === v.source
                                            ? "bg-blue-500/10 text-blue-500"
                                            : "hover:bg-app-fg/5 text-app-fg"
                                    )}
                                >
                                    <div className="flex items-center gap-3">
                                        <div className="opacity-70 group-hover:opacity-100 transition-opacity">
                                            {getIcon(v.source)}
                                        </div>
                                        <div className="flex flex-col items-start leading-none">
                                            <span className={clsx("text-sm font-medium", selectedSource === v.source && "font-bold")}>
                                                {getLabel(v.source)}
                                            </span>
                                            <span className="text-[10px] text-app-muted font-mono mt-1 group-hover:text-app-fg/70 transition-colors">
                                                v{v.version}
                                            </span>
                                        </div>
                                    </div>
                                    {selectedSource === v.source && (
                                        <Check size={14} className="text-blue-500" />
                                    )}
                                </button>
                            ))}
                        </div>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
};

export default RepoSelector;
