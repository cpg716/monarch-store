import { useState, useEffect, useRef } from 'react';
import { Terminal, CheckCircle2, XCircle, Loader2, Lock, Play, Minimize2, Maximize2, ShieldCheck, RefreshCw } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { clsx } from 'clsx';

interface InstallMonitorProps {
    pkg: { name: string; source: string; } | null;
    onClose: () => void;
}

export default function InstallMonitor({ pkg, onClose }: InstallMonitorProps) {
    const [status, setStatus] = useState<'idle' | 'running' | 'success' | 'error'>('idle');
    const [password, setPassword] = useState('');
    const [logs, setLogs] = useState<string[]>([]);
    const [progress, setProgress] = useState(0);
    const [minimized, setMinimized] = useState(false);
    const logsEndRef = useRef<HTMLDivElement>(null);

    // Auto-scroll logs
    useEffect(() => {
        logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [logs, minimized]);

    // Listeners
    useEffect(() => {
        if (!pkg) return;

        const unlistenOutput = listen('install-output', (event: { payload: unknown }) => {
            if (typeof event.payload !== 'string') return;
            const line = event.payload;
            setLogs(prev => [...prev, line]);

            // Simple heuristics for progress
            // Pacman/Yay usually prints (1/5) or [ 10%]
            if (line.includes('%')) {
                const match = line.match(/(\d+)%/);
                if (match) setProgress(parseInt(match[1]));
            } else if (line.toLowerCase().includes('compiling')) {
                setProgress(prev => Math.min(prev + 1, 90)); // Slow increment
            }
        });

        const unlistenComplete = listen('install-complete', (event: { payload: string }) => {
            if (event.payload === 'success') {
                setStatus('success');
                setProgress(100);
            } else {
                setStatus('error');
            }
        });

        return () => {
            unlistenOutput.then(f => f());
            unlistenComplete.then(f => f());
        };
    }, [pkg]);

    const handleInstall = async () => {
        if (!pkg) return;
        setStatus('running');
        setLogs(['Starting installation engine...', `Target: ${pkg.name} (${pkg.source})`]);
        setProgress(5);

        try {
            await invoke('install_package', {
                name: pkg.name,
                source: pkg.source,
                password: password.length > 0 ? password : null
            });
            // The command is async spawned, completion comes via event
        } catch (e) {
            setLogs(prev => [...prev, `Error launching: ${e}`]);
            setStatus('error');
        }
    };

    if (!pkg) return null;

    if (minimized) {
        return (
            <div className="fixed bottom-4 right-4 z-50 bg-app-card border border-app-border p-4 rounded-xl shadow-2xl flex items-center gap-4 w-80 animate-in slide-in-from-bottom-4 transition-colors">
                <div className="bg-blue-500/20 p-2 rounded-lg text-blue-500 dark:text-blue-400">
                    <Loader2 size={20} className="animate-spin" />
                </div>
                <div className="flex-1">
                    <div className="text-sm font-bold text-app-fg">Installing {pkg.name}</div>
                    <div className="w-full bg-app-fg/10 h-1.5 mt-2 rounded-full overflow-hidden">
                        <div className="h-full bg-blue-500 transition-all duration-500" style={{ width: `${progress}%` }} />
                    </div>
                </div>
                <button onClick={() => setMinimized(false)} className="p-2 hover:bg-app-fg/10 rounded-lg text-app-muted">
                    <Maximize2 size={16} />
                </button>
            </div>
        );
    }
    const [isRepairing, setIsRepairing] = useState(false);
    const [repairSuccess, setRepairSuccess] = useState(false);

    // Heuristic Scan for Keyring Issues
    const hasKeyringError = logs.some(l =>
        l.includes("GPGME error") ||
        l.includes("PGP signature") ||
        l.includes("corrupted database") ||
        l.includes("invalid or corrupted")
    );

    const handleRepair = async () => {
        setIsRepairing(true);
        setLogs(prev => [...prev, '\n--- STARTING AUTO-REPAIR ---', 'Re-initializing keyring...', 'Refreshing package keys...', 'This may take a minute...']);
        try {
            const result = await invoke<string>('optimize_system');
            setLogs(prev => [...prev, result, '--- REPAIR COMPLETE ---', 'Please try installing again.']);
            setRepairSuccess(true);
        } catch (e) {
            setLogs(prev => [...prev, `Repair Failed: ${e}`]);
        } finally {
            setIsRepairing(false);
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-8 bg-app-bg/60 backdrop-blur-sm animate-in fade-in duration-200">
            <div className="w-full max-w-2xl bg-app-card border border-app-border rounded-3xl shadow-2xl overflow-hidden flex flex-col max-h-[80vh] transition-colors">
                {/* Header */}
                <div className="p-6 border-b border-app-border flex items-center justify-between bg-app-fg/5">
                    <div className="flex items-center gap-3">
                        <div className={clsx("w-10 h-10 rounded-full flex items-center justify-center",
                            status === 'success' ? "bg-green-500/20 text-green-500" :
                                status === 'error' ? "bg-red-500/20 text-red-500" :
                                    "bg-blue-500/20 text-blue-500"
                        )}>
                            {status === 'success' ? <CheckCircle2 size={20} /> :
                                status === 'error' ? <XCircle size={20} /> :
                                    <Terminal size={20} />}
                        </div>
                        <div>
                            <h2 className="text-xl font-bold text-app-fg">
                                {status === 'idle' ? 'Install Package' :
                                    status === 'success' ? 'Installation Complete' :
                                        status === 'error' ? 'Installation Failed' :
                                            `Installing ${pkg.name}`}
                            </h2>
                            <p className="text-app-muted text-sm">{pkg.source.toUpperCase()} Repository</p>
                        </div>
                    </div>
                    <div className="flex items-center gap-2">
                        {status === 'running' && (
                            <button onClick={() => setMinimized(true)} className="p-2 hover:bg-app-fg/10 rounded-lg text-app-muted transition-colors">
                                <Minimize2 size={20} />
                            </button>
                        )}
                        <button onClick={onClose} className="p-2 hover:bg-red-500/10 hover:text-red-500 rounded-lg text-app-muted transition-colors">
                            <XCircle size={20} />
                        </button>
                    </div>
                </div>

                {/* Body */}
                <div className="p-0 flex-1 overflow-hidden flex flex-col">
                    {status === 'idle' ? (
                        <div className="p-8 flex flex-col items-center justify-center space-y-6">
                            <div className="text-center space-y-2">
                                <p className="text-app-fg font-bold text-lg">
                                    Authentication Required
                                </p>
                                <p className="text-app-muted text-sm max-w-sm">
                                    {pkg.source === 'aur'
                                        ? "AUR packages require your password to build and install. Please enter it below."
                                        : "Leave the password field empty to use your system's native authentication prompt (Polkit)."}
                                </p>
                            </div>

                            <div className="w-full max-w-sm space-y-3">
                                <div className="bg-app-fg/5 p-2 rounded-xl border border-app-border flex items-center gap-3 px-4 focus-within:border-blue-500/50 transition-colors">
                                    <Lock size={18} className="text-app-muted" />
                                    <input
                                        type="password"
                                        placeholder={pkg.source === 'aur' ? "Sudo Password" : "Password (Optional)"}
                                        className="bg-transparent border-none outline-none text-app-fg w-full py-2 placeholder:text-app-muted/40"
                                        value={password}
                                        onChange={e => setPassword(e.target.value)}
                                        onKeyDown={e => e.key === 'Enter' && handleInstall()}
                                    />
                                </div>

                                {pkg.source !== 'aur' && !password && (
                                    <p className="text-[10px] text-blue-500/80 text-center font-medium animate-pulse">
                                        Fingerprint / System Prompt will be used
                                    </p>
                                )}
                            </div>

                            <div className="w-full max-w-sm flex gap-3">
                                <button
                                    onClick={onClose}
                                    className="flex-1 bg-app-fg/5 hover:bg-app-fg/10 text-app-fg font-medium py-3 rounded-xl transition-colors"
                                >
                                    Cancel
                                </button>
                                <button
                                    onClick={handleInstall}
                                    className="flex-[2] bg-blue-600 hover:bg-blue-500 text-white font-bold py-3 rounded-xl flex items-center justify-center gap-2 shadow-lg shadow-blue-900/20 transition-all active:scale-95"
                                >
                                    <Play size={18} fill="currentColor" /> {password ? 'Verify & Install' : 'Install'}
                                </button>
                            </div>
                        </div>
                    ) : (
                        <div className="flex-1 flex flex-col h-full bg-app-bg transition-colors">
                            {/* Progress Bar Area */}
                            <div className="bg-app-card p-6 border-b border-app-border">
                                {hasKeyringError && status === 'error' && !repairSuccess && (
                                    <div className="mb-4 p-3 bg-amber-500/10 border border-amber-500/20 rounded-xl flex items-center justify-between animate-in slide-in-from-top-2">
                                        <div className="flex items-center gap-3">
                                            <div className="p-2 bg-amber-500/20 rounded-lg text-amber-500">
                                                <ShieldCheck size={18} />
                                            </div>
                                            <div>
                                                <h4 className="font-bold text-amber-500 text-sm">Keyring Issue Detected</h4>
                                                <p className="text-xs text-app-muted">Your system keys seem outdated or corrupted.</p>
                                            </div>
                                        </div>
                                        <button
                                            onClick={handleRepair}
                                            disabled={isRepairing}
                                            className="px-4 py-2 bg-amber-500 hover:bg-amber-600 text-white text-xs font-bold rounded-lg transition-colors flex items-center gap-2 shadow-lg shadow-amber-500/20"
                                        >
                                            {isRepairing ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}
                                            {isRepairing ? "Fixing..." : "Auto-Repair"}
                                        </button>
                                    </div>
                                )}

                                <div className="flex justify-between text-sm text-app-muted mb-2">
                                    <span>Status: {status.toUpperCase()}</span>
                                    <span>{progress}%</span>
                                </div>
                                <div className="w-full bg-app-fg/10 h-2 rounded-full overflow-hidden">
                                    <div
                                        className={clsx("h-full transition-all duration-300",
                                            status === 'success' ? "bg-green-500" :
                                                status === 'error' ? "bg-red-500" : "bg-blue-500 relative"
                                        )}
                                        style={{ width: `${progress}%` }}
                                    >
                                        {status === 'running' && <div className="absolute inset-0 bg-white/20 animate-pulse" />}
                                    </div>
                                </div>
                            </div>

                            {/* Logs Terminal */}
                            <div className="flex-1 overflow-auto p-4 font-mono text-xs text-app-muted space-y-1 scrollbar-thin transition-colors">
                                {logs.map((log, i) => (
                                    <div key={i} className="break-all whitespace-pre-wrap">
                                        <span className="text-app-muted opacity-50 mr-2">[{new Date().toLocaleTimeString()}]</span>
                                        {log}
                                    </div>
                                ))}
                                <div ref={logsEndRef} />
                            </div>
                        </div>
                    )}
                </div>

                {/* Footer Actions */}
                {(status === 'success' || (status === 'error' && !isRepairing)) && (
                    <div className="p-4 bg-app-fg/5 border-t border-app-border flex justify-end gap-3">
                        {status === 'error' && !hasKeyringError && (
                            <button
                                onClick={handleInstall}
                                className="bg-app-accent hover:bg-app-accent/80 text-white px-6 py-2 rounded-lg font-medium transition-colors shadow-lg shadow-app-accent/20"
                            >
                                Retry
                            </button>
                        )}
                        <button
                            onClick={onClose}
                            className="bg-app-fg/10 hover:bg-app-fg/20 text-app-fg px-6 py-2 rounded-lg font-medium transition-colors"
                        >
                            Close
                        </button>
                    </div>
                )}
            </div>
        </div>
    );
}
