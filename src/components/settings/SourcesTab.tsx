import React from 'react';
import { ShieldCheck, Info, Package, Terminal, Globe, AlertTriangle } from 'lucide-react';
import { clsx } from 'clsx';
import { useDistro } from '../../hooks/useDistro';
import { useSettings } from '../../hooks/useSettings';

export default function SourcesTab() {
    const { distro } = useDistro();
    const {
        isAurEnabled, toggleAur,
        isFlatpakEnabled, toggleFlatpak,
        repos, toggleRepo,
        repoCounts
    } = useSettings();

    const chaoticRepo = repos.find(r => r.name.toLowerCase() === 'chaotic-aur' || r.id === 'chaotic-aur');
    const isChaoticBlocked = distro.capabilities.chaotic_aur_support === 'blocked';

    const officialRepos = repos.filter(r =>
        ['core', 'extra', 'multilib', 'community'].includes(r.name.toLowerCase()) ||
        r.id === 'official-arch-linux'
    );

    return (
        <div className="space-y-8 animate-in fade-in slide-in-from-bottom-2 duration-300">
            {/* Section 1: Host System (Read-Only) */}
            <section className="bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-2xl p-6 shadow-sm dark:shadow-none">
                <div className="flex items-center gap-3 mb-6">
                    <div className="p-2 bg-blue-500/10 rounded-lg text-blue-600 dark:text-blue-400">
                        <ShieldCheck size={24} />
                    </div>
                    <div>
                        <h2 className="text-xl font-bold text-slate-900 dark:text-white">Host System</h2>
                        <p className="text-sm text-slate-500 dark:text-white/50">Base system configuration detected by MonARCH.</p>
                    </div>
                </div>

                <div className="flex flex-col md:flex-row items-start md:items-center justify-between gap-6 p-4 bg-slate-50 dark:bg-white/[0.02] rounded-xl border border-slate-100 dark:border-white/5">
                    <div className="flex items-center gap-4">
                        <div className="w-12 h-12 flex items-center justify-center bg-white dark:bg-white/10 rounded-xl shadow-sm border border-slate-200 dark:border-white/10 overflow-hidden">
                            {/* Distro-specific logic/icon could go here */}
                            <span className="text-2xl font-bold text-blue-600 dark:text-blue-400">{distro.pretty_name.charAt(0)}</span>
                        </div>
                        <div>
                            <div className="text-xs font-black uppercase tracking-widest text-blue-600 dark:text-blue-400 mb-0.5">Detected Identity</div>
                            <div className="text-lg font-bold text-slate-900 dark:text-white uppercase tracking-tight">{distro.pretty_name}</div>
                        </div>
                    </div>

                    <div className="flex flex-wrap gap-2">
                        {officialRepos.length > 0 ? (
                            officialRepos.flatMap(r => r.name.toLowerCase() === 'official arch linux' ? ['Core', 'Extra', 'Multilib'] : [r.name]).map(name => (
                                <span key={name} className="px-3 py-1 bg-green-500/10 text-green-600 dark:text-green-400 text-xs font-bold rounded-full border border-green-500/20 flex items-center gap-1.5">
                                    <div className="w-1.5 h-1.5 bg-green-500 rounded-full" />
                                    {name}
                                </span>
                            ))
                        ) : (
                            <span className="px-3 py-1 bg-green-500/10 text-green-600 dark:text-green-400 text-xs font-bold rounded-full border border-green-500/20 flex items-center gap-1.5">
                                <div className="w-1.5 h-1.5 bg-green-500 rounded-full" />
                                Official (Active)
                            </span>
                        )}
                    </div>
                </div>
            </section>

            {/* Section 2: Universal Extensions */}
            <section className="space-y-4">
                <h2 className="text-lg font-bold text-slate-900 dark:text-white px-1">Universal Extensions</h2>

                <div className="grid grid-cols-1 gap-4">
                    {/* Chaotic-AUR */}
                    <SourceToggle
                        title="Chaotic-AUR"
                        description="Pre-built community packages. Fast updates, no compiling required."
                        enabled={chaoticRepo?.enabled || false}
                        onToggle={() => chaoticRepo && toggleRepo(chaoticRepo.id)}
                        disabled={isChaoticBlocked}
                        tooltip={isChaoticBlocked ? "Not available on Manjaro due to stability risks." : undefined}
                        icon={<Globe size={20} className="text-purple-500" />}
                        count={repoCounts['chaotic-aur']}
                    />

                    {/* Flatpak */}
                    <SourceToggle
                        title="Flatpak Support"
                        description="Enable sandboxed applications from Flathub. Portable & distribution-agnostic."
                        enabled={isFlatpakEnabled}
                        onToggle={() => toggleFlatpak(!isFlatpakEnabled)}
                        icon={<Package size={20} className="text-sky-500" />}
                    />

                    {/* AUR */}
                    <SourceToggle
                        title="AUR Support"
                        description="Enable compiling community packages from the Arch User Repository."
                        enabled={isAurEnabled}
                        onToggle={() => toggleAur(!isAurEnabled)}
                        icon={<Terminal size={20} className="text-amber-500" />}
                    />
                </div>
            </section>
        </div>
    );
}

