import { motion, AnimatePresence } from 'framer-motion';
import { clsx } from 'clsx';
import { AlertTriangle, CheckCircle2, Info } from 'lucide-react';

interface ConfirmationModalProps {
    isOpen: boolean;
    onClose: () => void;
    onConfirm: () => void;
    title: string;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    variant?: 'danger' | 'info' | 'success';
}

export default function ConfirmationModal({
    isOpen,
    onClose,
    onConfirm,
    title,
    message,
    confirmLabel = "Confirm",
    cancelLabel = "Cancel",
    variant = 'info'
}: ConfirmationModalProps) {
    if (!isOpen) return null;

    const getIcon = () => {
        switch (variant) {
            case 'danger': return <AlertTriangle className="text-red-500" size={32} />;
            case 'success': return <CheckCircle2 className="text-green-500" size={32} />;
            default: return <Info className="text-blue-500" size={32} />;
        }
    };

    const getButtonColor = () => {
        switch (variant) {
            case 'danger': return "bg-red-500 hover:bg-red-600";
            case 'success': return "bg-green-500 hover:bg-green-600";
            default: return "bg-blue-600 hover:bg-blue-700";
        }
    };

    return (
        <AnimatePresence>
            <div className="fixed inset-0 z-[200] flex items-center justify-center bg-black/60 backdrop-blur-sm">
                <motion.div
                    initial={{ opacity: 0, scale: 0.9 }}
                    animate={{ opacity: 1, scale: 1 }}
                    exit={{ opacity: 0, scale: 0.9 }}
                    className="w-full max-w-md bg-app-card border border-app-border rounded-2xl shadow-2xl p-6 overflow-hidden relative"
                >
                    <div className="flex flex-col items-center text-center gap-4">
                        <div className="p-4 bg-app-subtle rounded-full">
                            {getIcon()}
                        </div>

                        <div>
                            <h3 className="text-xl font-bold text-app-fg mb-2">{title}</h3>
                            <p className="text-app-muted text-sm leading-relaxed">{message}</p>
                        </div>

                        <div className="flex gap-3 w-full mt-4">
                            <button
                                onClick={onClose}
                                className="flex-1 py-2.5 rounded-xl border border-app-border text-app-fg hover:bg-app-subtle font-medium transition-colors"
                            >
                                {cancelLabel}
                            </button>
                            <button
                                onClick={() => {
                                    onConfirm();
                                    onClose();
                                }}
                                className={clsx(
                                    "flex-1 py-2.5 rounded-xl text-white font-bold shadow-lg transition-all active:scale-95",
                                    getButtonColor()
                                )}
                            >
                                {confirmLabel}
                            </button>
                        </div>
                    </div>
                </motion.div>
            </div>
        </AnimatePresence>
    );
}
