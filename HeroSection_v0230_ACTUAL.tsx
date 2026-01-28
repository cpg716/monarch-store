import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertCircle, Terminal, RefreshCw, X } from 'lucide-react';

import { motion, AnimatePresence } from 'framer-motion';

interface HealthIssue {
    category: string;
    severity: string;
    message: string;
    action_label: string;
    action_command?: string;
}

export default function HeroSection({ onNavigateToFix }: { onNavigateToFix?: () => void }) {
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
                // Map frontend action labels to backend repair commands
                let cmd = issue.action_command;
                if (cmd === 'keyring' || cmd === 'trigger_repair_flow') {
                    cmd = 'repair_reset_keyring';
                }

                if (['repair_reset_keyring', 'repair_unlock_pacman', 'repair_emergency_sync', 'install_monarch_policy'].includes(cmd)) {
                    await invoke(cmd, { password: null });
                } else {
                    // Fallback for unknown commands
                    await invoke('launch_app', { name: "alacritty", args: ["-e", "bash", "-c", `${issue.action_command}; read -p 'Done! Press enter to close...'`] });
                }

                // Refresh health status
                setTimeout(async () => {
                    const newIssues = await invoke<HealthIssue[]>('check_system_health');
                    setIssues(newIssues);
                }, 1000);
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
                        className="bg-red-500/10 border border-red-500/20 rounded-3xl p-6 relative overflow-hidden cursor-pointer hover:bg-red-500/15 transition-colors"
                        onClick={() => {
                            if (onNavigateToFix) onNavigateToFix();
                        }}
                    >
                        <div className="absolute top-4 right-4 focus:outline-none">
                            <button onClick={(e) => {
                                e.stopPropagation();
                                setDismissed(true);
                            }} className="p-1 rounded-full hover:bg-red-500/10 text-red-500/50 hover:text-red-500 transition-colors">
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

            <div className="relative w-full min-h-[140px] rounded-3xl overflow-hidden group select-none shadow-2xl border border-white/10 flex items-center justify-center">
                {/* Background */}
                <div className="absolute inset-0 bg-gradient-to-br from-[#0f172a] via-[#1e1b4b] to-[#0f172a] transition-all duration-700" />

                <div className="absolute inset-0 bg-[url('/grid.svg')] opacity-20" />

                {/* Glow Effects */}
                <div className="absolute top-[-20%] left-[-10%] w-[600px] h-[600px] rounded-full bg-blue-500/10 blur-[100px] animate-pulse duration-[8000ms]" />
                <div className="absolute bottom-[-20%] right-[-10%] w-[500px] h-[500px] rounded-full bg-purple-500/10 blur-[100px]" />

                {/* Content Container */}
                <div className="relative z-10 flex flex-col items-center justify-center text-center px-6 py-8 text-white max-w-4xl mx-auto">
                    <motion.div
                        initial={{ opacity: 0, scale: 0.95 }}
                        animate={{ opacity: 1, scale: 1 }}
                        transition={{ duration: 0.4 }}
                        className="flex flex-col items-center gap-2"
                    >
                        <motion.h2
                            initial={{ y: 10, opacity: 0 }}
                            animate={{ y: 0, opacity: 1 }}
                            transition={{ delay: 0.1 }}
                            className="text-3xl md:text-5xl font-black text-white tracking-tight leading-none drop-shadow-xl"
                        >
                            Order from <span className="text-transparent bg-clip-text bg-gradient-to-r from-blue-400 to-purple-400">Chaos</span>.
                        </motion.h2>

                        <motion.p
                            initial={{ y: 10, opacity: 0 }}
                            animate={{ y: 0, opacity: 1 }}
                            transition={{ delay: 0.2 }}
                            className="text-sm md:text-base text-indigo-200/80 font-medium max-w-2xl leading-relaxed mt-2"
                        >
                            Get apps from <strong>Chaotic-AUR</strong>, <strong>CachyOS</strong>, <strong>Manjaro</strong>, <strong>Garuda</strong>, and <strong>EndeavourOS</strong> with a single click. Also features a powerful <strong>AUR Native Builder</strong>.
                        </motion.p>
                    </motion.div>
                </div>
            </div>
        </div>
    );
}
