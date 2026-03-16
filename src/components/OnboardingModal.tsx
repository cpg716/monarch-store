import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
    ChevronRight,
    ChevronLeft,
    Shield,
    Server,
    Key,
    Lock,
    Package,
    Terminal,
    Globe,
    Zap,
    Copy,
    X,
    Loader2,
    RefreshCw,
    Activity,
    Sun,
    Moon,
    Monitor,
    Palette,
    CheckCircle2,
    AlertTriangle,
    Fingerprint,
} from 'lucide-react';
import { useTheme } from '../hooks/useTheme';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { API } from '../services/api';
import { commands } from '../services/bindings';
import { unwrap } from '../utils/specta';
import { clsx } from 'clsx';
import logoFull from '../assets/logo_full.png';
import { useAppStore } from '../store/internal_store';
import { useSessionPassword } from '../context/useSessionPassword';
import { useErrorService } from '../context/ErrorContext';
import { useDistro } from '../hooks/useDistro';

const CHAOTIC_PACMAN_CONF_SNIPPET = `[chaotic-aur]
Include = /etc/pacman.d/chaotic-mirrorlist`;

interface OnboardingModalProps {
    onComplete: () => void;
    reason?: string;
}

const STEP_KEYS = ['welcome', 'flatpak', 'aur', 'chaotic', 'security', 'theme', 'confirm'] as const;
type StepKey = (typeof STEP_KEYS)[number];

