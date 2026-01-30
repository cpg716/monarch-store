import { motion, AnimatePresence } from 'framer-motion';
import { AlertTriangle, X, RefreshCw, Unlock, Key, Wifi, HardDrive, Terminal } from 'lucide-react';
import { clsx } from 'clsx';
import { useErrorService } from '../context/ErrorContext';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { useFocusTrap } from '../hooks/useFocusTrap';

/**
 * Modal for displaying critical errors with recovery actions
 */
export default function ErrorModal() {
    const { currentCriticalError, dismissCritical } = useErrorService();
    const isOpen = !!currentCriticalError;
    
    useEscapeKey(dismissCritical, isOpen);
    const focusTrapRef = useFocusTrap(isOpen);

    if (!currentCriticalError) return null;

    const error = currentCriticalError;
    const recoveryAction = error.recoveryAction;

    // Get recovery icon based on action type
    const getRecoveryIcon = () => {
        if (!recoveryAction) return RefreshCw;
        const type = recoveryAction.type.toLowerCase();
        if (type.includes('unlock') || type.includes('database')) return Unlock;
        if (type.includes('keyring') || type.includes('key')) return Key;
        if (type.includes('mirror') || type.includes('network') || type.includes('download')) return Wifi;
        if (type.includes('cache') || type.includes('disk') || type.includes('space')) return HardDrive;
        if (type.includes('refresh') || type.includes('sync')) return RefreshCw;
        return Terminal;
    };

    const RecoveryIcon = getRecoveryIcon();

    // Get recovery button color
    const getRecoveryColor = () => {
        if (!recoveryAction) return 'bg-blue-500 hover:bg-blue-600';
        const type = recoveryAction.type.toLowerCase();
        if (type.includes('unlock')) return 'bg-amber-500 hover:bg-amber-600';
        if (type.includes('keyring')) return 'bg-purple-500 hover:bg-purple-600';
        if (type.includes('disk') || type.includes('cache')) return 'bg-red-500 hover:bg-red-600';
        return 'bg-blue-500 hover:bg-blue-600';
    };

    const handleRecovery = async () => {
        if (recoveryAction?.handler) {
            try {
                await recoveryAction.handler();
            } catch (e) {
                console.error('Recovery action failed:', e);
            }
        }
        dismissCritical();
    };

    return (
        <AnimatePresence>
            {isOpen && (
                <div className="fixed inset-0 z-[300] flex items-center justify-center bg-black/70 backdrop-blur-sm">
                    <motion.div
                        ref={focusTrapRef}
                        initial={{ opacity: 0, scale: 0.9, y: 20 }}
                        animate={{ opacity: 1, scale: 1, y: 0 }}
                        exit={{ opacity: 0, scale: 0.95, y: 10 }}
                        className="w-full max-w-lg bg-app-card border border-app-border rounded-2xl shadow-2xl overflow-hidden"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="error-modal-title"
                    >
                        {/* Header */}
                        <div className="p-6 border-b border-app-border bg-red-500/10">
                            <div className="flex items-start justify-between gap-4">
                                <div className="flex items-start gap-4 flex-1">
                                    <div className="p-3 bg-red-500/20 rounded-xl text-red-500 shrink-0">
                                        <AlertTriangle size={24} />
                                    </div>
                                    <div className="flex-1 min-w-0">
                                        <h2 id="error-modal-title" className="text-xl font-bold text-app-fg mb-1">
                                            {error.title}
                                        </h2>
                                        <p className="text-sm text-app-muted leading-relaxed">
                                            {error.description}
                                        </p>
                                    </div>
                                </div>
                                <button
                                    onClick={dismissCritical}
                                    className="p-2 hover:bg-app-fg/10 rounded-lg text-app-muted hover:text-app-fg transition-colors shrink-0"
                                    aria-label="Close"
                                >
                                    <X size={20} />
                                </button>
                            </div>
                        </div>

                        {/* Body */}
                        <div className="p-6 space-y-4">
                            {/* Raw error message (if available and technical) */}
                            {error.raw && error.friendly?.isTechnical && (
                                <details className="bg-app-bg/50 border border-app-border rounded-xl p-4">
                                    <summary className="text-xs font-bold text-app-muted uppercase tracking-wider cursor-pointer hover:text-app-fg transition-colors">
                                        Technical Details
                                    </summary>
                                    <pre className="mt-3 text-xs font-mono text-app-muted overflow-auto max-h-32 whitespace-pre-wrap">
                                        {error.raw}
                                    </pre>
                                </details>
                            )}

                            {/* Classified error raw message */}
                            {error.classified?.raw_message && (
                                <details className="bg-app-bg/50 border border-app-border rounded-xl p-4">
                                    <summary className="text-xs font-bold text-app-muted uppercase tracking-wider cursor-pointer hover:text-app-fg transition-colors">
                                        Error Log
                                    </summary>
                                    <pre className="mt-3 text-xs font-mono text-app-muted overflow-auto max-h-32 whitespace-pre-wrap">
                                        {error.classified.raw_message}
                                    </pre>
                                </details>
                            )}
                        </div>

                        {/* Footer with Recovery Actions */}
                        <div className="p-6 border-t border-app-border bg-app-fg/5 flex gap-3">
                            {recoveryAction ? (
                                <>
                                    <button
                                        onClick={handleRecovery}
                                        className={clsx(
                                            "flex-1 py-3 rounded-xl text-white font-bold shadow-lg transition-all active:scale-95 flex items-center justify-center gap-2",
                                            getRecoveryColor()
                                        )}
                                    >
                                        <RecoveryIcon size={18} />
                                        {recoveryAction.label || 'Retry'}
                                    </button>
                                    <button
                                        onClick={dismissCritical}
                                        className="px-6 py-3 bg-app-fg/10 hover:bg-app-fg/20 text-app-fg rounded-xl font-medium transition-colors"
                                    >
                                        Dismiss
                                    </button>
                                </>
                            ) : (
                                <button
                                    onClick={dismissCritical}
                                    className="flex-1 py-3 bg-blue-600 hover:bg-blue-500 text-white rounded-xl font-bold transition-colors active:scale-95"
                                >
                                    Close
                                </button>
                            )}
                        </div>
                    </motion.div>
                </div>
            )}
        </AnimatePresence>
    );
}
