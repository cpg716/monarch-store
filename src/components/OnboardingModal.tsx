import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronRight, Check, Palette, ShieldCheck, Sun, Moon, Lock, Terminal, RefreshCw, Activity, Package, Info } from 'lucide-react';
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

export default function OnboardingModal({ onComplete }: OnboardingModalProps) {
    const [step, setStep] = useState(0);
    const { themeMode, setThemeMode, accentColor, setAccentColor } = useTheme();
    const { requestSessionPassword } = useSessionPassword();
    const errorService = useErrorService();

    // Store State
    const telemetryEnabled = useAppStore((state: AppState) => state.telemetryEnabled);
    const setTelemetry = useAppStore((state: AppState) => state.setTelemetry);

    const setReducePasswordPrompts = useAppStore((state: AppState) => state.setReducePasswordPrompts);

    const [localTelemetry, setLocalTelemetry] = useState(telemetryEnabled);
    useEffect(() => { setLocalTelemetry(telemetryEnabled); }, [telemetryEnabled]);

    const handleTelemetryToggle = async () => {
        const target = !localTelemetry;
        setLocalTelemetry(target);
        try { await setTelemetry(target); } catch { setLocalTelemetry(telemetryEnabled); }
    };

    // Sources State
    const [aurEnabled, setAurEnabled] = useState(false);
    const [flatpakEnabled, setFlatpakEnabled] = useState(() => typeof localStorage !== 'undefined' && localStorage.getItem('flatpak-enabled') === 'true');
    const [oneClickEnabled, setOneClickEnabled] = useState(true);

    const [isSaving, setIsSaving] = useState(false);
    const [systemInfo, setSystemInfo] = useState<{ kernel: string; distro: string; cpu_optimization: string; pacman_version: string } | null>(null);

    // Bootstrap State
    const [bootstrapStatus, setBootstrapStatus] = useState<"idle" | "running" | "success" | "error">("idle");
    const [bootstrapError, setBootstrapError] = useState<string | null>(null);
    const [bootstrapSkipped, setBootstrapSkipped] = useState(false);

    // Initial Load
    // Initial Load
    useEffect(() => {
        invoke<any>('get_system_info').then(setSystemInfo).catch(e => errorService.reportError(e));
        invoke<boolean>('is_aur_enabled').then(setAurEnabled).catch(e => errorService.reportError(e));
    }, []);

    useEscapeKey(onComplete, true);
    const focusTrapRef = useFocusTrap(true);

    const enableSystem = async (): Promise<boolean> => {
        setBootstrapStatus("running");
        setBootstrapError(null);
        try {
            const pwd = await requestSessionPassword();
            await invoke("fix_keyring_issues", { password: pwd });
            // Also set the one-click preference in backend if enabled
            if (oneClickEnabled) {
                await invoke('set_one_click_enabled', { enabled: true, password: null }).catch(() => { });
            }
            setBootstrapStatus("success");
            localStorage.setItem('monarch_infra_v2_2', 'true');
            setBootstrapSkipped(false);
            return true;
        } catch (e) {
            errorService.reportError(e as Error | string);
            setBootstrapError(String(e));
            setBootstrapStatus("error");
            return false;
        }
    };

    const handleFinish = async () => {
        setIsSaving(true);
        try {
            await invoke('set_aur_enabled', { enabled: aurEnabled });
            localStorage.setItem('flatpak-enabled', String(flatpakEnabled));
            localStorage.setItem('monarch_onboarding_v3', 'true');
            await invoke('set_one_click_enabled', { enabled: oneClickEnabled, password: null }).catch(() => { });
            await setTelemetry(localTelemetry).catch(() => { });

            invoke('track_event', {
                event: 'onboarding_completed',
                payload: { aur_enabled: aurEnabled, flatpak_enabled: flatpakEnabled, telemetry_enabled: localTelemetry }
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

    const steps = [
        { title: "Welcome & Setup", subtitle: "Security, permissions & one-click.", color: "bg-blue-600", icon: <ShieldCheck size={24} className="text-white" /> },
        { title: "Package Sources", subtitle: "AUR, Flatpak & Repositories.", color: "bg-amber-600", icon: <Package size={24} className="text-white" /> },
        { title: "Privacy", subtitle: "Anonymous usage stats.", color: "bg-teal-600", icon: <Activity size={24} className="text-white" /> },
        { title: "Theme", subtitle: "Light, dark & accent.", color: "bg-pink-600", icon: <Palette size={24} className="text-white" /> },
    ];

    const nextStep = () => {
        if (step === 0 && !bootstrapSkipped && bootstrapStatus !== 'success') return; // Must complete setup unless skipped
        if (step < steps.length - 1) setStep(step + 1);
        else handleFinish();
    };

    const handleSkipSetup = () => {
        setBootstrapSkipped(true);
        setBootstrapStatus('idle');
        setBootstrapError(null);
        setStep((prev) => Math.min(prev + 1, steps.length - 1));
    };

    const safeStep = Math.min(step, steps.length - 1);
    const stepInfo = steps[safeStep];

    return (
        <div className="fixed inset-0 z-40 flex items-center justify-center p-3 sm:p-4 bg-black/90 backdrop-blur-xl overflow-hidden">
            <motion.div
                ref={focusTrapRef}
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                className="w-full max-w-3xl h-[min(70vh,500px)] max-h-[min(70vh,500px)] bg-app-card border border-app-border rounded-xl shadow-2xl overflow-hidden flex flex-col md:flex-row flex-shrink-0"
                role="dialog" aria-modal="true"
            >
                {/* Branding Panel */}
                <div className={clsx("w-full md:w-5/12 flex flex-col transition-colors duration-500 relative overflow-hidden shrink-0", stepInfo.color)}>
                    <div className="absolute inset-0 opacity-5 pointer-events-none">
                        <svg width="100%" height="100%"><pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse"><path d="M 40 0 L 0 0 0 40" fill="none" stroke="currentColor" strokeWidth="1" /></pattern><rect width="100%" height="100%" fill="url(#grid)" /></svg>
                    </div>
                    <div className="relative z-10 bg-white/50 backdrop-blur-lg p-5 flex justify-center items-center shadow-sm">
                        <img src={logoFull} alt="MonARCH Store" className="h-10 w-auto object-contain" />
                    </div>
                    <div className="relative z-10 flex-1 flex flex-col p-6 items-center justify-center text-center space-y-4">
                        <div className="bg-white/20 p-5 rounded-full backdrop-blur-sm">{stepInfo.icon}</div>
                        <h2 className="text-2xl font-black text-white">{stepInfo.title}</h2>
                        <p className="text-white/80 text-sm max-w-[200px]">{stepInfo.subtitle}</p>
                    </div>
                    <div className="p-4 flex justify-center gap-2">
                        {steps.map((_, i) => <div key={i} className={clsx("h-1.5 rounded-full transition-all", i === step ? "w-6 bg-white" : "w-1.5 bg-white/40")} />)}
                    </div>
                </div>

                {/* Content Panel */}
                <div className="w-full md:w-7/12 bg-app-bg flex flex-col min-h-0">
                    <div className="flex-1 overflow-y-auto p-6 flex flex-col items-center justify-center">
                        <AnimatePresence mode="wait">

                            {/* STEP 0: WELCOME & SETUP */}
                            {step === 0 && (
                                <motion.div key="step0" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="w-full space-y-6">
                                    <div className="space-y-2 text-center">
                                        <h3 className="text-lg font-bold text-app-fg">Initialize System</h3>
                                        <p className="text-sm text-app-muted">Prepare keyrings and security policies for software installation.</p>
                                    </div>

                                    {/* Merged Password & One-Click Toggle */}
                                    <div className="bg-app-card border border-app-border p-4 rounded-xl space-y-4">
                                        <div className="flex items-center justify-between gap-4">
                                            <div className="space-y-0.5">
                                                <div className="font-bold text-sm text-app-fg flex items-center gap-2">
                                                    <Lock size={14} className="text-accent" /> One-Click Authentication
                                                </div>
                                                <p className="text-[11px] text-app-muted max-w-[220px]">
                                                    {oneClickEnabled
                                                        ? "Enabled: Ask for password once per launch."
                                                        : "Off: System will prompt for every action."
                                                    }
                                                </p>
                                            </div>
                                            <button
                                                onClick={() => {
                                                    setOneClickEnabled(!oneClickEnabled);
                                                    setReducePasswordPrompts(!oneClickEnabled);
                                                }}
                                                className={clsx("w-11 h-6 rounded-full p-1 transition-colors shrink-0", oneClickEnabled ? "bg-accent" : "bg-app-fg/20")}
                                            >
                                                <div className={clsx("w-4 h-4 bg-white rounded-full transition-transform", oneClickEnabled ? "translate-x-5" : "translate-x-0")} />
                                            </button>
                                        </div>
                                    </div>

                                    {/* Action Area */}
                                    <div className="space-y-3">
                                        {bootstrapStatus === 'success' ? (
                                            <div className="bg-emerald-500/10 border border-emerald-500/20 p-4 rounded-xl flex items-center gap-3">
                                                <div className="p-2 bg-emerald-500 rounded-full text-white"><Check size={18} /></div>
                                                <div>
                                                    <div className="font-bold text-emerald-500 text-sm">System Ready</div>
                                                    <div className="text-[11px] text-emerald-500/80">Keyrings and policies configured.</div>
                                                </div>
                                            </div>
                                        ) : (
                                            <>
                                                {bootstrapError && <div className="text-xs text-red-500 bg-red-500/10 p-3 rounded-lg">{bootstrapError}</div>}
                                                <button
                                                    onClick={enableSystem}
                                                    disabled={bootstrapStatus === 'running'}
                                                    className={clsx(
                                                        "w-full py-3 rounded-xl font-bold flex items-center justify-center gap-2 transition-all active:scale-95 disabled:opacity-50",
                                                        "btn-accent shadow-lg shadow-[color-mix(in_srgb,_var(--app-accent)_20%,_transparent)]"
                                                    )}
                                                >
                                                    {bootstrapStatus === 'running' ? <RefreshCw size={18} className="animate-spin" /> : <ShieldCheck size={18} />}
                                                    {bootstrapStatus === 'running' ? "Configuring..." : "Setup Security & Continue"}
                                                </button>
                                                <button
                                                    type="button"
                                                    onClick={handleSkipSetup}
                                                    className="w-full py-2.5 px-4 text-xs font-bold text-app-muted hover:text-app-fg transition-colors rounded-xl border border-app-border/60 hover:border-app-border/90"
                                                >
                                                    Skip for now (fix later in Settings)
                                                </button>
                                                <p className="text-[10px] text-center text-app-muted">
                                                    We recommend running this once to ensure keyrings and policies are healthy.
                                                </p>
                                            </>
                                        )}
                                    </div>
                                </motion.div>
                            )}

                            {/* STEP 1: SOURCES (AUR + Flatpak + Host Logic) */}
                            {step === 1 && (
                                <motion.div key="step1" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="w-full space-y-5">
                                    <div className="text-center space-y-1">
                                        <h3 className="text-lg font-bold text-app-fg">Package Sources</h3>
                                        <p className="text-sm text-app-muted">Expand your catalog beyond official repositories.</p>
                                    </div>

                                    <div className="space-y-3">
                                        {/* AUR Toggle */}
                                        <div onClick={() => setAurEnabled(!aurEnabled)} className={clsx("cursor-pointer border rounded-xl p-3.5 flex items-center justify-between transition-colors", aurEnabled ? "bg-amber-500/10 border-amber-500/50" : "bg-app-card border-app-border")}>
                                            <div className="flex gap-3 items-center">
                                                <div className={clsx("p-2 rounded-lg", aurEnabled ? "bg-amber-500 text-white" : "bg-app-fg/5 text-app-muted")}><Terminal size={18} /></div>
                                                <div>
                                                    <div className="font-bold text-sm text-app-fg">AUR Support</div>
                                                    <div className="text-[10px] text-app-muted">Community packages (build from source).</div>
                                                </div>
                                            </div>
                                            <div className={clsx("w-10 h-5 rounded-full p-1 transition-colors", aurEnabled ? "bg-amber-500" : "bg-app-fg/20")}>
                                                <div className={clsx("w-3 h-3 bg-white rounded-full transition-transform", aurEnabled ? "translate-x-5" : "translate-x-0")} />
                                            </div>
                                        </div>

                                        {/* Flatpak Toggle */}
                                        <div onClick={() => setFlatpakEnabled(!flatpakEnabled)} className={clsx("cursor-pointer border rounded-xl p-3.5 flex items-center justify-between transition-colors", flatpakEnabled ? "bg-sky-500/10 border-sky-500/50" : "bg-app-card border-app-border")}>
                                            <div className="flex gap-3 items-center">
                                                <div className={clsx("p-2 rounded-lg", flatpakEnabled ? "bg-sky-500 text-white" : "bg-app-fg/5 text-app-muted")}><Package size={18} /></div>
                                                <div>
                                                    <div className="font-bold text-sm text-app-fg">Flatpak Support</div>
                                                    <div className="text-[10px] text-app-muted">Sandboxed, universal applications.</div>
                                                </div>
                                            </div>
                                            <div className={clsx("w-10 h-5 rounded-full p-1 transition-colors", flatpakEnabled ? "bg-sky-500" : "bg-app-fg/20")}>
                                                <div className={clsx("w-3 h-3 bg-white rounded-full transition-transform", flatpakEnabled ? "translate-x-5" : "translate-x-0")} />
                                            </div>
                                        </div>
                                    </div>

                                    {/* Host-Adaptive Info Block */}
                                    <div className="bg-slate-50 dark:bg-white/5 border border-slate-200 dark:border-white/10 rounded-xl p-3.5 flex gap-3">
                                        <div className="shrink-0"><Info size={18} className="text-blue-500" /></div>
                                        <div className="space-y-1">
                                            <h4 className="font-bold text-xs text-app-fg flex items-center gap-1.5">
                                                Host-Adaptive Repositories
                                                <span className="px-1.5 py-0.5 rounded-full bg-blue-500/10 text-blue-600 text-[9px] uppercase tracking-wide">{systemInfo?.distro || "Linux"}</span>
                                            </h4>
                                            <p className="text-[10px] text-app-muted leading-relaxed">
                                                We detected your distribution. To use extra repositories like
                                                <span className="font-mono text-app-fg mx-1">Chaotic-AUR</span> or
                                                <span className="font-mono text-app-fg mx-1">CachyOS</span>,
                                                add them to your system's <code className="bg-black/10 dark:bg-white/10 px-1 rounded">/etc/pacman.conf</code>.
                                                MonARCH will detect and enable them automatically.
                                            </p>
                                        </div>
                                    </div>
                                </motion.div>
                            )}

                            {/* STEP 2: PRIVACY */}
                            {step === 2 && (
                                <motion.div key="step2" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="w-full space-y-6">
                                    <div className="text-center space-y-1">
                                        <h3 className="text-lg font-bold text-app-fg">Privacy</h3>
                                        <p className="text-sm text-app-muted">We respect your data. Opt-in only.</p>
                                    </div>
                                    <div onClick={handleTelemetryToggle} className={clsx("cursor-pointer border rounded-xl p-4 flex items-center justify-between transition-colors", localTelemetry ? "bg-teal-500/10 border-teal-500/50" : "bg-app-card border-app-border")}>
                                        <div className="flex gap-3 items-center">
                                            <div className={clsx("p-2 rounded-lg", localTelemetry ? "bg-teal-500 text-white" : "bg-app-fg/5 text-app-muted")}><Activity size={18} /></div>
                                            <div>
                                                <div className="font-bold text-sm text-app-fg">Anonymous Telemetry</div>
                                                <div className="text-[10px] text-app-muted">Share basic usage stats to help improvement.</div>
                                            </div>
                                        </div>
                                        <div className={clsx("w-10 h-5 rounded-full p-1 transition-colors", localTelemetry ? "bg-teal-500" : "bg-app-fg/20")}>
                                            <div className={clsx("w-3 h-3 bg-white rounded-full transition-transform", localTelemetry ? "translate-x-5" : "translate-x-0")} />
                                        </div>
                                    </div>
                                </motion.div>
                            )}

                            {/* STEP 3: THEME */}
                            {step === 3 && (
                                <motion.div key="step3" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="w-full space-y-6">
                                    <div className="text-center space-y-1">
                                        <h3 className="text-lg font-bold text-app-fg">Appearance</h3>
                                        <p className="text-sm text-app-muted">Make it yours.</p>
                                    </div>
                                    <div className="grid grid-cols-2 gap-3">
                                        <button onClick={() => setThemeMode('light')} className={clsx("p-4 rounded-xl border flex flex-col items-center gap-2", themeMode === 'light' ? "border-pink-500 bg-pink-500/5 text-pink-600" : "border-app-border bg-app-card text-app-muted opacity-60 hover:opacity-100")}>
                                            <Sun size={24} /> <span className="text-xs font-bold uppercase">Light</span>
                                        </button>
                                        <button onClick={() => setThemeMode('dark')} className={clsx("p-4 rounded-xl border flex flex-col items-center gap-2", themeMode === 'dark' ? "border-pink-500 bg-pink-500/5 text-pink-600" : "border-app-border bg-app-card text-app-muted opacity-60 hover:opacity-100")}>
                                            <Moon size={24} /> <span className="text-xs font-bold uppercase">Dark</span>
                                        </button>
                                    </div>
                                    <div className="flex justify-center gap-3 pt-2">
                                        {['#3b82f6', '#8b5cf6', '#10b981', '#f59e0b', '#ef4444'].map(c => (
                                            <button key={c} onClick={() => setAccentColor(c)} className={clsx("w-8 h-8 rounded-full border-2 hover:scale-110 transition-transform", accentColor === c ? "border-app-fg ring-2 ring-app-fg/20" : "border-transparent")} style={{ backgroundColor: c }} />
                                        ))}
                                    </div>
                                </motion.div>
                            )}

                        </AnimatePresence>
                    </div>

                    {/* Footer */}
                    <div className="p-4 border-t border-app-border bg-app-bg flex justify-between items-center">
                        <button onClick={() => setStep(step - 1)} disabled={step === 0 || isSaving} className={clsx("px-4 py-2 rounded-lg text-xs font-bold transition-all", step === 0 ? "opacity-0 pointer-events-none" : "text-app-muted hover:bg-app-fg/5")}>Back</button>
                        <button onClick={nextStep} disabled={isSaving || (step === 0 && !bootstrapSkipped && bootstrapStatus !== 'success')} className={clsx("px-6 py-2 rounded-lg text-xs font-bold text-white shadow-lg transition-all flex items-center gap-2", (isSaving || (step === 0 && !bootstrapSkipped && bootstrapStatus !== 'success')) ? "opacity-30 cursor-not-allowed grayscale" : "hover:opacity-90 active:scale-95")} style={{ backgroundColor: accentColor }}>
                            {isSaving ? "Finalizing..." : <>{step === steps.length - 1 ? "Start Using MonARCH" : "Next Step"} <ChevronRight size={14} /></>}
                        </button>
                    </div>
                </div>
            </motion.div>
        </div>
    );
}
