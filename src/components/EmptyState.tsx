import React from 'react';
import { Search, LucideIcon } from 'lucide-react';
import { motion } from 'framer-motion';

interface EmptyStateProps {
    icon?: LucideIcon;
    title: string;
    description: string;
    actionLabel?: string;
    onAction?: () => void;
    variant?: 'default' | 'error';
}

import { clsx } from "clsx";

const EmptyState: React.FC<EmptyStateProps> = ({
    icon: Icon = Search,
    title,
    description,
    actionLabel,
    onAction,
    variant = 'default'
}) => {
    return (
        <motion.div
            initial={{ opacity: 0, scale: 0.95 }}
            animate={{ opacity: 1, scale: 1 }}
            className="flex flex-col items-center justify-center py-20 text-center space-y-4 h-full"
        >
            <div className={clsx(
                "p-6 rounded-full border shadow-sm",
                variant === 'error' ? "bg-red-500/10 border-red-500/20 text-red-500" : "bg-app-card border-app-border text-app-muted"
            )}>
                <Icon size={48} strokeWidth={1.5} />
            </div>
            <div className="space-y-1">
                <h3 className="text-xl font-bold text-app-fg">{title}</h3>
                <p className="text-app-muted max-w-sm mx-auto text-sm leading-relaxed">
                    {description}
                </p>
            </div>
            {actionLabel && onAction && (
                <button
                    onClick={onAction}
                    className="mt-4 px-6 py-2 rounded-xl bg-app-subtle hover:bg-app-hover text-blue-500 font-bold hover:text-blue-400 transition-all active:scale-95"
                >
                    {actionLabel}
                </button>
            )}
        </motion.div>
    );
};

export default EmptyState;
