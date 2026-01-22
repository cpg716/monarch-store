import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import { AlertTriangle, Check, Cpu, X, Terminal } from "lucide-react";

interface RepoSetupModalProps {
    repoName: string;
    isOpen: boolean;
    onClose: () => void;
    onSuccess: () => void;
}

export default function RepoSetupModal({
    repoName,
    isOpen,
    onClose,
    onSuccess,
}: RepoSetupModalProps) {
    const [status, setStatus] = useState<"idle" | "enabling" | "success" | "error">(
        "idle"
    );
    const [logs, setLogs] = useState<string[]>([]);

    const handleEnable = async () => {
        setStatus("enabling");
        setLogs((prev) => [...prev, `Starting setup for ${repoName}...`]);
        setLogs((prev) => [...prev, "Requesting root privileges via pkexec..."]);

        try {
            const result = await invoke<string>("enable_repo", { name: repoName });
            setLogs((prev) => [...prev, result]);
            setLogs((prev) => [...prev, "Setup complete!"]);
            setStatus("success");
            setTimeout(() => {
                onSuccess();
                onClose();
            }, 2000);
        } catch (e: any) {
            setLogs((prev) => [...prev, `Error: ${e}`]);
            setStatus("error");
        }
    };

    if (!isOpen) return null;

    return (
        <AnimatePresence>
            <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
                <motion.div
                    initial={{ opacity: 0, scale: 0.95 }}
                    animate={{ opacity: 1, scale: 1 }}
                    exit={{ opacity: 0, scale: 0.95 }}
                    className="w-full max-w-lg overflow-hidden rounded-2xl border border-white/10 bg-[#1a1b26] shadow-2xl"
                >
                    {/* Header */}
                    <div className="flex items-center justify-between border-b border-white/5 px-6 py-4">
                        <div className="flex items-center gap-3">
                            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-purple-500/20 text-purple-400">
                                <Cpu size={20} />
                            </div>
                            <div>
                                <h3 className="text-lg font-semibold text-white">Enable {repoName}</h3>
                                <p className="text-xs text-white/50">System Configuration Required</p>
                            </div>
                        </div>
                        <button
                            onClick={onClose}
                            className="rounded-lg p-2 text-white/40 hover:bg-white/5 hover:text-white"
                        >
                            <X size={20} />
                        </button>
                    </div>

                    {/* Content */}
                    <div className="p-6">
                        {status === "idle" && (
                            <div className="space-y-4">
                                <div className="flex gap-4 rounded-xl bg-orange-500/10 p-4 text-orange-200">
                                    <AlertTriangle className="mt-1 min-w-[20px]" size={20} />
                                    <div className="text-sm">
                                        <p className="font-medium">Missing Repository Configuration</p>
                                        <p className="mt-1 text-orange-200/70">
                                            The <strong>{repoName}</strong> repository is required for this application to function correctly (pre-built binaries).
                                        </p>
                                    </div>
                                </div>
                                <p className="text-sm text-white/60">
                                    We can automatically configure your system to enable it. This will:
                                </p>
                                <ul className="list-disc space-y-1 pl-5 text-sm text-white/60">
                                    <li>Import & Sign Security Keys</li>
                                    <li>Install Keyring and Mirrorlist packages</li>
                                    <li>Update <code>/etc/pacman.conf</code></li>
                                </ul>
                            </div>
                        )}

                        {(status === "enabling" || status === "success" || status === "error") && (
                            <div className="h-48 overflow-auto rounded-xl bg-black/50 p-4 font-mono text-xs text-white/70">
                                {logs.map((log, i) => (
                                    <div key={i} className="mb-1 break-all flex items-start gap-2">
                                        <span className="text-purple-400 shrink-0 mt-0.5">➜</span>
                                        <span>{log}</span>
                                    </div>
                                ))}
                                {status === "enabling" && (
                                    <span className="animate-pulse text-purple-400">_</span>
                                )}
                            </div>
                        )}
                    </div>

                    {/* Footer */}
                    <div className="flex justify-end gap-3 border-t border-white/5 bg-white/5 px-6 py-4">
                        {status === "idle" ? (
                            <>
                                <button
                                    onClick={onClose}
                                    className="rounded-lg px-4 py-2 text-sm font-medium text-white/60 hover:bg-white/5 hover:text-white"
                                >
                                    Skip (Not Recommended)
                                </button>
                                <button
                                    onClick={handleEnable}
                                    className="flex items-center gap-2 rounded-lg bg-purple-600 px-4 py-2 text-sm font-medium text-white shadow-lg transition-colors hover:bg-purple-500"
                                >
                                    <Cpu size={16} />
                                    Enable Now
                                </button>
                            </>
                        ) : status === "success" ? (
                            <button
                                disabled
                                className="flex items-center gap-2 rounded-lg bg-green-500/20 px-4 py-2 text-sm font-medium text-green-400"
                            >
                                <Check size={16} />
                                Done
                            </button>
                        ) : status === "error" ? (
                            <button
                                onClick={handleEnable}
                                className="flex items-center gap-2 rounded-lg bg-red-500/20 px-4 py-2 text-sm font-medium text-red-400 hover:bg-red-500/30"
                            >
                                Retry
                            </button>
                        ) : (
                            <button
                                disabled
                                className="flex items-center gap-2 rounded-lg bg-white/5 px-4 py-2 text-sm font-medium text-white/50"
                            >
                                <Terminal size={16} className="animate-pulse" />
                                Processing...
                            </button>
                        )}
                    </div>
                </motion.div>
            </div>
        </AnimatePresence>
    );
}
