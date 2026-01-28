import React, { useState, useEffect } from 'react';
import { LayoutGrid, Download, Settings, RefreshCw, Search, Heart, ChevronLeft, ChevronRight } from 'lucide-react';
import { clsx } from 'clsx';
import logoIcon from '../assets/logo.png';
import { motion } from 'framer-motion';

interface SidebarProps {
    activeTab: string;
    setActiveTab: (tab: string) => void;
}

const Sidebar: React.FC<SidebarProps> = ({ activeTab, setActiveTab }) => {
    const [isExpanded, setIsExpanded] = useState(() => {
        return localStorage.getItem('monarch_sidebar_expanded') === 'true';
    });

    useEffect(() => {
        localStorage.setItem('monarch_sidebar_expanded', String(isExpanded));
    }, [isExpanded]);

    // Responsive Auto-Collapse
    useEffect(() => {
        const handleResize = () => {
            if (window.innerWidth < 1024) {
                setIsExpanded(false);
            }
        };

        // Initial check
        handleResize();

        window.addEventListener('resize', handleResize);
        return () => window.removeEventListener('resize', handleResize);
    }, []);

    const [updateCount, setUpdateCount] = useState(0);

    useEffect(() => {
        // Check for updates to show notification badge
        import('@tauri-apps/api/core').then(({ invoke }) => {
            invoke('check_for_updates')
                .then((updates) => setUpdateCount((updates as any[]).length))
                .catch(() => { });
        });
    }, []);

    const tabs = [
        { id: 'search', icon: Search, label: 'Search', desc: 'Find apps quickly' },
        { id: 'explore', icon: LayoutGrid, label: 'Explore', desc: 'Browse categories' },
        { id: 'installed', icon: Download, label: 'Installed', desc: 'Manage your apps' },
        { id: 'favorites', icon: Heart, label: 'Favorites', desc: 'Your saved apps' },
        { id: 'updates', icon: RefreshCw, label: 'Updates', desc: 'Check for updates', badge: updateCount },
        { id: 'settings', icon: Settings, label: 'Settings', desc: 'Preferences' },
    ];

    return (
        <motion.div
            animate={{ width: isExpanded ? 260 : 80 }}
            transition={{ type: "spring", stiffness: 300, damping: 30 }}
            className="h-full bg-app-sidebar/80 backdrop-blur-3xl border-r border-app-border flex flex-col py-6 relative z-50 transition-colors duration-200"
        >
            {/* Logo Section */}
            <div className={clsx(
                "flex items-center mb-10 transition-all duration-300",
                isExpanded ? "px-6 gap-4" : "justify-center"
            )}>
                <img src={logoIcon} alt="MonARCH" className="w-10 h-10 object-contain drop-shadow-[0_0_12px_rgba(59,130,246,0.3)] animate-flap" />
                {isExpanded && (
                    <motion.div
                        initial={{ opacity: 0, x: -10 }}
                        animate={{ opacity: 1, x: 0 }}
                        className="flex flex-col"
                    >
                        <span className="text-lg font-black tracking-tighter text-app-fg leading-none">MonARCH</span>
                        <span className="text-[10px] font-bold text-blue-500 uppercase tracking-widest">Universal Arch Linux App Manager</span>
                    </motion.div>
                )}
            </div>

            {/* Navigation Tabs */}
            <div className="flex-1 px-3 space-y-2">
                {tabs.map((tab) => (
                    <div key={tab.id} className="relative group">
                        <button
                            onClick={() => setActiveTab(tab.id)}
                            className={clsx(
                                "w-full flex items-center rounded-2xl transition-all duration-300 relative group/btn",
                                isExpanded ? "px-4 py-3.5 gap-4" : "p-3.5 justify-center",
                                activeTab === tab.id
                                    ? "bg-blue-600/10 text-blue-500 shadow-sm"
                                    : "text-app-muted hover:text-app-fg hover:bg-app-subtle/50"
                            )}
                        >
                            <tab.icon size={22} strokeWidth={activeTab === tab.id ? 2.5 : 2} className={clsx(
                                "transition-transform group-hover/btn:scale-110",
                                activeTab === tab.id && "drop-shadow-[0_0_8px_rgba(59,130,246,0.5)]"
                            )} />

                            {/* Notification Badge */}
                            {(tab as any).badge > 0 && (
                                <div className={clsx(
                                    "absolute top-3 right-3 w-2.5 h-2.5 bg-red-500 rounded-full border border-app-sidebar shadow-sm",
                                    isExpanded && "top-4 right-4"
                                )} />
                            )}

                            {isExpanded && (
                                <motion.div
                                    initial={{ opacity: 0 }}
                                    animate={{ opacity: 1 }}
                                    className="flex flex-col items-start overflow-hidden"
                                >
                                    <span className="text-sm font-bold whitespace-nowrap">{tab.label}</span>
                                </motion.div>
                            )}

                            {activeTab === tab.id && (
                                <motion.div
                                    layoutId="activeTabGlow"
                                    className="absolute inset-0 bg-blue-500/5 rounded-2xl blur-lg -z-10"
                                />
                            )}

                            {/* Active Indicator Strip */}
                            {activeTab === tab.id && (
                                <motion.div
                                    layoutId="activeTabStrip"
                                    className="absolute left-0 w-1 h-6 bg-blue-500 rounded-r-full"
                                />
                            )}
                        </button>

                        {/* Floating Tooltip (Only when collapsed) */}
                        {!isExpanded && (
                            <div className="absolute left-full ml-4 top-1/2 -translate-y-1/2 bg-app-card border border-app-border px-3 py-2 rounded-xl shadow-2xl opacity-0 translate-x-2 group-hover:opacity-100 group-hover:translate-x-0 transition-all pointer-events-none w-max z-[100] backdrop-blur-xl">
                                <p className="text-sm font-bold text-app-fg">{tab.label}</p>
                                <p className="text-[10px] text-app-muted whitespace-nowrap">{tab.desc}</p>
                            </div>
                        )}
                    </div>
                ))}
            </div>

            {/* Bottom Section: Infrastructure & Toggle */}
            <div className="px-3 space-y-4">

                {/* Sidebar Toggle Button */}
                <button
                    onClick={() => setIsExpanded(!isExpanded)}
                    className="w-full flex items-center justify-center p-3 rounded-2xl text-app-muted hover:text-app-fg hover:bg-app-subtle/50 border border-transparent hover:border-app-border transition-all group"
                >
                    {isExpanded ? (
                        <div className="flex items-center gap-2">
                            <ChevronLeft size={18} className="group-hover:-translate-x-1 transition-transform" />
                            <span className="text-xs font-bold uppercase tracking-widest">Collapse</span>
                        </div>
                    ) : (
                        <ChevronRight size={18} className="group-hover:translate-x-1 transition-transform" />
                    )}
                </button>
            </div>
        </motion.div>
    );
};

export default Sidebar;
