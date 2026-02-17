import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { motion, AnimatePresence } from 'framer-motion';
import { Database, ShieldCheck, Zap, Cpu } from 'lucide-react';
import { useDistro } from '../hooks/useDistro';
import logoIcon from '../assets/logo.png';

const GENERIC_TIPS = [
    "Optimizing search index...",
    "Did you know? Chaotic-AUR builds packages automatically!",
    "Loading 'Essentials' collection...",
    "Preparing the identity matrix...",
    "Scanning for local updates..."
];

const DISTRO_TIPS: Record<string, string[]> = {
    manjaro: [
        "Respecting Manjaro stability branches...",
        "Checking Manjaro official mirrors...",
        "Applying Manjaro Stability Guard policies...",
    ],
    arch: [
        "Syncing with Arch Linux mirrors...",
        "Refreshing Arch core databases...",
        "Verifying ALPM integrity...",
    ],
    cachyos: [
        "Detection: v3/v4 optimized x86_64 binaries...",
        "Connecting to CachyOS performance mirrors...",
        "Enabling CachyOS system optimizations...",
    ],
    endeavouros: [
        "Respecting EndeavourOS repository priority...",
        "Syncing with EndeavourOS mirrors...",
    ],
    garuda: [
        "Initializing Garuda gaming enhancements...",
        "Connecting to Garuda chaotic mirrors...",
    ]
};

