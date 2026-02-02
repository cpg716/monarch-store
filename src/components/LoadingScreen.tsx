import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Database, ShieldCheck, Zap } from 'lucide-react';
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
        ...(DISTRO_TIPS[distro.id] || DISTRO_TIPS['arch'] || []),
        ...GENERIC_TIPS
    ];

    // Rotate tips every 3 seconds for variety
    useEffect(() => {
        const interval = setInterval(() => {
            setTipIndex(prev => (prev + 1) % tips.length);
        }, 3000);
        return () => clearInterval(interval);
    }, [tips.length]);

    // Listen for real-time progress from backend
    useEffect(() => {
        let unlisten: any;
        const setupListener = async () => {
            const { listen } = await import('@tauri-apps/api/event');
            unlisten = await listen<string>('sync-progress', (event) => {
                setStatus(event.payload);
                // Simple heuristic for progress bar
                if (event.payload.includes("Syncing")) setProgress(20);
                if (event.payload.includes("Updating")) setProgress(prev => Math.min(prev + 10, 80));
                if (event.payload.includes("Chaotic-AUR")) setProgress(90);
                if (event.payload.includes("complete")) setProgress(100);
            });
        };
        setupListener();
        return () => { if (unlisten) unlisten(); };
    }, []);

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
                <div className="flex gap-4 text-xs text-app-muted font-mono">
                    <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-app-subtle border border-app-border/20">
                        <Zap size={12} className="text-amber-500" />
                        <span>chaotic-aur</span>
                    </div>
                    <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-app-subtle border border-app-border/20">
                        <ShieldCheck size={12} className="text-blue-500" />
                        <span>{distro.id === 'arch' ? 'arch' : distro.id}</span>
                    </div>
                    <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-app-subtle border border-app-border/20">
                        <Database size={12} className="text-indigo-500" />
                        <span>extra</span>
                    </div>
                </div>
            </div>

            <div className="absolute bottom-8 text-xs text-app-muted opacity-50">
                Wait time depends on your internet connection
            </div>
        </div>
    );
}
