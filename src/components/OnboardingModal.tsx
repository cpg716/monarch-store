import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronRight, Check, Palette, ShieldCheck, Sun, Moon, Zap, Lock, AlertTriangle, Terminal, RefreshCw, Activity } from 'lucide-react';
import { useTheme } from '../hooks/useTheme';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import logoFull from '../assets/logo_full.png';
import { useAppStore, type AppState } from '../store/internal_store';
import { useSessionPassword } from '../context/useSessionPassword';
import { useErrorService } from '../context/ErrorContext';

interface OnboardingModalProps {
    onComplete: () => void;
    reason?: string;
}

export default function OnboardingModal({ onComplete, reason }: OnboardingModalProps) {
    const [step, setStep] = useState(0);
    const { themeMode, setThemeMode, accentColor, setAccentColor } = useTheme();
    const [aurEnabled, setAurEnabled] = useState(false);
    const { requestSessionPassword } = useSessionPassword();
    const errorService = useErrorService();

    // Use store directly for critical reactivity
    const telemetryEnabled = useAppStore((state: AppState) => state.telemetryEnabled);
    const setTelemetry = useAppStore((state: AppState) => state.setTelemetry);
    const reducePasswordPrompts = useAppStore((state: AppState) => state.reducePasswordPrompts);
    const setReducePasswordPrompts = useAppStore((state: AppState) => state.setReducePasswordPrompts);

    // Atomic local state for ZERO LATENCY UI
    const [localToggle, setLocalToggle] = useState(telemetryEnabled);
    useEffect(() => { setLocalToggle(telemetryEnabled); }, [telemetryEnabled]);

    const handleToggle = async () => {
        const target = !localToggle;
        setLocalToggle(target); // Immediate visual flip
        try {
            await setTelemetry(target);
        } catch (e) {
            errorService.reportError(e as Error | string);
            setLocalToggle(telemetryEnabled); // Rollback visual on error
        }
    };

    const [oneClickEnabled, setOneClickEnabled] = useState(true);
    const [isSaving, setIsSaving] = useState(false);

    // System info & CPU optimization (same as Settings > Performance)
    const [systemInfo, setSystemInfo] = useState<{ kernel: string; distro: string; cpu_optimization: string; pacman_version: string } | null>(null);
    const [prioritizeOptimized, setPrioritizeOptimized] = useState(() => {
        const saved = typeof localStorage !== 'undefined' ? localStorage.getItem('prioritize-optimized-binaries') : null;
        return saved === 'true';
    });

    // System Bootstrap State
    const [bootstrapStatus, setBootstrapStatus] = useState<"idle" | "running" | "success" | "error">("idle");
    const [bootstrapError, setBootstrapError] = useState<string | null>(null);
    const [classifiedError, setClassifiedError] = useState<any | null>(null);

    useEffect(() => {
        let unlisten: any;
        const setup = async () => {
            const { listen } = await import('@tauri-apps/api/event');
            unlisten = await listen('repair-error-classified', (event) => {
                setClassifiedError(event.payload);
            });
        };
        setup();
        return () => { if (unlisten) unlisten(); };
    }, []);

    // Call these hooks unconditionally and early (Rules of Hooks)
    useEscapeKey(onComplete, true);
    const focusTrapRef = useFocusTrap(true);

    const enableSystem = async (): Promise<boolean> => {
        setBootstrapStatus("running");
        setBootstrapError(null);
        setClassifiedError(null);
        try {
            // One password for entire setup (Apple Store–like): ask once, reuse for all steps.
            const pwd = await requestSessionPassword();
            // Refactored: Call fix_keyring_issues instead of missing bootstrap_system
            await invoke("fix_keyring_issues", { password: pwd });
            setBootstrapStatus("success");
            localStorage.setItem('monarch_infra_v2_2', 'true'); // Keep infra flag
            localStorage.setItem('monarch_onboarding_v3', 'true'); // Set migration flag early just in case
            return true;
        } catch (e: unknown) {
            errorService.reportError(e as Error | string);
            setBootstrapError(String(e));
            setBootstrapStatus("error");
            return false;
        }
    };

    // Initial Load & System Detection
    useEffect(() => {
        invoke<any>('get_system_info').then(info => {
            setSystemInfo(info);
            // On CachyOS or when CPU supports v3/v4/znver4, default "prioritize optimized" to ON
            const isCachyOS = (info.distro || '').toLowerCase().includes('cachyos');
            const hasCpuOpt = info.cpu_optimization && info.cpu_optimization !== 'None';
            const saved = localStorage.getItem('prioritize-optimized-binaries');
            if (saved === null && (isCachyOS || hasCpuOpt)) {
                setPrioritizeOptimized(true);
                localStorage.setItem('prioritize-optimized-binaries', 'true');
            }
        }).catch((e) => errorService.reportError(e as Error | string));

        invoke<boolean>('is_aur_enabled').then(setAurEnabled).catch((e) => errorService.reportError(e as Error | string));
    }, [errorService]);

    const handleFinish = async () => {
        setIsSaving(true);
        try {
            await invoke('set_aur_enabled', { enabled: aurEnabled });

            // Finalize settings
            localStorage.setItem('monarch_onboarding_v3', 'true');
            await setTelemetry(localToggle).catch(() => { }); // Persist telemetry choice before event

            invoke('track_event', {
                event: 'onboarding_completed',
                payload: {
                    step_count: steps.length,
                    aur_enabled: aurEnabled,
                    telemetry_enabled: localToggle,
                    completed_at_step: steps.length,
                },
            }).catch(() => { });

            await new Promise(r => setTimeout(r, 800));
            onComplete();
        } catch (e) {
            errorService.reportError(e as Error | string);
            onComplete();
        } finally {
            setIsSaving(false);
        }
    };

    // Refactored Steps: Condensed and aligned with Host Detection philosophy
    const steps = [
        { title: "Session password", subtitle: "Enter once for this setup.", color: "bg-amber-600", icon: <Lock size={24} className="text-white" /> },
        { title: "Security & Performance", subtitle: "Keyring & optimizations.", color: "bg-emerald-600", icon: <ShieldCheck size={24} className="text-white" /> },
        { title: "AUR", subtitle: "Community-built packages.", color: "bg-amber-600", icon: <Lock size={24} className="text-white" /> },
        { title: "Privacy", subtitle: "Anonymous usage stats.", color: "bg-teal-600", icon: <Activity size={24} className="text-white" /> },
        { title: "Theme", subtitle: "Light, dark & accent.", color: "bg-pink-600", icon: <Palette size={24} className="text-white" /> },
    ];

    const nextStep = () => {
        if (step === 0) { /* Password: always allow */ }
        else if (step === 1 && bootstrapStatus !== 'success') return;

        if (step < steps.length - 1) {
            setStep(step + 1);
        } else {
            handleFinish();
        }
    };

    const safeStep = Math.min(step, steps.length - 1);
    const stepInfo = steps[safeStep];

    return (
        <div className="fixed inset-0 z-40 flex items-center justify-center p-3 sm:p-4 bg-black/90 backdrop-blur-xl overflow-hidden">
            <motion.div
                ref={focusTrapRef}
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                className="w-full max-w-2xl h-[min(65vh,380px)] max-h-[min(65vh,380px)] bg-app-card border border-app-border rounded-xl shadow-2xl overflow-hidden flex flex-col md:flex-row flex-shrink-0"
                role="dialog"
                aria-modal="true"
                aria-labelledby="onboarding-title"
            >
                {/* Left panel: step branding (old style), never scrolls */}
                <div className={clsx("w-full md:w-5/12 flex flex-col transition-colors duration-500 relative overflow-hidden shrink-0", stepInfo.color)}>
                    <div className="absolute inset-0 opacity-5 pointer-events-none" aria-hidden>
                        <svg width="100%" height="100%"><pattern id="onboarding-grid" width="40" height="40" patternUnits="userSpaceOnUse"><path d="M 40 0 L 0 0 0 40" fill="none" stroke="currentColor" strokeWidth="1" /></pattern><rect width="100%" height="100%" fill="url(#onboarding-grid)" /></svg>
                    </div>
                    {/* Frosted top band: readable on all step colors */}
                    <div className="relative z-10 shrink-0 bg-white/50 backdrop-blur-lg pt-4 pb-3 px-4 md:pt-5 md:pb-4 md:px-5 flex justify-center items-center rounded-b-xl shadow-[inset_0_1px_0_0_rgba(255,255,255,0.6)]">
                        <img
                            src={logoFull}
                            alt="MonARCH Store"
                            className="h-12 w-auto object-contain drop-shadow-[0_1px_2px_rgba(0,0,0,0.4)]"
                        />
                    </div>
                    <div className="relative z-10 flex flex-col flex-1 min-h-0 p-4 md:p-6 pt-2 md:pt-3">
                        {reason && (
                            <div className="bg-amber-500/20 text-white p-2.5 rounded-xl border border-amber-500/30 mb-3 text-[10px] leading-tight shrink-0">
                                {reason}
                            </div>
                        )}
                        <div id="onboarding-title" className="text-white/70 font-black tracking-widest text-[10px] uppercase mb-3 shrink-0" aria-live="polite">Step {safeStep + 1} / {steps.length}</div>
                        <div className="flex-1 flex flex-col items-center justify-center text-center space-y-3 min-h-0 py-2">
                            <div className="bg-white/20 p-4 md:p-5 rounded-full backdrop-blur-sm shrink-0">{stepInfo.icon}</div>
                            <h2 className="text-lg md:text-xl font-black text-white leading-tight">{stepInfo.title}</h2>
                            <p className="text-white/80 text-xs md:text-sm max-w-[200px]">{stepInfo.subtitle}</p>
                        </div>
                        <div className="flex justify-center gap-1.5 shrink-0">
                            {steps.map((_, i) => (
                                <div key={i} className={clsx("h-1 rounded-full transition-all", i === step ? "w-5 bg-white" : "w-1 bg-white/40")} />
                            ))}
                        </div>
                    </div>
                </div>
                {/* Right panel: compact content */}
                <div className="w-full md:w-7/12 flex flex-col min-h-0 flex-1 bg-app-bg overflow-hidden">
                    <div className="flex-1 min-h-0 overflow-y-auto p-2.5 md:p-3 flex flex-col items-center justify-center">
                        <AnimatePresence mode="wait">
                            {/* Step 0: Session password */}
                            {step === 0 && (
                                <motion.div key="step0" initial={{ opacity: 0, x: 12 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -12 }} className="space-y-1.5 w-full max-w-sm">
                                    <h3 className="text-sm font-bold text-app-fg">Session password (optional)</h3>
                                    <p className="text-app-muted text-[11px] leading-snug">
                                        Enter once for this setup; we won’t store it. You can change this in Settings later.
                                    </p>
                                    <div className="bg-amber-500/10 border border-amber-500/20 p-2.5 rounded-lg space-y-2">
                                        <div className="flex items-center justify-between gap-2">
                                            <div className="min-w-0">
                                                <h4 className="text-xs font-bold text-amber-500 flex items-center gap-1.5">
                                                    <Lock size={12} />
                                                    Fewer password prompts
                                                </h4>
                                                <p className="text-[10px] text-app-muted">One entry for installs during this session.</p>
                                            </div>
                                            <button
                                                type="button"
                                                role="switch"
                                                aria-checked={reducePasswordPrompts}
                                                aria-label={reducePasswordPrompts ? "Disable fewer password prompts" : "Enable fewer password prompts"}
                                                onClick={() => setReducePasswordPrompts(!reducePasswordPrompts)}
                                                className={clsx(
                                                    "w-9 h-4 rounded-full p-0.5 transition-all shrink-0",
                                                    reducePasswordPrompts ? "bg-amber-500" : "bg-app-fg/20"
                                                )}
                                            >
                                                <div className={clsx(
                                                    "w-3 h-3 bg-white rounded-full transition-transform duration-200",
                                                    reducePasswordPrompts ? "translate-x-4" : "translate-x-0"
                                                )} />
                                            </button>
                                        </div>
                                        {reducePasswordPrompts && (
                                            <div className="pt-1.5 border-t border-amber-500/20">
                                                <button
                                                    type="button"
                                                    onClick={async () => {
                                                        try {
                                                            await requestSessionPassword();
                                                        } catch (e) {
                                                            errorService.reportError(e as Error | string);
                                                        }
                                                    }}
                                                    className="w-full py-2 rounded-lg font-semibold text-xs bg-amber-500/20 text-amber-500 border border-amber-500/30 hover:bg-amber-500/30 transition-colors"
                                                >
                                                    Enter password now
                                                </button>
                                            </div>
                                        )}
                                    </div>
                                </motion.div>
                            )}

                            {/* Step 1: Security setup (bootstrap / One-Click) */}
                            {step === 1 && (
                                <motion.div key="step1" initial={{ opacity: 0, x: 12 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -12 }} className="space-y-1.5 w-full max-w-sm">
                                    <h3 className="text-sm font-bold text-app-fg">Security & one-click install</h3>
                                    <p className="text-app-muted text-[11px]">We’ll set up keyrings and permissions so installs work. One click = one password for this session.</p>

                                    {bootstrapStatus !== 'success' ? (
                                        <div className="bg-app-card border border-app-border p-2.5 rounded-lg flex flex-col items-center gap-1.5 w-full">
                                            <div className="p-2 rounded-full bg-emerald-500/10 text-emerald-500">
                                                <ShieldCheck size={28} className={bootstrapStatus === 'running' ? "animate-pulse" : ""} />
                                            </div>
                                            {!bootstrapStatus || bootstrapStatus === 'idle' || bootstrapStatus === 'error' ? (
                                                <>
                                                    <div className="bg-app-bg border border-app-border p-2 rounded-lg space-y-1.5 w-full">
                                                        <div className="flex items-center justify-between gap-2">
                                                            <div className="min-w-0">
                                                                <h4 className="text-xs font-bold text-app-fg">One-click install</h4>
                                                                <p className="text-[10px] text-app-muted">
                                                                    {oneClickEnabled
                                                                        ? "Recommended: one password for this session."
                                                                        : <span className="text-orange-500 font-semibold">Off: you’ll be asked each time.</span>
                                                                    }
                                                                </p>
                                                            </div>
                                                            <button
                                                                onClick={() => setOneClickEnabled(!oneClickEnabled)}
                                                                className={clsx(
                                                                    "w-9 h-4 rounded-full p-0.5 transition-all shrink-0",
                                                                    oneClickEnabled ? "bg-blue-600" : "bg-app-fg/20"
                                                                )}
                                                            >
                                                                <div className={clsx(
                                                                    "w-3 h-3 bg-white rounded-full transition-transform",
                                                                    oneClickEnabled ? "translate-x-4" : "translate-x-0"
                                                                )} />
                                                            </button>
                                                        </div>
                                                    </div>

                                                    {systemInfo?.cpu_optimization && systemInfo.cpu_optimization !== 'None' && (
                                                        (systemInfo.distro || '').toLowerCase().includes('cachyos') ? (
                                                            <div className="bg-app-bg border border-app-border p-2 rounded-lg w-full flex items-center gap-1.5">
                                                                <Zap size={10} className="text-purple-500 shrink-0" />
                                                                <p className="text-[10px] text-app-muted">
                                                                    <span className="font-semibold text-app-fg">Optimized binaries:</span> automatic on CachyOS. Change in Settings.
                                                                </p>
                                                            </div>
                                                        ) : (
                                                            <div className="bg-app-bg border border-app-border p-2 rounded-lg w-full flex items-center justify-between gap-2">
                                                                <div className="min-w-0">
                                                                    <h4 className="text-xs font-bold text-app-fg flex items-center gap-1">
                                                                        <Zap size={10} className="text-purple-500 shrink-0" />
                                                                        Optimized binaries
                                                                    </h4>
                                                                    <p className="text-[10px] text-app-muted">Use {systemInfo.cpu_optimization.toUpperCase()} for better performance.</p>
                                                                </div>
                                                                <button
                                                                    type="button"
                                                                    role="switch"
                                                                    aria-checked={prioritizeOptimized}
                                                                    onClick={() => {
                                                                        const newVal = !prioritizeOptimized;
                                                                        setPrioritizeOptimized(newVal);
                                                                        localStorage.setItem('prioritize-optimized-binaries', String(newVal));
                                                                    }}
                                                                    className={clsx(
                                                                        "w-9 h-4 rounded-full p-0.5 transition-all shrink-0",
                                                                        prioritizeOptimized ? "bg-purple-500" : "bg-app-fg/20"
                                                                    )}
                                                                >
                                                                    <div className={clsx(
                                                                        "w-3 h-3 bg-white rounded-full transition-transform",
                                                                        prioritizeOptimized ? "translate-x-4" : "translate-x-0"
                                                                    )} />
                                                                </button>
                                                            </div>
                                                        )
                                                    )}

                                                    <div className="flex flex-col gap-2 w-full">
                                                        {classifiedError ? (
                                                            <div className="bg-red-500/10 border border-red-500/20 p-2.5 rounded-lg space-y-1 animate-in slide-in-from-bottom-2">
                                                                <div className="flex items-center gap-1.5 text-red-500 font-bold text-[10px] uppercase tracking-wider">
                                                                    <AlertTriangle size={12} />
                                                                    {classifiedError.title}
                                                                </div>
                                                                <p className="text-[10px] text-red-500/90 leading-tight">{classifiedError.description}</p>
                                                                <div className="h-10 overflow-hidden bg-black/20 rounded p-1.5 font-mono text-[8px] opacity-60">
                                                                    {classifiedError.raw_message}
                                                                </div>
                                                            </div>
                                                        ) : bootstrapError && (
                                                            <div className="bg-red-500/10 border border-red-500/20 p-2 rounded-lg text-[10px] text-red-500 font-mono overflow-hidden max-h-14">
                                                                <span className="font-bold block mb-0.5">Error:</span>
                                                                {bootstrapError}
                                                            </div>
                                                        )}

                                                        <button onClick={enableSystem} className="w-full py-2 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-bold transition-all flex items-center justify-center gap-1.5">
                                                            <Terminal size={14} /> {bootstrapStatus === 'error' ? "Retry" : "Set up keyring"}
                                                        </button>

                                                        {bootstrapStatus === 'error' && (
                                                            <button
                                                                onClick={() => setBootstrapStatus('success')}
                                                                className="w-full py-1.5 text-[10px] font-semibold text-app-muted hover:text-app-fg transition-colors"
                                                            >
                                                                Skip (may leave repos broken)
                                                            </button>
                                                        )}
                                                    </div>
                                                </>
                                            ) : (
                                                <div className="text-center py-2">
                                                    <RefreshCw className="animate-spin mx-auto text-emerald-500 mb-0.5" size={22} />
                                                    <p className="text-[10px] font-bold text-emerald-500 uppercase tracking-wider">Setting up...</p>
                                                </div>
                                            )}
                                        </div>
                                    ) : (
                                        <div className="bg-emerald-500/10 border border-emerald-500/20 p-2.5 rounded-lg flex flex-col gap-1.5 w-full">
                                            <div className="flex items-center gap-2 w-full">
                                                <div className="p-2 bg-emerald-500 rounded-full shrink-0">
                                                    <Check size={16} className="text-white" />
                                                </div>
                                                <div>
                                                    <h4 className="text-xs font-bold text-emerald-500">Ready</h4>
                                                    <p className="text-[10px] text-app-muted">Security set up.</p>
                                                </div>
                                            </div>
                                            <div className="w-full bg-app-card/50 p-2 rounded-lg border border-app-border flex items-center justify-between">
                                                <div>
                                                    <h4 className="text-[10px] font-bold text-app-fg">One-click</h4>
                                                    <p className="text-[9px] text-app-muted">{oneClickEnabled ? "On" : "Off"}</p>
                                                </div>
                                                <button
                                                    onClick={async () => {
                                                        const newVal = !oneClickEnabled;
                                                        setOneClickEnabled(newVal);
                                                        try { await invoke('set_one_click_enabled', { enabled: newVal, password: null }); } catch (e) { errorService.reportError(e as Error | string); }
                                                    }}
                                                    className={clsx("w-9 h-4 rounded-full p-0.5 transition-all", oneClickEnabled ? "bg-emerald-500" : "bg-app-fg/20")}
                                                >
                                                    <div className={clsx("w-3 h-3 bg-white rounded-full transition-transform", oneClickEnabled ? "translate-x-4" : "translate-x-0")} />
                                                </button>
                                            </div>
                                            {systemInfo?.cpu_optimization && systemInfo.cpu_optimization !== 'None' && (
                                                (systemInfo.distro || '').toLowerCase().includes('cachyos') ? (
                                                    <div className="w-full bg-app-card/50 p-2 rounded-lg border border-app-border flex items-center gap-1.5">
                                                        <Zap size={10} className="text-purple-500 shrink-0" />
                                                        <p className="text-[9px] text-app-muted">Optimized binaries: automatic on CachyOS.</p>
                                                    </div>
                                                ) : (
                                                    <div className="w-full bg-app-card/50 p-2 rounded-lg border border-app-border flex items-center justify-between">
                                                        <span className="text-[10px] font-medium text-app-fg">Optimized: {systemInfo.cpu_optimization.toUpperCase()}</span>
                                                        <button type="button" role="switch" aria-checked={prioritizeOptimized} onClick={() => { const v = !prioritizeOptimized; setPrioritizeOptimized(v); localStorage.setItem('prioritize-optimized-binaries', String(v)); }} className={clsx("w-9 h-4 rounded-full p-0.5 shrink-0", prioritizeOptimized ? "bg-purple-500" : "bg-app-fg/20")}>
                                                            <div className={clsx("w-3 h-3 bg-white rounded-full transition-transform", prioritizeOptimized ? "translate-x-4" : "translate-x-0")} />
                                                        </button>
                                                    </div>
                                                )
                                            )}
                                        </div>
                                    )}
                                </motion.div>
                            )}

                            {/* Step 2: AUR */}
                            {step === 2 && (
                                <motion.div key="step2" initial={{ opacity: 0, x: 12 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -12 }} className="space-y-4 w-full max-w-sm">
                                    <h3 className="text-sm font-bold text-app-fg">Arch User Repository</h3>
                                    <p className="text-app-muted text-[11px] leading-snug">
                                        Access community-built packages. Build from source; huge catalog of apps.
                                    </p>
                                    <div
                                        onClick={() => setAurEnabled(!aurEnabled)}
                                        className={clsx(
                                            "cursor-pointer border rounded-lg p-3 transition-all flex items-center justify-between",
                                            aurEnabled ? "border-amber-500 bg-amber-500/10" : "border-app-border bg-app-card"
                                        )}
                                    >
                                        <div className="flex items-center gap-2">
                                            <div className={clsx("p-1.5 rounded", aurEnabled ? "bg-amber-500/20 text-amber-500" : "bg-app-fg/5 text-app-muted")}>
                                                <Lock size={14} />
                                            </div>
                                            <span className="font-bold text-app-fg text-xs">Enable AUR support</span>
                                        </div>
                                        <div className={clsx("w-8 h-4 rounded-full p-0.5 transition-all shrink-0", aurEnabled ? "bg-amber-500" : "bg-app-fg/20")}>
                                            <div className={clsx("w-3 h-3 bg-white rounded-full transition-transform", aurEnabled ? "translate-x-3.5" : "translate-x-0")} />
                                        </div>
                                    </div>
                                </motion.div>
                            )}

                            {/* Step 3: Privacy */}
                            {step === 3 && (
                                <motion.div key="step3" initial={{ opacity: 0, x: 12 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -12 }} className="space-y-4 w-full max-w-sm">
                                    <h3 className="text-sm font-bold text-app-fg">Privacy</h3>
                                    <p className="text-app-muted text-[11px] leading-snug">
                                        Help us improve with anonymous usage statistics. No personal data is ever collected.
                                    </p>
                                    <div
                                        onClick={handleToggle}
                                        className={clsx(
                                            "cursor-pointer border rounded-lg p-3 transition-all flex items-center justify-between",
                                            localToggle ? "border-teal-500 bg-teal-500/10" : "border-app-border bg-app-card"
                                        )}
                                    >
                                        <div className="flex items-center gap-2">
                                            <div className={clsx("p-1.5 rounded", localToggle ? "bg-teal-500/20 text-teal-500" : "bg-app-fg/5 text-app-muted")}>
                                                <Activity size={14} />
                                            </div>
                                            <span className="font-bold text-app-fg text-xs">Share anonymous stats</span>
                                        </div>
                                        <div className={clsx("w-8 h-4 rounded-full p-0.5 transition-all shrink-0", localToggle ? "bg-teal-500" : "bg-app-fg/20")}>
                                            <div className={clsx("w-3 h-3 bg-white rounded-full transition-transform", localToggle ? "translate-x-3.5" : "translate-x-0")} />
                                        </div>
                                    </div>
                                </motion.div>
                            )}

                            {/* Step 4: Theme */}
                            {step === 4 && (
                                <motion.div key="step4" initial={{ opacity: 0, x: 12 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -12 }} className="w-full max-w-sm space-y-4">
                                    <h3 className="text-sm font-bold text-app-fg">Theme</h3>
                                    <div className="grid grid-cols-2 gap-2">
                                        <button
                                            onClick={() => setThemeMode('light')}
                                            className={clsx(
                                                "p-3 rounded-lg border flex flex-col items-center gap-1.5 transition-all text-app-fg hover:bg-app-fg/5",
                                                themeMode === 'light' ? "border-app-accent bg-app-accent/10" : "border-app-border bg-app-card"
                                            )}
                                        >
                                            <Sun size={20} />
                                            <span className="font-bold text-[10px] uppercase tracking-wider">Light</span>
                                        </button>
                                        <button
                                            onClick={() => setThemeMode('dark')}
                                            className={clsx(
                                                "p-3 rounded-lg border flex flex-col items-center gap-1.5 transition-all text-app-fg hover:bg-app-fg/5",
                                                themeMode === 'dark' ? "border-app-accent bg-app-accent/10" : "border-app-border bg-app-card"
                                            )}
                                        >
                                            <Moon size={20} />
                                            <span className="font-bold text-[10px] uppercase tracking-wider">Dark</span>
                                        </button>
                                    </div>
                                    <div className="flex justify-center gap-2.5 pt-2">
                                        {['#3b82f6', '#8b5cf6', '#10b981', '#f59e0b', '#ef4444'].map((c) => (
                                            <button
                                                key={c}
                                                onClick={() => setAccentColor(c)}
                                                className={clsx(
                                                    "w-7 h-7 rounded-full border-2 transition-transform hover:scale-110",
                                                    accentColor === c ? "border-app-fg ring-2 ring-app-fg/20" : "border-transparent"
                                                )}
                                                style={{ backgroundColor: c }}
                                            />
                                        ))}
                                    </div>
                                </motion.div>
                            )}
                        </AnimatePresence>
                    </div>
                    {/* Footer */}
                    <div className="shrink-0 flex justify-between items-center px-2.5 md:px-3 py-2 border-t border-app-border bg-app-bg">
                        <button onClick={() => setStep(step - 1)} disabled={step === 0 || isSaving} className={clsx("text-xs font-bold transition-colors px-3 py-1.5 rounded-lg", step === 0 ? "opacity-0 pointer-events-none" : "text-app-muted hover:text-app-fg hover:bg-app-fg/5")}>Back</button>
                        <button
                            onClick={nextStep}
                            disabled={
                                isSaving ||
                                (step === 1 && bootstrapStatus !== 'success')
                            }
                            className={clsx(
                                "text-white px-5 py-2 rounded-lg font-bold text-xs active:scale-95 transition-all flex items-center gap-1.5 shadow-lg uppercase tracking-wider",
                                (isSaving || (step === 1 && bootstrapStatus !== 'success')) ? "opacity-40 grayscale cursor-not-allowed" : "hover:opacity-90 hover:scale-[1.02]"
                            )}
                            style={{ backgroundColor: accentColor }}
                        >
                            {isSaving ? "Finalizing…" : <>{step === steps.length - 1 ? "Get started" : "Next"} <ChevronRight size={14} /></>}
                        </button>
                    </div>
                </div>
            </motion.div>
        </div>
    );
}