export default function OnboardingModal({ onComplete, reason }: OnboardingModalProps) {
    const [stepIndex, setStepIndex] = useState(0);
    const {
        themeMode,
        setThemeMode,
        accentColor,
        setAccentColor,
        hostAccentColor,
        resolvedTheme,
        effectiveAccentColor,
        isFollowingSystemTheme,
    } = useTheme();
    const { requestSessionPassword } = useSessionPassword();
    const errorService = useErrorService();
    const { distro } = useDistro();

    const {
        setAurEnabled,
        setFlatpakEnabled,
        setOneClickEnabled,
        setOnboardingCompleted,
        setTelemetry,
        telemetryEnabled
    } = useAppStore();

    // Local Wizard State (pushed to store/backend on finish)
    const [isAurEnabledLocal, setIsAurEnabledLocal] = useState(false);
    const [isFlatpakEnabledLocal, setIsFlatpakEnabledLocal] = useState(true);
    const [localTelemetry, setLocalTelemetry] = useState(telemetryEnabled);
    const [oneClickEnabledLocal, setOneClickEnabledLocal] = useState(true);
    const [isSaving, setIsSaving] = useState(false);
    const [finishError, setFinishError] = useState<string | null>(null);

    // System Checks
    const [missingBins, setMissingBins] = useState<string[]>([]);
    const [chaoticAlreadyInAlpm, setChaoticAlreadyInAlpm] = useState<boolean | null>(null);
    const [chaoticSetupRunning, setChaoticSetupRunning] = useState(false);
    const [showChaoticFinalModal, setShowChaoticFinalModal] = useState(false);
    const [chaoticCheckAgain, setChaoticCheckAgain] = useState(false);
    const [chaoticError, setChaoticError] = useState<string | null>(null);

    const supportsChaotic = distro.capabilities.chaotic_aur_support !== 'blocked';
    const chaoticNative = distro.capabilities.chaotic_aur_support === 'native';
    const ACCENT_PRESETS = ['#3b82f6', '#8b5cf6', '#10b981', '#f59e0b', '#ef4444'];

    // Dynamic step list based on distro capabilities
    const steps: StepKey[] = [
        'welcome',
        'flatpak',
        'aur',
        ...(supportsChaotic ? ['chaotic'] : []),
        'security',
        'theme',
        'confirm'
    ] as StepKey[];

    const totalSteps = steps.length;
    const currentStepKey = steps[stepIndex];
    const isLastStep = stepIndex === totalSteps - 1;

    // Run system checks on mount and step change
    useEffect(() => {
        let cancelled = false;
        commands.getMissingRequiredBins()
            .then(unwrap)
            .then(bins => { if (!cancelled) setMissingBins(bins); })
            .catch(() => { if (!cancelled) setMissingBins([]); });

        if (supportsChaotic && (currentStepKey === 'chaotic' || currentStepKey === 'confirm')) {
            API.system.checkChaoticStatus()
                .then((s) => { if (!cancelled) setChaoticAlreadyInAlpm(s?.chaotic_in_alpm ?? false); })
                .catch(() => { if (!cancelled) setChaoticAlreadyInAlpm(false); });
        }
        return () => { cancelled = true; };
    }, [currentStepKey, supportsChaotic]);

    useEffect(() => {
        setLocalTelemetry(telemetryEnabled);
    }, [telemetryEnabled]);

    useEscapeKey(onComplete, true);
    const focusTrapRef = useFocusTrap(true);

    const handleFinish = async () => {
        setIsSaving(true);
        setFinishError(null);
        try {
            // Push wizard state to backend and store
            // Iron Core: Ensure backend is updated before completing
            await Promise.all([
                setAurEnabled(isAurEnabledLocal),
                setFlatpakEnabled(isFlatpakEnabledLocal),
                setOneClickEnabled(oneClickEnabledLocal),
                setTelemetry(localTelemetry),
            ]);

            // Persist core flags to backend (returns true if was already completed)
            const wasAlreadyCompleted = await setOnboardingCompleted(true);

            const sessionPassword = oneClickEnabledLocal
                ? await requestSessionPassword()
                : null;

            // Infrastructure Setup (if enabled)
            if (isFlatpakEnabledLocal) {
                // Ensure Flatpak runtime + Flathub remote are ready during onboarding.
                await commands.prepareFlatpak(sessionPassword ?? null).then(unwrap).catch(() => { });
                await commands.ensureFlathubRemote().then(unwrap).catch(() => { });
            }

            if (oneClickEnabledLocal) {
                // Inject the Polkit policy for "One-Click Authentication"
                await commands.installMonarchPolicy(sessionPassword ?? null).then(unwrap).catch(() => { });
            }

            commands.trackTelemetryEvent('onboarding_completed', {
                aur_enabled: isAurEnabledLocal,
                flatpak_enabled: isFlatpakEnabledLocal,
                telemetry_enabled: localTelemetry,
                one_click_enabled: oneClickEnabledLocal
            }).catch(() => { });
            if (!wasAlreadyCompleted) {
                commands.trackTelemetryEvent('store_installed', {}).catch(() => { });
            }

            await new Promise((r) => setTimeout(r, 600));
            onComplete();
        } catch (e) {
            errorService.reportError(e as Error | string);
            setFinishError('Setup could not be completed. Please retry.');
        } finally {
            setIsSaving(false);
        }
    };

    const nextStep = () => {
        if (stepIndex < totalSteps - 1) {
            setStepIndex((i) => i + 1);
        } else {
            handleFinish();
        }
    };

    const prevStep = () => {
        if (stepIndex > 0) setStepIndex((i) => i - 1);
    };

    const startChaoticSetup = async () => {
        if (!supportsChaotic) return;
        setChaoticError(null);
        setChaoticSetupRunning(true);
        try {
            // Reuse the session password when One-Click Authentication is enabled.
            const password = oneClickEnabledLocal
                ? await requestSessionPassword()
                : null;
            await commands.prepareChaoticComponents(password).then(unwrap);
            // After successful preparation, we also open the terminal for the user to finish mirror setup
            await commands.openChaoticTerminal().then(unwrap);
            setShowChaoticFinalModal(true);
        } catch (e) {
            errorService.reportError(e as Error | string);
            setChaoticError(String(e));
        } finally {
            setChaoticSetupRunning(false);
        }
    };

    const chaoticCheckAgainClick = async () => {
        setChaoticCheckAgain(true);
        try {
            const refreshPassword = oneClickEnabledLocal
                ? await requestSessionPassword()
                : null;
            unwrap(await commands.forceRefreshDatabases(refreshPassword ?? null));
            const s = unwrap(await commands.checkChaoticStatus());
            if (s?.chaotic_in_alpm) {
                setChaoticAlreadyInAlpm(true);
                setShowChaoticFinalModal(false);
            } else {
                setChaoticError("Status check failed. Did you finish the terminal setup?");
            }
        } catch (e) {
            errorService.reportError(e as Error | string);
        } finally {
            setChaoticCheckAgain(false);
        }
    };

    const stepInfo = {
        welcome: { title: 'Welcome to MonARCH', subtitle: 'Host-adaptive. Universal. Safe.', color: 'bg-blue-600', icon: <Shield size={24} className="text-white" /> },
        flatpak: { title: 'Flatpak Support', subtitle: 'Universal & Sandboxed.', color: 'bg-sky-600', icon: <Globe size={24} className="text-white" /> },
        aur: { title: 'Arch User Repository', subtitle: 'Community-driven software.', color: 'bg-amber-600', icon: <Terminal size={24} className="text-white" /> },
        chaotic: { title: 'Chaotic-AUR Setup', subtitle: 'Pre-built community apps.', color: 'bg-purple-600', icon: <Zap size={24} className="text-white" /> },
        security: { title: 'One-Click Authentication', subtitle: 'Session unlock for privileged actions.', color: 'bg-teal-600', icon: <Lock size={24} className="text-white" /> },
        theme: { title: 'Appearance', subtitle: 'Light, dark & accent.', color: 'bg-pink-600', icon: <Palette size={24} className="text-white" /> },
        confirm: { title: 'Ready for Launch', subtitle: 'Review and continue.', color: 'bg-emerald-600', icon: <CheckCircle2 size={24} className="text-white" /> },
    }[currentStepKey] ?? { title: '', subtitle: '', color: '', icon: null };

    return (
        <div className="fixed inset-0 z-40 flex items-center justify-center p-3 sm:p-4 bg-black/90 backdrop-blur-xl overflow-hidden">
            <motion.div
                ref={focusTrapRef}
                initial={{ opacity: 0, scale: 0.96 }}
                animate={{ opacity: 1, scale: 1 }}
                className="w-full max-w-3xl max-h-[85vh] bg-app-card border border-app-border rounded-xl shadow-2xl overflow-hidden flex flex-col md:flex-row flex-shrink-0"
                role="dialog"
                aria-modal="true"
            >
                {/* Branding panel */}
                <div className={clsx('w-full md:w-5/12 flex flex-col relative overflow-hidden shrink-0 min-h-[180px] md:min-h-0', stepInfo.color)}>
                    <div className="relative z-10 bg-white/50 backdrop-blur-lg p-4 flex justify-center items-center">
                        <img src={logoFull} alt="MonARCH Store" className="h-9 w-auto object-contain" />
                    </div>
                    <div className="relative z-10 flex-1 flex flex-col p-5 md:p-6 items-center justify-center text-center space-y-3">
                        <div className="bg-white/20 p-4 rounded-full backdrop-blur-sm">{stepInfo.icon}</div>
                        <h2 className="text-xl font-black text-white leading-tight">{stepInfo.title}</h2>
                        <p className="text-white/90 text-sm max-w-[200px]">{stepInfo.subtitle}</p>
                    </div>
                    <div className="p-3 flex justify-center gap-1.5">
                        {steps.map((_, i) => (
                            <div key={i} className={clsx('h-1.5 rounded-full transition-all', i === stepIndex ? 'w-5 bg-white' : 'w-1.5 bg-white/40')} />
                        ))}
                    </div>
                </div>

                {/* Content panel */}
                <div className="w-full md:w-7/12 bg-app-bg flex flex-col min-h-0 flex-1">
                    <div className="flex-1 overflow-y-auto p-5 md:p-6 flex flex-col justify-center">
                        <AnimatePresence mode="wait">
                            {currentStepKey === 'welcome' && (
                                <motion.div key="welcome" initial={{ opacity: 0, x: 24 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -24 }} className="space-y-5">
                                    <p className="text-sm text-app-muted">Detected system: <strong className="text-app-fg">{distro.pretty_name}</strong></p>
                                    {reason && <p className="text-sm text-amber-600 dark:text-amber-400 bg-amber-500/10 border border-amber-500/30 rounded-lg px-3 py-2">{reason}</p>}
                                    <ul className="space-y-3 text-sm text-app-fg">
                                        <li className="flex gap-3 items-start"><span className="text-blue-500">•</span><span><strong>Host-Adaptive:</strong> We respect your config, never overwriting your files.</span></li>
                                        <li className="flex gap-3 items-start"><span className="text-blue-500">•</span><span><strong>Universal Search:</strong> Official repos, AUR, and Flatpaks in one place.</span></li>
                                        <li className="flex gap-3 items-start"><span className="text-blue-500">•</span><span><strong>Safe Transactions:</strong> Prioritized to keep your system stable.</span></li>
                                    </ul>
                                </motion.div>
                            )}

                            {currentStepKey === 'flatpak' && (
                                <motion.div key="flatpak" initial={{ opacity: 0, x: 24 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -24 }} className="space-y-4">
                                    <div className="p-4 rounded-2xl bg-sky-500/5 border border-sky-500/20 space-y-2">
                                        <h3 className="font-bold text-sky-500 flex items-center gap-2 text-sm"><Globe size={16} /> Universal Sandboxing</h3>
                                        <p className="text-xs text-app-muted leading-relaxed">
                                            Flatpaks are containerized apps that run on any distro. They include all dependencies and don't touch your core system.
                                        </p>
                                    </div>
                                    <SourceToggle
                                        icon={Package}
                                        label="Enable Flatpak & Flathub"
                                        description="Access thousands of apps from flathub.org"
                                        checked={isFlatpakEnabledLocal}
                                        onChange={setIsFlatpakEnabledLocal}
                                        accentColor={accentColor}
                                        recommended
                                    />
                                    {missingBins.includes('flatpak') ? (
                                        <div className="p-3 rounded-xl bg-amber-500/10 border border-amber-500/30 flex gap-3 items-start">
                                            <AlertTriangle size={18} className="text-amber-500 shrink-0" />
                                            <p className="text-[10px] text-amber-700 dark:text-amber-300">
                                                <strong>Dependency Missing:</strong> The `flatpak` binary is not installed. MonARCH will attempt to install it during finalization if you enable this source.
                                            </p>
                                        </div>
                                    ) : (
                                        <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-green-500/10 text-green-600 dark:text-green-400 text-[10px] font-bold self-start">
                                            <CheckCircle2 size={12} /> System Ready
                                        </div>
                                    )}
                                </motion.div>
                            )}

                            {currentStepKey === 'aur' && (
                                <motion.div key="aur" initial={{ opacity: 0, x: 24 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -24 }} className="space-y-4">
                                    <div className="p-4 rounded-2xl bg-amber-500/5 border border-amber-500/20 space-y-2">
                                        <h3 className="font-bold text-amber-600 dark:text-amber-400 flex items-center gap-2 text-sm"><Terminal size={16} /> Arch User Repository (AUR)</h3>
                                        <p className="text-xs text-app-muted leading-relaxed">
                                            The AUR contains community-maintained packages. Most are built from source on your machine. <strong>Use with caution</strong> and always review PKGBUILDs.
                                        </p>
                                    </div>
                                    <SourceToggle
                                        icon={Terminal}
                                        label="Enable AUR Support"
                                        description="Search and build from the community repo"
                                        checked={isAurEnabledLocal}
                                        onChange={setIsAurEnabledLocal}
                                        accentColor={accentColor}
                                    />
                                    {(missingBins.includes('git') || missingBins.includes('makepkg')) ? (
                                        <div className="p-3 rounded-xl bg-red-500/10 border border-red-500/30 flex gap-3 items-start">
                                            <AlertTriangle size={18} className="text-red-500 shrink-0" />
                                            <p className="text-[10px] text-red-700 dark:text-red-300">
                                                <strong>Build Tools Missing:</strong> {missingBins.includes('git') ? '`git` ' : ''} {missingBins.includes('makepkg') ? '`base-devel` ' : ''} must be installed to build AUR packages.
                                            </p>
                                        </div>
                                    ) : (
                                        <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-green-500/10 text-green-600 dark:text-green-400 text-[10px] font-bold self-start">
                                            <CheckCircle2 size={12} /> Build Environment Ready
                                        </div>
                                    )}
                                </motion.div>
                            )}

                            {currentStepKey === 'chaotic' && (
                                <motion.div key="chaotic" initial={{ opacity: 0, x: 24 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -24 }} className="space-y-4">
                                    <div className="p-4 rounded-2xl bg-purple-500/5 border border-purple-500/20 space-y-2">
                                        <h3 className="font-bold text-purple-600 dark:text-purple-400 flex items-center gap-2 text-sm"><Zap size={16} /> Chaotic-AUR</h3>
                                        <p className="text-xs text-app-muted leading-relaxed">
                                            A pre-built repository for popular AUR packages. Saves hours of compile time by downloading ready-to-run binaries.
                                        </p>
                                    </div>
                                    {(chaoticNative || chaoticAlreadyInAlpm) ? (
                                        <div className="p-4 rounded-xl bg-green-500/10 border border-green-500/30 text-green-700 dark:text-green-300 text-sm flex items-center gap-2">
                                            <CheckCircle2 size={18} /><span>Chaotic-AUR is ready to use.</span>
                                        </div>
                                    ) : (
                                        <div className="space-y-4">
                                            <p className="text-xs text-app-muted">MonARCH will launch a terminal to initialize the Chaotic-AUR keyring and mirrorlist automatically.</p>
                                            <button onClick={startChaoticSetup} disabled={chaoticSetupRunning} className="w-full py-3 rounded-xl font-bold flex items-center justify-center gap-2 bg-purple-500/10 text-purple-700 dark:text-purple-300 border border-purple-500/40 hover:bg-purple-500/20 transition-all">
                                                {chaoticSetupRunning ? <Loader2 size={18} className="animate-spin" /> : <Key size={18} />}
                                                {chaoticSetupRunning ? 'Initializing…' : 'Start Automated Setup'}
                                            </button>
                                        </div>
                                    )}
                                </motion.div>
                            )}

                            {currentStepKey === 'security' && (
                                <motion.div key="security" initial={{ opacity: 0, x: 24 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -24 }} className="space-y-5">
                                    <div className="p-4 rounded-2xl bg-teal-500/5 border border-teal-500/20 space-y-3">
                                        <div className="flex items-center gap-2">
                                            <Lock size={18} className="text-teal-500" />
                                            <h3 className="font-bold text-teal-600 dark:text-teal-400 text-sm">One-Click Authentication</h3>
                                        </div>
                                        <p className="text-xs text-app-muted leading-relaxed">
                                            Enable <strong>One-Click Authentication</strong> to unlock MonARCH once per app session. Your password stays in memory only, is cleared when MonARCH closes, and the system prompt remains available for per-action approval.
                                        </p>
                                        <div className="pt-1 px-3 py-2 bg-white/5 rounded-lg border border-white/5 text-[10px] text-app-muted italic">
                                            Note: some advanced actions may still fall back to the system prompt when MonARCH needs a separate approval path.
                                        </div>
                                    </div>
                                    <ConfigOption icon={Fingerprint} label="Enable One-Click Authentication" description="Use MonARCH's prompt once per app session (Recommended)" checked={oneClickEnabledLocal} onChange={setOneClickEnabledLocal} accentColor={accentColor} />
                                    <ConfigOption icon={Activity} label="Anonymous Telemetry" description="Help refine the store experience. No personal data." checked={localTelemetry} onChange={setLocalTelemetry} accentColor={accentColor} />
                                </motion.div>
                            )}

                            {currentStepKey === 'theme' && (
                                <motion.div key="theme" initial={{ opacity: 0, x: 24 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -24 }} className="space-y-5">
                                    <div className="grid grid-cols-3 gap-3">
                                        <ThemeButton label="Light" icon={<Sun size={20} />} active={themeMode === 'light'} onClick={() => setThemeMode('light')} />
                                        <ThemeButton label="Dark" icon={<Moon size={20} />} active={themeMode === 'dark'} onClick={() => setThemeMode('dark')} />
                                        <ThemeButton label="System" icon={<Monitor size={20} />} active={themeMode === 'system'} onClick={() => setThemeMode('system')} />
                                    </div>
                                    <div className="text-xs text-app-muted">
                                        Effective theme: <strong className="text-app-fg">{resolvedTheme}</strong>
                                        {isFollowingSystemTheme && ' (following host)'}
                                    </div>
                                    <div className="flex justify-center gap-2 mt-4">
                                        {ACCENT_PRESETS.map(c => (
                                            <button key={c} onClick={() => setAccentColor(c)} className={clsx('w-8 h-8 rounded-full border-2 transition-transform', accentColor === c ? 'border-app-fg scale-110' : 'border-transparent hover:scale-105')} style={{ backgroundColor: c }} />
                                        ))}
                                        {hostAccentColor && (
                                            <button
                                                onClick={() => setAccentColor(hostAccentColor)}
                                                className="ml-2 px-2.5 py-1.5 rounded-lg border border-app-border text-[11px] font-bold text-app-fg hover:bg-app-subtle transition-colors"
                                            >
                                                Use Host Accent
                                            </button>
                                        )}
                                    </div>
                                </motion.div>
                            )}

                            {currentStepKey === 'confirm' && (
                                <motion.div key="confirm" initial={{ opacity: 0, x: 24 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -24 }} className="space-y-4 text-sm">
                                    <p className="text-app-muted">Review your selection to continue.</p>
                                    <div className="space-y-2">
                                        <SummaryItem label="Sources" value={`${isFlatpakEnabledLocal ? 'Flatpak' : ''} ${isAurEnabledLocal ? '· AUR' : ''}`} />
                                        <SummaryItem label="Authentication" value={oneClickEnabledLocal ? 'One-Click Authentication' : 'System Prompt'} />
                                        <SummaryItem label="Telemetry" value={localTelemetry ? 'Enabled' : 'Disabled'} />
                                        <SummaryItem label="Theme" value={themeMode} />
                                        <SummaryItem label="Effective Look" value={`${resolvedTheme} / ${isFollowingSystemTheme ? 'Host-adaptive' : 'Manual'}`} />
                                    </div>
                                </motion.div>
                            )}
                        </AnimatePresence>
                    </div>

                    {/* Footer */}
                    <div className="p-4 border-t border-app-border bg-app-bg flex items-center justify-between">
                        <button onClick={prevStep} disabled={stepIndex === 0 || isSaving} className={clsx('px-4 py-2 rounded-lg text-xs font-bold transition-all text-app-muted hover:bg-app-fg/5', stepIndex === 0 && 'invisible')}>Back</button>
                        <div className="flex flex-col items-center">
                            <span className="text-[10px] text-app-muted font-medium">Step {stepIndex + 1} of {totalSteps}</span>
                            {finishError && (
                                <span className="text-[10px] text-red-500 mt-1">{finishError}</span>
                            )}
                        </div>
                        <button onClick={nextStep} disabled={isSaving} className="px-5 py-2 rounded-lg text-xs font-bold text-white shadow-lg transition-all" style={{ backgroundColor: effectiveAccentColor }}>
                            {isSaving ? 'Finalizing…' : isLastStep ? 'Start Using MonARCH' : 'Next'}
                        </button>
                    </div>
                </div>
            </motion.div>

            {/* Chaotic Final Modal */}
            {showChaoticFinalModal && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
                    <div className="bg-app-card border border-app-border rounded-2xl p-6 max-w-md w-full space-y-4">
                        <div className="flex items-center justify-between"><h3 className="font-bold text-lg">Setup in Progress</h3><button onClick={() => setShowChaoticFinalModal(false)}><X size={20} /></button></div>
                        <p className="text-sm text-app-muted">A terminal window has been launched.</p>
                        <div className="p-3 bg-app-bg border border-app-border rounded-lg space-y-2">
                            <div className="flex gap-2 items-center text-sm font-medium"><Terminal size={16} /> <span>Follow the instructions in the terminal</span></div>
                            <p className="text-xs text-app-muted pl-6">Accept the keys and confirm the installation. When the script says "Success", close the terminal and click Check Connection below.</p>
                        </div>
                        <div className="flex gap-2">
                            <button onClick={chaoticCheckAgainClick} disabled={chaoticCheckAgain} className="flex-1 py-2 bg-blue-600 text-white rounded-lg text-sm font-bold flex items-center justify-center gap-2">
                                {chaoticCheckAgain ? <Loader2 size={16} className="animate-spin" /> : <RefreshCw size={16} />}
                                Check Connection
                            </button>
                        </div>
                        {chaoticError && <p className="text-xs text-red-500 text-center">{chaoticError}</p>}
                    </div>
                </div>
            )}
        </div>
    );
}

