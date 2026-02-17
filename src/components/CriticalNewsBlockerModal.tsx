/**
 * Safety gate: blocks "Update All" until user acknowledges unread critical news.
 * Operation Town Crier.
 */

import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { AlertTriangle } from 'lucide-react';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { NewsItem } from '../services/bindings';

interface CriticalNewsBlockerModalProps {
    isOpen: boolean;
    onClose: () => void;
    onProceed: () => void;
    criticalItems: NewsItem[];
}

export default function CriticalNewsBlockerModal({
    isOpen,
    onClose,
    onProceed,
    criticalItems,
}: CriticalNewsBlockerModalProps) {
    const [acknowledged, setAcknowledged] = useState(false);
    const handleClose = () => {
        setAcknowledged(false);
        onClose();
    };
    useEscapeKey(handleClose, isOpen);
    const focusTrapRef = useFocusTrap(isOpen);

    const handleOpenLink = async (link: string) => {
        if (!link) return;
        try {
            await openUrl(link);
        } catch {
            window.open(link, '_blank', 'noopener');
        }
    };

    const handleProceed = () => {
        if (!acknowledged) return;
        setAcknowledged(false);
        onProceed();
        handleClose();
    };

    if (!isOpen) return null;

    return (
        <AnimatePresence>
            <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
                <motion.div
                    ref={focusTrapRef}
                    initial={{ opacity: 0, scale: 0.9 }}
                    animate={{ opacity: 1, scale: 1 }}
                    exit={{ opacity: 0, scale: 0.9 }}
                    className="w-full max-w-lg bg-app-card border border-red-500/30 dark:border-red-500/20 rounded-2xl shadow-2xl p-6 overflow-hidden relative"
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="critical-news-title"
                >
                    <div className="flex flex-col gap-4">
                        <div className="flex items-start gap-4">
                            <div className="p-3 bg-red-500/20 rounded-full shrink-0">
                                <AlertTriangle className="text-red-500" size={28} />
                            </div>
                            <div>
                                <h3 id="critical-news-title" className="text-xl font-bold text-slate-900 dark:text-white">
                                    Safety Check: Critical News
                                </h3>
                                <p className="text-app-muted text-sm mt-1 leading-relaxed">
                                    The following announcements may require manual intervention. You must read them before updating.
                                </p>
                            </div>
                        </div>

                        <ul className="space-y-2 max-h-48 overflow-y-auto custom-scrollbar rounded-xl border border-black/10 dark:border-white/10 p-3 bg-black/5 dark:bg-black/20">
                            {criticalItems.map((item, idx) => (
                                <li key={typeof item.id === 'string' || typeof item.id === 'number' ? String(item.id) : `critical-${idx}`}>
                                    <button
                                        type="button"
                                        onClick={() => handleOpenLink(item.link)}
                                        className="w-full text-left text-sm font-medium text-blue-600 dark:text-blue-400 hover:underline truncate block"
                                    >
                                        {item.title}
                                    </button>
                                    {item.source_label && (
                                        <span className="text-xs text-app-muted">{item.source_label}</span>
                                    )}
                                </li>
                            ))}
                        </ul>

                        <label className="flex items-center gap-3 cursor-pointer select-none">
                            <input
                                type="checkbox"
                                checked={acknowledged}
                                onChange={(e) => setAcknowledged(e.target.checked)}
                                className="w-4 h-4 rounded border-slate-300 dark:border-white/30 text-blue-600 focus:ring-blue-500"
                            />
                            <span className="text-sm text-slate-700 dark:text-slate-300">
                                I have read these and understand the risks.
                            </span>
                        </label>

                        <div className="flex gap-3 mt-2">
                            <button
                                type="button"
                                onClick={handleClose}
                                className="flex-1 py-2.5 rounded-xl border border-app-border text-app-fg hover:bg-app-subtle font-medium transition-colors"
                            >
                                Cancel
                            </button>
                            <button
                                type="button"
                                onClick={handleProceed}
                                disabled={!acknowledged}
                                className="flex-1 py-2.5 rounded-xl bg-blue-600 hover:bg-blue-500 text-white font-bold shadow-lg transition-all active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed disabled:active:scale-100"
                            >
                                Proceed with Update
                            </button>
                        </div>
                    </div>
                </motion.div>
            </div>
        </AnimatePresence>
    );
}
