import React, { useState, useEffect } from 'react';
import {
    Settings, Globe, Terminal,
    RefreshCw, Trash2, Key, Info, Lock,
    Moon, Sun, Monitor,
    Eye, Zap, AlertTriangle, Palette,
    Activity, HardDrive, Fingerprint,
    type LucideIcon,
} from 'lucide-react';
import { clsx } from 'clsx';
import { useTheme } from '../hooks/useTheme';
import { useToast } from '../context/ToastContext';
import { useSettings } from '../hooks/useSettings';
import { useDistro } from '../hooks/useDistro';
import { useSessionPassword } from '../context/useSessionPassword';
import { commands } from '../services/bindings';
import { unwrap } from '../utils/specta';

import SourcesTab from '../components/settings/SourcesTab';
import StorageTab from '../components/settings/StorageTab';
import BuilderTab from '../components/settings/BuilderTab';
import ConfirmationModal from '../components/ConfirmationModal';

interface SettingsPageProps {
    onRestartOnboarding?: () => void;
    onRepairComplete?: () => Promise<void>;
}

/** Section heading for the single-scroll settings layout (matches app glassmorphism). */
function SectionHeader({
    id,
    icon: Icon,
    iconClassName,
    title,
    description,
}: {
    id?: string;
    icon: LucideIcon;
    iconClassName?: string;
    title: string;
    description?: string;
}) {
    return (
        <header id={id} className="scroll-mt-24">
            <div className="flex items-center gap-3 mb-4">
                <div className={clsx('p-2 rounded-xl shrink-0', iconClassName ?? 'bg-blue-500/10 text-blue-600 dark:text-blue-400')}>
                    <Icon size={22} />
                </div>
                <div>
                    <h2 className="text-xl font-bold text-slate-900 dark:text-white tracking-tight">{title}</h2>
                    {description && (
                        <p className="text-sm text-slate-500 dark:text-white/50 mt-0.5">{description}</p>
                    )}
                </div>
            </div>
        </header>
    );
}