// Helper Components
function SourceToggle({ icon: Icon, label, description, checked, onChange, accentColor, recommended }: any) {
    return (
        <div onClick={() => onChange(!checked)} className={clsx('cursor-pointer border rounded-xl p-3.5 flex items-center justify-between transition-colors', checked ? 'bg-app-fg/5 border-app-fg/20' : 'bg-app-card border-app-border')}>
            <div className="flex gap-3 items-center">
                <div className={clsx('p-2 rounded-lg', checked ? 'text-white' : 'bg-app-fg/5 text-app-muted')} style={checked ? { backgroundColor: accentColor } : {}}>
                    <Icon size={18} />
                </div>
                <div>
                    <div className="font-bold text-sm text-app-fg flex items-center gap-2">{label} {recommended && <span className="px-1.5 py-0.5 rounded text-[10px] bg-sky-500/20 text-sky-500">Recommended</span>}</div>
                    <div className="text-[10px] text-app-muted">{description}</div>
                </div>
            </div>
            <div className={clsx('w-10 h-5 rounded-full p-1 transition-colors', checked ? '' : 'bg-app-fg/20')} style={checked ? { backgroundColor: accentColor } : {}}>
                <div className={clsx('w-3 h-3 bg-white rounded-full transition-transform', checked ? 'translate-x-5' : 'translate-x-0')} />
            </div>
        </div>
    );
}

