import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
    LockOpen,
    Key,
    RefreshCw,
    Terminal,
    CheckCircle,
    AlertTriangle,
} from "lucide-react";

export default function SystemHealth() {
    const [logs, setLogs] = useState<string[]>([]);
    const [isLocked, setIsLocked] = useState(false);
    const [loading, setLoading] = useState(false);
    const [password, setPassword] = useState("");
    const [showPasswordInput, setShowPasswordInput] = useState(false);
    const [pendingAction, setPendingAction] = useState<string | null>(null);

    useEffect(() => {
        checkLock();
        const unlisten = listen("repair-log", (event) => {
            setLogs((prev) => [...prev, event.payload as string]);
        });
        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    const checkLock = async () => {
        try {
            const locked = await invoke("check_pacman_lock");
            setIsLocked(locked as boolean);
        } catch (e) {
            console.error(e);
        }
    };

    const handleAction = async (action: string) => {
        setPendingAction(action);
        setShowPasswordInput(true);
        setLogs([]); // Clear logs for new run
    };

    const executePendingAction = async () => {
        if (!pendingAction) return;
        setLoading(true);
        setShowPasswordInput(false);

        // Add banner
        setLogs((p) => [...p, `>>> STARTING: ${pendingAction.toUpperCase()} ...`]);

        try {
            let cmd = "";
            switch (pendingAction) {
                case "unlock":
                    cmd = "repair_unlock_pacman";
                    break;
                case "keyring":
                    cmd = "repair_reset_keyring";
                    break;
                case "emergency_sync":
                    cmd = "repair_emergency_sync";
                    break;
                case "orphans":
                    // Special Handling for Orphans (Scan first)
                    try {
                        setLogs(p => [...p, "Scanning for orphans..."]);
                        const orphans = await invoke<string[]>("get_orphans");
                        if (orphans && orphans.length > 0) {
                            setLogs(p => [...p, `Found ${orphans.length} orphans: ${orphans.join(", ")}`]);
                            // We need a specific backend command that takes 'orphans' arg or just 'remove_all_orphans'
                            // Since 'remove_orphans' takes args, simpler to use a dedicated cleanup command or assume the backend handles it.
                            // Let's assume we invoke 'remove_orphans' with the list.
                            await invoke("remove_orphans", { orphans });
                            setLogs(p => [...p, ">>> SUCCESS: Orphans removed."]);
                        } else {
                            setLogs(p => [...p, "System is clean. No orphans found."]);
                        }
                    } catch (e) {
                        throw e;
                    }
                    cmd = ""; // Already handled above
                    break;
                case "cache":
                    cmd = "clear_cache";
                    break;
                case "reset_config":
                    cmd = "reset_pacman_conf";
                    break;
            }

            if (cmd) {
                await invoke(cmd, { password: password || null });
                setLogs((p) => [...p, `>>> SUCCESS: ${pendingAction.toUpperCase()} COMPLETED.`]);
            }
        } catch (e) {
            setLogs((p) => [...p, `>>> ERROR: ${e}`]);
        } finally {
            setLoading(false);
            setPendingAction(null);
            checkLock(); // Refresh state
        }
    };

    return (
        <div className="p-6 max-w-4xl mx-auto space-y-8 animate-in fade-in duration-500">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-red-500 to-orange-500">
                        System Repair
                    </h1>
                    <p className="text-gray-400 mt-2 text-base">
                        If your system is acting weird or updates are failing, try these tools. They are safe to use.
                    </p>
                </div>
            </div>

            {/* Health Status Cards */}
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className={`p-4 rounded-xl border ${isLocked ? "bg-red-500/10 border-red-500/50" : "bg-green-500/10 border-green-500/50"} flex items-center gap-4`}>
                    {isLocked ? <AlertTriangle className="text-red-400" size={24} /> : <CheckCircle className="text-green-400" size={24} />}
                    <div>
                        <div className="font-bold text-lg">Pacman Lock Status</div>
                        <div className="text-sm opacity-80">{isLocked ? "LOCKED (db.lck exists)" : "Clean (No lock file)"}</div>
                    </div>
                </div>
            </div>

            {/* Actions Grid */}
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <button
                    onClick={() => handleAction("unlock")}
                    disabled={loading}
                    className="p-6 bg-slate-800 rounded-xl hover:bg-slate-700 transition flex flex-col items-center gap-3 border border-slate-700 hover:border-blue-500/50"
                >
                    <LockOpen className="text-blue-400" size={32} />
                    <span className="font-semibold text-lg">Fix Stuck Updates</span>
                    <span className="text-xs text-center text-gray-400">Updates stuck at 0%? Click here to unlock the database.</span>
                </button>

                <button
                    onClick={() => handleAction("keyring")}
                    disabled={loading}
                    className="p-6 bg-slate-800 rounded-xl hover:bg-slate-700 transition flex flex-col items-center gap-3 border border-slate-700 hover:border-purple-500/50"
                >
                    <Key className="text-purple-400" size={32} />
                    <span className="font-semibold text-lg">Fix Security Keys</span>
                    <span className="text-xs text-center text-gray-400">Solves "invalid signature" or "corrupted package" errors.</span>
                </button>

                <button
                    onClick={() => handleAction("emergency_sync")}
                    disabled={loading}
                    className="p-6 bg-slate-800 rounded-xl hover:bg-slate-700 transition flex flex-col items-center gap-3 border border-slate-700 hover:border-red-500/50"
                >
                    <RefreshCw className="text-red-400" size={32} />
                    <span className="font-semibold text-lg">Force System Update</span>
                    <span className="text-xs text-center text-gray-400">Prevents "partial upgrade" errors by refreshing everything.</span>
                </button>
            </div>

            {/* Maintenance Section (Migrated from Settings) */}
            <div>
                <h2 className="text-2xl font-bold mb-4 flex items-center gap-2">
                    <span className="text-blue-500">Device Care</span>
                    <hr className="flex-1 border-slate-800 ml-4" />
                </h2>
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                    <button
                        onClick={() => handleAction("orphans")}
                        disabled={loading}
                        className="p-6 bg-slate-800 rounded-xl hover:bg-slate-700 transition flex flex-col items-center gap-3 border border-slate-700 hover:border-blue-500/50"
                    >
                        <Terminal className="text-blue-400" size={32} />
                        <span className="font-semibold text-lg">Remove Unused Files</span>
                        <span className="text-xs text-center text-gray-400">Safe cleanup of old leftovers.</span>
                    </button>

                    <button
                        onClick={() => handleAction("cache")}
                        disabled={loading}
                        className="p-6 bg-slate-800 rounded-xl hover:bg-slate-700 transition flex flex-col items-center gap-3 border border-slate-700 hover:border-orange-500/50"
                    >
                        <RefreshCw className="text-orange-400" size={32} />
                        <span className="font-semibold text-lg">Free Disk Space</span>
                        <span className="text-xs text-center text-gray-400">Deletes temporary downloads.</span>
                    </button>

                    <button
                        onClick={() => handleAction("reset_config")}
                        disabled={loading}
                        className="p-6 bg-slate-800 rounded-xl hover:bg-red-900/20 transition flex flex-col items-center gap-3 border border-slate-700 hover:border-red-500"
                    >
                        <AlertTriangle className="text-red-500" size={32} />
                        <span className="font-semibold text-lg text-red-500">Factory Reset Config</span>
                        <span className="text-xs text-center text-red-400/70">Restores default settings. Use as a last resort.</span>
                    </button>
                </div>
            </div>

            {/* Terminal Output */}
            <div className="bg-black/80 rounded-xl overflow-hidden border border-slate-700 font-mono text-sm shadow-2xl">
                <div className="bg-slate-900/50 px-4 py-2 flex items-center gap-2 border-b border-slate-700">
                    <Terminal className="text-green-400" size={16} />
                    <span className="text-gray-400">Repair Log</span>
                </div>
                <div className="p-4 h-64 overflow-y-auto space-y-1 text-gray-300">
                    {logs.length === 0 && <span className="opacity-50 italic">Ready for commands...</span>}
                    {logs.map((log, i) => (
                        <div key={i} className="break-all whitespace-pre-wrap">{log}</div>
                    ))}
                    {loading && <div className="animate-pulse text-blue-400">Processing...</div>}
                </div>
            </div>

            {/* Password Modal */}
            {
                showPasswordInput && (
                    <div className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4 z-50">
                        <div className="bg-slate-900 rounded-2xl p-6 w-full max-w-md border border-slate-700 shadow-2xl transform transition-all scale-100">
                            <h3 className="text-xl font-bold mb-4">Privileged Action Required</h3>
                            <p className="text-gray-400 mb-6">Enter your sudo password to proceed with <span className="text-blue-400 font-mono">{pendingAction}</span>.</p>

                            <input
                                type="password"
                                className="w-full bg-slate-950 border border-slate-700 rounded-lg px-4 py-3 focus:ring-2 focus:ring-blue-500 outline-none mb-6 text-white"
                                placeholder="Sudo Password"
                                autoFocus
                                value={password}
                                onChange={(e) => setPassword(e.target.value)}
                                onKeyDown={(e) => e.key === "Enter" && executePendingAction()}
                            />

                            <div className="flex justify-end gap-3">
                                <button
                                    onClick={() => setShowPasswordInput(false)}
                                    className="px-4 py-2 hover:bg-slate-800 rounded-lg transition"
                                >
                                    Cancel
                                </button>
                                <button
                                    onClick={executePendingAction}
                                    className="px-6 py-2 bg-blue-600 hover:bg-blue-500 rounded-lg font-medium shadow-lg shadow-blue-500/20 transition"
                                >
                                    Execute
                                </button>
                            </div>
                        </div>
                    </div>
                )
            }
        </div >
    );
}
