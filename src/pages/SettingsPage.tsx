import React, { useState } from 'react';
import {
    Settings, Globe, Terminal,
    RefreshCw, Trash2, Key, Database, Info,
    ChevronRight, Moon, Sun, Monitor,
    Eye, Lock, AlertTriangle, Zap,
    Activity, HardDrive, Layout, Fingerprint
} from 'lucide-react';
import { clsx } from 'clsx';
import { useTheme } from '../hooks/useTheme';
import { useToast } from '../context/ToastContext';
import { useSettings } from '../hooks/useSettings';
import { useDistro } from '../hooks/useDistro';
import { useAppStore } from '../store/internal_store';
import { invoke } from '@tauri-apps/api/core';

// Internal Components
import SourcesTab from '../components/settings/SourcesTab';
import BuilderTab from '../components/settings/BuilderTab';
import ConfirmationModal from '../components/ConfirmationModal';

type TabId = 'general' | 'sources' | 'builder' | 'maintenance' | 'about';

interface SettingsPageProps {
    onRestartOnboarding?: () => void;
    onRepairComplete?: () => Promise<void>;
}

export default function SettingsPage({ onRestartOnboarding, onRepairComplete }: SettingsPageProps) {
    const [activeTab, setActiveTab] = useState<TabId>('general');
    const { themeMode, setThemeMode } = useTheme();
    const { success, error, show } = useToast();
    const { distro } = useDistro();
    const {
        telemetryEnabled, toggleTelemetry,
        // isAurEnabled, toggleAur, // These are in useSettings if needed
        // repoCounts,               // These are in useSettings if needed
        advancedMode, toggleAdvancedMode
    } = useSettings();
    const {
        reducePasswordPrompts, setReducePasswordPrompts,
    } = useAppStore();

    const handleClearCache = async () => {
        await invoke('clear_cache');
    };

    // UI State
    const [isRefreshingKeyring, setIsRefreshingKeyring] = useState(false);
    const [isCleaningCache, setIsCleaningCache] = useState(false);
    const [isRepairingLock, setIsRepairingLock] = useState(false);
    const [modalConfig, setModalConfig] = useState({ isOpen: false, title: '', message: '', variant: 'info' as 'info' | 'danger', onConfirm: () => { } });

    const pkgVersion = "0.4.0-alpha"; // Sync with Cargo.toml
    const installMode = 'system'; // Detected

    const tabs = [
        { id: 'general', label: 'General', icon: <Layout size={18} /> },
        { id: 'sources', label: 'Sources', icon: <Globe size={18} /> },
        { id: 'builder', label: 'Builder', icon: <Terminal size={18} /> },
        { id: 'maintenance', label: 'Maintenance', icon: <Zap size={18} /> },
        { id: 'about', label: 'About', icon: <Info size={18} /> },
    ];

    const handleRepairKeyring = async () => {
        setIsRefreshingKeyring(true);
        show("Initializing keyring repair sequence...");
        try {
            await invoke('fix_keyring_issues');
            success("Keyring issues resolved successfully.");
            if (onRepairComplete) await onRepairComplete();
        } catch (e) {
            error("Repair failed: " + String(e));
        } finally {
            setIsRefreshingKeyring(false);
        }
    };

    const handleUnlockPacman = async () => {
        setIsRepairingLock(true);
        try {
            await invoke('repair_unlock_pacman');
            success("Pacman database unlocked.");
            if (onRepairComplete) await onRepairComplete();
        } catch (e) {
            error("Unlock failed: " + String(e));
        } finally {
            setIsRepairingLock(false);
        }
    };

    return (
        <div className="flex flex-col h-screen bg-app-bg animate-in fade-in duration-500 overflow-hidden">
            {/* Header Area */}
            <div className="shrink-0 px-8 py-10 mt-6 border-b border-app-border bg-gradient-to-b from-white/50 to-transparent dark:from-white/[0.02]">
                <div className="max-w-6xl mx-auto flex flex-col md:flex-row md:items-center justify-between gap-6">
                    <div>
                        <div className="flex items-center gap-3 mb-2">
                            <div className="p-2 bg-blue-600 rounded-xl shadow-lg shadow-blue-600/20 text-white">
                                <Settings size={20} />
                            </div>
                            <h1 className="text-3xl font-black text-slate-900 dark:text-white tracking-tight">Mission Control</h1>
                        </div>
                        <p className="text-slate-500 dark:text-white/40 font-medium">Configure your MonARCH experience and system preferences.</p>
                    </div>

                    <div className="flex bg-slate-100 dark:bg-white/5 p-1 rounded-xl border border-slate-200 dark:border-white/10 overflow-x-auto no-scrollbar">
                        {tabs.map(tab => (
                            <button
                                key={tab.id}
                                onClick={() => setActiveTab(tab.id as TabId)}
                                className={clsx(
                                    "flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-bold transition-all whitespace-nowrap",
                                    activeTab === tab.id
                                        ? "bg-white dark:bg-white/10 text-blue-600 dark:text-white shadow-sm ring-1 ring-slate-200 dark:ring-white/20"
                                        : "text-slate-500 dark:text-white/40 hover:text-slate-700 dark:hover:text-white/60"
                                )}
                            >
                                {tab.icon}
                                {tab.label}
                            </button>
                        ))}
                    </div>
                </div>
            </div>

            {/* Scrollable Content Zone */}
            <div className="grow overflow-y-auto px-8 py-12 scroll-smooth">
                <div className="max-w-4xl mx-auto">
                    {activeTab === 'general' && (
                        <div className="space-y-10 animate-in fade-in slide-in-from-bottom-4 duration-300">
                            {/* Appearance */}
                            <section className="space-y-4">
                                <h2 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
                                    <Eye size={20} className="text-indigo-500" />
                                    Appearance
                                </h2>
                                <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                                    {[
                                        { id: 'light', label: 'Light', icon: <Sun size={20} /> },
                                        { id: 'dark', label: 'Dark', icon: <Moon size={20} /> },
                                        { id: 'system', label: 'System', icon: <Monitor size={20} /> }
                                    ].map(opt => (
                                        <button
                                            key={opt.id}
                                            onClick={() => setThemeMode(opt.id as any)}
                                            className={clsx(
                                                "flex flex-col items-center justify-center gap-3 p-6 rounded-2xl border transition-all duration-300",
                                                themeMode === opt.id
                                                    ? "bg-blue-600/5 border-blue-600 dark:bg-blue-500/10 dark:border-blue-400 text-blue-600 dark:text-blue-400 shadow-md ring-4 ring-blue-500/5"
                                                    : "bg-white dark:bg-white/5 border-app-border text-slate-500 dark:text-white/40 hover:bg-slate-50 dark:hover:bg-white/10"
                                            )}
                                        >
                                            <div className={clsx(
                                                "p-3 rounded-full transition-colors",
                                                themeMode === opt.id ? "bg-blue-600 text-white" : "bg-slate-100 dark:bg-white/10"
                                            )}>
                                                {opt.icon}
                                            </div>
                                            <span className="font-bold">{opt.label}</span>
                                        </button>
                                    ))}
                                </div>
                            </section>

                            <div className="h-px bg-slate-100 dark:bg-white/5 w-full" />

                            {/* Authentication */}
                            <section className="space-y-4">
                                <h2 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
                                    <Fingerprint size={20} className="text-green-500" />
                                    Security & Privacy
                                </h2>
                                <div className="space-y-4">
                                    <ToggleSetting
                                        icon={<Key size={20} className="text-amber-500" />}
                                        title="One-Click Authentication"
                                        description="Store password in volatile memory for the current session to reduce prompts."
                                        enabled={reducePasswordPrompts}
                                        onToggle={() => setReducePasswordPrompts(!reducePasswordPrompts)}
                                    />
                                    <ToggleSetting
                                        icon={<Activity size={20} className="text-indigo-500" />}
                                        title="Anonymous Telemetry"
                                        description="Help us improve MonARCH by sharing anonymous usage data and crash reports."
                                        enabled={telemetryEnabled}
                                        onToggle={() => toggleTelemetry(!telemetryEnabled)}
                                    />
                                </div>
                            </section>
                        </div>
                    )}

                    {activeTab === 'sources' && <SourcesTab />}

                    {activeTab === 'builder' && <BuilderTab />}

                    {activeTab === 'maintenance' && (
                        <div className="space-y-10 animate-in fade-in slide-in-from-bottom-4 duration-300">
                            <section className="space-y-4">
                                <h2 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
                                    <Zap size={20} className="text-yellow-500" />
                                    Critical Repairs
                                </h2>
                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    <RepairAction
                                        title="Repair Keyring"
                                        description="Fix GPG signature errors by refreshing master keys."
                                        icon={<Key className="text-blue-500" />}
                                        loading={isRefreshingKeyring}
                                        onClick={handleRepairKeyring}
                                    />
                                    <RepairAction
                                        title="Unlock Pacman"
                                        description="Force removal of stale db.lck files from aborted installs."
                                        icon={<Lock className="text-red-500" />}
                                        loading={isRepairingLock}
                                        onClick={handleUnlockPacman}
                                    />
                                </div>
                            </section>

                            <section className="space-y-4">
                                <h2 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
                                    <Database size={20} className="text-purple-500" />
                                    Data Management
                                </h2>
                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    <RepairAction
                                        title="Sync Databases"
                                        description="Force refresh of all repository package indices."
                                        icon={<RefreshCw className="text-emerald-500" />}
                                        onClick={async () => {
                                            success("Syncing package databases...");
                                            await invoke('sync_system_databases');
                                        }}
                                    />
                                    <RepairAction
                                        title="System Cleanup"
                                        description="Clear local metadata and chaotic-AUR caches."
                                        icon={<Trash2 className="text-orange-500" />}
                                        loading={isCleaningCache}
                                        onClick={async () => {
                                            setIsCleaningCache(true);
                                            try {
                                                await handleClearCache();
                                                success("Metatdata cache cleared.");
                                            } finally {
                                                setIsCleaningCache(false);
                                            }
                                        }}
                                    />
                                </div>
                            </section>

                            <section className="pt-6 border-t border-slate-100 dark:border-white/5">
                                <div className="bg-red-500/5 dark:bg-red-500/10 border border-red-500/20 rounded-2xl p-6 flex flex-col sm:flex-row items-center justify-between gap-6">
                                    <div className="flex gap-4">
                                        <div className="p-2 bg-red-500/10 rounded-lg text-red-600 dark:text-red-400 h-fit">
                                            <AlertTriangle size={24} />
                                        </div>
                                        <div>
                                            <h3 className="font-bold text-slate-900 dark:text-white">Advanced Repository Mode</h3>
                                            <p className="text-sm text-slate-500 dark:text-white/50 max-w-md mt-1">
                                                Enable \"God Mode\" to bypass safety checks. WARNING: Can cause system breakage on hybrid distros (Manjaro).
                                            </p>
                                        </div>
                                    </div>
                                    <button
                                        onClick={() => {
                                            if (!advancedMode) {
                                                setModalConfig({
                                                    isOpen: true,
                                                    title: "Enable Advanced Repository Mode?",
                                                    message: "⚠ CRITICAL WARNING ⚠\n\nYou are about to disable Distro-Safety Locks.\n\nOnly proceed if you are an expert capable of recovering a broken system.",
                                                    variant: 'danger',
                                                    onConfirm: () => {
                                                        toggleAdvancedMode(true);
                                                        success("Advanced Mode Enabled. Safety locks removed.");
                                                    }
                                                });
                                            } else {
                                                toggleAdvancedMode(false);
                                                success("Safety locks re-engaged.");
                                            }
                                        }}
                                        className={clsx(
                                            "px-6 py-2 rounded-xl font-bold transition-all",
                                            advancedMode
                                                ? "bg-red-600 text-white shadow-lg shadow-red-600/20"
                                                : "bg-slate-200 dark:bg-white/10 text-slate-600 dark:text-white/60 hover:bg-slate-300 dark:hover:bg-white/20"
                                        )}
                                    >
                                        {advancedMode ? "DEACTIVATE" : "ACTIVATE"}
                                    </button>
                                </div>
                            </section>
                        </div>
                    )}

                    {activeTab === 'about' && (
                        <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-300">
                            <div className="bg-white dark:bg-white/5 backdrop-blur-xl border border-app-border rounded-3xl p-10 flex flex-col items-center text-center space-y-6">
                                <div className="w-24 h-24 bg-blue-600 dark:bg-blue-500 rounded-3xl flex items-center justify-center shadow-2xl shadow-blue-500/30">
                                    <div className="w-14 h-14 border-8 border-white rounded-full flex items-center justify-center font-black text-white text-2xl">M</div>
                                </div>
                                <div>
                                    <h3 className="text-2xl font-black text-slate-900 dark:text-white tracking-tight">MonARCH Store</h3>
                                    <p className="text-slate-500 dark:text-white/40 font-medium">Operation Mission Control</p>
                                </div>
                                <div className="flex flex-wrap justify-center gap-3">
                                    <span className="px-4 py-1.5 bg-slate-100 dark:bg-white/10 text-slate-600 dark:text-white/60 text-xs font-mono font-bold rounded-full">v{pkgVersion}</span>
                                    <span className="px-4 py-1.5 bg-blue-100 dark:bg-blue-500/10 text-blue-700 dark:text-blue-400 text-xs font-bold rounded-full border border-blue-200 dark:border-blue-500/20">Production Alpha</span>
                                    <span className="px-4 py-1.5 bg-green-100 dark:bg-green-500/10 text-green-700 dark:text-green-400 text-xs font-bold rounded-full border border-green-200 dark:border-green-500/20">Arch-Native</span>
                                </div>
                                <p className="text-slate-600 dark:text-white/60 max-w-md leading-relaxed">
                                    The ultimate software management interface for Arch-based Linux distributions.
                                    Designed for performance, built for security, and tailored for you.
                                </p>
                                <div className="pt-2">
                                    <button
                                        onClick={onRestartOnboarding}
                                        className="px-6 py-2.5 bg-slate-100 dark:bg-white/5 hover:bg-slate-200 dark:hover:bg-white/10 text-slate-600 dark:text-white/60 text-sm font-bold rounded-xl transition-all border border-slate-200 dark:border-white/10"
                                    >
                                        Restart Onboarding Wizard
                                    </button>
                                </div>
                            </div>

                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                <AboutCard icon={<HardDrive size={20} />} title="Installation" value={installMode === 'system' ? 'Managed (Pacman)' : 'Standalone (AppImage)'} />
                                <AboutCard icon={<Activity size={20} />} title="Host Kernel" value={distro.pretty_name} />
                            </div>

                            <div className="text-center text-slate-400 dark:text-white/20 text-[10px] pb-8 pt-4">
                                Licensed under MIT License • Project MonARCH 2026
                            </div>
                        </div>
                    )}
                </div>
            </div>

            <ConfirmationModal
                isOpen={modalConfig.isOpen}
                onClose={() => setModalConfig({ ...modalConfig, isOpen: false })}
                onConfirm={modalConfig.onConfirm}
                title={modalConfig.title}
                message={modalConfig.message}
                variant={modalConfig.variant}
            />
        </div>
    );
}