export default function LoadingScreen() {
    const { distro } = useDistro();
    const [tipIndex, setTipIndex] = useState(0);
    const [status, setStatus] = useState("Initializing system...");
    const [progress, setProgress] = useState(0);

    const tips = [
        ...(DISTRO_TIPS[typeof distro.id === 'string' ? distro.id : 'arch'] || DISTRO_TIPS['arch'] || []),
        ...GENERIC_TIPS
    ];

    // Add CPU-aware tips
    if (distro.cpu_tier === 'v4') tips.unshift("Performance: AVX-512 instructions detected and primed.");
    if (distro.cpu_tier === 'v3') tips.unshift("Performance: x86-64-v3 (AVX2) optimizations enabled.");

    // Rotate tips every 3 seconds for variety
    useEffect(() => {
        const interval = setInterval(() => {
            setTipIndex(prev => (prev + 1) % tips.length);
        }, 3000);
        return () => clearInterval(interval);
    }, [tips.length]);

    // Listen for real-time progress from backend; map status text to bar so startup always shows movement
    useEffect(() => {
        let unlisten: (() => void) | undefined;
        listen<string>('sync-progress', (event) => {
            const msg = event.payload;
            setStatus(msg);
            if (msg.includes("complete") || msg.includes("up to date")) setProgress(95);
            else if (msg.includes("Chaotic-AUR")) setProgress(90);
            else if (msg.includes("Updating")) setProgress((p) => Math.min(p + 8, 85));
            else if (msg.includes("Syncing")) setProgress((p) => Math.max(p, 25));
            else if (msg.includes("Loading Essentials")) setProgress(55);
            else if (msg.includes("Analyzing")) setProgress(75);
            else if (msg.includes("Preparing") || msg.includes("Preparing...")) setProgress(20);
            else if (msg.includes("Checking health") || msg.includes("Checking system")) setProgress(10);
            else if (msg.includes("Checking lock") || msg.includes("Unlock")) setProgress(8);
        }).then((fn) => { unlisten = fn; });
        return () => { unlisten?.(); };
    }, []);

    // If no backend progress within 2s, nudge bar so user sees activity (avoids "stuck at 0" on slow first steps)
    useEffect(() => {
        const t = setTimeout(() => {
            setProgress((p) => (p < 5 ? 5 : p));
        }, 2000);
        return () => clearTimeout(t);
    }, []);

    // Map repo ids to friendly names
    const getRepoIcon = (repo: string) => {
        const lower = repo.toLowerCase();
        if (lower.includes('chaotic')) return <Zap size={12} className="text-amber-500" />;
        if (lower.includes('cachyos')) return <Zap size={12} className="text-emerald-500" />;
        if (lower.includes('manjaro')) return <ShieldCheck size={12} className="text-green-500" />;
        return <Database size={12} className="text-indigo-500" />;
    };

    return (
        <div className="fixed inset-0 z-50 bg-app-bg flex flex-col items-center justify-center text-app-fg p-8 overflow-hidden">
            <div className="absolute inset-0 bg-gradient-to-br from-blue-500/10 via-transparent to-purple-500/10 pointer-events-none" />

            {/* Animated particles background */}
            <div className="absolute inset-0 overflow-hidden pointer-events-none opacity-20">
                <div className="absolute top-1/4 left-1/4 w-96 h-96 bg-blue-500/20 blur-[120px] rounded-full animate-pulse" />
                <div className="absolute bottom-1/4 right-1/4 w-96 h-96 bg-purple-500/20 blur-[120px] rounded-full animate-pulse" style={{ animationDelay: '1s' }} />
            </div>

            <div className="relative flex flex-col items-center w-full max-w-md text-center">
                <div className="mb-12 relative group">
                    <div className="absolute inset-[-40%] bg-gradient-to-br from-blue-500/40 via-violet-500/30 to-cyan-500/40 blur-3xl rounded-full animate-pulse group-hover:scale-110 transition-transform duration-1000" />
                    <motion.div
                        initial={{ scale: 0.8, opacity: 0 }}
                        animate={{ scale: 1, opacity: 1 }}
                        transition={{ type: "spring", stiffness: 100 }}
                        className="relative z-10 w-32 h-32 flex items-center justify-center"
                    >
                        <img src={logoIcon} alt="MonARCH" className="w-full h-full object-contain drop-shadow-2xl animate-flap" />
                    </motion.div>
                </div>

                <div className="space-y-2 mb-8 w-full">
                    <h1 className="text-3xl font-black bg-gradient-to-r from-blue-400 via-indigo-400 to-purple-500 bg-clip-text text-transparent">
                        Preparing MonARCH
                    </h1>
                    <p className="text-app-muted text-sm font-medium h-4">{status}</p>
                </div>

                {/* Real Progress Bar */}
                <div className="w-full h-1.5 bg-app-subtle rounded-full overflow-hidden mb-10 border border-app-border/30">
                    <motion.div
                        className="h-full bg-gradient-to-r from-blue-500 to-purple-600"
                        initial={{ width: "0%" }}
                        animate={{ width: `${progress}%` }}
                        transition={{ duration: 0.5 }}
                    />
                </div>

                <div className="h-12 overflow-hidden relative w-full mb-8 italic">
                    <AnimatePresence mode="wait">
                        <motion.p
                            key={tipIndex}
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            exit={{ opacity: 0, y: -10 }}
                            className="text-app-muted text-xs absolute inset-0 flex items-center justify-center px-4"
                        >
                            " {tips[tipIndex]} "
                        </motion.p>
                    </AnimatePresence>
                </div>

                {/* Progress/Status indicators */}
                <div className="flex flex-wrap justify-center gap-3 text-xs text-app-muted font-mono">
                    {distro.active_repos.slice(0, 3).map(repo => (
                        <div key={repo} className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-app-subtle border border-app-border/20">
                            {getRepoIcon(repo)}
                            <span>{repo}</span>
                        </div>
                    ))}
                    {distro.cpu_tier !== 'v1' && (
                        <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-app-subtle border border-app-border/20">
                            <Cpu size={12} className="text-orange-400" />
                            <span>{distro.cpu_tier} optimized</span>
                        </div>
                    )}
                </div>
            </div>

            <div className="absolute bottom-8 text-xs text-app-muted opacity-50">
                Wait time depends on your internet connection
            </div>
        </div>
    );
}
