import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ShieldCheck, Zap } from 'lucide-react';

import { motion } from 'framer-motion';



interface SystemInfo {
    kernel: string;
    distro: string;
    pacman_version: string;
    chaotic_enabled: boolean;
    cpu_optimization: string;
}

export default function HeroSection() {
    const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);

    useEffect(() => {
        invoke<SystemInfo>('get_system_info').then(setSystemInfo).catch(console.error);
    }, []);

    return (
        <div className="flex flex-col gap-4 mb-8">
            <div className="relative w-full min-h-[160px] rounded-[40px] overflow-hidden group select-none shadow-2xl border border-white/10 flex items-center justify-center">
                {/* Background */}
                <div className="absolute inset-0 bg-gradient-to-br from-[#0f172a] via-[#1e1b4b] to-[#0f172a] transition-all duration-700" />

                <div className="absolute inset-0 bg-[url('/grid.svg')] opacity-20" />

                {/* Glow Effects */}
                <div className="absolute top-[-20%] left-[-10%] w-[600px] h-[600px] rounded-full bg-blue-500/10 blur-[100px] animate-pulse duration-[8000ms]" />
                <div className="absolute bottom-[-20%] right-[-10%] w-[500px] h-[500px] rounded-full bg-purple-500/10 blur-[100px]" />

                {/* Content Container */}
                <div className="relative z-10 flex flex-col items-center justify-center text-center px-8 py-10 text-white max-w-4xl mx-auto">
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
                            className="text-4xl md:text-6xl font-black text-white tracking-tight leading-none drop-shadow-xl"
                        >
                            Order from <span className="text-transparent bg-clip-text bg-gradient-to-r from-blue-400 to-purple-400">Chaos</span>.
                        </motion.h2>

                        <motion.div
                            initial={{ y: 10, opacity: 0 }}
                            animate={{ y: 0, opacity: 1 }}
                            transition={{ delay: 0.2 }}
                            className="flex flex-wrap items-center justify-center gap-3 mt-4"
                        >
                            {/* System Status Badge */}
                            <div className="flex items-center gap-2 px-4 py-2 rounded-2xl bg-teal-500/10 border border-teal-500/20 backdrop-blur-xl">
                                <ShieldCheck size={14} className="text-teal-400" />
                                <span className="text-[10px] font-black uppercase tracking-widest text-teal-400/90">
                                    System Optimized
                                </span>
                            </div>

                            {/* CPU Optimization Badge */}
                            {systemInfo && (
                                <div className="flex items-center gap-2 px-4 py-2 rounded-2xl bg-blue-500/10 border border-blue-500/20 backdrop-blur-xl">
                                    <Zap size={14} className="text-blue-400 fill-blue-400" />
                                    <span className="text-[10px] font-black uppercase tracking-widest text-blue-400/90">
                                        {systemInfo.cpu_optimization} Native
                                    </span>
                                </div>
                            )}
                        </motion.div>

                        <motion.p
                            initial={{ y: 10, opacity: 0 }}
                            animate={{ y: 0, opacity: 1 }}
                            transition={{ delay: 0.3 }}
                            className="text-sm md:text-base text-indigo-200/60 font-medium max-w-2zl leading-relaxed mt-4"
                        >
                            Get apps from <strong>Chaotic-AUR</strong>, <strong>CachyOS</strong>, and the <strong>AUR</strong> with a single click. High performance guaranteed by native CPU optimizations.
                        </motion.p>
                    </motion.div>
                </div>
            </div>
        </div>
    );
}