function ToggleSetting({ icon, title, description, enabled, onToggle }: { icon: React.ReactNode, title: string, description: string, enabled: boolean, onToggle: () => void }) {
    return (
        <div className="flex items-center justify-between gap-6 p-6 bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-2xl hover:bg-app-card/80 dark:hover:bg-white/10 transition-all duration-300">
            <div className="flex gap-4">
                <div className="mt-1 p-2 bg-slate-100 dark:bg-white/5 rounded-xl h-fit">
                    {icon}
                </div>
                <div className="space-y-1">
                    <h3 className="font-bold text-slate-900 dark:text-white">{title}</h3>
                    <p className="text-sm text-slate-500 dark:text-white/50 max-w-md leading-relaxed">
                        {description}
                    </p>
                </div>
            </div>
            <button
                onClick={onToggle}
                className={clsx(
                    "relative w-14 h-8 rounded-full p-1 transition-all duration-300 focus:outline-none focus:ring-2 focus:ring-blue-500/50 shrink-0",
                    enabled ? "bg-blue-600 shadow-lg shadow-blue-600/20" : "bg-slate-200 dark:bg-white/10"
                )}
            >
                <div className={clsx(
                    "w-6 h-6 bg-white rounded-full transition-transform duration-300 shadow-sm",
                    enabled ? "translate-x-6" : "translate-x-0"
                )} />
            </button>
        </div>
    );
}