function ConfigOption({ icon: Icon, label, description, checked, onChange, accentColor }: any) {
    return (
        <div className="flex items-center justify-between gap-4 p-3.5 rounded-xl border border-app-border bg-app-card">
            <div className="flex gap-3 items-center">
                <Icon size={18} className="text-app-muted" />
                <div>
                    <div className="font-bold text-sm text-app-fg">{label}</div>
                    <p className="text-[10px] text-app-muted">{description}</p>
                </div>
            </div>
            <button onClick={() => onChange(!checked)} className={clsx('w-10 h-5 rounded-full p-1 transition-colors shrink-0', checked ? '' : 'bg-app-fg/20')} style={checked ? { backgroundColor: accentColor } : {}}>
                <div className={clsx('w-3 h-3 bg-white rounded-full transition-transform', checked ? 'translate-x-5' : 'translate-x-0')} />
            </button>
        </div>
    );
}

function ThemeButton({ label, icon, active, onClick }: any) {
    return (
        <button onClick={onClick} className={clsx('p-4 rounded-xl border flex flex-col items-center gap-2 transition-all flex-1', active ? 'border-app-fg bg-app-fg/5' : 'border-app-border bg-app-card opacity-60 hover:opacity-100')}>
            {icon}
            <span className="text-xs font-bold uppercase">{label}</span>
        </button>
    );
}

function SummaryItem({ label, value }: any) {
    return (
        <div className="flex justify-between items-center py-2 border-b border-app-border/50">
            <span className="text-app-muted">{label}</span>
            <span className="font-medium text-app-fg">{value}</span>
        </div>
    );
}
