import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ChevronRight, Check, Palette, ShieldCheck, Sun, Moon, Server, Zap, Database, Globe, Lock, Cpu, AlertTriangle, Terminal, RefreshCw } from 'lucide-react';
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
    members: string[]; // Added missing interface property
}

export default function OnboardingModal({ onComplete }: OnboardingModalProps) {
    const [step, setStep] = useState(0);
    const { themeMode, setThemeMode, accentColor, setAccentColor } = useTheme();
    const [aurEnabled, setAurEnabled] = useState(false);
    const [isSaving, setIsSaving] = useState(false);
    const [repoFamilies, setRepoFamilies] = useState<RepoFamily[]>([]);

    // Chaotic Setup State
    const [missingChaotic, setMissingChaotic] = useState<boolean>(false);
    const [chaoticStatus, setChaoticStatus] = useState<"idle" | "checking" | "enabling" | "success" | "error">("checking");
    const [chaoticLogs, setChaoticLogs] = useState<string[]>([]);

    // Initial Load
    useEffect(() => {
        // Load global AUR state
        invoke<boolean>('is_aur_enabled').then(setAurEnabled).catch(console.error);

        // Define Repo Families for Step 4
        invoke<{ name: string; enabled: boolean; source: string }[]>('get_repo_states').then(backendRepos => {
            const families = [
                { id: 'cachyos', name: 'CachyOS', description: 'Performance optimized (x86_64-v3/v4)', members: ['cachyos', 'cachyos-v3', 'cachyos-core-v3', 'cachyos-extra-v3', 'cachyos-v4', 'cachyos-core-v4', 'cachyos-extra-v4', 'cachyos-znver4'], icon: Zap },
                { id: 'manjaro', name: 'Manjaro', description: 'Stable & Tested updates', members: ['manjaro-core', 'manjaro-extra', 'manjaro-multilib'], icon: Database },
                { id: 'garuda', name: 'Garuda', description: 'Gaming & Performance focus', members: ['garuda'], icon: Server },
                { id: 'endeavouros', name: 'EndeavourOS', description: 'Minimalist & Lightweight', members: ['endeavouros'], icon: Globe },
            ];

            const mapped = families.map(fam => {
                const isEnabled = backendRepos.some(r => fam.members.includes(r.name) && r.enabled);
                return { ...fam, enabled: isEnabled };
            });
            setRepoFamilies(mapped);
        }).catch(console.error);

        // Check Chaotic Status
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
            const res = await invoke<string>("enable_repo", { name: "chaotic-aur" });
            setChaoticLogs(prev => [...prev, res, "Setup complete!"]);
            setChaoticStatus("success");
            setMissingChaotic(false);
        } catch (e: any) {
            setChaoticLogs(prev => [...prev, `Error: ${e}`]);
            setChaoticStatus("error");
        }
    };

    const steps = [
        // Step 0: Welcome & Chaotic-AUR (The "Why")
        {
            title: "Welcome to MonARCH",
            subtitle: "The ultimate store for Arch Linux.",
            color: "bg-blue-600",
            icon: <img src={logoSmall} alt="MonARCH" className="w-32 h-32 object-contain drop-shadow-2xl" />
        },
        // Step 1: Arch Official (The Foundation)
        {
            title: "The Foundation",
            subtitle: "Official Arch Linux repositories.",
            color: "bg-slate-700",
            icon: <Server size={48} className="text-white" />
        },
        // Step 2: The Ecosystem (Cachy/Manjaro/etc)
        {
            title: "The Ecosystem",
            subtitle: "Performance & stability from the community.",
            color: "bg-emerald-600",
            icon: <Zap size={48} className="text-white" />
        },
        // Step 3: Deployment Strategy (Repo Config)
        {
            title: "Configure Sources",
            subtitle: "Choose which repositories to enable.",
            color: "bg-indigo-600",
            icon: <Database size={48} className="text-white" />
        },
        // Step 4: The AUR (Power & Risk)
        {
            title: "Community Power",
            subtitle: "The Arch User Repository (AUR).",
            color: "bg-amber-600",
            icon: <ShieldCheck size={48} className="text-white" />
        },
        // Step 5: Aesthetics
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

    const handleFinish = async () => {
        setIsSaving(true);
        try {
            // 1. Apply AUR setting
            await invoke('set_aur_enabled', { enabled: aurEnabled });

            // 2. Apply Repo Families
            for (const fam of repoFamilies) {
                // Toggle family logic (backend might just use enable/disable toggle)
                // But for setup, we want to run enable_repo if it's enabled and wasn't before?
                // Actually, `toggle_repo_family` handles enabling/disabling in the app state.
                // WE ALSO need to run `enable_repo` (system setup) if they enabled it.
                await invoke('toggle_repo_family', { family: fam.name, enabled: fam.enabled });

                if (fam.enabled) {
                    try {
                        // We use fam.id which matches the key in repo_setup.rs (cachyos, garuda, etc)
                        // But repo_setup expects "cachyos", "garuda".
                        await invoke('enable_repo', { name: fam.id });
                    } catch (e) {
                        console.error(`Failed to enable ${fam.name}:`, e);
                        // We continue even if one fails
                    }
                }
            }

            // Artificial delay for smooth UX
            await new Promise(r => setTimeout(r, 800));
            onComplete();
        } catch (e) {
            console.error(e);
            onComplete();
        }
    };

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/90 backdrop-blur-xl">
            <motion.div
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                className="w-full max-w-4xl bg-app-card border border-app-border rounded-3xl shadow-2xl overflow-hidden flex flex-col md:flex-row h-[600px]"
            >
                {/* Left Panel - Context aware */}
                <div className={clsx(
                    "w-full md:w-4/12 p-8 flex flex-col justify-between transition-colors duration-700 relative overflow-hidden",
                    steps[step].color
                )}>
                    {/* Background Pattern */}
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
                        </div>
                    </div>

                    <div className="flex justify-center gap-2 z-10">
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

                {/* Right Panel - Content */}
                <div className="w-full md:w-8/12 p-10 bg-app-bg flex flex-col relative">
                    <div className="flex-1 flex flex-col items-center justify-center">
                        <AnimatePresence mode='wait'>

                            {/* STEP 0: Welcome & Chaotic */}
                            {step === 0 && (
                                <motion.div
                                    key="step0"
                                    initial={{ opacity: 0, x: 20 }}
                                    animate={{ opacity: 1, x: 0 }}
                                    exit={{ opacity: 0, x: -20 }}
                                    className="space-y-6 max-w-lg"
                                >
                                    <h3 className="text-2xl font-bold text-app-fg">Supercharged Repository</h3>
                                    <p className="text-app-muted text-base leading-relaxed">
                                        MonARCH Store is powered by <strong className="text-blue-500">Chaotic-AUR</strong>.
                                    </p>
                                    <div className="bg-blue-500/10 border border-blue-500/20 p-5 rounded-2xl flex gap-4 items-start">
                                        <Zap className="text-blue-500 shrink-0 mt-1" />
                                        <div>
                                            <h4 className="font-bold text-blue-500 mb-1">Pre-built Binaries</h4>
                                            <p className="text-sm text-app-muted">
                                                Unlike traditional AUR helpers that compile everything from source (slow), we serve pre-compiled binaries for thousands of popular packages. It's fast, reliable, and safer.
                                            </p>
                                        </div>
                                    </div>
                                    <p className="text-xs text-app-muted italic">
                                        We automatically check Chaotic-AUR first for any package you request.
                                    </p>

                                    {/* Chaotic Status / Action Area */}
                                    <div className="pt-2">
                                        {chaoticStatus === 'checking' && (
                                            <div className="flex items-center gap-2 text-app-muted text-sm">
                                                <RefreshCw size={14} className="animate-spin" /> Checking system status...
                                            </div>
                                        )}

                                        {chaoticStatus === 'success' && !missingChaotic && (
                                            <div className="bg-green-500/10 border border-green-500/20 p-3 rounded-xl flex items-center gap-3 text-green-500 text-sm font-bold">
                                                <Check size={18} /> System Configured & Ready
                                            </div>
                                        )}

                                        {(chaoticStatus === 'idle' || chaoticStatus === 'error' || chaoticStatus === 'enabling') && missingChaotic && (
                                            <div className="space-y-3 bg-app-card border border-app-border p-4 rounded-xl shadow-inner">
                                                <div className="flex items-start gap-3">
                                                    <AlertTriangle className="text-orange-500 shrink-0 mt-0.5" size={18} />
                                                    <div>
                                                        <h4 className="text-sm font-bold text-app-fg">Setup Required</h4>
                                                        <p className="text-xs text-app-muted">The Chaotic-AUR repository is missing. Enable it to access pre-built binaries.</p>
                                                    </div>
                                                </div>

                                                {/* Logs */}
                                                {(chaoticStatus === 'enabling' || chaoticLogs.length > 0) && (
                                                    <div className="h-24 overflow-auto bg-black/50 rounded-lg p-3 font-mono text-[10px] text-white/70">
                                                        {chaoticLogs.map((l, i) => (
                                                            <div key={i} className="mb-1"><span className="text-purple-400 mr-1">➜</span>{l}</div>
                                                        ))}
                                                        {chaoticStatus === 'enabling' && <span className="animate-pulse">_</span>}
                                                    </div>
                                                )}

                                                <button
                                                    onClick={enableChaotic}
                                                    disabled={chaoticStatus === 'enabling'}
                                                    className={clsx(
                                                        "w-full py-2 rounded-lg text-sm font-bold flex items-center justify-center gap-2 transition-all",
                                                        chaoticStatus === 'enabling' ? "bg-app-fg/10 text-app-muted" : "bg-purple-600 text-white hover:bg-purple-500 shadow-lg"
                                                    )}
                                                >
                                                    {chaoticStatus === 'enabling' ? (
                                                        <> <Terminal size={16} className="animate-pulse" /> Configuring...</>
                                                    ) : (
                                                        <> <Cpu size={16} /> Auto-Configure Now</>
                                                    )}
                                                </button>
                                            </div>
                                        )}
                                    </div>
                                </motion.div>
                            )}

                            {/* STEP 1: Official Repos */}
                            {step === 1 && (
                                <motion.div
                                    key="step1"
                                    initial={{ opacity: 0, x: 20 }}
                                    animate={{ opacity: 1, x: 0 }}
                                    exit={{ opacity: 0, x: -20 }}
                                    className="space-y-6 max-w-lg"
                                >
                                    <h3 className="text-2xl font-bold text-app-fg">The Immutable Core</h3>
                                    <p className="text-app-muted text-base leading-relaxed">
                                        Your system is built on the robust foundation of <strong>Arch Linux Official Repositories</strong>.
                                    </p>

                                    <div className="grid grid-cols-1 gap-3">
                                        <div className="bg-app-card border border-app-border p-4 rounded-xl flex items-center gap-3">
                                            <div className="bg-slate-500/10 p-2 rounded-lg"><Server size={20} className="text-slate-500" /></div>
                                            <div>
                                                <h4 className="font-bold text-app-fg text-sm">Core & Extra</h4>
                                                <p className="text-xs text-app-muted">Essential system packages and vetted applications.</p>
                                            </div>
                                        </div>
                                        <div className="bg-app-card border border-app-border p-4 rounded-xl flex items-center gap-3">
                                            <div className="bg-slate-500/10 p-2 rounded-lg"><Database size={20} className="text-slate-500" /></div>
                                            <div>
                                                <h4 className="font-bold text-app-fg text-sm">Multilib</h4>
                                                <p className="text-xs text-app-muted">32-bit libraries for gaming (Steam, Wine) and legacy support.</p>
                                            </div>
                                        </div>
                                    </div>
                                    <div className="text-xs text-green-500 bg-green-500/10 px-3 py-2 rounded-lg inline-flex items-center gap-2">
                                        <Check size={14} /> Always enabled by default
                                    </div>
                                </motion.div>
                            )}

                            {/* STEP 2: The Ecosystem */}
                            {step === 2 && (
                                <motion.div
                                    key="step2"
                                    initial={{ opacity: 0, x: 20 }}
                                    animate={{ opacity: 1, x: 0 }}
                                    exit={{ opacity: 0, x: -20 }}
                                    className="space-y-6 max-w-lg"
                                >
                                    <h3 className="text-2xl font-bold text-app-fg">Pre-Built Ecosystem</h3>
                                    <p className="text-app-muted text-base leading-relaxed">
                                        Just like Chaotic-AUR, these partner repositories provide <strong>pre-compiled binaries</strong>. No compiling source code required - installs are instant.
                                    </p>

                                    <div className="grid grid-cols-2 gap-4">
                                        <div className="bg-app-card p-4 rounded-xl border border-app-border">
                                            <Zap className="text-amber-500 mb-2" />
                                            <h4 className="font-bold text-sm">CachyOS</h4>
                                            <p className="text-[10px] text-app-muted mt-1">CPU-optimized binaries (v3/v4) for extreme performance.</p>
                                        </div>
                                        <div className="bg-app-card p-4 rounded-xl border border-app-border">
                                            <ShieldCheck className="text-green-500 mb-2" />
                                            <h4 className="font-bold text-sm">Manjaro</h4>
                                            <p className="text-[10px] text-app-muted mt-1">Stability-focused packages tested for reliability.</p>
                                        </div>
                                        <div className="bg-app-card p-4 rounded-xl border border-app-border">
                                            <Server className="text-violet-600 mb-2" />
                                            <h4 className="font-bold text-sm">Garuda</h4>
                                            <p className="text-[10px] text-app-muted mt-1">Gaming optimizations & tools (Chaotic based).</p>
                                        </div>
                                        <div className="bg-app-card p-4 rounded-xl border border-app-border">
                                            <Globe className="text-blue-500 mb-2" />
                                            <h4 className="font-bold text-sm">Endeavour</h4>
                                            <p className="text-[10px] text-app-muted mt-1">Minimalist approach close to Arch.</p>
                                        </div>
                                    </div>
                                    <p className="text-xs text-app-muted italic bg-app-fg/5 p-3 rounded-lg">
                                        Note: The AUR is the <strong>only</strong> source that typically requires compiling from source.
                                    </p>
                                </motion.div>
                            )}

                            {/* STEP 3: Configuration */}
                            {step === 3 && (
                                <motion.div
                                    key="step3"
                                    initial={{ opacity: 0, x: 20 }}
                                    animate={{ opacity: 1, x: 0 }}
                                    exit={{ opacity: 0, x: -20 }}
                                    className="w-full max-w-lg space-y-4"
                                >
                                    <div className="mb-4">
                                        <h3 className="text-2xl font-bold text-app-fg">Active Sources</h3>
                                        <p className="text-app-muted text-sm">Select which ecosystem repositories you want to search.</p>
                                    </div>

                                    <div className="space-y-3 max-h-[350px] overflow-y-auto pr-2">
                                        {repoFamilies.map((fam) => (
                                            <div
                                                key={fam.id}
                                                onClick={() => toggleRepoFamily(fam.id)}
                                                className={clsx(
                                                    "flex items-center justify-between p-4 rounded-xl border cursor-pointer transition-all",
                                                    fam.enabled
                                                        ? "bg-indigo-500/10 border-indigo-500/50 shadow-sm"
                                                        : "bg-app-card border-app-border hover:border-app-fg/30"
                                                )}
                                            >
                                                <div className="flex items-center gap-4">
                                                    <div className={clsx("p-2 rounded-lg", fam.enabled ? "bg-indigo-500 text-white" : "bg-app-fg/5 text-app-muted")}>
                                                        <fam.icon size={20} />
                                                    </div>
                                                    <div>
                                                        <h4 className="font-bold text-app-fg text-sm">{fam.name}</h4>
                                                        <p className="text-xs text-app-muted">{fam.description}</p>
                                                    </div>
                                                </div>
                                                <div className={clsx(
                                                    "w-10 h-6 rounded-full p-1 transition-colors",
                                                    fam.enabled ? "bg-indigo-500" : "bg-app-fg/20"
                                                )}>
                                                    <div className={clsx(
                                                        "w-4 h-4 bg-white rounded-full shadow-sm transition-transform",
                                                        fam.enabled ? "translate-x-4" : "translate-x-0"
                                                    )} />
                                                </div>
                                            </div>
                                        ))}
                                    </div>
                                    <p className="text-xs text-app-muted bg-app-fg/5 p-3 rounded-lg mt-2">
                                        Recommended: Enable relevant repos for your hardware. CachyOS is great for Ryzen/Intel newer CPUs.
                                    </p>
                                </motion.div>
                            )}

                            {/* STEP 4: AUR */}
                            {step === 4 && (
                                <motion.div
                                    key="step4"
                                    initial={{ opacity: 0, x: 20 }}
                                    animate={{ opacity: 1, x: 0 }}
                                    exit={{ opacity: 0, x: -20 }}
                                    className="space-y-6 max-w-lg"
                                >
                                    <h3 className="text-2xl font-bold text-app-fg">The Arch User Repository</h3>
                                    <div className="bg-amber-500/10 border border-amber-500/20 p-5 rounded-2xl">
                                        <div className="flex items-center gap-3 mb-2">
                                            <Lock size={20} className="text-amber-500" />
                                            <h4 className="font-bold text-amber-500">Powerful but Risky</h4>
                                        </div>
                                        <p className="text-sm text-app-fg/80 leading-relaxed mb-4">
                                            The AUR contains package descriptions (PKGBUILDs) that allow you to compile a package from source. It contains almost every software imaginable.
                                        </p>
                                        <ul className="text-xs text-app-muted space-y-2 list-disc pl-4">
                                            <li>Maintained by the community (users like you).</li>
                                            <li>Requires compilation (can take a long time).</li>
                                            <li>Not vetted by Arch Linux directly.</li>
                                        </ul>
                                    </div>

                                    <div
                                        onClick={() => setAurEnabled(!aurEnabled)}
                                        className={clsx(
                                            "cursor-pointer border-2 rounded-2xl p-4 transition-all hover:scale-[1.02] flex items-center justify-between",
                                            aurEnabled
                                                ? "border-amber-500 bg-amber-500/5"
                                                : "border-app-border bg-app-card/30"
                                        )}
                                    >
                                        <div>
                                            <span className="font-bold text-app-fg block">Enable AUR Support</span>
                                            <span className="text-xs text-app-muted">Allow searching and building from AUR</span>
                                        </div>
                                        <div className={clsx(
                                            "w-12 h-6 rounded-full p-1 transition-colors",
                                            aurEnabled ? "bg-amber-500" : "bg-app-fg/20"
                                        )}>
                                            <div className={clsx(
                                                "w-4 h-4 bg-white rounded-full shadow-sm transition-transform",
                                                aurEnabled ? "translate-x-6" : "translate-x-0"
                                            )} />
                                        </div>
                                    </div>
                                </motion.div>
                            )}

                            {/* STEP 5: Theme */}
                            {step === 5 && (
                                <motion.div
                                    key="step5"
                                    initial={{ opacity: 0, x: 20 }}
                                    animate={{ opacity: 1, x: 0 }}
                                    exit={{ opacity: 0, x: -20 }}
                                    className="w-full max-w-lg space-y-8"
                                >
                                    <div className="text-center">
                                        <h3 className="text-2xl font-bold text-app-fg mb-2">Final Touches</h3>
                                        <p className="text-app-muted">Customize the visual style of your store.</p>
                                    </div>

                                    <div className="grid grid-cols-2 gap-4">
                                        <button
                                            onClick={() => setThemeMode('light')}
                                            className={clsx(
                                                "p-6 rounded-2xl border-2 flex flex-col items-center gap-4 transition-all",
                                                themeMode === 'light'
                                                    ? "border-app-accent bg-app-accent/10 shadow-lg"
                                                    : "border-app-border bg-app-card hover:border-app-fg/20"
                                            )}
                                        >
                                            <Sun size={32} className={themeMode === 'light' ? "text-app-accent" : "text-app-muted"} />
                                            <span className="font-bold">Light Mode</span>
                                        </button>
                                        <button
                                            onClick={() => setThemeMode('dark')}
                                            className={clsx(
                                                "p-6 rounded-2xl border-2 flex flex-col items-center gap-4 transition-all",
                                                themeMode === 'dark'
                                                    ? "border-app-accent bg-app-accent/10 shadow-lg"
                                                    : "border-app-border bg-app-card hover:border-app-fg/20"
                                            )}
                                        >
                                            <Moon size={32} className={themeMode === 'dark' ? "text-app-accent" : "text-app-muted"} />
                                            <span className="font-bold">Dark Mode</span>
                                        </button>
                                    </div>

                                    <div className="space-y-3">
                                        <label className="text-xs font-bold text-app-muted uppercase tracking-wider block text-center">Accent Color</label>
                                        <div className="flex justify-center gap-4 flex-wrap">
                                            {[
                                                { c: '#3b82f6', n: 'Blue' },
                                                { c: '#8b5cf6', n: 'Purple' },
                                                { c: '#10b981', n: 'Green' },
                                                { c: '#f59e0b', n: 'Orange' },
                                                { c: '#ef4444', n: 'Red' },
                                            ].map((color) => (
                                                <button
                                                    key={color.c}
                                                    onClick={() => setAccentColor(color.c)}
                                                    className={clsx(
                                                        "w-10 h-10 rounded-full border-2 transition-all hover:scale-110",
                                                        accentColor === color.c ? "border-app-fg scale-110 shadow-lg ring-2 ring-offset-2 ring-app-fg/20" : "border-transparent"
                                                    )}
                                                    style={{ backgroundColor: color.c }}
                                                    title={color.n}
                                                />
                                            ))}
                                        </div>
                                    </div>
                                </motion.div>
                            )}

                        </AnimatePresence>
                    </div>

                    <div className="flex justify-between items-center pt-8 border-t border-app-border/50 mt-auto z-50 bg-app-bg">
                        <button
                            onClick={() => step > 0 && setStep(step - 1)}
                            disabled={step === 0}
                            className={clsx(
                                "text-sm font-medium transition-colors px-4 py-2 rounded-lg hover:bg-app-fg/5",
                                step === 0 ? "opacity-0 pointer-events-none" : "text-app-muted hover:text-app-fg"
                            )}
                        >
                            Back
                        </button>

                        <button
                            onClick={nextStep}
                            disabled={isSaving}
                            className={clsx(
                                "text-white px-8 py-3 rounded-xl font-bold text-sm hover:opacity-90 active:scale-95 transition-all flex items-center gap-2 shadow-xl",
                                steps[step].color.replace('bg-', 'bg-').replace('700', '600') // Match button to step color approximately
                            )}
                            style={{ backgroundColor: step === 5 ? accentColor : undefined }} // Use accent for last step
                        >
                            {isSaving ? (
                                <span>Configuring...</span>
                            ) : (
                                <>
                                    {step === steps.length - 1 ? "Finish Setup" : "Next Step"}
                                    {step < steps.length - 1 && <ChevronRight size={16} />}
                                </>
                            )}
                        </button>
                    </div>
                </div>
            </motion.div>
        </div>
    );
}
