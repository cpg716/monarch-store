import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronRight, Check, Palette, ShieldCheck, Sun, Moon, Server, Zap, Database, Globe, Lock, Cpu, AlertTriangle, Terminal, RefreshCw, Star } from 'lucide-react';
import { useTheme } from '../hooks/useTheme';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import logoSmall from '../assets/logo_small.png';

interface OnboardingModalProps {
    onComplete: () => void;
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

export default function OnboardingModal({ onComplete }: OnboardingModalProps) {
    const [step, setStep] = useState(0);
    const { themeMode, setThemeMode, accentColor, setAccentColor } = useTheme();
    const [aurEnabled, setAurEnabled] = useState(false);
    const [isSaving, setIsSaving] = useState(false);
    const [repoFamilies, setRepoFamilies] = useState<RepoFamily[]>([]);

    // Chaotic & System State
    const [systemInfo, setSystemInfo] = useState<{ distro: string; has_avx2: boolean } | null>(null);
    const [missingChaotic, setMissingChaotic] = useState<boolean>(false);
    const [chaoticStatus, setChaoticStatus] = useState<"idle" | "checking" | "enabling" | "success" | "error">("checking");
    const [chaoticLogs, setChaoticLogs] = useState<string[]>([]);

    // Initial Load & System Detection
    useEffect(() => {
        // 1. Detect System Info for Recommendations
        invoke<any>('get_system_info').then(info => {
            setSystemInfo(info);

            // 2. Load Repo States and apply intelligent badges
            invoke<{ name: string; enabled: boolean; source: string }[]>('get_repo_states').then(backendRepos => {
                const families: RepoFamily[] = [
                    {
                        id: 'cachyos',
                        name: 'CachyOS',
                        description: 'Performance optimized (x86_64-v3/v4)',
                        members: ['cachyos', 'cachyos-v3', 'cachyos-core-v3', 'cachyos-extra-v3', 'cachyos-v4', 'cachyos-core-v4', 'cachyos-extra-v4', 'cachyos-znver4'],
                        icon: Zap,
                        enabled: false,
                        recommendation: info.has_avx2 ? "Performance Pick" : null
                    },
                    {
                        id: 'manjaro',
                        name: 'Manjaro',
                        description: 'Stable & Tested updates',
                        members: ['manjaro-core', 'manjaro-extra', 'manjaro-multilib'],
                        icon: Database,
                        enabled: false,
                        recommendation: info.distro.toLowerCase().includes('manjaro') ? "Matches your OS" : null
                    },
                    { id: 'garuda', name: 'Garuda', description: 'Gaming & Performance focus', members: ['garuda'], icon: Server, enabled: false },
                    { id: 'endeavouros', name: 'EndeavourOS', description: 'Minimalist & Lightweight', members: ['endeavouros'], icon: Globe, enabled: false },
                ];

                const mapped = families.map(fam => {
                    const isEnabled = backendRepos.some(r => fam.members.includes(r.name) && r.enabled);
                    // Pre-select recommended repos for a smoother experience
                    const shouldAutoEnable = !isEnabled && fam.recommendation;
                    return { ...fam, enabled: isEnabled || !!shouldAutoEnable };
                });
                setRepoFamilies(mapped);
            }).catch(console.error);

        }).catch(console.error);

        // 3. Load AUR state
        invoke<boolean>('is_aur_enabled').then(setAurEnabled).catch(console.error);

        // 4. Check Chaotic Status
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
        setChaoticLogs(prev => [...prev, "Requesting root privileges..."]);
        try {
            await invoke("bootstrap_infrastructure");
            const res = await invoke<string>("enable_repo", { name: "chaotic-aur" });
            setChaoticLogs(prev => [...prev, res, "Setup complete!"]);
            setChaoticStatus("success");
            setMissingChaotic(false);
        } catch (e: any) {
            setChaoticLogs(prev => [...prev, `Error: ${e}`]);
            setChaoticStatus("error");
        }
    };

    const handleFinish = async () => {
        setIsSaving(true);
        try {
            await invoke("bootstrap_infrastructure");
            await invoke('set_aur_enabled', { enabled: aurEnabled });

            const reposToEnable: string[] = [];
            for (const fam of repoFamilies) {
                await invoke('toggle_repo_family', { family: fam.name, enabled: fam.enabled });
                if (fam.enabled) {
                    reposToEnable.push(fam.id);
                }
            }

            if (reposToEnable.length > 0) {
                try {
                    await invoke('enable_repos_batch', { names: reposToEnable });
                } catch (e) {
                    console.error("Batch setup failed:", e);
                }
            }

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
            title: "Software Sources",
            subtitle: "The bridge between Official & Community repos.",
            color: "bg-slate-700",
            icon: <Server size={48} className="text-white" />
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
            icon: <ShieldCheck size={48} className="text-white" />
        },
        {
            title: "Make it Yours",
            subtitle: "Customize your visual experience.",
            color: "bg-pink-600",
            icon: <Palette size={48} className="text-white" />
        }
    ];

    const nextStep = () => {
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
                className="w-full max-w-4xl bg-app-card border border-app-border rounded-3xl shadow-2xl overflow-hidden flex flex-col md:flex-row h-[600px]"
            >
                {/* Left Panel */}
                <div className={clsx(
                    "w-full md:w-4/12 p-8 flex flex-col justify-between transition-colors duration-700 relative overflow-hidden",
                    steps[step].color
                )}>
                    <div className="absolute inset-0 opacity-10 pointer-events-none">
                        <svg width="100%" height="100%">
                            <pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse">
                                <path d="M 40 0 L 0 0 0 40" fill="none" stroke="currentColor" strokeWidth="1" />
                            </pattern>
                            <rect width="100%" height="100%" fill="url(#grid)" />
                        </svg>
                    </div>

                    <div className="text-white/80 font-bold tracking-wider text-xs uppercase z-10">Step {step + 1} of {steps.length}</div>

                    <div className="flex flex-col items-center text-center space-y-8 z-10">
                        <motion.div
                            key={step}
                            initial={{ scale: 0.5, opacity: 0, rotate: -10 }}
                            animate={{ scale: 1, opacity: 1, rotate: 0 }}
                            transition={{ type: "spring", stiffness: 200, damping: 15 }}
                            className={clsx(
                                "backdrop-blur-sm shadow-inner transition-all duration-500",
                                step === 0 ? "bg-transparent p-0 shadow-none scale-125" : "bg-white/20 p-8 rounded-full"
                            )}
                        >
                            {steps[step].icon}
                        </motion.div>
                        <div>
                            <motion.h2
                                key={`t-${step}`}
                                initial={{ opacity: 0, y: 20 }}
                                animate={{ opacity: 1, y: 0 }}
                                className="text-3xl font-black text-white mb-3 leading-tight"
                            >
                                {steps[step].title}
                            </motion.h2>
                            <motion.p
                                key={`s-${step}`}
                                initial={{ opacity: 0 }}
                                animate={{ opacity: 1 }}
                                transition={{ delay: 0.2 }}
                                className="text-white/90 text-sm font-medium leading-relaxed max-w-[200px] mx-auto"
                            >
                                {steps[step].subtitle}
                            </motion.p>
                            {step === 0 && systemInfo && (
                                <div className="mt-4 text-[10px] text-white/40 uppercase tracking-widest font-bold">
                                    Detected: {systemInfo.distro}
                                </div>
                            )}
                        </div>
                    </div>

                    {/* Quick Start Button - Only on Step 0 */}
                    {step === 0 && (
                        <button
                            onClick={handleFinish}
                            className="mt-4 px-4 py-2 bg-white/20 hover:bg-white/30 text-white rounded-xl text-xs font-bold transition-all border border-white/20 backdrop-blur-md flex items-center justify-center gap-2 self-center"
                        >
                            Express Quick Start <ChevronRight size={14} />
                        </button>
                    )}

                    <div className="flex justify-center gap-2 z-10 mt-auto pt-4">
                        {steps.map((_, i) => (
                            <div
                                key={i}
                                className={clsx(
                                    "h-1.5 rounded-full transition-all duration-300",
                                    i === step ? "w-8 bg-white" : "w-1.5 bg-white/30"
                                )}
                            />
                        ))}
                    </div>
                </div>

                {/* Right Panel */}
                <div className="w-full md:w-8/12 p-10 bg-app-bg flex flex-col relative">
                    <div className="flex-1 flex flex-col items-center justify-center overflow-y-auto">
                        <AnimatePresence mode='wait'>

                            {/* STEP 0: Welcome & Chaotic */}
                            {step === 0 && (
                                <motion.div key="step0" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-6 max-w-lg">
                                    <h3 className="text-2xl font-bold text-app-fg">Supercharged Repository</h3>
                                    <p className="text-app-muted text-base leading-relaxed">
                                        MonARCH Store is powered by <strong className="text-blue-500">Chaotic-AUR</strong>.
                                    </p>
                                    <div className="bg-blue-500/10 border border-blue-500/20 p-5 rounded-2xl flex gap-4 items-start">
                                        <Zap className="text-blue-500 shrink-0 mt-1" />
                                        <div>
                                            <h4 className="font-bold text-blue-500 mb-1">Pre-built Binaries</h4>
                                            <p className="text-sm text-app-muted">
                                                Fast, reliable, and pre-compiled. We host thousands of AUR packages so you don't have to compile them from source.
                                            </p>
                                        </div>
                                    </div>

                                    {/* Chaotic Setup */}
                                    <div className="pt-2">
                                        {chaoticStatus === 'checking' && (
                                            <div className="flex items-center gap-2 text-app-muted text-sm"><RefreshCw size={14} className="animate-spin" /> Checking system status...</div>
                                        )}
                                        {chaoticStatus === 'success' && !missingChaotic && (
                                            <div className="bg-green-500/10 border border-green-500/20 p-3 rounded-xl flex items-center gap-3 text-green-500 text-sm font-bold"><Check size={18} /> System Configured & Ready</div>
                                        )}
                                        {(chaoticStatus === 'idle' || chaoticStatus === 'error' || chaoticStatus === 'enabling') && missingChaotic && (
                                            <div className="space-y-3 bg-app-card border border-app-border p-4 rounded-xl shadow-inner">
                                                <div className="flex items-start gap-3">
                                                    <AlertTriangle className="text-orange-500 shrink-0 mt-0.5" size={18} />
                                                    <div>
                                                        <h4 className="text-sm font-bold text-app-fg">Setup Required</h4>
                                                        <p className="text-xs text-app-muted">Chaotic-AUR is missing. Enable it to get instant binary installs.</p>
                                                    </div>
                                                </div>

                                                {/* Logs */}
                                                {(chaoticStatus === 'enabling' || chaoticLogs.length > 0) && (
                                                    <div className="h-20 overflow-auto bg-black/50 rounded-lg p-2 font-mono text-[9px] text-white/70">
                                                        {chaoticLogs.map((l, i) => (
                                                            <div key={i} className="mb-0.5"><span className="text-purple-400 mr-1">➜</span>{l}</div>
                                                        ))}
                                                        {chaoticStatus === 'enabling' && <span className="animate-pulse">_</span>}
                                                    </div>
                                                )}

                                                <button onClick={enableChaotic} disabled={chaoticStatus === 'enabling'} className={clsx("w-full py-2 rounded-lg text-sm font-bold flex items-center justify-center gap-2 transition-all", chaoticStatus === 'enabling' ? "bg-app-fg/10 text-app-muted" : "bg-purple-600 text-white hover:bg-purple-500 shadow-lg")}>
                                                    {chaoticStatus === 'enabling' ? <><Terminal size={16} className="animate-pulse" /> Configuring...</> : <><Cpu size={16} /> Auto-Configure Now</>}
                                                </button>
                                            </div>
                                        )}
                                    </div>
                                </motion.div>
                            )}

                            {/* STEP 1: Software Sources (Unified Architecture) */}
                            {step === 1 && (
                                <motion.div key="step1" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-6 max-w-lg">
                                    <h3 className="text-2xl font-bold text-app-fg">The Unified Architecture</h3>
                                    <p className="text-app-muted text-base leading-relaxed">
                                        MonARCH seamlessly merges multiple software worlds into one interface.
                                    </p>
                                    <div className="space-y-3">
                                        <div className="bg-app-card border border-app-border p-4 rounded-xl flex items-center gap-4">
                                            <Server className="text-blue-500" size={24} />
                                            <div><h4 className="font-bold text-sm text-app-fg">Official Core</h4><p className="text-xs text-app-muted">Arch Linux stable repositories (Immutable core).</p></div>
                                        </div>
                                        <div className="bg-app-card border border-app-border p-4 rounded-xl flex items-center gap-4">
                                            <Zap className="text-violet-500" size={24} />
                                            <div><h4 className="font-bold text-sm text-app-fg">Chaotic Ecosystem</h4><p className="text-xs text-app-muted">Pre-compiled AUR binaries for everything else.</p></div>
                                        </div>
                                        <div className="bg-app-card border border-app-border p-4 rounded-xl flex items-center gap-4">
                                            <Globe className="text-emerald-500" size={24} />
                                            <div><h4 className="font-bold text-sm text-app-fg">Partner Repos</h4><p className="text-xs text-app-muted">Manual control over CachyOS, Manjaro, and more.</p></div>
                                        </div>
                                    </div>
                                </motion.div>
                            )}

                            {/* STEP 2: Intelligent Config */}
                            {step === 2 && (
                                <motion.div key="step2" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="w-full max-w-lg space-y-4">
                                    <div>
                                        <h3 className="text-2xl font-bold text-app-fg">Smart Configuration</h3>
                                        <p className="text-app-muted text-sm">We've pre-selected sources based on your architecture.</p>
                                    </div>
                                    <div className="space-y-2 max-h-[350px] overflow-y-auto pr-2">
                                        {repoFamilies.map((fam) => (
                                            <div key={fam.id} onClick={() => toggleRepoFamily(fam.id)} className={clsx("flex items-center justify-between p-4 rounded-xl border cursor-pointer transition-all", fam.enabled ? "bg-indigo-500/10 border-indigo-500/50 shadow-sm" : "bg-app-card border-app-border hover:border-app-fg/30")}>
                                                <div className="flex items-center gap-4">
                                                    <div className={clsx("p-2 rounded-lg", fam.enabled ? "bg-indigo-500 text-white" : "bg-app-fg/5 text-app-muted")}><fam.icon size={20} /></div>
                                                    <div>
                                                        <div className="flex items-center gap-2">
                                                            <h4 className="font-bold text-app-fg text-sm">{fam.name}</h4>
                                                            {fam.recommendation && <span className="text-[9px] bg-green-500/20 text-green-500 px-2 py-0.5 rounded-full border border-green-500/30 flex items-center gap-1"><Star size={8} fill="currentColor" /> {fam.recommendation}</span>}
                                                        </div>
                                                        <p className="text-xs text-app-muted">{fam.description}</p>
                                                    </div>
                                                </div>
                                                <div className={clsx("w-10 h-6 rounded-full p-1 transition-colors", fam.enabled ? "bg-indigo-500" : "bg-app-fg/20")}><div className={clsx("w-4 h-4 bg-white rounded-full transition-transform", fam.enabled ? "translate-x-4" : "translate-x-0")} /></div>
                                            </div>
                                        ))}
                                    </div>
                                </motion.div>
                            )}

                            {/* STEP 3: AUR */}
                            {step === 3 && (
                                <motion.div key="step3" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="space-y-6 max-w-lg">
                                    <h3 className="text-2xl font-bold text-app-fg">Community Power (AUR)</h3>
                                    <div className="bg-amber-500/10 border border-amber-500/20 p-5 rounded-2xl">
                                        <div className="flex items-center gap-3 mb-2"><Lock size={20} className="text-amber-500" /><h4 className="font-bold text-amber-500">Powerful but Risky</h4></div>
                                        <p className="text-sm text-app-fg/80 leading-relaxed mb-4">The Arch User Repository allows you to compile software from source. It contains almost everything imaginable, but use it with care.</p>
                                    </div>
                                    <div onClick={() => setAurEnabled(!aurEnabled)} className={clsx("cursor-pointer border-2 rounded-2xl p-4 transition-all hover:scale-[1.02] flex items-center justify-between", aurEnabled ? "border-amber-500 bg-amber-500/5" : "border-app-border bg-app-card/30")}>
                                        <div><span className="font-bold text-app-fg block">Enable AUR Support</span><span className="text-xs text-app-muted">Allow searching and building packages from source</span></div>
                                        <div className={clsx("w-12 h-6 rounded-full p-1 transition-colors", aurEnabled ? "bg-amber-500" : "bg-app-fg/20")}><div className={clsx("w-4 h-4 bg-white rounded-full transition-transform", aurEnabled ? "translate-x-6" : "translate-x-0")} /></div>
                                    </div>
                                </motion.div>
                            )}

                            {/* STEP 4: Aesthetics & Live Preview */}
                            {step === 4 && (
                                <motion.div key="step4" initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -20 }} className="w-full max-w-lg space-y-8">
                                    <div className="text-center"><h3 className="text-2xl font-bold text-app-fg mb-1">Make it Yours</h3><p className="text-app-muted">Customize the look and feel of MonARCH.</p></div>

                                    {/* Live Preview Card */}
                                    <div className="bg-app-card/50 border border-app-border rounded-2xl p-4 animate-in slide-in-from-bottom-4 shadow-2xl">
                                        <p className="text-[10px] font-bold text-app-muted uppercase tracking-wider mb-3">Live Preview</p>
                                        <div className="bg-app-bg rounded-xl p-4 border border-app-border flex items-center gap-4">
                                            <div className="w-12 h-12 rounded-lg flex items-center justify-center text-white" style={{ backgroundColor: accentColor }}>
                                                <Zap fill="currentColor" />
                                            </div>
                                            <div className="flex-1">
                                                <h4 className="font-bold text-app-fg">Awesome App</h4>
                                                <p className="text-xs text-app-muted">Version 1.2.3</p>
                                            </div>
                                            <button className="px-4 py-2 rounded-lg text-white font-bold text-xs" style={{ backgroundColor: accentColor }}>Install</button>
                                        </div>
                                    </div>

                                    <div className="grid grid-cols-2 gap-4">
                                        <button onClick={() => setThemeMode('light')} className={clsx("p-4 rounded-xl border-2 flex flex-col items-center gap-2 transition-all", themeMode === 'light' ? "border-app-accent bg-app-accent/10" : "border-app-border bg-app-card")}>
                                            <Sun size={24} className={themeMode === 'light' ? "text-app-accent" : "text-app-muted"} /><span className="font-bold text-sm">Light</span>
                                        </button>
                                        <button onClick={() => setThemeMode('dark')} className={clsx("p-4 rounded-xl border-2 flex flex-col items-center gap-2 transition-all", themeMode === 'dark' ? "border-app-accent bg-app-accent/10" : "border-app-border bg-app-card")}>
                                            <Moon size={24} className={themeMode === 'dark' ? "text-app-accent" : "text-app-muted"} /><span className="font-bold text-sm">Dark</span>
                                        </button>
                                    </div>

                                    <div className="flex justify-center gap-4 flex-wrap">
                                        {['#3b82f6', '#8b5cf6', '#10b981', '#f59e0b', '#ef4444'].map((color) => (
                                            <button key={color} onClick={() => setAccentColor(color)} className={clsx("w-8 h-8 rounded-full border-2 transition-all hover:scale-110", accentColor === color ? "border-app-fg scale-110 ring-2 ring-offset-2 ring-app-fg/20" : "border-transparent")} style={{ backgroundColor: color }} />
                                        ))}
                                    </div>
                                </motion.div>
                            )}

                        </AnimatePresence>
                    </div>

                    <div className="flex justify-between items-center pt-8 border-t border-app-border/50 mt-auto">
                        <button onClick={() => setStep(step - 1)} disabled={step === 0} className={clsx("text-sm font-medium transition-colors px-4 py-2 rounded-lg", step === 0 ? "opacity-0 pointer-events-none" : "text-app-muted hover:text-app-fg hover:bg-app-fg/5")}>Back</button>
                        <button onClick={nextStep} disabled={isSaving} className="text-white px-8 py-3 rounded-xl font-bold text-sm hover:opacity-90 active:scale-95 transition-all flex items-center gap-2 shadow-xl" style={{ backgroundColor: step === steps.length - 1 ? accentColor : (steps[step].color.includes('blue') ? '#2563eb' : steps[step].color.includes('slate') ? '#475569' : steps[step].color.includes('indigo') ? '#4f46e5' : steps[step].color.includes('amber') ? '#d97706' : accentColor) }}>
                            {isSaving ? <span>Configuring...</span> : <>{step === steps.length - 1 ? "Finish Setup" : "Next Step"} <ChevronRight size={16} /></>}
                        </button>
                    </div>
                </div>
            </motion.div>
        </div>
    );
}