function RepairAction({ title, description, icon, onClick, loading }: { title: string, description: string, icon: React.ReactNode, onClick: () => void, loading?: boolean }) {
    return (
        <button
            onClick={onClick}
            disabled={loading}
            className="flex flex-col text-left p-6 bg-app-card/30 dark:bg-white/[0.03] border border-app-border rounded-2xl hover:bg-app-card/50 dark:hover:bg-white/10 hover:border-blue-500/30 transition-all group disabled:opacity-50"
        >
            <div className="flex items-center justify-between w-full mb-3">
                <div className="p-2 bg-slate-100 dark:bg-white/5 rounded-lg group-hover:scale-110 transition-transform">
                    {loading ? <RefreshCw size={18} className="animate-spin text-blue-500" /> : icon}
                </div>
                <ChevronRight size={16} className="text-slate-300 dark:text-white/10 group-hover:translate-x-1 transition-transform" />
            </div>
            <h4 className="font-bold text-slate-900 dark:text-white text-sm">{title}</h4>
            <p className="text-xs text-slate-500 dark:text-white/40 mt-1 leading-relaxed">{description}</p>
        </button>
    );
}

function AboutCard({ icon, title, value }: { icon: React.ReactNode, title: string, value: string }) {
    return (
        <div className="flex items-center gap-4 p-5 bg-app-card/50 dark:bg-white/5 border border-app-border rounded-2xl">
            <div className="p-2.5 bg-slate-100 dark:bg-white/10 text-slate-400 dark:text-white/40 rounded-xl">
                {icon}
            </div>
            <div>
                <div className="text-[10px] font-black uppercase tracking-widest text-slate-400 dark:text-white/20 mb-0.5">{title}</div>
                <div className="text-sm font-bold text-slate-700 dark:text-white/80">{value}</div>
            </div>
        </div>
    );
}
