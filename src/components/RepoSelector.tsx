import React, { useState, useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronDown, Check, Zap, Globe, ShieldCheck, Hammer, Server } from 'lucide-react';
import { clsx } from 'clsx';

import { PackageSource } from '../services/bindings';
import { useDistro } from '../hooks/useDistro';
import { getSourceTierForSort, isSameSource } from '../utils/repoHelper';
import { getSourceKey } from '../utils/packageKey';

/** From pkg_name (e.g. firefox-nightly, signal-desktop-beta) return a short channel label for the selector, or empty. */
function channelSuffix(pkg_name?: string): string {
    if (!pkg_name) return '';
    const n = pkg_name.toLowerCase();
    if (n.includes('-nightly')) return ' · Nightly';
    if (n.includes('-beta')) return ' · Beta';
    if (n.includes('-developer-edition')) return ' · Developer Edition';
    if (n.includes('-esr')) return ' · ESR';
    if (n.includes('-canary')) return ' · Canary';
    if (n.includes('-ptb')) return ' · PTB';
    if (n.includes('-unstable') || n.includes('-edge')) return ' · Unstable';
    return '';
}

interface RepoVariant {
    source: PackageSource | string;
    version: string;
    repo_name?: string;
    /** AUR/repo package name (e.g. vlc vs vlc-git) so dropdown distinguishes multiple AUR entries */
    pkg_name?: string;
}

interface RepoSelectorProps {
    variants: RepoVariant[];
    selectedSource: PackageSource | string;
    onChange: (source: PackageSource | string) => void;
}