export default function SettingsPage({ onRestartOnboarding, onRepairComplete }: SettingsPageProps) {
    const {
        themeMode,
        setThemeMode,
        accentColor,
        setAccentColor,
        hostAccentColor,
        resolvedTheme,
        isFollowingSystemTheme,
    } = useTheme();
    const { success, error, show } = useToast();
    const { distro } = useDistro();
    const { requestSessionPassword } = useSessionPassword();
    const {
        telemetryEnabled,
        toggleTelemetry,
        advancedMode,
        toggleAdvancedMode,
        automaticHousekeepingEnabled,
        toggleAutomaticHousekeeping,
        performHousekeeping,
        oneClickEnabled,
        updateOneClick,
    } = useSettings();

    const [missingRequiredBins, setMissingRequiredBins] = useState<string[]>([]);
    const [isRefreshingKeyring, setIsRefreshingKeyring] = useState(false);
    const [isCleaningCache, setIsCleaningCache] = useState(false);
    const [isRepairingLock, setIsRepairingLock] = useState(false);
    const [modalConfig, setModalConfig] = useState({
        isOpen: false,
        title: '',
        message: '',
        variant: 'info' as 'info' | 'danger',
        onConfirm: () => { },
    });

    const pkgVersion = '0.4.8-alpha';
    const installMode = 'system';
    const ACCENT_PRESETS = ['#3b82f6', '#8b5cf6', '#10b981', '#f59e0b', '#ef4444'];

    useEffect(() => {
        commands.getMissingRequiredBins()
            .then(unwrap)
            .then(setMissingRequiredBins)
            .catch(() => setMissingRequiredBins([]));
    }, []);

    const handleClearCache = async () => {
        unwrap(await commands.clearMetadataCaches());
    };

    const handleRebuildMetadataIndex = async () => {
        unwrap(await commands.rebuildMetadataIndex());
    };

    const handleRepairKeyring = async () => {
        setIsRefreshingKeyring(true);
        show('Initializing keyring repair sequence...');
        try {
            const pwd = await requestSessionPassword();
            unwrap(await commands.fixKeyringIssues(pwd ?? null));
            success('Keyring issues resolved successfully.');
            if (onRepairComplete) await onRepairComplete();
        } catch (e) {
            error('Repair failed: ' + String(e));
        } finally {
            setIsRefreshingKeyring(false);
        }
    };

    const handleUnlockPacman = async () => {
        setIsRepairingLock(true);
        try {
            const pwd = await requestSessionPassword();
            unwrap(await commands.repairUnlockPacman(pwd ?? null));
            success('Pacman database unlocked.');
            if (onRepairComplete) await onRepairComplete();
        } catch (e) {
            error('Unlock failed: ' + String(e));
        } finally {
            setIsRepairingLock(false);
        }
    };

    return (
        <div className="h-full flex flex-col bg-app-bg animate-in fade-in duration-300 transition-colors">
            {/* Sticky header — same pattern as Updates / Installed */}
            <div className="shrink-0 px-4 sm:px-6 lg:px-8 py-6 pb-4 border-b border-black/5 dark:border-white/5 bg-app-bg/95 backdrop-blur-3xl z-10 transition-colors shadow-sm dark:shadow-2xl dark:shadow-black/20 sticky top-0">
                <div className="flex items-center gap-3">
                    <div className="p-2 rounded-2xl bg-blue-500/10 text-blue-500">
                        <Settings size={28} />
                    </div>
                    <div>
                        <h1 className="text-2xl sm:text-3xl lg:text-4xl font-black text-slate-900 dark:text-white tracking-tight leading-none">
                            Settings
                        </h1>
                        <p className="text-slate-500 dark:text-app-muted font-medium text-sm mt-1">
                            Configure trusted sources, workflow, privacy, maintenance, and appearance.
                        </p>
                        <p className="text-slate-500 dark:text-app-muted/80 text-xs mt-1">
                            These controls change how MonARCH discovers software and how it behaves during installs, updates, and maintenance.
                        </p>
                    </div>
                </div>
            </div>

            {/* Single scrollable content — no tabs */}
            <main className="flex-1 overflow-y-auto px-4 sm:px-6 lg:px-8 py-6 lg:py-8 custom-scrollbar min-h-0">
                <div className="max-w-4xl mx-auto space-y-12">
                    {/* --- Appearance --- */}
                    <section className="space-y-4">
                        <SectionHeader
                            id="appearance"
                            icon={Eye}
                            iconClassName="bg-indigo-500/10 text-indigo-600 dark:text-indigo-400"
                            title="Appearance"
                            description="Host-adaptive and user theme preferences"
                        />
                        <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                            {[
                                { id: 'light', label: 'Light', icon: <Sun size={20} /> },
                                { id: 'dark', label: 'Dark', icon: <Moon size={20} /> },
                                { id: 'system', label: 'System', icon: <Monitor size={20} /> },
                            ].map((opt) => (
                                <button
                                    key={opt.id}
                                    type="button"
                                    onClick={() => setThemeMode(opt.id as 'light' | 'dark' | 'system')}
                                    className={clsx(
                                        'flex flex-col items-center justify-center gap-3 p-6 rounded-2xl border transition-all duration-300',
                                        themeMode === opt.id
                                            ? 'bg-blue-600/5 border-blue-600 dark:bg-blue-500/10 dark:border-blue-400 text-blue-600 dark:text-blue-400 shadow-md ring-4 ring-blue-500/5'
                                            : 'bg-white dark:bg-app-card border-app-border text-slate-500 dark:text-white/40 hover:bg-slate-50 dark:hover:bg-white/10'
                                    )}
                                >
                                    <div
                                        className={clsx(
                                            'p-3 rounded-full transition-colors',
                                            themeMode === opt.id ? 'bg-blue-600 text-white' : 'bg-slate-100 dark:bg-white/10'
                                        )}
                                    >
                                        {opt.icon}
                                    </div>
                                    <span className="font-bold">{opt.label}</span>
                                </button>
                            ))}
                        </div>
                        <div className="rounded-2xl border border-app-border bg-app-card p-4 space-y-3">
                            <div className="flex items-center justify-between gap-3">
                                <div className="flex items-center gap-2 text-sm font-bold text-slate-900 dark:text-white">
                                    <Palette size={16} className="text-indigo-500" />
                                    Accent Color
                                </div>
                                <div className="text-[11px] text-slate-600 dark:text-app-muted">
                                    {isFollowingSystemTheme
                                        ? `System-managed (${resolvedTheme})`
                                        : `Manual (${resolvedTheme})`}
                                </div>
                            </div>
                            <div className="flex items-center gap-2">
                                {ACCENT_PRESETS.map((c) => (
                                    <button
                                        key={c}
                                        type="button"
                                        onClick={() => setAccentColor(c)}
                                        className={clsx(
                                            'w-8 h-8 rounded-full border-2 transition-transform',
                                            accentColor === c ? 'border-app-fg scale-110' : 'border-transparent hover:scale-105'
                                        )}
                                        style={{ backgroundColor: c }}
                                        title={`Set accent ${c}`}
                                        aria-label={`Set accent ${c}`}
                                    />
                                ))}
                                {hostAccentColor && (
                                    <button
                                        type="button"
                                        onClick={() => setAccentColor(hostAccentColor)}
                                        className="ml-2 px-2.5 py-1.5 rounded-lg border border-app-border text-[11px] font-bold text-slate-700 dark:text-white/80 hover:bg-app-subtle transition-colors"
                                    >
                                        Use Host Accent
                                    </button>
                                )}
                            </div>
                            <div className="text-[11px] text-slate-600 dark:text-app-muted flex items-center gap-2">
                                <span>Current:</span>
                                <span className="inline-block w-3 h-3 rounded-full border border-app-border" style={{ backgroundColor: accentColor }} />
                                <code>{accentColor}</code>
                                {hostAccentColor && (
                                    <>
                                        <span className="opacity-50">|</span>
                                        <span>Host:</span>
                                        <span className="inline-block w-3 h-3 rounded-full border border-app-border" style={{ backgroundColor: hostAccentColor }} />
                                        <code>{hostAccentColor}</code>
                                    </>
                                )}
                            </div>
                        </div>
                    </section>

                    {/* --- Security & Privacy --- */}
                    <section className="space-y-4">
                        <SectionHeader
                            id="security"
                            icon={Fingerprint}
                            iconClassName="bg-green-500/10 text-green-600 dark:text-green-400"
                            title="Security & Privacy"
                            description="Authentication and telemetry"
                        />
                        <div className="space-y-4">
                            {missingRequiredBins.length > 0 && (
                                <div className="rounded-xl border border-amber-500/40 bg-amber-500/10 p-4 flex items-start gap-3">
                                    <AlertTriangle size={20} className="text-amber-500 shrink-0 mt-0.5" />
                                    <div>
                                        <p className="font-medium text-slate-900 dark:text-white text-sm">Missing runtime: {missingRequiredBins.join(', ')}</p>
                                        <p className="text-xs text-slate-600 dark:text-slate-400 mt-1">AUR builds and system updates may not work. Install the missing packages (e.g. base-devel for git, pacman-contrib for checkupdates, polkit for pkexec).</p>
                                    </div>
                                </div>
                            )}
                            <ToggleSetting
                                icon={<Key size={20} className="text-amber-500" />}
                                title="One-Click Authentication"
                                description="Use MonARCH's in-app password prompt once per app session instead of the system auth dialog for every privileged action. The password stays in memory only and clears when MonARCH closes."
                                enabled={oneClickEnabled}
                                onToggle={() => updateOneClick(!oneClickEnabled)}
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

                    {/* --- Sources --- */}
                    <section className="space-y-4">
                        <SectionHeader
                            id="sources"
                            icon={Globe}
                            iconClassName="bg-sky-500/10 text-sky-600 dark:text-sky-400"
                            title="Sources"
                            description="Repositories and package sources"
                        />
                        <SourcesTab />
                    </section>

                    {/* --- Disk & Cache --- */}
                    <section className="space-y-4">
                        <SectionHeader
                            id="storage"
                            icon={HardDrive}
                            iconClassName="bg-purple-500/10 text-purple-600 dark:text-purple-400"
                            title="Disk & Cache"
                            description="Package cache and disk usage"
                        />
                        <StorageTab />
                    </section>

                    {/* --- Native AUR Engine --- */}
                    <section className="space-y-4">
                        <SectionHeader
                            id="builder"
                            icon={Terminal}
                            iconClassName="bg-amber-500/10 text-amber-600 dark:text-amber-400"
                            title="Native AUR Engine"
                            description="Build and compile settings"
                        />
                        <BuilderTab />
                    </section>

                    {/* --- Maintenance & Repair --- */}
                    <section className="space-y-4">
                        <SectionHeader
                            id="maintenance"
                            icon={Zap}
                            iconClassName="bg-yellow-500/10 text-yellow-600 dark:text-yellow-400"
                            title="Maintenance & Repair"
                            description="Keyring, lock, sync and cleanup"
                        />
                        <div className="space-y-6">
                            <ToggleSetting
                                icon={<RefreshCw size={20} className="text-yellow-500" />}
                                title="Automatic Housekeeping"
                                description="Automatically remove unused dependencies (orphans) and clean package cache after every installation or update."
                                enabled={automaticHousekeepingEnabled}
                                onToggle={() => toggleAutomaticHousekeeping(!automaticHousekeepingEnabled)}
                            />
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
                                    description="Force removal of stale db.lck from aborted installs."
                                    icon={<Lock className="text-red-500" />}
                                    loading={isRepairingLock}
                                    onClick={handleUnlockPacman}
                                />
                            </div>
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                <RepairAction
                                    title="Sync Databases"
                                    description="Force refresh of all repository package indices."
                                    icon={<RefreshCw className="text-emerald-500" />}
                                    onClick={async () => {
                                        success('Syncing package databases...');
                                        const pwd = await requestSessionPassword();
                                        unwrap(await commands.syncSystemDatabases(pwd ?? null));
                                    }}
                                />
                                <RepairAction
                                    title="Clear Metadata Caches"
                                    description="Clear in-memory metadata, search, and API caches only."
                                    icon={<Trash2 className="text-orange-500" />}
                                    loading={isCleaningCache}
                                    onClick={async () => {
                                        setIsCleaningCache(true);
                                        try {
                                            await handleClearCache();
                                            success('Metadata caches cleared.');
                                        } finally {
                                            setIsCleaningCache(false);
                                        }
                                    }}
                                />
                                <RepairAction
                                    title="Rebuild Metadata Index"
                                    description="Reinitialize local metadata without syncing repositories."
                                    icon={<RefreshCw className="text-sky-500" />}
                                    onClick={async () => {
                                        setIsCleaningCache(true);
                                        try {
                                            await handleRebuildMetadataIndex();
                                            success('Metadata index rebuilt.');
                                        } finally {
                                            setIsCleaningCache(false);
                                        }
                                    }}
                                />
                            </div>
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                <RepairAction
                                    title="Run Housekeeping"
                                    description="Manually remove orphans and refresh sync databases now."
                                    icon={<Zap className="text-yellow-500" />}
                                    onClick={async () => {
                                        try {
                                            success('Starting housekeeping sequence...');
                                            await performHousekeeping();
                                            success('Housekeeping complete.');
                                        } catch (e) {
                                            error('Housekeeping failed: ' + String(e));
                                        }
                                    }}
                                />
                            </div>
                            <div className="pt-4 border-t border-slate-100 dark:border-white/5">
                                <div className="bg-red-500/5 dark:bg-red-500/10 border border-red-500/20 rounded-2xl p-6 flex flex-col sm:flex-row items-center justify-between gap-6">
                                    <div className="flex gap-4">
                                        <div className="p-2 bg-red-500/10 rounded-lg text-red-600 dark:text-red-400 h-fit">
                                            <AlertTriangle size={24} />
                                        </div>
                                        <div>
                                            <h3 className="font-bold text-slate-900 dark:text-white">Expert Repo Overrides</h3>
                                            <p className="text-sm text-slate-500 dark:text-white/50 max-w-md mt-1">
                                                Enables high-risk distro compatibility overrides intended for experienced users. Default users should keep this off.
                                            </p>
                                        </div>
                                    </div>
                                    <button
                                        type="button"
                                        onClick={() => {
                                            if (!advancedMode) {
                                                setModalConfig({
                                                    isOpen: true,
                                                    title: 'Enable Expert Repo Overrides?',
                                                    message:
                                                        '⚠ CRITICAL WARNING ⚠\n\nThis enables high-risk repository overrides on blocked distros.\n\nOnly continue if you can recover dependency and pacman breakage manually.',
                                                    variant: 'danger',
                                                    onConfirm: () => {
                                                        toggleAdvancedMode(true);
                                                        success('Expert overrides enabled.');
                                                    },
                                                });
                                            } else {
                                                toggleAdvancedMode(false);
                                                success('Expert overrides disabled.');
                                            }
                                        }}
                                        className={clsx(
                                            'px-6 py-2 rounded-xl font-bold transition-all shrink-0',
                                            advancedMode
                                                ? 'bg-red-600 text-white shadow-lg shadow-red-600/20'
                                                : 'bg-slate-200 dark:bg-white/10 text-slate-600 dark:text-white/60 hover:bg-slate-300 dark:hover:bg-white/20'
                                        )}
                                    >
                                        {advancedMode ? 'DEACTIVATE' : 'ACTIVATE'}
                                    </button>
                                </div>
                            </div>
                        </div>
                    </section>

                    {/* --- About --- */}
                    <section className="space-y-6">
                        <SectionHeader
                            id="about"
                            icon={Info}
                            iconClassName="bg-blue-500/10 text-blue-600 dark:text-blue-400"
                            title="About MonARCH"
                            description="Version and system info"
                        />
                        <div className="bg-white dark:bg-app-card/50 backdrop-blur-xl border border-app-border rounded-3xl p-8 sm:p-10 flex flex-col items-center text-center space-y-6">
                            <div className="w-24 h-24 bg-blue-600 dark:bg-blue-500 rounded-3xl flex items-center justify-center shadow-2xl shadow-blue-500/30">
                                <div className="w-14 h-14 border-8 border-white rounded-full flex items-center justify-center font-black text-white text-2xl">
                                    M
                                </div>
                            </div>
                            <div>
                                <h3 className="text-2xl font-black text-slate-900 dark:text-white tracking-tight">MonARCH Store</h3>
                                <p className="text-slate-500 dark:text-white/40 font-medium">Distro-aware package management</p>
                            </div>
                            <div className="flex flex-wrap justify-center gap-3">
                                <span className="px-4 py-1.5 bg-slate-100 dark:bg-white/10 text-slate-600 dark:text-white/60 text-xs font-mono font-bold rounded-full">
                                    v{pkgVersion}
                                </span>
                                <span className="px-4 py-1.5 bg-blue-100 dark:bg-blue-500/10 text-blue-700 dark:text-blue-400 text-xs font-bold rounded-full border border-blue-200 dark:border-blue-500/20">
                                    Production Alpha
                                </span>
                                <span className="px-4 py-1.5 bg-green-100 dark:bg-green-500/10 text-green-700 dark:text-green-400 text-xs font-bold rounded-full border border-green-200 dark:border-green-500/20">
                                    Arch-Native
                                </span>
                            </div>
                            <p className="text-slate-600 dark:text-white/60 max-w-md leading-relaxed">
                                The ultimate software management interface for Arch-based Linux distributions. Designed for performance,
                                built for security, and tailored for you.
                            </p>
                            <div className="pt-2">
                                <button
                                    type="button"
                                    onClick={onRestartOnboarding}
                                    className="px-6 py-2.5 bg-slate-100 dark:bg-white/5 hover:bg-slate-200 dark:hover:bg-white/10 text-slate-600 dark:text-white/60 text-sm font-bold rounded-xl transition-all border border-slate-200 dark:border-white/10"
                                >
                                    Restart Onboarding Wizard
                                </button>
                            </div>
                        </div>
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <AboutCard
                                icon={<HardDrive size={20} />}
                                title="Installation"
                                value={installMode === 'system' ? 'Managed (Pacman)' : 'Standalone (AppImage)'}
                            />
                            <AboutCard icon={<Activity size={20} />} title="Host Kernel" value={distro.pretty_name} />
                        </div>
                        <div className="text-center text-slate-400 dark:text-white/20 text-[10px] pb-4 pt-2">
                            Licensed under MIT License • Project MonARCH 2026
                        </div>
                    </section>
                </div>
            </main >

            <ConfirmationModal
                isOpen={modalConfig.isOpen}
                onClose={() => setModalConfig({ ...modalConfig, isOpen: false })}
                onConfirm={modalConfig.onConfirm}
                title={modalConfig.title}
                message={modalConfig.message}
                variant={modalConfig.variant}
            />
        </div >
    );
}

