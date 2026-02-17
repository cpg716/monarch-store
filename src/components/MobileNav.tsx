import React from 'react';
import { LayoutGrid, Download, Settings, RefreshCw, Search, Heart, Newspaper } from 'lucide-react';
import { clsx } from 'clsx';
import { useAppStore } from '../store/internal_store';

interface MobileNavProps {
    activeTab: string;
    setActiveTab: (tab: string) => void;
}

const MobileNav: React.FC<MobileNavProps> = ({ activeTab, setActiveTab }) => {
    const pendingUpdates = useAppStore((s) => s.pendingUpdates);

    const tabs = [
        { id: 'explore', icon: LayoutGrid, label: 'Explore' },
        { id: 'search', icon: Search, label: 'Search' },
        { id: 'installed', icon: Download, label: 'Manage' },
        { id: 'updates', icon: RefreshCw, label: 'Updates', badge: pendingUpdates.total },
        { id: 'favorites', icon: Heart, label: 'Saved' },
        { id: 'news', icon: Newspaper, label: 'News' },
        { id: 'settings', icon: Settings, label: 'Menu' },
    ];

    return (
        <div className="md:hidden fixed bottom-0 left-0 right-0 z-[60] bg-app-sidebar/90 backdrop-blur-2xl border-t border-app-border px-2 pb-safe">
            <div className="flex items-center justify-around h-20">
                {tabs.map((tab) => (
                    <button
                        key={tab.id}
                        onClick={() => setActiveTab(tab.id)}
                        className={clsx(
                            "flex flex-col items-center justify-center gap-1.5 transition-all w-full relative",
                            activeTab === tab.id ? "text-app-accent" : "text-app-muted"
                        )}
                    >
                        <div className={clsx(
                            "p-2 rounded-xl transition-all",
                            activeTab === tab.id ? "bg-app-accent/10 scale-110" : "hover:bg-app-subtle/50"
                        )}>
                            <tab.icon size={20} strokeWidth={activeTab === tab.id ? 2.5 : 2} />
                        </div>
                        <span className="text-[10px] font-bold uppercase tracking-tighter">{tab.label}</span>

                        {tab.badge != null && tab.badge > 0 && (
                            <div className="absolute top-2 right-1/2 translate-x-4 w-4 h-4 bg-red-500 rounded-full border-2 border-app-sidebar flex items-center justify-center">
                                <span className="text-[8px] font-black text-white">{tab.badge}</span>
                            </div>
                        )}

                        {activeTab === tab.id && (
                            <div className="absolute -bottom-1 w-1 h-1 bg-app-accent rounded-full shadow-[0_0_8px_var(--app-accent)]" />
                        )}
                    </button>
                ))}
            </div>
        </div>
    );
};

export default MobileNav;