const RepoSelector: React.FC<RepoSelectorProps> = ({ variants, selectedSource, onChange }) => {
    const { distro } = useDistro();
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

    const selectedVariant = variants.find(v => isSameSource(v.source, selectedSource));

    const getSourceInfo = (variant?: RepoVariant): { label: string; badge: string; hint?: string; icon: typeof Server; color: string; bg: string; recommended?: boolean } => {
        if (!variant) return { label: 'Select Source', badge: '', icon: Globe, color: 'text-app-muted', bg: 'bg-app-card' };

        const { source, repo_name, pkg_name } = variant;
        const distroId = typeof distro.id === 'string' ? distro.id : (distro.id as any).Unknown ?? '';

        // --- STRUCT LOGIC (distro-aware labels) ---
        if (typeof source !== 'string') {
            const { source_type, id, label } = source;
            const idLower = id.toLowerCase();

            if (source_type === 'repo') {
                const ch = channelSuffix(pkg_name);
                // TIER 1: CachyOS (note when optimized repo: v3, v4, znver4, extra)
                if (idLower.startsWith('cachyos')) {
                    const isOptimized = idLower.includes('v3') || idLower.includes('v4') || idLower.includes('znver4') || idLower.includes('extra');
                    return {
                        label: (isOptimized ? 'CachyOS (Optimized)' : 'CachyOS') + ch,
                        badge: 'Optimized',
                        hint: 'Optimized for your CPU—fast and tested.',
                        icon: Zap,
                        color: 'text-cyan-500',
                        bg: 'bg-cyan-500/10 border-cyan-500/20',
                        recommended: distroId === 'cachyos',
                    };
                }
                const isArchOfficialRepo = ['core', 'extra', 'community', 'multilib', 'official'].includes(idLower);
                if (isArchOfficialRepo && distroId === 'manjaro') {
                    return { label: 'Manjaro (Official)' + ch, badge: 'Optimized', hint: 'Your system\'s official software—tested and secure.', icon: ShieldCheck, color: 'text-teal-500', bg: 'bg-teal-500/10 border-teal-500/20', recommended: true };
                }
                if (idLower === 'chaotic-aur') {
                    const isChaoticRecommended = distroId === 'garuda' || distroId === 'arch';
                    return { label: 'Chaotic' + ch, badge: 'Pre-Built', hint: 'Pre-built by the community—no compilation needed.', icon: ShieldCheck, color: 'text-purple-500', bg: 'bg-purple-500/10 border-purple-500/20', recommended: isChaoticRecommended };
                }
                if (isArchOfficialRepo) {
                    const isArchRecommended = distroId === 'arch' || distroId === 'endeavouros';
                    return { label: 'Arch (Official)' + ch, badge: 'OFFICIAL', hint: 'Your system\'s official software—tested and secure.', icon: Server, color: 'text-emerald-500', bg: 'bg-emerald-500/10 border-emerald-500/20', recommended: isArchRecommended };
                }
                if (idLower.includes('manjaro')) {
                    return { label: 'Manjaro' + ch, badge: 'MANJARO', hint: 'Software from Manjaro.', icon: ShieldCheck, color: 'text-teal-500', bg: 'bg-teal-500/10 border-teal-500/20' };
                }
                if (idLower.includes('garuda')) return { label: 'Garuda' + ch, badge: 'GARUDA', hint: 'Software from Garuda Linux.', icon: Zap, color: 'text-orange-500', bg: 'bg-orange-500/10 border-orange-500/20' };
                if (idLower.includes('endeavour')) return { label: 'EndeavourOS' + ch, badge: 'ENDEAVOUR', hint: 'Software from EndeavourOS.', icon: Zap, color: 'text-violet-500', bg: 'bg-violet-500/10 border-violet-500/20' };

                const isArchOfficialLabel = (label === 'Official' || label === 'Official Repository' || label === 'Arch Official');
                if (isArchOfficialLabel) {
                    const isArchRecommended = distroId === 'arch' || distroId === 'endeavouros';
                    return { label: 'Arch (Official)' + ch, badge: 'OFFICIAL', hint: 'Your system\'s official software—tested and secure.', icon: Server, color: 'text-emerald-500', bg: 'bg-emerald-500/10 border-emerald-500/20', recommended: isArchRecommended };
                }

                const isGenericRepo = label === 'Other repository' || label === 'Custom Repository';
                const safeId = (id && id.toLowerCase() !== 'unknown') ? id : '';
                const displayLabel = ((isGenericRepo && safeId) ? `Repository (${safeId})` : (label && label !== 'Unknown' ? label : safeId || 'Other repository')) + ch;
                return { label: displayLabel, badge: 'REPO', hint: safeId ? `From repository: ${safeId}` : undefined, icon: Server, color: 'text-slate-500', bg: 'bg-slate-500/10 border-slate-500/20' };
            }

            if (source_type === 'aur') {
                const aurLabel = pkg_name ? `AUR (${pkg_name})` : 'AUR';
                return { label: aurLabel, badge: 'AUR', hint: 'Community-built—may take a few minutes to install.', icon: Hammer, color: 'text-orange-500', bg: 'bg-orange-500/10 border-orange-500/20' };
            }
            if (source_type === 'flatpak') {
                const isBeta = idLower === 'flathub-beta';
                const flatpakCh = channelSuffix(pkg_name);
                const label = isBeta ? 'Flatpak (Beta)' : ('Flatpak' + (flatpakCh || ' (Stable)'));
                const hint = isBeta
                    ? 'Beta builds from Flathub—may be less stable. Runs in a sandbox.'
                    : 'Runs in a sandbox—works across many systems. Default branch is stable.';
                return { label, badge: isBeta ? 'Beta' : 'Universal', hint, icon: Globe, color: 'text-blue-500', bg: 'bg-blue-500/10 border-blue-500/20' };
            }
            if (source_type === 'local') {
                return { label: label || 'Local', badge: 'LOCAL', hint: 'Already installed on your system.', icon: Server, color: 'text-neutral-500', bg: 'bg-neutral-500/10 border-neutral-500/20' };
            }

            const fallbackLabel = (label && label !== 'Unknown') ? label : ((id && id.toLowerCase() !== 'unknown') ? id : 'Other source');
            return { label: fallbackLabel, badge: 'REPO', hint: id ? `From: ${id}` : undefined, icon: Server, color: 'text-slate-500', bg: 'bg-slate-100 border-slate-200' };
        }

        // --- LEGACY STRING LOGIC (distro-aware) ---
        const isOptimized = repo_name?.includes('v3') || repo_name?.includes('v4') || repo_name?.includes('znver4');
        const ch = channelSuffix(pkg_name);

        switch (source) {
            case 'chaotic':
                return { label: 'Chaotic' + ch, badge: 'Pre-Built', hint: 'Pre-built by the community—no compilation needed.', icon: ShieldCheck, color: 'text-purple-500', bg: 'bg-purple-500/10 border-purple-500/20', recommended: distroId === 'garuda' || distroId === 'arch' };
            case 'cachyos':
                return { label: (isOptimized ? 'CachyOS (Optimized)' : 'CachyOS') + ch, badge: 'Optimized', hint: 'Optimized for your CPU—fast and tested.', icon: Zap, color: 'text-cyan-500', bg: 'bg-cyan-500/10 border-cyan-500/20', recommended: distroId === 'cachyos' };
            case 'manjaro':
                return { label: 'Manjaro' + ch, badge: 'Optimized', hint: 'Your system\'s official software—tested and secure.', icon: ShieldCheck, color: 'text-teal-500', bg: 'bg-teal-500/10 border-teal-500/20', recommended: distroId === 'manjaro' };
            case 'garuda':
                return { label: 'Garuda' + ch, badge: 'GARUDA', hint: 'Software from Garuda Linux.', icon: Zap, color: 'text-orange-500', bg: 'bg-orange-500/10 border-orange-500/20' };
            case 'endeavour':
                return { label: 'EndeavourOS' + ch, badge: 'ENDEAVOUR', hint: 'Software from EndeavourOS.', icon: Zap, color: 'text-violet-500', bg: 'bg-violet-500/10 border-violet-500/20' };
            case 'official':
                const isOfficalRecommended = distroId === 'arch' || distroId === 'endeavouros' || distroId === 'manjaro';
                return { label: (distroId === 'manjaro' ? 'Manjaro (Official)' : 'Arch (Official)') + ch, badge: 'OFFICIAL', hint: 'Your system\'s official software—tested and secure.', icon: Server, color: distroId === 'manjaro' ? 'text-teal-500' : 'text-emerald-500', bg: distroId === 'manjaro' ? 'bg-teal-500/10 border-teal-500/20' : 'bg-emerald-500/10 border-emerald-500/20', recommended: isOfficalRecommended };
            case 'aur':
                return { label: 'AUR', badge: 'AUR', hint: 'Community-built—may take a few minutes to install.', icon: Hammer, color: 'text-orange-500', bg: 'bg-orange-500/10 border-orange-500/20' };
            case 'flatpak':
                return { label: 'Flatpak' + (channelSuffix(pkg_name) || ' (Stable)'), badge: 'Universal', hint: 'Runs in a sandbox—works across many systems. Default branch is stable.', icon: Globe, color: 'text-blue-500', bg: 'bg-blue-500/10 border-blue-500/20' };
            case 'local':
                return { label: 'Local', badge: 'LOCAL', hint: 'Already installed on your system.', icon: Server, color: 'text-neutral-500', bg: 'bg-neutral-500/10 border-neutral-500/20' };
            case 'unknown':
            case 'other':
                return { label: 'Other repository', badge: 'REPO', hint: 'From a configured repository.', icon: Server, color: 'text-slate-500', bg: 'bg-slate-500/10 border-slate-500/20' };
            default:
                return { label: source === 'Unknown' ? 'Other repository' : source, badge: (source === 'Unknown' ? 'REPO' : source).toUpperCase(), icon: Server, color: 'text-slate-600 dark:text-app-muted', bg: 'bg-slate-100 dark:bg-app-subtle border-slate-200 dark:border-app-border' };
        }
    };

    const info = getSourceInfo(selectedVariant);
    const Icon = info.icon;

    return (
        <div className="relative w-full min-w-0" ref={containerRef}>
            <button
                type="button"
                onClick={() => setIsOpen(!isOpen)}
                title={info.hint}
                className={clsx(
                    "w-full min-w-0 flex items-center justify-between gap-1.5 sm:gap-2 px-2.5 sm:px-3 md:px-4 py-2 sm:py-2.5 md:py-3 rounded-lg sm:rounded-xl border transition-all text-left shadow-sm dark:shadow-none",
                    info.bg,
                    isOpen ? "ring-2 ring-blue-500/10 border-blue-400/50" : "hover:brightness-105 border-slate-200 dark:border-white/5"
                )}
            >
                <div className="flex items-center gap-2 sm:gap-3 min-w-0 flex-1">
                    <Icon size={16} className={clsx("shrink-0 sm:w-[18px] sm:h-[18px]", info.color)} />
                    <div className="flex flex-col leading-none min-w-0 flex-1">
                        <div className="flex items-center gap-1.5 sm:gap-2 min-w-0">
                            <span className={clsx("text-xs sm:text-sm font-bold truncate", info.color)}>
                                {info.label}
                            </span>
                            {(info as any).recommended && (
                                <span className="bg-blue-500 text-white text-[9px] sm:text-[10px] font-bold px-1 sm:px-1.5 py-0.5 rounded shadow-sm shrink-0">
                                    RECOMMENDED
                                </span>
                            )}
                        </div>
                        {selectedVariant && (
                            <span className="text-[9px] sm:text-[10px] text-app-muted font-mono mt-0.5 sm:mt-1 opacity-70 truncate">
                                {info.badge} • v{selectedVariant.version}
                            </span>
                        )}
                    </div>
                </div>
                <motion.div
                    animate={{ rotate: isOpen ? 180 : 0 }}
                    transition={{ duration: 0.2 }}
                    className="shrink-0"
                >
                    <ChevronDown size={14} className={clsx("sm:w-4 sm:h-4 opacity-50", info.color)} />
                </motion.div>
            </button>

            <AnimatePresence>
                {isOpen && (
                    <motion.div
                        initial={{ opacity: 0, y: 5, scale: 0.98 }}
                        animate={{ opacity: 1, y: 0, scale: 1 }}
                        exit={{ opacity: 0, y: 5, scale: 0.98 }}
                        transition={{ duration: 0.15 }}
                        className="absolute top-full left-0 mt-2 p-1 bg-[#121212] border border-white/10 rounded-xl shadow-[0_20px_50px_rgba(0,0,0,0.5)] z-[110] overflow-hidden w-full min-w-[280px] ring-1 ring-white/5 backdrop-blur-xl"
                    >
                        <div className="flex flex-col gap-1 max-h-[350px] overflow-y-auto custom-scrollbar">
                            {(() => {
                                // Deduplicate variants by composite key to prevent React key collisions
                                const seen = new Set<string>();
                                const sorted = [...variants].sort((a, b) => {
                                    const dId = typeof distro.id === 'string' ? distro.id : (distro.id as any).Unknown ?? '';
                                    return getSourceTierForSort(b.source, dId) - getSourceTierForSort(a.source, dId);
                                });
                                return sorted.filter(v => {
                                    const key = `${getSourceKey(v.source)}-${String(v.pkg_name ?? '')}-${String(v.version ?? '')}`;
                                    if (seen.has(key)) return false;
                                    seen.add(key);
                                    return true;
                                }).map((v, idx) => {
                                    const vInfo = getSourceInfo(v);
                                    const VIcon = vInfo.icon;
                                    const isSelected = isSameSource(selectedSource, v.source);
                                    return (
                                        <button
                                            key={`${getSourceKey(v.source)}-${String(v.pkg_name ?? v.version ?? '')}-${String(v.version ?? '')}-${idx}`}
                                            onClick={() => {
                                                onChange(v.source);
                                                setIsOpen(false);
                                            }}
                                            className={clsx(
                                                "flex items-center justify-between px-3 py-3 rounded-lg transition-all duration-200 group text-left",
                                                isSelected ? "bg-white/5 border border-white/10 shadow-inner" : "hover:bg-white/5 hover:scale-[1.01]"
                                            )}
                                        >
                                            <div className="flex items-center gap-4 min-w-0">
                                                <div className={clsx("w-10 h-10 rounded-lg flex items-center justify-center shrink-0 border border-white/5", isSelected ? "bg-white/10" : "bg-white/[0.02]")}>
                                                    <VIcon size={18} className={vInfo.color} />
                                                </div>
                                                <div className="flex flex-col min-w-0">
                                                    <div className="flex items-center gap-2">
                                                        <span className={clsx("text-sm font-black truncate", isSelected ? "text-white" : "text-white/70")}>
                                                            {vInfo.label}
                                                        </span>
                                                        {(vInfo as any).recommended && (
                                                            <span className="text-[8px] bg-blue-500 text-white px-1.5 py-0.5 rounded font-black tracking-widest border border-blue-400/20">
                                                                TOP
                                                            </span>
                                                        )}
                                                    </div>
                                                    <div className="flex items-center gap-2 mt-1">
                                                        <span className="text-[10px] font-bold text-app-muted uppercase tracking-widest opacity-60">
                                                            {vInfo.badge} · v{v.version.split('-')[0]}
                                                        </span>
                                                        {typeof v.source === 'string' ? (
                                                            v.source !== 'aur' ? <span className="text-[8px] font-black text-emerald-400 uppercase tracking-tighter">Instant</span> : <span className="text-[8px] font-black text-amber-500 uppercase tracking-tighter">Compile</span>
                                                        ) : (
                                                            v.source.source_type !== 'aur' ? <span className="text-[8px] font-black text-emerald-400 uppercase tracking-tighter">Instant</span> : <span className="text-[8px] font-black text-amber-500 uppercase tracking-tighter">Compile</span>
                                                        )}
                                                    </div>
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
