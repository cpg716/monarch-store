import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Server, Database, Download } from 'lucide-react';
import logo from '../assets/logo.png';

const LOADING_TIPS = [
    "Syncing with Arch Linux mirrors...",
    "Updating Chaotic-AUR binaries...",
    "Refreshing package databases...",
    "Checking for latest updates...",
    "Optimizing search index...",
    "Did you know? Chaotic-AUR builds packages automatically!",
    "Loading 'Essentials' collection..."
];

export default function LoadingScreen() {
    const [tipIndex, setTipIndex] = useState(0);

    // Rotate tips every 2 seconds
    useEffect(() => {
        const interval = setInterval(() => {
            setTipIndex(prev => (prev + 1) % LOADING_TIPS.length);
        }, 2000);
        return () => clearInterval(interval);
    }, []);

    return (
        <div className="fixed inset-0 z-50 bg-app-bg flex flex-col items-center justify-center text-app-fg p-8">
            {/* Background elements */}
            <div className="absolute inset-0 bg-gradient-to-br from-blue-500/5 via-transparent to-purple-500/5 pointer-events-none" />

            <div className="relative flex flex-col items-center max-w-md text-center">
                {/* Main Icon - Animated Logo */}
                <div className="mb-8 relative">
                    {/* Glow effect */}
                    <div className="absolute inset-[-30%] bg-gradient-to-br from-blue-500/40 via-violet-500/30 to-cyan-500/40 blur-3xl rounded-full animate-pulse" />
                    <div className="relative z-10 w-28 h-28 flex items-center justify-center">
                        <img
                            src={logo}
                            alt="MonARCH"
                            className="w-full h-full object-contain animate-pulse drop-shadow-2xl"
                        />
                    </div>
                </div>

                <h1 className="text-2xl font-bold mb-2 bg-gradient-to-r from-blue-400 to-purple-500 bg-clip-text text-transparent">
                    Updating Repositories
                </h1>

                <div className="h-6 overflow-hidden relative w-full mb-8">
                    <AnimatePresence mode="wait">
                        <motion.p
                            key={tipIndex}
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            exit={{ opacity: 0, y: -10 }}
                            className="text-app-muted text-sm absolute inset-0 flex items-center justify-center"
                        >
                            {LOADING_TIPS[tipIndex]}
                        </motion.p>
                    </AnimatePresence>
                </div>

                {/* Progress/Status indicators */}
                <div className="flex gap-4 text-xs text-app-muted font-mono">
                    <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-app-subtle">
                        <Server size={12} />
                        <span>chaotic-aur</span>
                    </div>
                    <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-app-subtle">
                        <Database size={12} />
                        <span>core</span>
                    </div>
                    <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-app-subtle">
                        <Download size={12} />
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
