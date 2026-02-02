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
    Shield,
    Info,
    Trash2,
    Activity,
    ChevronDown,
    Eye,
    EyeOff
} from "lucide-react";
import { clsx } from 'clsx';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { useErrorService } from '../context/ErrorContext';

interface HealthIssue {
    category: string;
    severity: string;
    message: string;
    action_label: string;
    action_command: string | null;
}

export default function SystemHealthSection() {
    const errorService = useErrorService();
    const [logs, setLogs] = useState<string[]>([]);
    const [isLocked, setIsLocked] = useState(false);
    const [loading, setLoading] = useState(false);
    const [showPasswordInput, setShowPasswordInput] = useState(false);
    const [pendingAction, setPendingAction] = useState<string | null>(null);
    const [healthIssues, setHealthIssues] = useState<HealthIssue[]>([]);
    const [password, setPassword] = useState("");
    const [showPassword, setShowPassword] = useState(false);
    const [classifiedError, setClassifiedError] = useState<any | null>(null);
    const [isCheckingHealth, setIsCheckingHealth] = useState(true);

    useEscapeKey(() => setShowPasswordInput(false), showPasswordInput);
    const authModalRef = useFocusTrap(showPasswordInput);

    useEffect(() => {
        const runHealthCheck = async () => {
            setIsCheckingHealth(true);
            await Promise.all([checkLock(), checkHealth()]);
            // Small delay to show loading state (ensures user sees the check is happening)
            setTimeout(() => setIsCheckingHealth(false), 500);
        };
        runHealthCheck();
        const unlisten = listen("repair-log", (event) => {
            setLogs((prev) => [...prev, event.payload as string]);
        });
        const unlistenErr = listen("repair-error-classified", (event) => {
            setClassifiedError(event.payload);
        });
        return () => {
            unlisten.then((f) => f());
            unlistenErr.then((f) => f());
        };
    }, [errorService]);

    const checkLock = async () => {
        try {
            const locked = await invoke("check_pacman_lock");
            setIsLocked(locked as boolean);
        } catch (e) {
            errorService.reportError(e as Error | string);
        }
    };

    const checkHealth = async () => {
        try {
            const issues = await invoke<HealthIssue[]>("check_system_health");
            setHealthIssues(issues);
        } catch (e) {
            errorService.reportError(e as Error | string);
        }
    };

    const handleAction = async (action: string) => {
        setPendingAction(action);
        setShowPasswordInput(true);
        setPassword(""); // Reset password on new action
        setClassifiedError(null);
        setLogs([]);
    };

    const executePendingAction = async () => {
        if (!pendingAction) return;
        setLoading(true);
        setShowPasswordInput(false);
        setClassifiedError(null);
        setLogs((p) => [...p, `>>> STARTING: ${pendingAction.toUpperCase()} ...`]);

        try {
            let cmd = "";
            let args = {};

            switch (pendingAction) {
                case "unlock":
                    cmd = "repair_unlock_pacman";
                    break;
                case "keyring":
                case "trigger_repair_flow":
                case "repair_reset_keyring":
                    cmd = "fix_keyring_issues_alias";
                    break;
                case "emergency_sync":
                    cmd = "repair_emergency_sync";
                    break;
                case "install_monarch_policy":
                    cmd = "install_monarch_policy";
                    break;
                case "orphans":
                    setLogs(p => [...p, "Scanning for unused files..."]);
                    const orphans = await invoke<string[]>("get_orphans");
                    if (orphans && orphans.length > 0) {
                        setLogs(p => [...p, `Found ${orphans.length} leftovers: ${orphans.join(", ")}`]);
                        await invoke("remove_orphans", { orphans });
                        setLogs(p => [...p, ">>> SUCCESS: System cleaned."]);
                    } else {
                        setLogs(p => [...p, "System is already clean!"]);
                    }
                    cmd = "";
                    break;
                case "cache":
                    cmd = "clear_cache";
                    break;
            }

            if (cmd) {
                await invoke(cmd, { password: password || null, ...args });
                setLogs((p) => [...p, `>>> SUCCESS: ${pendingAction.toUpperCase()} COMPLETED.`]);
                checkHealth();
                checkLock();
            }
        } catch (e) {
            errorService.reportError(e as Error | string);
            setLogs((p) => [...p, `>>> ERROR: ${e}`]);
        } finally {
            setLoading(false);
            setPendingAction(null);
        }
    };

    return (
        <section className="space-y-8">
            <h2 className="text-xl font-bold mb-4 flex items-center gap-3 text-app-fg">
                <Activity size={22} className="text-app-muted" />
                System Health & Maintenance
            </h2>

            {/* Proactive Health Check Loading State */}
            {isCheckingHealth && (
                <div className="p-4 bg-blue-500/10 border border-blue-500/20 rounded-2xl flex items-center gap-3 animate-in slide-in-from-top-2 duration-300 mb-4">
                    <RefreshCw className="animate-spin text-blue-500" size={20} />
                    <span className="text-sm font-medium text-app-fg">Checking system health...</span>
                </div>
            )}

            {/* Grandma-Proof Alerts */}
            {healthIssues.filter(i => i.severity === 'Critical' || i.severity === 'Warning').length > 0 && (
                <div className="space-y-4">
                    {healthIssues.filter(i => i.severity === 'Critical' || i.severity === 'Warning').map((issue, idx) => (
                        <div key={idx} className={clsx(
                            "p-6 rounded-3xl border flex flex-col md:flex-row items-center justify-between gap-6",
                            issue.severity === 'Critical' ? 'bg-red-500/10 border-red-500/20' : 'bg-yellow-500/10 border-yellow-500/20'
                        )}>
                            <div className="flex items-center gap-5">
                                <div className={clsx(
                                    "p-3 rounded-2xl",
                                    issue.severity === 'Critical' ? 'bg-red-500/20 text-red-500' : 'bg-yellow-500/20 text-yellow-500'
                                )}>
                                    <AlertTriangle size={24} />
                                </div>
                                <div>
                                    <div className="font-bold text-lg text-app-fg">{issue.message}</div>
                                    <p className="text-sm text-app-muted">Recommended fix: Click the button to repair this automatically.</p>
                                </div>
                            </div>
                            {issue.action_command && (
                                <button
                                    onClick={() => handleAction(issue.action_command!)}
                                    className="px-6 py-3 bg-app-fg text-app-bg rounded-2xl font-bold hover:opacity-90 transition-all shadow-xl active:scale-95"
                                >
                                    {issue.action_label}
                                </button>
                            )}
                        </div>
                    ))}
                </div>
            )}

            <div className="bg-app-card/30 border border-app-border/50 rounded-3xl p-8 space-y-8">
                {/* Status Indicator */}
                <div className="flex items-center justify-between p-4 bg-app-bg/50 rounded-2xl border border-app-border/30">
                    <div className="flex items-center gap-4">
                        {isLocked ? <AlertTriangle className="text-red-500" /> : <CheckCircle className="text-green-500" />}
                        <div>
                            <span className="font-bold text-app-fg">System Database Status</span>
                            <p className="text-xs text-app-muted">{isLocked ? "System is currently locked. Updates might be stuck." : "System is clean and ready."}</p>
                        </div>
                    </div>
                    {isLocked && (
                        <button onClick={() => handleAction("unlock")} className="text-blue-500 text-xs font-bold hover:underline">Unlock Now</button>
                    )}
                </div>

                {/* Repair Grid */}
                <div>
                    <h3 className="text-sm font-black uppercase tracking-widest text-app-muted mb-4 px-2">One-Click Repair Tools</h3>
                    <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                        <RepairButton
                            icon={<LockOpen className="text-blue-500" />}
                            title="Fix Stuck Updates"
                            desc="Clears blocks that prevent new apps from installing."
                            onClick={() => handleAction("unlock")}
                            loading={loading}
                        />
                        <RepairButton
                            icon={<Key className="text-purple-500" />}
                            title="Fix Permissions"
                            desc="Repair security keys to solve 'Invalid Signature' errors."
                            onClick={() => handleAction("keyring")}
                            loading={loading}
                        />
                        <RepairButton
                            icon={<RefreshCw className="text-red-500" />}
                            title="Emergency Repair"
                            desc="Force synchronize and update the entire system catalog."
                            onClick={() => handleAction("emergency_sync")}
                            loading={loading}
                        />
                    </div>
                </div>

                {/* Maintenance Grid */}
                <div className="pt-8 border-t border-app-border/30">
                    <h3 className="text-sm font-black uppercase tracking-widest text-app-muted mb-4 px-2">System Cleanup & Reset</h3>
                    <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                        <RepairButton
                            icon={<Trash2 className="text-emerald-500" />}
                            title="Cleanup Leftovers"
                            desc="Deletes unused system files and dependencies."
                            onClick={() => handleAction("orphans")}
                            loading={loading}
                        />
                        <RepairButton
                            icon={<Terminal className="text-orange-500" />}
                            title="Free Disk Space"
                            desc="Clear temporary downloads and cached data."
                            onClick={() => handleAction("cache")}
                            loading={loading}
                        />
                    </div>
                </div>

                {/* Log Output (Collapsible for Cleanliness) */}
                <div className="mt-4 space-y-4">
                    {classifiedError && (
                        <div className="bg-red-500/10 border border-red-500/20 rounded-2xl p-6 animate-in slide-in-from-top-2 duration-300">
                            <div className="flex items-center gap-3 text-red-500 font-black text-xs uppercase tracking-widest mb-2">
                                <AlertTriangle size={18} />
                                {classifiedError.title}
                            </div>
                            <p className="text-sm text-red-500/90 leading-relaxed font-medium mb-4">
                                {classifiedError.description}
                            </p>
                            <div className="bg-black/40 rounded-xl p-4 font-mono text-[10px] text-red-500/60 max-h-32 overflow-y-auto border border-red-500/10">
                                {classifiedError.raw_message}
                            </div>
                        </div>
                    )}

                    <details className="group" open={!classifiedError}>
                        <summary className="flex items-center gap-2 text-xs font-bold text-app-muted cursor-pointer hover:text-app-fg transition-colors select-none list-none">
                            <Terminal size={14} />
                            <span>View Technician Logs</span>
                            <ChevronDown size={14} className="group-open:rotate-180 transition-transform" />
                        </summary>
                        <div className="mt-4 bg-black/40 rounded-2xl p-6 font-mono text-[11px] text-app-muted max-h-48 overflow-y-auto border border-app-border/30">
                            {logs.length === 0 && <span className="opacity-30 italic">No activity logs...</span>}
                            {logs.map((log, i) => (
                                <div key={i} className="mb-1"><span className="text-blue-500 mr-2">➜</span>{log}</div>
                            ))}
                            {loading && <div className="animate-pulse text-app-accent mt-2">Processing task...</div>}
                        </div>
                    </details>
                </div>
            </div>

            {/* Auth Modal */}
            {showPasswordInput && (
                <div className="fixed inset-0 bg-black/60 backdrop-blur-md flex items-center justify-center p-6 z-40 animate-in fade-in duration-200" role="dialog" aria-modal="true" aria-labelledby="auth-modal-title">
                    <div ref={authModalRef} className="bg-app-card rounded-[32px] p-10 w-full max-w-lg border border-app-border shadow-2xl space-y-8 animate-in zoom-in-95 duration-300">
                        <div className="flex items-center gap-5">
                            <div className="p-4 rounded-[20px] bg-blue-600/20 text-blue-500">
                                <Shield size={32} />
                            </div>
                            <div>
                                <h3 id="auth-modal-title" className="text-2xl font-black text-app-fg leading-tight">Authorize Task</h3>
                                <p className="text-app-muted text-sm mt-1">MonARCH needs your permission to repair: <span className="text-app-fg font-mono font-bold uppercase">{pendingAction}</span></p>
                            </div>
                        </div>

                        <div className="bg-app-bg/50 border border-app-border/50 p-6 rounded-3xl flex flex-col gap-4 text-sm text-app-muted leading-relaxed">
                            <div className="flex start gap-4">
                                <Info className="shrink-0 mt-0.5 text-blue-500" size={20} />
                                <p>This action will perform system-level changes. Entering your password here allows MonARCH to handle the permissions securely without multiple system prompts.</p>
                            </div>

                            <div className="relative group/passwd">
                                <input
                                    type={showPassword ? "text" : "password"}
                                    placeholder="System Password (Optional)"
                                    value={password}
                                    onChange={(e) => setPassword(e.target.value)}
                                    className="w-full bg-black/40 border border-white/10 rounded-xl px-4 py-3 text-sm focus:outline-none focus:border-blue-500/50 transition-all group-hover/passwd:border-white/20 text-app-fg"
                                />
                                <button
                                    onClick={() => setShowPassword(!showPassword)}
                                    className="absolute right-4 top-1/2 -translate-y-1/2 text-white/30 hover:text-white/60"
                                >
                                    {showPassword ? <EyeOff size={18} /> : <Eye size={18} />}
                                </button>
                            </div>
                        </div>

                        <div className="flex gap-4">
                            <button
                                onClick={() => setShowPasswordInput(false)}
                                className="flex-1 py-4 hover:bg-app-fg/5 rounded-2xl font-bold transition-all text-app-muted"
                            >
                                Cancel
                            </button>
                            <button
                                onClick={executePendingAction}
                                className="flex-[2] py-4 bg-blue-600 hover:bg-blue-500 text-white rounded-2xl font-bold shadow-xl shadow-blue-600/20 transition-all active:scale-95 flex items-center justify-center gap-3"
                            >
                                <Shield size={18} /> Authorize & Start
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </section>
    );
}

function RepairButton({ icon, title, desc, onClick, loading }: { icon: React.ReactNode, title: string, desc: string, onClick: () => void, loading: boolean }) {
    return (
        <button
            onClick={onClick}
            disabled={loading}
            className="group p-6 bg-app-card/60 rounded-3xl border border-app-border/50 hover:border-app-accent/50 hover:bg-app-card transition-all text-left flex flex-col gap-3 shadow-sm hover:shadow-lg disabled:opacity-50"
        >
            <div className="p-3 bg-app-bg/50 rounded-2xl w-fit group-hover:scale-110 transition-transform">
                {icon}
            </div>
            <div>
                <h4 className="font-bold text-app-fg group-hover:text-app-accent transition-colors">{title}</h4>
                <p className="text-[10px] text-app-muted leading-relaxed mt-1 opacity-80">{desc}</p>
            </div>
        </button>
    );
}