interface SourceToggleProps {
    title: string;
    description: string;
    enabled: boolean;
    onToggle: () => void;
    icon: React.ReactNode;
    disabled?: boolean;
    tooltip?: string;
    count?: number;
}

function SourceToggle({ title, description, enabled, onToggle, icon, disabled, tooltip, count }: SourceToggleProps) {
    return (
        <div className={clsx(
            "group relative flex flex-col sm:flex-row sm:items-center justify-between gap-4 p-6 bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-2xl transition-all duration-300",
            disabled ? "opacity-60 grayscale-[0.5]" : "hover:bg-app-card/80 dark:hover:bg-white/10 hover:border-blue-500/30"
        )}>
            <div className="flex gap-4">
                <div className="mt-1 p-2 bg-slate-100 dark:bg-white/5 rounded-xl h-fit">
                    {icon}
                </div>
                <div className="space-y-1">
                    <div className="flex items-center gap-2">
                        <h3 className="font-bold text-slate-900 dark:text-white">{title}</h3>
                        {count && (
                            <span className="text-[10px] px-1.5 py-0.5 bg-slate-100 dark:bg-white/10 text-slate-500 dark:text-white/40 rounded-md font-mono">{count.toLocaleString()}</span>
                        )}
                        {disabled && tooltip && (
                            <div className="relative group/tooltip">
                                <Info size={14} className="text-slate-400 dark:text-white/30" />
                                <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-3 py-1.5 bg-slate-900 text-white text-[10px] rounded-lg opacity-0 group-hover/tooltip:opacity-100 transition-opacity pointer-events-none w-48 text-center leading-tight">
                                    {tooltip}
                                </div>
                            </div>
                        )}
                    </div>
                    <p className="text-sm text-slate-500 dark:text-white/50 max-w-md leading-relaxed">
                        {description}
                    </p>
                </div>
            </div>

            <button
                onClick={onToggle}
                disabled={disabled}
                className={clsx(
                    "relative w-14 h-8 rounded-full p-1 transition-all duration-300 focus:outline-none focus:ring-2 focus:ring-blue-500/50 shrink-0",
                    enabled ? "bg-blue-600 shadow-lg shadow-blue-600/20" : "bg-slate-200 dark:bg-white/10",
                    disabled && "cursor-not-allowed opacity-50"
                )}
            >
                <div className={clsx(
                    "w-6 h-6 bg-white rounded-full transition-transform duration-300 shadow-sm",
                    enabled ? "translate-x-6" : "translate-x-0"
                )} />
            </button>

            {disabled && (
                <div className="absolute inset-x-0 -bottom-2 flex justify-center opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none">
                    <div className="px-3 py-1 bg-red-500/10 text-red-600 dark:text-red-400 text-[10px] font-bold rounded-full border border-red-500/20 flex items-center gap-1">
                        <AlertTriangle size={10} />
                        System Restricted
                    </div>
                </div>
            )}
        </div>
    );
}
