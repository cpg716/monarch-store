import React from 'react';
import { LayoutGrid, Download, Settings, RefreshCw, Search, Heart } from 'lucide-react';
import { clsx } from 'clsx';
import { useAppStore } from '../store/internal_store';
import logoSmall from '../assets/logo_small.png';

interface SidebarProps {
    activeTab: string;
    setActiveTab: (tab: string) => void;
}

const Sidebar: React.FC<SidebarProps> = ({ activeTab, setActiveTab }) => {
    const { infraStats } = useAppStore();
    const tabs = [
        { id: 'search', icon: Search, label: 'Search', desc: 'Find apps quickly' },
        { id: 'explore', icon: LayoutGrid, label: 'Explore', desc: 'Browse categories' },
        { id: 'installed', icon: Download, label: 'Installed', desc: 'Manage your installed apps' },
        { id: 'favorites', icon: Heart, label: 'Favorites', desc: 'Your saved apps' },
        { id: 'updates', icon: RefreshCw, label: 'Updates', desc: 'Check for available updates' },
        { id: 'settings', icon: Settings, label: 'Settings', desc: 'Configure preferences' },
    ];

    return (
        <div className="w-20 bg-app-sidebar/80 backdrop-blur-xl border-r border-app-border flex flex-col items-center py-6 gap-6 h-full z-50 transition-colors duration-200">
            <div className="flex flex-col items-center mb-6 group cursor-default pt-2">
                <img src={logoSmall} alt="MonARCH Store" className="w-11 h-11 object-contain drop-shadow-[0_0_12px_rgba(255,255,255,0.3)] group-hover:scale-110 transition-transform duration-300" />
            </div>
            {
                tabs.map((tab) => (
                    <div key={tab.id} className="relative group flex items-center justify-center">
                        <button
                            onClick={() => setActiveTab(tab.id)}
                            className={clsx(
                                "p-3 rounded-xl transition-all duration-300 relative",
                                activeTab === tab.id
                                    ? "bg-app-subtle text-app-fg shadow-lg shadow-purple-500/10"
                                    : "text-app-muted hover:text-app-fg hover:bg-app-subtle"
                            )}
                        >
                            <tab.icon size={24} strokeWidth={activeTab === tab.id ? 2.5 : 2} />
                            {activeTab === tab.id && (
                                <div className="absolute inset-0 bg-blue-500/10 rounded-xl blur-md -z-10" />
                            )}
                        </button>

                        {/* Tooltip */}
                        <div className="absolute left-16 bg-app-card border border-app-border px-3 py-2 rounded-lg shadow-xl opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none w-max z-50 backdrop-blur-md">
                            <p className="text-sm font-bold text-app-fg">{tab.label}</p>
                            <p className="text-[10px] text-app-muted">{tab.desc}</p>
                        </div>
                    </div>
                ))
            }

            {/* Chaotic Status (Live) */}
            <div className="mt-auto flex flex-col items-center gap-2 group relative cursor-help mb-4">
                <div className="relative">
                    <div className={clsx(
                        "w-2.5 h-2.5 rounded-full shadow-[0_0_8px_rgba(34,197,94,0.6)] animate-pulse transition-colors",
                        infraStats ? "bg-green-500" : "bg-gray-500"
                    )} />
                    <div className={clsx(
                        "absolute inset-0 w-2.5 h-2.5 rounded-full animate-ping opacity-20 transition-colors",
                        infraStats ? "bg-green-500" : "bg-gray-500"
                    )} />
                </div>

                {/* Tooltip */}
                <div className="absolute left-16 bottom-0 bg-app-card border border-app-border px-4 py-3 rounded-xl shadow-2xl text-left opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50 w-52 backdrop-blur-xl">
                    <p className="text-xs font-bold text-green-500 mb-1 flex items-center justify-between">
                        <span>Chaotic-AUR</span>
                        <span className="px-1.5 py-0.5 bg-green-500/10 rounded text-[9px]">ONLINE</span>
                    </p>
                    <div className="space-y-1">
                        <div className="flex justify-between text-[10px] text-app-fg">
                            <span>Active Builders</span>
                            <span className="font-mono font-bold">{infraStats?.builders || 0}</span>
                        </div>
                        <div className="flex justify-between text-[10px] text-app-muted">
                            <span>Packages Served</span>
                            <span className="font-mono">3,198</span>
                        </div>
                    </div>
                </div>
            </div>

            {/* Sync Status - Bottom Indicator */}
            <div className="mb-6 flex flex-col items-center gap-2 group relative cursor-help">
                <div className="relative">
                    <div className="w-8 h-8 rounded-full bg-app-subtle border border-app-border flex items-center justify-center relative overflow-hidden group-hover:border-blue-500/30 transition-colors">
                        <RefreshCw size={14} className="text-blue-500 opacity-80" />
                    </div>
                </div>

                {/* Tooltip */}
                <div className="absolute left-16 bottom-0 bg-app-card border border-app-border p-3 rounded-xl shadow-2xl text-left opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50 w-56 backdrop-blur-xl">
                    <div className="flex items-center justify-between mb-1">
                        <p className="text-xs font-bold text-app-fg">Repositories Synced</p>
                        <span className="text-[10px] text-app-muted bg-app-subtle px-1.5 py-0.5 rounded">IDLE</span>
                    </div>
                    <p className="text-[10px] text-app-muted leading-snug">
                        Local cache is up to date with CachyOS, Garuda, and Official mirrors.
                    </p>
                </div>
            </div>
        </div >
    );
};

export default Sidebar;