function ToggleSetting({
    icon,
    title,
    description,
    enabled,
    onToggle,
}: {
    icon: React.ReactNode;
    title: string;
    description: string;
    enabled: boolean;
    onToggle: () => void;
}) {
    return (
        <div className="flex items-center justify-between gap-6 p-6 bg-app-card/50 dark:bg-white/5 backdrop-blur-md border border-app-border rounded-2xl hover:bg-app-card/80 dark:hover:bg-white/10 transition-all duration-300">
            <div className="flex gap-4">
                <div className="mt-1 p-2 bg-slate-100 dark:bg-white/5 rounded-xl h-fit">{icon}</div>
                <div className="space-y-1">
                    <h3 className="font-bold text-slate-900 dark:text-white">{title}</h3>
                    <p className="text-sm text-slate-500 dark:text-white/50 max-w-md leading-relaxed">{description}</p>
                </div>
            </div>
            <button
                type="button"
                onClick={onToggle}
                className={clsx(
                    'relative w-14 h-8 rounded-full p-1 transition-all duration-300 focus:outline-none focus:ring-2 focus:ring-blue-500/50 shrink-0',
                    enabled ? 'bg-blue-600 shadow-lg shadow-blue-600/20' : 'bg-slate-200 dark:bg-white/10'
                )}
            >
                <div
                    className={clsx(
                        'w-6 h-6 bg-white rounded-full transition-transform duration-300 shadow-sm',
                        enabled ? 'translate-x-6' : 'translate-x-0'
                    )}
                />
            </button>
        </div>
    );
}

