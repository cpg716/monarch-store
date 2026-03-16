import React, { useEffect } from 'react';
import { LayoutGrid, Download, Settings, RefreshCw, Search, Heart, ChevronLeft, ChevronRight, Newspaper, LucideIcon } from 'lucide-react';
import { clsx } from 'clsx';
import logoFull from '../assets/logo_full.png';
import archLogo from '../assets/arch-logo.png';
import { motion } from 'framer-motion';
import { useAppStore } from '../store/internal_store';
import { useDistro } from '../hooks/useDistro';

interface SidebarProps {
    activeTab: string;
    setActiveTab: (tab: string) => void;
}

interface SidebarTab {
    id: string;
    icon: LucideIcon;
    label: string;
    desc: string;
    badge?: number;
}

const Sidebar: React.FC<SidebarProps> = ({ activeTab, setActiveTab }) => {
    const isExpanded = useAppStore(s => s.isSidebarExpanded);
    const setSidebarExpanded = useAppStore(s => s.setSidebarExpanded);
    const { distro } = useDistro();

    // Initial Responsive check only (Iron Core: don't fight the user on every resize)
    useEffect(() => {
        if (window.innerWidth < 1280 && isExpanded) {
            setSidebarExpanded(false);
        }
    }, []); // Run once on mount

    const pendingUpdates = useAppStore((s) => s.pendingUpdates);

    const tabs: SidebarTab[] = [
        { id: 'explore', icon: LayoutGrid, label: 'Explore', desc: 'Browse curated collections' },
        { id: 'search', icon: Search, label: 'Search', desc: 'Find applications' },
        { id: 'installed', icon: Download, label: 'Installed', desc: 'Your installed apps' },
        { id: 'favorites', icon: Heart, label: 'Favorites', desc: 'Your saved apps' },
        { id: 'updates', icon: RefreshCw, label: 'Updates', desc: 'Check for system updates', badge: pendingUpdates.total },
        { id: 'news', icon: Newspaper, label: 'News', desc: 'Safety advisories' },
        { id: 'settings', icon: Settings, label: 'Settings', desc: 'System preferences' },
    ];

    return (
        <motion.div
            animate={{ width: isExpanded ? 280 : 88 }}
            transition={{ type: "spring", stiffness: 400, damping: 35 }}
            className="h-full bg-app-sidebar/40 backdrop-blur-3xl border-r border-app-border flex flex-col pt-8 pb-6 relative z-50 transition-colors duration-300 shrink-0 overflow-hidden"
        >
            {/* Logo Section - Iron Core Branding */}
            <div className={clsx(
                "flex items-center mb-12 transition-all duration-500 px-6",
                isExpanded ? "gap-4" : "justify-center px-0"
            )}>
                <div className="relative shrink-0">
                    <img
                        src={isExpanded ? logoFull : archLogo}
                        alt="MonARCH"
                        className={clsx(
                            "object-contain",
                            isExpanded
                                ? "h-10 w-10 rounded-lg"
                                : "h-10 w-10"
                        )}
                    />
                    {!isExpanded && pendingUpdates.total > 0 && (
                        <div className="absolute -top-1 -right-1 w-3 h-3 bg-red-500 rounded-full border-2 border-app-sidebar animate-pulse" />
                    )}
                </div>

                {isExpanded && (
                    <motion.div
                        initial={{ opacity: 0, x: -10 }}
                        animate={{ opacity: 1, x: 0 }}
                        className="flex flex-col min-w-0"
                    >
                        <span className="text-lg font-black tracking-tight text-app-fg leading-none">
                            MonARCH Store
                        </span>
                        <span className="text-[10px] font-semibold text-app-muted truncate">
                            {distro.pretty_name || 'Arch-based Linux'}
                        </span>
                    </motion.div>
                )}
            </div>

            {/* Navigation Tabs */}
            <nav className="flex-1 px-4 space-y-1.5 scrollbar-hide overflow-y-auto">
                {tabs.map((tab) => (
                    <div key={tab.id} className="relative group">
                        <button
                            onClick={() => setActiveTab(tab.id)}
                            className={clsx(
                                "w-full flex items-center rounded-2xl transition-all duration-300 relative group/btn overflow-hidden",
                                isExpanded ? "px-4 py-4 gap-4" : "p-4 justify-center",
                                activeTab === tab.id
                                    ? "bg-app-accent/10 text-app-accent shadow-sm"
                                    : "text-app-muted hover:text-app-fg hover:bg-app-subtle/50"
                            )}
                            aria-label={tab.label}
                        >
                            {/* Visual Background Glow for Active */}
                            {activeTab === tab.id && (
                                <motion.div
                                    layoutId="activeTabGlow"
                                    className="absolute inset-0 bg-gradient-to-r from-app-accent/5 via-transparent to-transparent opacity-50"
                                />
                            )}

                            <tab.icon size={22} strokeWidth={activeTab === tab.id ? 2.5 : 2} className={clsx(
                                "transition-transform group-hover/btn:scale-110 shrink-0",
                                activeTab === tab.id && "drop-shadow-[0_0_10px_var(--app-accent)]"
                            )} />

                            {/* Label */}
                            {isExpanded && (
                                <motion.div
                                    initial={{ opacity: 0, x: -5 }}
                                    animate={{ opacity: 1, x: 0 }}
                                    className="flex flex-col items-start min-w-0"
                                >
                                    <span className="text-sm font-bold whitespace-nowrap">{tab.label}</span>
                                    <span className="text-[10px] opacity-60 truncate w-full font-medium">{tab.desc}</span>
                                </motion.div>
                            )}

                            {/* Badge with source breakdown tooltip */}
                            {tab.badge != null && tab.badge > 0 && (
                                <div
                                    className={clsx(
                                        "bg-red-500 text-white rounded-full flex items-center justify-center font-black",
                                        isExpanded
                                            ? "px-2 py-0.5 text-[10px] min-w-[20px]"
                                            : "absolute top-2 right-2 w-4 h-4 text-[8px] border-2 border-app-sidebar"
                                    )}
                                    title={tab.id === 'updates' && pendingUpdates.total > 0
                                        ? [
                                            pendingUpdates.repo > 0 ? `${pendingUpdates.repo} Repo` : '',
                                            pendingUpdates.aur > 0 ? `${pendingUpdates.aur} AUR` : '',
                                            pendingUpdates.flatpak > 0 ? `${pendingUpdates.flatpak} Flatpak` : '',
                                        ].filter(Boolean).join(' · ')
                                        : undefined
                                    }
                                >
                                    {tab.badge}
                                </div>
                            )}

                            {/* Active Indicator Line */}
                            {activeTab === tab.id && (
                                <motion.div
                                    layoutId="activeTabStrip"
                                    className="absolute left-0 w-1 h-6 bg-app-accent rounded-r-full shadow-[0_0_8px_var(--app-accent)]"
                                />
                            )}
                        </button>

                        {/* Iron Core Tooltip (Only when collapsed) */}
                        {!isExpanded && (
                            <div className="absolute left-full ml-4 top-1/2 -translate-y-1/2 bg-app-card/90 border border-app-border px-4 py-3 rounded-2xl shadow-2xl opacity-0 -translate-x-2 group-hover:opacity-100 group-hover:translate-x-0 transition-all pointer-events-none w-max z-[100] backdrop-blur-2xl">
                                <div className="flex items-center gap-2 mb-1">
                                    <tab.icon size={16} className="text-app-accent" />
                                    <p className="text-sm font-black text-app-fg">{tab.label}</p>
                                </div>
                                <p className="text-[11px] text-app-muted leading-tight max-w-[160px] italic">{tab.desc}</p>
                            </div>
                        )}
                    </div>
                ))}
            </nav>

            {/* Bottom Utilities: Toggle */}
            <div className="px-4 mt-8">
                <button
                    onClick={() => {
                        setSidebarExpanded(!isExpanded);
                    }}
                    className={clsx(
                        "w-full flex items-center py-4 rounded-2xl transition-all border border-transparent shadow-sm overflow-hidden group",
                        isExpanded ? "px-4 gap-4 bg-app-subtle/30 hover:bg-app-subtle/50 hover:border-app-border/50" : "justify-center bg-app-subtle/20 hover:bg-app-accent/10 hover:border-app-accent/20"
                    )}
                    aria-label={isExpanded ? "Collapse sidebar" : "Expand sidebar"}
                >
                    {isExpanded ? (
                        <>
                            <ChevronLeft size={20} className="text-app-muted group-hover:-translate-x-1 transition-transform group-hover:text-app-accent" />
                            <div className="flex flex-col items-start">
                                <span className="text-xs font-black uppercase tracking-widest text-app-muted group-hover:text-app-fg">Collapse</span>
                                <span className="text-[9px] opacity-40 font-bold uppercase">Sidebar Interface</span>
                            </div>
                        </>
                    ) : (
                        <ChevronRight size={22} className="text-app-muted group-hover:translate-x-1 transition-transform group-hover:text-app-accent" />
                    )}
                </button>
            </div>
        </motion.div>
    );
};

export default Sidebar;
