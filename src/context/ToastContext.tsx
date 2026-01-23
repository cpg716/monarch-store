import React, { createContext, useContext, useState, useCallback, ReactNode } from 'react';
import { X, CheckCircle, AlertTriangle, Info, AlertCircle } from 'lucide-react';
import { clsx } from 'clsx';
import { AnimatePresence, motion } from 'framer-motion';

export type ToastType = 'success' | 'error' | 'warning' | 'info';

interface Toast {
    id: number;
    message: string;
    type: ToastType;
}

interface ToastContextType {
    show: (message: string, type?: ToastType) => void;
    success: (message: string) => void;
    error: (message: string) => void;
}

const ToastContext = createContext<ToastContextType | undefined>(undefined);

export function ToastProvider({ children }: { children: ReactNode }) {
    const [toasts, setToasts] = useState<Toast[]>([]);
    const [counter, setCounter] = useState(0);

    const remove = useCallback((id: number) => {
        setToasts(prev => prev.filter(t => t.id !== id));
    }, []);

    const show = useCallback((message: string, type: ToastType = 'info') => {
        const id = counter;
        setCounter(c => c + 1);
        setToasts(prev => [...prev, { id, message, type }]);

        // Auto dismiss
        setTimeout(() => remove(id), 5000);
    }, [counter, remove]);

    const success = useCallback((msg: string) => show(msg, 'success'), [show]);
    const error = useCallback((msg: string) => show(msg, 'error'), [show]);

    return (
        <ToastContext.Provider value={{ show, success, error }}>
            {children}
            <div className="fixed bottom-6 right-6 z-50 flex flex-col gap-3 pointer-events-none">
                <AnimatePresence>
                    {toasts.map(toast => (
                        <motion.div
                            key={toast.id}
                            initial={{ opacity: 0, x: 50, scale: 0.9 }}
                            animate={{ opacity: 1, x: 0, scale: 1 }}
                            exit={{ opacity: 0, x: 20, scale: 0.95 }}
                            layout
                            className={clsx(
                                "pointer-events-auto min-w-[300px] max-w-sm rounded-xl shadow-2xl p-4 flex items-start gap-4 border backdrop-blur-md",
                                toast.type === 'success' && "bg-green-500/10 border-green-500/20 text-green-100",
                                toast.type === 'error' && "bg-red-500/10 border-red-500/20 text-red-100",
                                toast.type === 'warning' && "bg-amber-500/10 border-amber-500/20 text-amber-100",
                                toast.type === 'info' && "bg-blue-500/10 border-blue-500/20 text-blue-100",
                                "bg-app-card/95" // Fallback/Mix
                            )}
                        >
                            <div className="shrink-0 mt-0.5">
                                {toast.type === 'success' && <CheckCircle className="text-green-500" size={20} />}
                                {toast.type === 'error' && <AlertCircle className="text-red-500" size={20} />}
                                {toast.type === 'warning' && <AlertTriangle className="text-amber-500" size={20} />}
                                {toast.type === 'info' && <Info className="text-blue-500" size={20} />}
                            </div>
                            <div className="flex-1 text-sm font-medium leading-relaxed">
                                {toast.message}
                            </div>
                            <button
                                onClick={() => remove(toast.id)}
                                className="shrink-0 p-1 hover:bg-white/10 rounded-full transition-colors opacity-60 hover:opacity-100"
                            >
                                <X size={16} />
                            </button>
                        </motion.div>
                    ))}
                </AnimatePresence>
            </div>
        </ToastContext.Provider>
    );
}

export function useToast() {
    const context = useContext(ToastContext);
    if (!context) throw new Error("useToast must be used within ToastProvider");
    return context;
}