function RepairAction({
    title,
    description,
    icon,
    onClick,
    loading,
}: {
    title: string;
    description: string;
    icon: React.ReactNode;
    onClick: () => void;
    loading?: boolean;
}) {
    return (
        <button
            type="button"
            onClick={onClick}
            disabled={loading}
            className="flex flex-col text-left p-6 bg-app-card/30 dark:bg-white/[0.03] border border-app-border rounded-2xl hover:bg-app-card/50 dark:hover:bg-white/10 hover:border-blue-500/30 transition-all group disabled:opacity-50"
        >
            <div className="flex items-center justify-between w-full mb-3">
                <div className="p-2 bg-slate-100 dark:bg-white/5 rounded-lg group-hover:scale-110 transition-transform">
                    {loading ? <RefreshCw size={18} className="animate-spin text-blue-500" /> : icon}
                </div>
            </div>
            <h4 className="font-bold text-slate-900 dark:text-white text-sm">{title}</h4>
            <p className="text-xs text-slate-500 dark:text-white/40 mt-1 leading-relaxed">{description}</p>
        </button>
    );
}

function AboutCard({ icon, title, value }: { icon: React.ReactNode; title: string; value: string }) {
    return (
        <div className="flex items-center gap-4 p-5 bg-app-card/50 dark:bg-white/5 border border-app-border rounded-2xl">
            <div className="p-2.5 bg-slate-100 dark:bg-white/10 text-slate-400 dark:text-white/40 rounded-xl">{icon}</div>
            <div>
                <div className="text-[10px] font-black uppercase tracking-widest text-slate-400 dark:text-white/20 mb-0.5">
                    {title}
                </div>
                <div className="text-sm font-bold text-slate-700 dark:text-white/80">{value}</div>
            </div>
        </div>
    );
}
