import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronRight, Check, Palette, ShieldCheck, Sun, Moon, Server, Zap, Database, Globe, Lock, Cpu, AlertTriangle, Terminal, RefreshCw, Star } from 'lucide-react';
import { useTheme } from '../hooks/useTheme';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import logoSmall from '../assets/logo_small.png';

interface OnboardingModalProps {
    onComplete: () => void;
    reason?: string;
}

interface RepoFamily {
    id: string;
    name: string;
    description: string;
    enabled: boolean;
    icon: any;
    members: string[];
    recommendation?: string | null;
}

export default function OnboardingModal({ onComplete, reason }: OnboardingModalProps) {
    const [step, setStep] = useState(0);
    const { themeMode, setThemeMode, accentColor, setAccentColor } = useTheme();
    const [aurEnabled, setAurEnabled] = useState(false);
    const [oneClickEnabled, setOneClickEnabled] = useState(true);
    const [isSaving, setIsSaving] = useState(false);
    const [repoFamilies, setRepoFamilies] = useState<RepoFamily[]>([]);

    // Chaotic & System State
    const [missingChaotic, setMissingChaotic] = useState<boolean>(false);
    const [chaoticStatus, setChaoticStatus] = useState<"idle" | "checking" | "enabling" | "success" | "error">("checking");
    const [chaoticLogs, setChaoticLogs] = useState<string[]>([]);

    // System Bootstrap State
    // System Bootstrap State
    const [bootstrapStatus, setBootstrapStatus] = useState<"idle" | "running" | "success" | "error">("idle");
    const [bootstrapError, setBootstrapError] = useState<string | null>(null);

    useEffect(() => {
        let unlisten: any;
        const setup = async () => {
        };
        setup();
        return () => { if (unlisten) unlisten(); };
    }, []);

    const enableSystem = async (): Promise<boolean> => {
        setBootstrapStatus("running");
        try {
            await invoke("bootstrap_system", { password: null, oneClick: oneClickEnabled });
            setBootstrapStatus("success");
            localStorage.setItem('monarch_infra_v2_2', 'true'); // Keep infra flag
            localStorage.setItem('monarch_onboarding_v3', 'true'); // Set migration flag early just in case
            return true;
        } catch (e: any) {
            console.error(e);
            setChaoticLogs(prev => [...prev, `Bootstrap Error: ${e}`]);
            setBootstrapError(e.toString());
            setBootstrapStatus("error");
            return false;
        }
    };

    // Initial Load & System Detection
    useEffect(() => {
        invoke<any>('get_system_info').then(info => {
            invoke<{ name: string; enabled: boolean; source: string }[]>('get_repo_states').then(backendRepos => {
                const families: RepoFamily[] = [
                    {
                        id: 'chaotic-aur',
                        name: 'Chaotic-AUR',
                        description: 'Pre-built binaries for Community Apps',
                        members: ['chaotic-aur'],
                        icon: Zap,
                        enabled: true,
                        recommendation: "Essential"
                    },
                    {
                        id: 'official-arch',
                        name: 'Official Arch Linux',
                        description: 'The foundation (Core, Extra, Multilib)',
                        members: ['core', 'extra', 'multilib'],
                        icon: ShieldCheck,
                        enabled: true,
                    },
                    {
                        id: 'cachyos',
                        name: 'CachyOS',
                        description: 'Performance optimized (x86_64-v3/v4)',
                        members: ['cachyos', 'cachyos-v3', 'cachyos-core-v3', 'cachyos-extra-v3', 'cachyos-v4', 'cachyos-core-v4', 'cachyos-extra-v4', 'cachyos-znver4'],
                        icon: Zap,
                        enabled: false,
                        recommendation: info.cpu_optimization.includes('v3') || info.cpu_optimization.includes('v4') ? "Performance Pick" : null
                    },
                    {
                        id: 'manjaro',
                        name: 'Manjaro',
                        description: 'Stable Manjaro packages (Experimental on Arch)',
                        members: ['manjaro-core', 'manjaro-extra'],
                        icon: Database,
                        enabled: false,
                        recommendation: info.distro.toLowerCase().includes('manjaro') ? "Matches your OS" : null
                    },
                    { id: 'garuda', name: 'Garuda', description: 'Gaming & Performance focus', members: ['garuda'], icon: Server, enabled: false },
                    { id: 'endeavouros', name: 'EndeavourOS', description: 'Minimalist & Lightweight', members: ['endeavouros'], icon: Globe, enabled: false },
                ];

                const mapped = families.map(fam => {
                    const isEnabledInBackend = backendRepos.some(r => fam.members.includes(r.name.toLowerCase()) && r.enabled);
                    // Smart Pre-selection: Enable if it's the foundation OR if it's CachyOS and CPU is optimized
                    const isOptimizedCachy = fam.id === 'cachyos' && (info.cpu_optimization.includes('v3') || info.cpu_optimization.includes('v4'));
                    const shouldAutoEnable = !isEnabledInBackend && (fam.id === 'official-arch' || fam.id === 'chaotic-aur' || isOptimizedCachy);
                    return { ...fam, enabled: isEnabledInBackend || !!shouldAutoEnable };
                });
                setRepoFamilies(mapped);
            }).catch(console.error);

        }).catch(console.error);

        invoke<boolean>('is_aur_enabled').then(setAurEnabled).catch(console.error);

        invoke<boolean>('check_repo_status', { name: 'chaotic-aur' })
            .then(exists => {
                setMissingChaotic(!exists);
                setChaoticStatus(exists ? 'success' : 'idle');
            })
            .catch(err => {
                console.error(err);
                setChaoticStatus('error');
            });
    }, []);

    const enableChaotic = async () => {
        setChaoticStatus("enabling");
        setChaoticLogs(prev => [...prev, "Checking system requirements..."]);
        try {
            // If already bootstrapped or currently bootstrapping, wait/skip
            if (bootstrapStatus !== 'success') {
                setChaoticLogs(prev => [...prev, "System bootstrap required. Starting..."]);
                const success = await enableSystem();

                if (!success) throw new Error("Bootstrap failed. Check logs.");
            }

            setChaoticLogs(prev => [...prev, "System ready. Enabling Chaotic-AUR..."]);
            const res = await invoke<string>("enable_repo", { name: "chaotic-aur", password: null });
            setChaoticLogs(prev => [...prev, res, "Setup complete!"]);
            setChaoticStatus("success");
            setMissingChaotic(false);
        } catch (e: any) {
            setChaoticLogs(prev => [...prev, `Error: ${e.message || e}`]);
            setChaoticStatus("error");
        }
    };

    const handleFinish = async () => {
        setIsSaving(true);
        try {
            await invoke('set_aur_enabled', { enabled: aurEnabled });

            // 1. First, set families (no OS sync yet)
            for (const fam of repoFamilies) {
                await invoke('toggle_repo_family', { family: fam.name, enabled: fam.enabled, skipOsSync: true });
            }

            // 2. Commit everything to the OS in one go
            try {
                await invoke('apply_os_config', { password: null });
                await invoke('optimize_system', { password: null });
            } catch (e) {
                console.error("Final system config failed:", e);
            }

            localStorage.setItem('monarch_onboarding_v3', 'true');
            await new Promise(r => setTimeout(r, 800));
            onComplete();
        } catch (e) {
            console.error(e);
            onComplete();
        }
    };

    const steps = [
        {
            title: "Welcome to MonARCH",
            subtitle: "The ultimate store for Arch Linux.",
            color: "bg-blue-600",
            icon: <img src={logoSmall} alt="MonARCH" className="w-32 h-32 object-contain drop-shadow-2xl" />
        },
        {
            title: "System Preparation",
            subtitle: "Initializing secure keyrings & verifying environment.",
            color: "bg-emerald-600",
            icon: <ShieldCheck size={48} className="text-white" />
        },
        {
            title: "Configure Repos",
            subtitle: "Intelligent selection based on your hardware.",
            color: "bg-indigo-600",
            icon: <Database size={48} className="text-white" />
        },
        {
            title: "Community Power",
            subtitle: "The Arch User Repository (AUR).",
            color: "bg-amber-600",
            icon: <Lock size={48} className="text-white" />
        },
        {
            title: "Make it Yours",
            subtitle: "Customize your visual experience.",
            color: "bg-pink-600",
            icon: <Palette size={48} className="text-white" />
        }
    ];

    const nextStep = () => {
        // Step 0: Chaotic Check
        if (step === 0 && (chaoticStatus !== 'success' || missingChaotic)) {
            return;
        }
        // Step 1: System Bootstrap check
        if (step === 1 && bootstrapStatus !== 'success') {
            return;
        }

        if (step < steps.length - 1) {
            setStep(step + 1);
        } else {
            handleFinish();
        }
    };

    const toggleRepoFamily = (id: string) => {
        setRepoFamilies(prev => prev.map(f => f.id === id ? { ...f, enabled: !f.enabled } : f));
    };

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/90 backdrop-blur-xl">
            <motion.div
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                className="w-full max-w-4xl bg-app-card border border-app-border rounded-3xl shadow-2xl overflow-hidden flex flex-col md:flex-row h-full max-h-[85vh] md:h-[600px]"
            >
                {/* Left Panel */}
                <div className={clsx(
                    "w-full md:w-4/12 p-6 md:p-8 flex flex-col transition-colors duration-700 relative overflow-y-auto custom-scrollbar shrink-0",
                    steps[step].color
                )}>
                    <div className="absolute inset-0 opacity-5 pointer-events-none">
                        <svg width="100%" height="100%">
                            <pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse">
                                <path d="M 40 0 L 0 0 0 40" fill="none" stroke="currentColor" strokeWidth="1" />
                            </pattern>
                            <rect width="100%" height="100%" fill="url(#grid)" />
                        </svg>
                    </div>

                    <div className="relative z-10 flex flex-col h-full">
                        {reason && (
                            <div className="bg-amber-500/20 text-white p-3 rounded-xl border border-amber-500/30 mb-6 backdrop-blur-md animate-in slide-in-from-top-4 shadow-lg shrink-0">
                                <div className="flex items-center gap-2 font-bold text-[10px] mb-1">
                                    <AlertTriangle size={14} className="text-amber-400" />
                                    <span>SYSTEM INTEGRITY CHECK</span>
                                </div>
                                <p className="text-[10px] opacity-90 leading-tight">{reason}</p>
                            </div>
                        )}

                        <div className="text-white/60 font-black tracking-widest text-[10px] uppercase mb-4 md:mb-8 text-center md:text-left">Step {step + 1} / {steps.length}</div>

                        <div className="flex-1 flex flex-col items-center justify-center text-center space-y-4 md:space-y-8 min-h-0 py-4">
                            <motion.div
                                key={step}
                                initial={{ scale: 0.5, opacity: 0, rotate: -10 }}
                                animate={{ scale: 1, opacity: 1, rotate: 0 }}
                                transition={{ type: "spring", stiffness: 200, damping: 15 }}
                                className={clsx(
                                    "backdrop-blur-sm shadow-inner transition-all duration-500 shrink-0",
                                    step === 0 ? "bg-transparent p-0 shadow-none scale-110 md:scale-125" : "bg-white/20 p-6 md:p-8 rounded-full"
                                )}
                            >
                                {reason ? (
                                    <div className="bg-white/10 p-4 rounded-3xl shrink-0 backdrop-blur-sm">
                                        {steps[step].icon}
                                    </div>
                                ) : (
                                    steps[step].icon
                                )}
                            </motion.div>
                            <div className="px-2">
                                <motion.h2
                                    key={`t-${step}`}
                                    initial={{ opacity: 0, y: 20 }}
                                    animate={{ opacity: 1, y: 0 }}
                                    className="text-2xl md:text-3xl font-black text-white mb-2 md:mb-3 leading-tight"
                                >
                                    {steps[step].title}
                                </motion.h2>
                                <motion.p
                                    key={`s-${step}`}
                                    initial={{ opacity: 0 }}
                                    animate={{ opacity: 1 }}
                                    transition={{ delay: 0.2 }}
                                    className="text-white/80 text-xs md:text-sm font-medium leading-relaxed max-w-[180px] md:max-w-[200px] mx-auto"
                                >
                                    {steps[step].subtitle}
                                </motion.p>
                            </div>
                        </div>

                        <div className="flex justify-center gap-1.5 mt-6 md:mt-auto">
                            {steps.map((_, i) => (
                                <div
                                    key={i}
                                    className={clsx(
                                        "h-1 rounded-full transition-all duration-300",
                                        i === step ? "w-6 bg-white" : "w-1 bg-white/30"
                                    )}
                                />
                            ))}
                        </div>
                    </div>
                </div>

                {/* Right Panel */}
                <div className="w-full md:w-8/12 p-6 md:p-10 bg-app-bg flex flex-col relative h-full min-h-0">
                    <div className="flex-1 flex flex-col items-center justify-center overflow-y-auto no-scrollbar scroll-smooth">
                        <AnimatePresence mode='wait'>
                            {step === 0 && (
                                <motion.div key="step0" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-6 max-w-lg">
                                    <h3 className="text-2xl font-bold text-app-fg">Supercharged Repository</h3>
                                    <p className="text-app-muted text-base leading-relaxed">
                                        MonARCH Store is optimized by <strong className="text-blue-500">Chaotic-AUR</strong> for instant binary installs.
                                    </p>
                                    <div className="bg-blue-500/10 border border-blue-500/20 p-5 rounded-2xl flex gap-4 items-start">
                                        <Zap className="text-blue-500 shrink-0 mt-1" />
                                        <div>
                                            <h4 className="font-bold text-blue-500 mb-1">Fast Community Apps</h4>
                                            <p className="text-sm text-app-muted">
                                                We host thousands of pre-built apps. No more waiting for compilation.
                                            </p>
                                        </div>
                                    </div>

                                    <div className="pt-2">
                                        {chaoticStatus === 'checking' && (
                                            <div className="flex items-center gap-2 text-app-muted text-sm"><RefreshCw size={14} className="animate-spin" /> Verifying environment...</div>
                                        )}
                                        {chaoticStatus === 'success' && !missingChaotic && (
                                            <div className="bg-green-500/10 border border-green-500/20 p-3 rounded-xl flex items-center gap-3 text-green-500 text-sm font-bold"><Check size={18} /> System Verified & Ready</div>
                                        )}
                                        {(chaoticStatus === 'idle' || chaoticStatus === 'error' || chaoticStatus === 'enabling') && missingChaotic && (
                                            <div className="space-y-3 bg-app-card border border-app-border p-4 rounded-xl shadow-inner">
                                                <div className="flex items-start gap-3">
                                                    <AlertTriangle className="text-orange-500 shrink-0 mt-0.5" size={18} />
                                                    <div>
                                                        <h4 className="text-sm font-bold text-app-fg">Configuration Recommended</h4>
                                                        <p className="text-xs text-app-muted">Optimize your system sources for the best experience.</p>
                                                    </div>
                                                </div>

                                                {chaoticStatus === 'enabling' && (
                                                    <div className="space-y-2 py-2">
                                                        <div className="flex justify-between text-xs text-app-muted">
                                                            <span className="font-medium text-purple-500">Optimizing System...</span>
                                                            <span>{chaoticLogs[chaoticLogs.length - 1] || "Initializing..."}</span>
                                                        </div>
                                                        <div className="h-1.5 bg-app-fg/5 rounded-full overflow-hidden w-full">
                                                            <motion.div
                                                                initial={{ x: "-100%" }}
                                                                animate={{ x: "100%" }}
                                                                transition={{
                                                                    duration: 1.5,
                                                                    ease: "linear",
                                                                    repeat: Infinity
                                                                }}
                                                                className="h-full bg-purple-500 w-[50%]"
                                                            />
                                                        </div>
                                                    </div>
                                                )}

                                                {(chaoticStatus === 'error' && chaoticLogs.length > 0) && (
                                                    <div className="h-20 overflow-auto bg-red-500/10 border border-red-500/20 rounded-lg p-2 font-mono text-[9px] text-red-500">
                                                        {chaoticLogs.map((l, i) => (
                                                            <div key={i} className="mb-0.5">{l}</div>
                                                        ))}
                                                    </div>
                                                )}

                                                <div className="flex gap-2">
                                                    <button onClick={enableChaotic} disabled={chaoticStatus === 'enabling'} className={clsx("flex-1 py-2 rounded-lg text-sm font-bold flex items-center justify-center gap-2 transition-all", chaoticStatus === 'enabling' ? "bg-app-fg/10 text-app-muted" : "bg-purple-600 text-white hover:bg-purple-500 shadow-lg")}>
                                                        {chaoticStatus === 'enabling' ? <><Terminal size={16} className="animate-pulse" /> Running Fix...</> : <><Cpu size={16} /> Auto-Configure System</>}
                                                    </button>
                                                    <button
                                                        onClick={() => {
                                                            setMissingChaotic(false);
                                                            setChaoticStatus('success');
                                                        }}
                                                        disabled={chaoticStatus === 'enabling'}
                                                        className="px-4 py-2 rounded-lg text-sm font-bold border border-app-border bg-app-card hover:bg-app-fg/10 text-app-muted hover:text-app-fg transition-all"
                                                    >
                                                        Skip
                                                    </button>
                                                </div>
                                            </div>
                                        )}
                                    </div>
                                </motion.div>
                            )}

                            {step === 1 && (
                                <motion.div key="step1" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-6 max-w-lg">
                                    <div className="text-center">
                                        <h3 className="text-2xl font-bold text-app-fg">One-Click Preparation</h3>
                                        <p className="text-app-muted text-base">Authorize MonARCH to manage your software securely.</p>
                                    </div>

                                    {bootstrapStatus !== 'success' ? (
                                        <div className="bg-app-card border border-app-border p-6 rounded-2xl flex flex-col items-center gap-6 shadow-sm w-full">
                                            {/* ... existing loading/error state ... */}
                                            <div className="p-4 rounded-full bg-emerald-500/10 text-emerald-500">
                                                <ShieldCheck size={48} className={bootstrapStatus === 'running' ? "animate-pulse" : ""} />
                                            </div>
                                            {/* ... */}
                                            {!bootstrapStatus || bootstrapStatus === 'idle' || bootstrapStatus === 'error' ? (
                                                <>
                                                    <div className="bg-app-bg border border-app-border p-4 rounded-xl space-y-3 w-full">
                                                        <div className="flex items-center justify-between">
                                                            <div>
                                                                <h4 className="text-sm font-bold text-app-fg">One-Click Authentication</h4>
                                                                <p className="text-[11px] text-app-muted mt-1">
                                                                    {oneClickEnabled
                                                                        ? "MonARCH manages permissions securely (Recommended)."
                                                                        : <span className="text-orange-500 font-bold">Manual Mode: You must enter password for every action.</span>
                                                                    }
                                                                </p>
                                                            </div>
                                                            <button
                                                                onClick={() => setOneClickEnabled(!oneClickEnabled)}
                                                                className={clsx(
                                                                    "w-10 h-5 rounded-full p-1 transition-all",
                                                                    oneClickEnabled ? "bg-blue-600" : "bg-app-fg/20"
                                                                )}
                                                            >
                                                                <div className={clsx(
                                                                    "w-3 h-3 bg-white rounded-full transition-transform",
                                                                    oneClickEnabled ? "translate-x-5" : "translate-x-0"
                                                                )} />
                                                            </button>
                                                        </div>
                                                    </div>

                                                    <div className="flex flex-col gap-3 w-full">
                                                        {bootstrapError && (
                                                            <div className="bg-red-500/10 border border-red-500/20 p-3 rounded-xl text-xs text-red-500 font-mono overflow-x-auto max-h-24">
                                                                <span className="font-bold block mb-1">Error Log:</span>
                                                                {bootstrapError}
                                                            </div>
                                                        )}

                                                        <button onClick={enableSystem} className="w-full py-3 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-white font-bold transition-all flex items-center justify-center gap-2">
                                                            <Terminal size={18} /> {bootstrapStatus === 'error' ? "Retry Preparation" : "Initialize Keyring"}
                                                        </button>

                                                        {bootstrapStatus === 'error' && (
                                                            <button
                                                                onClick={() => setBootstrapStatus('success')}
                                                                className="w-full py-2 text-xs font-bold text-app-muted hover:text-app-fg transition-colors"
                                                            >
                                                                Skip Repair (Risky - Potentially Broken Repos)
                                                            </button>
                                                        )}
                                                    </div>
                                                </>
                                            ) : (
                                                <div className="text-center py-4">
                                                    <RefreshCw className="animate-spin mx-auto text-emerald-500 mb-2" size={32} />
                                                    <p className="text-xs font-bold text-emerald-500 uppercase tracking-widest">Running Security Setup...</p>
                                                </div>
                                            )}
                                        </div>
                                    ) : (
                                        <div className="bg-emerald-500/10 border border-emerald-500/20 p-6 rounded-2xl flex flex-col items-center gap-4 w-full relative overflow-hidden">
                                            <div className="absolute top-0 right-0 p-4 opacity-10">
                                                <ShieldCheck size={100} />
                                            </div>
                                            <div className="flex items-center gap-4 w-full z-10">
                                                <div className="p-3 bg-emerald-500 rounded-full shadow-lg shadow-emerald-500/40 shrink-0">
                                                    <Check size={24} className="text-white" />
                                                </div>
                                                <div className="text-left flex-1">
                                                    <h4 className="text-lg font-bold text-emerald-500">Infrastructure Ready</h4>
                                                    <p className="text-xs text-app-muted">System security layer is active.</p>
                                                </div>
                                            </div>

                                            {/* Retroactive Toggle */}
                                            <div className="w-full bg-white/5 p-4 rounded-xl border border-white/10 flex items-center justify-between z-10 mt-2">
                                                <div>
                                                    <h4 className="text-xs font-bold text-app-fg">One-Click Permission</h4>
                                                    <p className="text-[10px] text-app-muted">
                                                        {oneClickEnabled ? "Enabled (Recommended)" : "Disabled (Password Required)"}
                                                    </p>
                                                </div>
                                                <button
                                                    onClick={async () => {
                                                        const newVal = !oneClickEnabled;
                                                        setOneClickEnabled(newVal);
                                                        // Immediate Apply since we are already successful
                                                        try {
                                                            await invoke('set_one_click_control', { enabled: newVal, password: null });
                                                        } catch (e) { console.error(e); }
                                                    }}
                                                    className={clsx(
                                                        "w-10 h-5 rounded-full p-1 transition-all",
                                                        oneClickEnabled ? "bg-emerald-500" : "bg-app-fg/20"
                                                    )}
                                                >
                                                    <div className={clsx(
                                                        "w-3 h-3 bg-white rounded-full transition-transform",
                                                        oneClickEnabled ? "translate-x-5" : "translate-x-0"
                                                    )} />
                                                </button>
                                            </div>
                                        </div>
                                    )}
                                </motion.div>
                            )}

                            {step === 2 && (
                                <motion.div key="step2" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="w-full max-w-lg flex flex-col">
                                    <div className="mb-4">
                                        <h3 className="text-2xl font-bold text-app-fg">Software Sources</h3>
                                        <p className="text-app-muted text-sm">Recommended sources are pre-selected based on your hardware.</p>
                                    </div>
                                    <div className="space-y-2 flex-1 overflow-y-auto pr-2 min-h-0 w-full custom-scrollbar">
                                        {repoFamilies.map((fam) => (
                                            <div key={fam.id} onClick={() => toggleRepoFamily(fam.id)} className={clsx("flex items-center justify-between p-3 md:p-4 rounded-xl border cursor-pointer transition-all", fam.enabled ? "bg-indigo-500/10 border-indigo-500/50 shadow-sm" : "bg-app-card border-app-border hover:border-app-fg/30")}>
                                                <div className="flex items-center gap-3 md:gap-4 text-left">
                                                    <div className={clsx("p-2 rounded-lg shrink-0", fam.enabled ? "bg-indigo-500 text-white" : "bg-app-fg/5 text-app-muted")}><fam.icon size={18} /></div>
                                                    <div className="min-w-0">
                                                        <div className="flex items-center gap-2 flex-wrap">
                                                            <h4 className="font-bold text-app-fg text-[13px] md:text-sm truncate">{fam.name}</h4>
                                                            {fam.recommendation && <span className="text-[8px] md:text-[9px] bg-green-500/20 text-green-500 px-2 py-0.5 rounded-full border border-green-500/30 flex items-center gap-1 shrink-0"><Star size={8} fill="currentColor" /> {fam.recommendation}</span>}
                                                        </div>
                                                        <p className="text-[10px] md:text-[11px] text-app-muted leading-tight line-clamp-1">{fam.description}</p>
                                                    </div>
                                                </div>
                                                <div className={clsx("w-9 h-5 rounded-full p-1 transition-colors shrink-0", fam.enabled ? "bg-indigo-500" : "bg-app-fg/20")}><div className={clsx("w-3 h-3 bg-white rounded-full transition-transform", fam.enabled ? "translate-x-4" : "translate-x-0")} /></div>
                                            </div>
                                        ))}
                                    </div>
                                </motion.div>
                            )}

                            {step === 3 && (
                                <motion.div key="step3" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-6 max-w-lg">
                                    <h3 className="text-2xl font-bold text-app-fg">Arch User Repository</h3>
                                    <div className="bg-amber-500/10 border border-amber-500/20 p-5 rounded-2xl text-left">
                                        <div className="flex items-center gap-3 mb-2"><Lock size={20} className="text-amber-500" /><h4 className="font-bold text-amber-500 text-lg">Vast Community Catalog</h4></div>
                                        <p className="text-sm text-app-fg/80 leading-relaxed">The AUR allows you to build software from source. It contains almost every Linux app ever made.</p>
                                    </div>
                                    <div onClick={() => setAurEnabled(!aurEnabled)} className={clsx("cursor-pointer border-2 rounded-2xl p-4 transition-all flex items-center justify-between text-left", aurEnabled ? "border-amber-500 bg-amber-500/5" : "border-app-border bg-app-card/30")}>
                                        <div><span className="font-bold text-app-fg block text-base">Enable AUR Support</span><span className="text-xs text-app-muted">Search and build millions of packages</span></div>
                                        <div className={clsx("w-12 h-6 rounded-full p-1 transition-colors shrink-0", aurEnabled ? "bg-amber-500" : "bg-app-fg/20")}><div className={clsx("w-4 h-4 bg-white rounded-full transition-transform", aurEnabled ? "translate-x-6" : "translate-x-0")} /></div>
                                    </div>
                                </motion.div>
                            )}

                            {step === 4 && (
                                <motion.div key="step4" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="w-full max-w-lg space-y-8">
                                    <div className="text-center"><h3 className="text-2xl font-bold text-app-fg mb-1 uppercase tracking-tight">Style your MonARCH</h3><p className="text-app-muted">Personalize your visual experience.</p></div>
                                    <div className="grid grid-cols-2 gap-4">
                                        <button onClick={() => setThemeMode('light')} className={clsx("p-5 rounded-xl border-2 flex flex-col items-center gap-2 transition-all", themeMode === 'light' ? "border-app-accent bg-app-accent/10 scale-[1.02]" : "border-app-border bg-app-card hover:border-app-fg/20")}>
                                            <Sun size={32} /><span className="font-bold text-sm">Light Mode</span>
                                        </button>
                                        <button onClick={() => setThemeMode('dark')} className={clsx("p-5 rounded-xl border-2 flex flex-col items-center gap-2 transition-all", themeMode === 'dark' ? "border-app-accent bg-app-accent/10 scale-[1.02]" : "border-app-border bg-app-card hover:border-app-fg/20")}>
                                            <Moon size={32} /><span className="font-bold text-sm">Dark Mode</span>
                                        </button>
                                    </div>
                                    <div className="flex justify-center gap-3 md:gap-4 flex-wrap mt-2 md:mt-6">
                                        {['#3b82f6', '#8b5cf6', '#10b981', '#f59e0b', '#ef4444'].map((color) => (
                                            <button key={color} onClick={() => setAccentColor(color)} className={clsx("w-8 h-8 md:w-10 md:h-10 rounded-full border-2 transition-transform hover:scale-110", accentColor === color ? "border-app-fg scale-110 md:scale-125 ring-4 ring-app-fg/10" : "border-transparent")} style={{ backgroundColor: color }} />
                                        ))}
                                    </div>
                                </motion.div>
                            )}
                        </AnimatePresence>
                    </div>

                    <div className="flex justify-between items-center pt-6 border-t border-app-border/50 mt-6 shrink-0">
                        <button onClick={() => setStep(step - 1)} disabled={step === 0 || isSaving} className={clsx("text-sm font-bold transition-colors px-4 py-2 rounded-lg", step === 0 ? "opacity-0 pointer-events-none" : "text-app-muted hover:text-app-fg hover:bg-app-fg/5")}>Back</button>
                        <button
                            onClick={nextStep}
                            disabled={
                                isSaving ||
                                (step === 0 && (chaoticStatus !== 'success' || missingChaotic)) ||
                                (step === 1 && bootstrapStatus !== 'success')
                            }
                            className={clsx(
                                "text-white px-8 md:px-10 py-2.5 md:py-3 rounded-xl font-black text-xs md:text-sm active:scale-95 transition-all flex items-center gap-2 shadow-xl md:shadow-2xl uppercase tracking-wider",
                                (isSaving || (step === 0 && (chaoticStatus !== 'success' || missingChaotic)) || (step === 1 && bootstrapStatus !== 'success')) ? "opacity-30 grayscale cursor-not-allowed" : "hover:opacity-90 hover:scale-[1.02]"
                            )}
                            style={{ backgroundColor: accentColor }}
                        >
                            {isSaving ? <span>Configuring...</span> : <>{step === steps.length - 1 ? "Start Shopping" : "Next Step"} <ChevronRight size={18} /></>}
                        </button>
                    </div>
                </div>
            </motion.div>
        </div>
    );
}
