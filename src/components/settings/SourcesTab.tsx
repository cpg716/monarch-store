import React, { useState, useEffect } from 'react';
import { ShieldCheck, Info, Package, Terminal, Globe, AlertTriangle, Copy, RefreshCw, Loader2, X } from 'lucide-react';
import { clsx } from 'clsx';
import { commands } from '../../services/bindings';
import { unwrap } from '../../utils/specta';
import { API, ChaoticStatus } from '../../services/api';
import { useDistro } from '../../hooks/useDistro';
import { useSettings } from '../../hooks/useSettings';
import { useSessionPassword } from '../../context/useSessionPassword';
import { useErrorService } from '../../context/ErrorContext';

const CHAOTIC_PACMAN_CONF_SNIPPET = `[chaotic-aur]
Include = /etc/pacman.d/chaotic-mirrorlist`;



export default function SourcesTab() {
    const { distro } = useDistro();
    const {
        isAurEnabled, toggleAur,
        isFlatpakEnabled, toggleFlatpak,
        repos, toggleRepo,
        repoCounts,
        refresh
    } = useSettings();
    const { requestSessionPassword } = useSessionPassword();
    const errorService = useErrorService();

    const [chaoticStatus, setChaoticStatus] = useState<ChaoticStatus | null>(null);
    const [loadingChaoticStatus, setLoadingChaoticStatus] = useState(true);
    const [preparingChaotic, setPreparingChaotic] = useState(false);
    const [showChaoticFinalModal, setShowChaoticFinalModal] = useState(false);
    const [chaoticError, setChaoticError] = useState<string | null>(null);
    const [checkingAgain, setCheckingAgain] = useState(false);
    const [copied, setCopied] = useState(false);

    const chaoticRepo = repos.find(r => r.name.toLowerCase() === 'chaotic-aur' || r.id === 'chaotic-aur');
    const isChaoticBlocked = chaoticStatus ? !chaoticStatus.compatible : distro.capabilities.chaotic_aur_support === 'blocked';
    const chaoticActive = chaoticStatus ? (chaoticStatus.compatible && chaoticStatus.chaotic_in_alpm) : (chaoticRepo?.enabled ?? false);
    const chaoticInactive = chaoticStatus ? (chaoticStatus.compatible && !chaoticStatus.chaotic_in_alpm) : !chaoticRepo?.enabled && !isChaoticBlocked;
    const chaoticSupportedByDistro = chaoticActive && distro.capabilities.chaotic_aur_support === 'native';
    const chaoticPreConfigured = chaoticActive && distro.capabilities.chaotic_aur_support !== 'native';

    useEffect(() => {
        let cancelled = false;
        setLoadingChaoticStatus(true);
        API.system.checkChaoticStatus()
            .then((s) => { if (!cancelled) setChaoticStatus(s); })
            .catch((e) => { if (!cancelled) errorService.reportError(e); })
            .finally(() => { if (!cancelled) setLoadingChaoticStatus(false); });
        return () => { cancelled = true; };
    }, []);

    const fetchChaoticStatus = async () => {
        try {
            const s = await API.system.checkChaoticStatus();
            setChaoticStatus(s);
            return s;
        } catch (e) {
            errorService.reportError(e as Error | string);
            return null;
        }
    };

    const handleChaoticToggle = async () => {
        if (chaoticActive && chaoticRepo) {
            toggleRepo(chaoticRepo.id);
            return;
        }
        if (chaoticInactive) {
            setChaoticError(null);
            setPreparingChaotic(true);
            try {
                await commands.openChaoticTerminal().then(unwrap);
                setShowChaoticFinalModal(true);
            } catch (e) {
                errorService.reportError(e as Error | string);
                setChaoticError(String(e));
            } finally {
                setPreparingChaotic(false);
            }
        }
    };

    const handleCheckAgain = async () => {
        setCheckingAgain(true);
        try {
            const s = await fetchChaoticStatus();
            if (s?.chaotic_in_alpm) {
                setShowChaoticFinalModal(false);
                refresh?.();
            }
        } finally {
            setCheckingAgain(false);
        }
    };

    const copySnippet = async () => {
        try {
            await navigator.clipboard.writeText(CHAOTIC_PACMAN_CONF_SNIPPET);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        } catch {
            setChaoticError('Copy failed');
        }
    };

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
                            <span className="text-2xl font-bold text-blue-600 dark:text-blue-400">{distro.pretty_name.charAt(0)}</span>
                        </div>
                        <div>
                            <div className="text-xs font-black uppercase tracking-widest text-blue-600 dark:text-blue-400 mb-0.5">Detected Identity</div>
                            <div className="text-lg font-bold text-slate-900 dark:text-white uppercase tracking-tight">{distro.pretty_name}</div>
                        </div>
                    </div>

                    <div className="flex flex-wrap gap-2">
                        {officialRepos.length > 0 ? (
                            officialRepos.flatMap(r => r.name.toLowerCase() === 'official arch linux' ? ['Core', 'Extra', 'Multilib'] : [r.name]).map((name, i) => (
                                <span key={typeof name === 'string' ? name : `repo-${i}`} className="px-3 py-1 bg-green-500/10 text-green-600 dark:text-green-400 text-xs font-bold rounded-full border border-green-500/20 flex items-center gap-1.5">
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
                <h2 className="text-lg font-bold text-slate-900 dark:text-white px-1">Package Sources</h2>
                <p className="text-sm text-slate-600 dark:text-slate-400 px-1 max-w-xl">
                    Control which sources appear in Search, Trending, and Browse. When a source is <strong>off</strong>, already-installed packages from that source still receive updates; they just won&apos;t be shown in discovery.
                </p>

                <div className="grid grid-cols-1 gap-4">
                    {/* Chaotic-AUR: Traffic light (Active=green, Inactive=gray, Blocked=red) */}
                    <SourceToggle
                        title="Chaotic-AUR"
                        description={
                            isChaoticBlocked ? "Not available on this distro (Manjaro)." :
                                chaoticActive ? "Community-prebuilt binaries. High speed, no local compilation. Recommended for saving time." :
                                    "Pre-built community packages. One-time setup: initialize MonARCH polkit policy, install keys, then enable."
                        }
                        enabled={chaoticActive}
                        onToggle={handleChaoticToggle}
                        disabled={isChaoticBlocked}
                        tooltip={isChaoticBlocked ? "Not available on Manjaro due to fixed-branch stability risks." : undefined}
                        icon={<Globe size={20} className="text-purple-500" />}
                        count={repoCounts['chaotic-aur']}
                        loading={loadingChaoticStatus || preparingChaotic}
                        statusBadge={
                            isChaoticBlocked ? 'blocked' :
                                chaoticActive ? 'active' : 'inactive'
                        }
                        notice={chaoticSupportedByDistro ? 'Supported by your distro' : chaoticPreConfigured ? 'Pre-configured in pacman.conf' : undefined}
                        inlineError={chaoticError}
                        warning={!isChaoticBlocked && chaoticActive ? "Disabling hides binaries from search; updates for installed packages still run." : "Setup involves automated keyring installation."}
                    />

                    {/* Flatpak */}
                    <SourceToggle
                        title="Flatpak Support"
                        description="Universal sandboxed apps from Flathub. Distribution-agnostic and containerized. Excellent for proprietary or complex apps."
                        enabled={isFlatpakEnabled}
                        onToggle={() => toggleFlatpak(!isFlatpakEnabled)}
                        icon={<Package size={20} className="text-sky-500" />}
                        statusBadge={isFlatpakEnabled ? 'active' : 'inactive'}
                        notice={distro.id === 'cachyos' || distro.id === 'endeavouros' ? 'Native support detected' : undefined}
                        warning={isFlatpakEnabled ? "Apps run in a sandbox; some system access may require Permission Manager." : "Flathub will be automatically added as a remote."}
                    />

                    {/* AUR */}
                    <SourceToggle
                        title="AUR Support"
                        description="Arch User Repository (makepkg). Community build scripts. Most packages are compiled locally on your machine."
                        enabled={isAurEnabled}
                        onToggle={() => toggleAur(!isAurEnabled)}
                        icon={<Terminal size={20} className="text-amber-500" />}
                        statusBadge={isAurEnabled ? 'active' : 'inactive'}
                        warning={isAurEnabled ? "CAUTION: Built locally. Review PKGBUILDs. Build tools (base-devel, git) required." : "Packages are unsupported by Arch developers; use with discretion."}
                        notice="Builds require base-devel"
                    />
                </div>
            </section>

            {/* Final Step Modal: Add [chaotic-aur] to pacman.conf */}
            {showChaoticFinalModal && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4" role="dialog" aria-modal="true" aria-labelledby="chaotic-final-title">
                    <div className="bg-app-card dark:bg-slate-900 border border-app-border rounded-2xl shadow-xl max-w-md w-full p-6 space-y-4">
                        <div className="flex items-center justify-between">
                            <h2 id="chaotic-final-title" className="text-lg font-bold text-slate-900 dark:text-white">Setup in Progress</h2>
                            <button type="button" onClick={() => setShowChaoticFinalModal(false)} className="p-1 rounded-lg hover:bg-slate-200 dark:hover:bg-white/10 text-slate-500">
                                <X size={20} />
                            </button>
                        </div>
                        <p className="text-sm text-slate-600 dark:text-slate-300">
                            A terminal window has been launched with the setup script.
                        </p>
                        <div className="p-4 bg-slate-100 dark:bg-slate-800 rounded-xl space-y-3">
                            <div className="flex gap-3 items-center text-sm font-medium text-slate-900 dark:text-white"><Terminal size={18} /> <span>Steps:</span></div>
                            <ol className="list-decimal list-inside text-xs text-slate-600 dark:text-slate-400 space-y-1 pl-1">
                                <li>Follow the prompts in the terminal.</li>
                                <li>Enter your password (sudo) if asked.</li>
                                <li>Wait for "Success" message.</li>
                            </ol>
                        </div>
                        <div className="flex flex-wrap gap-2">
                            <button
                                type="button"
                                onClick={handleCheckAgain}
                                disabled={checkingAgain}
                                className="w-full inline-flex justify-center items-center gap-2 px-4 py-3 rounded-xl bg-blue-600 text-white font-medium hover:bg-blue-700 focus:ring-2 focus:ring-blue-500/50 disabled:opacity-50 transition-all"
                            >
                                {checkingAgain ? <Loader2 size={18} className="animate-spin" /> : <RefreshCw size={18} />}
                                Check Connection
                            </button>
                        </div>
                    </div>
                </div>
            )}
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
    loading?: boolean;
    statusBadge?: 'active' | 'inactive' | 'blocked';
    inlineError?: string | null;
    /** Short note shown below description (e.g. behavior when toggled off). */
    warning?: string | null;
    /** Optional badge in title row (e.g. "Supported by your distro"). */
    notice?: string | null;
}

function SourceToggle({ title, description, enabled, onToggle, icon, disabled, tooltip, count, loading, statusBadge, inlineError, warning, notice }: SourceToggleProps) {
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
                    <div className="flex items-center gap-2 flex-wrap">
                        <h3 className="font-bold text-slate-900 dark:text-white">{title}</h3>
                        {count != null && (
                            <span className="text-[10px] px-1.5 py-0.5 bg-slate-100 dark:bg-white/10 text-slate-500 dark:text-white/40 rounded-md font-mono">{count.toLocaleString()}</span>
                        )}
                        {statusBadge === 'active' && (
                            <span className="px-2 py-0.5 text-[10px] font-medium rounded-full bg-green-500/15 text-green-600 dark:text-green-400 border border-green-500/25">Active</span>
                        )}
                        {statusBadge === 'inactive' && (
                            <span className="px-2 py-0.5 text-[10px] font-medium rounded-full bg-slate-400/15 text-slate-600 dark:text-slate-400 border border-slate-400/25">Inactive</span>
                        )}
                        {statusBadge === 'blocked' && (
                            <span className="px-2 py-0.5 text-[10px] font-medium rounded-full bg-red-500/15 text-red-600 dark:text-red-400 border border-red-500/25">Not available on this distro</span>
                        )}
                        {notice && (
                            <span className="px-2 py-0.5 text-[10px] font-medium rounded-full bg-blue-500/15 text-blue-600 dark:text-blue-400 border border-blue-500/25">{notice}</span>
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
                    {warning && (
                        <p className="text-xs text-amber-600 dark:text-amber-400 max-w-md" role="note">
                            {warning}
                        </p>
                    )}
                    {inlineError && (
                        <p className="text-xs text-red-600 dark:text-red-400">{inlineError}</p>
                    )}
                </div>
            </div>

            <button
                onClick={onToggle}
                disabled={disabled || loading}
                className={clsx(
                    "relative w-14 h-8 rounded-full p-1 transition-all duration-300 focus:outline-none focus:ring-2 focus:ring-blue-500/50 shrink-0",
                    enabled ? "bg-blue-600 shadow-lg shadow-blue-600/20" : "bg-slate-200 dark:bg-white/10",
                    (disabled || loading) && "cursor-not-allowed opacity-50"
                )}
            >
                {loading ? (
                    <div className="w-6 h-6 flex items-center justify-center mx-auto">
                        <Loader2 size={18} className="animate-spin text-slate-500" />
                    </div>
                ) : (
                    <div className={clsx(
                        "w-6 h-6 bg-white rounded-full transition-transform duration-300 shadow-sm",
                        enabled ? "translate-x-6" : "translate-x-0"
                    )} />
                )}
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
