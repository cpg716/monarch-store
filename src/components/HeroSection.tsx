import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertCircle, Terminal, RefreshCw, X } from 'lucide-react';
import logoFull from '../assets/logo_full.png';
import { motion, AnimatePresence } from 'framer-motion';

interface HealthIssue {
    category: string;
    severity: string;
    message: string;
    action_label: string;
    action_command?: string;
}

export default function HeroSection() {
    const [issues, setIssues] = useState<HealthIssue[]>([]);
    const [dismissed, setDismissed] = useState(false);
    const [fixing, setFixing] = useState<string | null>(null);

    useEffect(() => {
        invoke<HealthIssue[]>('check_system_health').then(setIssues).catch(console.error);
    }, []);

    const handleAction = async (issue: HealthIssue) => {
        setFixing(issue.message);
        if (issue.action_command) {
            try {
                // In a real app we might use a terminal plugin or handle it in rust
                // For now we'll trigger the command and refresh
                await invoke('launch_app', { name: "alacritty", args: ["-e", "bash", "-c", `${issue.action_command}; read -p 'Done! Press enter to close...'`] });
                setTimeout(async () => {
                    const newIssues = await invoke<HealthIssue[]>('check_system_health');
                    setIssues(newIssues);
                }, 2000);
            } catch (e) {
                console.error("Fix failed", e);
            }
        } else if (issue.action_label === "Refresh Repositories") {
            const interval = localStorage.getItem('sync-interval-hours') || '3';
            await invoke('trigger_repo_sync', { syncIntervalHours: parseInt(interval, 10), force: true });
            const newIssues = await invoke<HealthIssue[]>('check_system_health');
            setIssues(newIssues);
        }
        setFixing(null);
    };

    return (
        <div className="flex flex-col gap-4 mb-8">
            <AnimatePresence>
                {!dismissed && issues.length > 0 && (
                    <motion.div
                        initial={{ opacity: 0, height: 0 }}
                        animate={{ opacity: 1, height: 'auto' }}
                        exit={{ opacity: 0, height: 0 }}
                        className="bg-red-500/10 border border-red-500/20 rounded-3xl p-6 relative overflow-hidden"
                    >
                        <div className="absolute top-4 right-4 focus:outline-none">
                            <button onClick={() => setDismissed(true)} className="p-1 rounded-full hover:bg-red-500/10 text-red-500/50 hover:text-red-500 transition-colors">
                                <X size={16} />
                            </button>
                        </div>

                        <div className="flex items-start gap-4">
                            <div className="p-3 rounded-2xl bg-red-500/20 text-red-500 flex-shrink-0">
                                <AlertCircle size={24} />
                            </div>
                            <div className="flex-1">
                                <h3 className="font-bold text-red-500 mb-1">System Health Alert</h3>
                                <div className="space-y-3">
                                    {issues.map((issue, idx) => (
                                        <div key={idx} className="flex flex-col md:flex-row md:items-center justify-between gap-4 py-2 border-t border-red-500/10">
                                            <p className="text-sm text-app-fg font-medium">{issue.message}</p>
                                            <button
                                                disabled={!!fixing}
                                                onClick={() => handleAction(issue)}
                                                className="flex items-center gap-2 px-4 py-2 rounded-xl bg-red-500 text-white text-xs font-bold hover:bg-red-600 transition-all shadow-lg shadow-red-500/20 disabled:opacity-50"
                                            >
                                                {fixing === issue.message ? (
                                                    <RefreshCw size={14} className="animate-spin" />
                                                ) : issue.action_command ? (
                                                    <Terminal size={14} />
                                                ) : (
                                                    <RefreshCw size={14} />
                                                )}
                                                {issue.action_label}
                                            </button>
                                        </div>
                                    ))}
                                </div>
                            </div>
                        </div>
                    </motion.div>
                )}
            </AnimatePresence>

            <div className="relative w-full rounded-3xl overflow-hidden group select-none shadow-lg">
                {/* Light Background */}
                <div className="absolute inset-0 bg-gradient-to-br from-purple-300 via-blue-300 to-cyan-200 transition-all duration-500 group-hover:scale-105" />

                {/* Animated Shapes */}
                <div className="absolute top-[-50%] left-[-20%] w-[800px] h-[800px] rounded-full bg-white/30 blur-3xl animate-pulse" />
                <div className="absolute bottom-[-20%] right-[-10%] w-[600px] h-[600px] rounded-full bg-purple-400/20 blur-3xl" />

                {/* Glass Overlay */}
                <div className="absolute inset-0 bg-white/20 backdrop-blur-[2px]" />

                {/* Content Container */}
                <div className="relative z-10 flex flex-row items-center justify-center gap-8 px-10 py-6 text-slate-800">
                    <div className="flex-shrink-0 animate-fade-in-up" style={{ animationDelay: '0.1s' }}>
                        <img
                            src={logoFull}
                            alt="MonARCH Store"
                            className="h-24 object-contain"
                            style={{
                                filter: 'drop-shadow(0 4px 6px rgba(0,0,0,0.2))'
                            }}
                        />
                    </div>

                    <div className="flex flex-col text-left max-w-2xl">
                        <h2 className="text-3xl font-black text-slate-800 mb-2 tracking-tight animate-fade-in-up leading-tight" style={{ animationDelay: '0.2s' }}>
                            Order from <span className="text-transparent bg-clip-text bg-gradient-to-r from-purple-600 to-indigo-600">Chaos</span>.
                        </h2>

                        <div className="text-sm text-slate-700 leading-relaxed animate-fade-in-up font-medium" style={{ animationDelay: '0.25s' }}>
                            <p>
                                The ultimate <strong>Chaotic-AUR</strong> interface. Pre-built binaries, fast downloads, and easy installation.
                            </p>
                            <p className="mt-1 text-slate-600 font-normal text-xs">
                                Supports <strong>Arch Official</strong>, <strong>AUR</strong>, <strong>CachyOS</strong>, <strong>Garuda</strong>, <strong>Manjaro</strong>, & <strong>EndeavourOS</strong>.
                            </p>
                        </div>
                    </div>
                </div>

                <div className="absolute right-0 top-1/2 -translate-y-1/2 w-96 h-96 opacity-15 pointer-events-none">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="0.5" className="w-full h-full text-purple-600 rotate-12">
                        <path d="M12 2L2 22h20L12 2zm0 3.5L18.5 20h-13L12 5.5z" />
                    </svg>
                </div>
            </div>
        </div>
    );
}
