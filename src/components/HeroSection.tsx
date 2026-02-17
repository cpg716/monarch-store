import React from 'react';
import { ShieldCheck, Rocket, Ship, Gamepad2, Layers, Cpu } from 'lucide-react';
import { motion } from 'framer-motion';
import { useDistro } from '../hooks/useDistro';
import { clsx } from 'clsx';
import logo from '../assets/logo.png';
import archLogo from '../assets/arch-logo.svg';

export default function HeroSection() {
    const { distro, loading } = useDistro();

    if (loading) return <div className="h-[120px] animate-pulse rounded-3xl bg-white/5 mx-6 mt-6 mb-8" />;

    // Helper logic for Distro Badge
    const getDistroBadge = () => {
        switch (distro.id) {
            case 'cachyos': return { icon: Rocket, label: 'CachyOS Optimized', color: 'text-emerald-400', border: 'border-emerald-500/20 bg-emerald-500/10' };
            case 'manjaro': return { icon: ShieldCheck, label: 'Manjaro Stability Guard', color: 'text-green-400', border: 'border-green-500/20 bg-green-500/10' };
            case 'garuda': return { icon: Gamepad2, label: 'Garuda Dr460nized', color: 'text-fuchsia-400', border: 'border-fuchsia-500/20 bg-fuchsia-500/10' };
            case 'endeavouros': return { icon: Ship, label: 'EndeavourOS Terminal', color: 'text-purple-400', border: 'border-purple-500/20 bg-purple-500/10' };
            default: return {
                icon: () => <img src={archLogo} className="w-4 h-4 object-contain brightness-0 invert" alt="Arch" />,
                label: 'Standard Arch System',
                color: 'text-blue-400',
                border: 'border-blue-500/20 bg-blue-500/10'
            };
        }
    };

    const badge = getDistroBadge();
    const BadgeIcon = badge.icon;

    // Map repo ids to colors and display names
    const getRepoLabel = (repo: string) => {
        const lower = repo.toLowerCase();
        if (lower.includes('cachyos')) return { label: 'CachyOS', color: 'text-emerald-400' };
        if (lower.includes('chaotic')) return { label: 'Chaotic', color: 'text-purple-400' };
        if (lower.includes('manjaro')) return { label: 'Manjaro', color: 'text-green-400' };
        if (lower === 'core' || lower === 'extra' || lower === 'community') return { label: 'Official', color: 'text-blue-400' };
        if (lower === 'aur') return { label: 'AUR', color: 'text-amber-400' };
        return { label: repo, color: 'text-app-muted' };
    };

    // Filter and unique repos for display
    const visibleRepos = Array.from(new Set(distro.active_repos.map(r => getRepoLabel(r).label)));
    if (!visibleRepos.includes('AUR')) visibleRepos.push('AUR'); // AUR is always available via our client

    return (
        <section className="relative px-6 pt-2 pb-4 flex flex-col items-center justify-center text-center z-10">
            {/* Background Glow */}
            <div className="absolute top-0 left-1/2 -translate-x-1/2 w-3/4 h-32 bg-blue-600/20 blur-[100px] rounded-full pointer-events-none" />

            {/* Logo Header - Composite Logo (Icon + Text) */}
            <motion.div
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ type: "spring", stiffness: 100, damping: 15 }}
                className="relative mb-0 group -mt-4"
            >
                <div className="absolute inset-[-10%] bg-blue-500/10 blur-3xl rounded-full opacity-0 group-hover:opacity-100 transition-opacity duration-1000" />
                <img
                    src={logo}
                    alt="MonARCH Store"
                    className="w-72 md:w-[420px] object-contain drop-shadow-[0_0_40px_rgba(59,130,246,0.3)] relative z-10"
                />
            </motion.div>


            {/* Tagline */}
            <motion.p
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                transition={{ delay: 0.1 }}
                className="text-lg text-app-muted font-medium mb-4 -mt-2"
            >
                Universal Arch Linux App Manager
            </motion.p>

            {/* Context Bar */}
            <motion.div
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ delay: 0.2 }}
                className="flex flex-col md:flex-row items-center gap-3 md:gap-6"
            >
                {/* Distro Badge (CachyOS Optimized / Chaotic-AUR value cue) */}
                <div className={clsx("badge-hover flex items-center gap-2 px-4 py-1.5 rounded-full border backdrop-blur-md shadow-lg", badge.border)}>
                    <BadgeIcon size={14} className={badge.color} />
                    <span className={clsx("text-xs font-bold uppercase tracking-wider", badge.color)}>
                        {badge.label}
                    </span>
                    {distro.cpu_tier !== 'v1' && (
                        <div className="flex items-center gap-1 pl-2 border-l border-white/10 ml-1">
                            <Cpu size={12} className="text-orange-400" />
                            <span className="text-[10px] text-orange-400 font-mono">{distro.cpu_tier}</span>
                        </div>
                    )}
                </div>

                {/* Separator (Hidden on mobile) */}
                <div className="hidden md:block w-1 h-1 rounded-full bg-white/20" />

                {/* Repo Access */}
                <div className="flex items-center gap-4 text-xs font-medium text-app-muted/80 px-4 py-1.5 rounded-full border border-slate-200 dark:border-white/5 bg-white/50 dark:bg-black/20 backdrop-blur-md shadow-sm dark:shadow-none">
                    <span className="flex items-center gap-1.5">
                        <Layers size={12} className="text-blue-400" /> Sources:
                    </span>
                    <span className="flex items-center gap-2">
                        {visibleRepos.map((label, i) => (
                            <React.Fragment key={label}>
                                <span className={clsx(
                                    label === 'AUR' ? 'text-amber-400/90' :
                                        label === 'Chaotic' ? 'text-purple-400/90' :
                                            label === 'CachyOS' ? 'text-emerald-400/90' :
                                                'text-slate-600 dark:text-white/70'
                                )}>
                                    {label}
                                </span>
                                {i < visibleRepos.length - 1 && <span className="w-0.5 h-3 bg-slate-300 dark:bg-white/10" />}
                            </React.Fragment>
                        ))}
                    </span>
                </div>
            </motion.div>
        </section>
    );
}
