import { createContext, useContext, useState, useCallback, useRef, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useToast } from './ToastContext';
import { friendlyError, FriendlyError } from '../utils/friendlyError';

/**
 * Error severity levels that determine how errors are displayed
 */
export type ErrorSeverity = 'info' | 'warning' | 'error' | 'critical';

/**
 * Backend ClassifiedError structure (matches Rust error_classifier.rs)
 */
export interface ClassifiedError {
    kind: string;
    title: string;
    description: string;
    recovery_action?: {
        type: string;
        payload?: string;
    };
    raw_message: string;
}

/**
 * Unified error input - can be a string, Error object, ClassifiedError, or FriendlyError
 */
export type ErrorInput = string | Error | ClassifiedError | FriendlyError;

/**
 * Error report with metadata
 */
export interface ErrorReport {
    id: string;
    severity: ErrorSeverity;
    title: string;
    description: string;
    raw?: string;
    classified?: ClassifiedError;
    friendly?: FriendlyError;
    recoveryAction?: {
        type: string;
        label: string;
        handler?: () => void | Promise<void>;
    };
    timestamp: number;
}

export interface ErrorContextType {
    /**
     * Report an error with automatic severity detection
     */
    report: (error: ErrorInput, severity?: ErrorSeverity, recoveryAction?: { type: string; label: string; handler?: () => void | Promise<void> }) => void;
    
    /**
     * Report a critical error that requires user attention (shows modal)
     */
    reportCritical: (error: ErrorInput, recoveryAction?: { type: string; label: string; handler?: () => void | Promise<void> }) => void;
    
    /**
     * Report a simple error (shows toast)
     */
    reportError: (error: ErrorInput) => void;
    
    /**
     * Report a warning (shows toast)
     */
    reportWarning: (error: ErrorInput) => void;
    
    /**
     * Report info (shows toast)
     */
    reportInfo: (message: string) => void;
    
    /**
     * Get current critical error (for ErrorModal)
     */
    currentCriticalError: ErrorReport | null;
    
    /**
     * Dismiss current critical error
     */
    dismissCritical: () => void;
}

const ErrorContext = createContext<ErrorContextType | undefined>(undefined);

const TOAST_DEDUPE_MS = 4000;

export function ErrorProvider({ children }: { children: ReactNode }) {
    const toast = useToast();
    const [criticalError, setCriticalError] = useState<ErrorReport | null>(null);
    // Error history for future features (logging, analytics, etc.)
    const [, setErrorHistory] = useState<ErrorReport[]>([]);
    // Dedupe toasts: same message within window → show only once (stops startup spam)
    const lastToastRef = useRef<{ key: string; at: number } | null>(null);

    /**
     * Normalize error input to a standardized format
     */
    const normalizeError = useCallback((error: ErrorInput): { title: string; description: string; raw?: string; classified?: ClassifiedError; friendly?: FriendlyError } => {
        // ClassifiedError from backend
        if (typeof error === 'object' && 'kind' in error && 'title' in error && 'description' in error) {
            const classified = error as ClassifiedError;
            return {
                title: classified.title,
                description: classified.description,
                raw: classified.raw_message,
                classified
            };
        }
        
        // FriendlyError from frontend
        if (typeof error === 'object' && 'title' in error && 'description' in error && !('kind' in error)) {
            const friendly = error as FriendlyError;
            return {
                title: friendly.title,
                description: friendly.description,
                friendly
            };
        }
        
        // Error object
        if (error instanceof Error) {
            const friendly = friendlyError(error.message);
            return {
                title: friendly.title,
                description: friendly.description,
                raw: error.message,
                friendly
            };
        }
        
        // String
        if (typeof error === 'string') {
            const friendly = friendlyError(error);
            return {
                title: friendly.title,
                description: friendly.description,
                raw: error,
                friendly
            };
        }
        
        // Fallback
        return {
            title: 'Error',
            description: 'An unexpected error occurred.',
            raw: String(error)
        };
    }, []);

    /**
     * Main report function
     */
    const report = useCallback((
        error: ErrorInput,
        severity: ErrorSeverity = 'error',
        recoveryAction?: { type: string; label: string; handler?: () => void | Promise<void> }
    ) => {
        const normalized = normalizeError(error);
        if (normalized.title === 'Backend Response Error' && normalized.raw) {
            console.error('[MonARCH] Backend Response Error (raw):', normalized.raw);
        }
        const report: ErrorReport = {
            id: `error-${Date.now()}-${Math.random()}`,
            severity,
            title: normalized.title,
            description: normalized.description,
            raw: normalized.raw,
            classified: normalized.classified,
            friendly: normalized.friendly,
            recoveryAction,
            timestamp: Date.now()
        };

        // Add to history (keep last 50)
        setErrorHistory(prev => [...prev.slice(-49), report]);

        // Aptabase: track error (non-blocking; never break UI)
        invoke('track_event', {
            event: 'error_reported',
            payload: {
                severity: report.severity,
                title: normalized.title,
                description: normalized.description?.slice(0, 300),
                kind: normalized.classified?.kind,
                raw_preview: normalized.raw ? normalized.raw.slice(0, 200) : undefined,
            },
        }).catch(() => {});

        // Route based on severity
        if (severity === 'critical') {
            setCriticalError(report);
        } else {
            // Dedupe: skip toast if same message was shown recently (e.g. many failed invokes at startup)
            let message = normalized.description || normalized.title;
            // When we hit the generic fallback, show the raw error so user (and we) see what actually failed
            const isGenericFallback = message === 'An unexpected error occurred. Check the logs for details.';
            if (isGenericFallback && normalized.raw?.trim()) {
                message = `${message} (${normalized.raw.slice(0, 100).trim()}${normalized.raw.length > 100 ? '…' : ''})`;
            }
            const toastKey = `${severity}:${(normalized.raw ?? message).slice(0, 120)}`;
            const now = Date.now();
            const last = lastToastRef.current;
            if (last && last.key === toastKey && now - last.at < TOAST_DEDUPE_MS) {
                // Still log; only skip showing another toast
            } else {
                lastToastRef.current = { key: toastKey, at: now };
                if (severity === 'warning') {
                    toast.show(message, 'warning');
                } else if (severity === 'info') {
                    toast.show(message, 'info');
                } else {
                    toast.error(message);
                }
            }
        }

        // Log to console for debugging
        console.error('[ErrorService]', {
            severity,
            title: normalized.title,
            description: normalized.description,
            raw: normalized.raw,
            classified: normalized.classified
        });
    }, [normalizeError, toast]);

    const reportCritical = useCallback((
        error: ErrorInput,
        recoveryAction?: { type: string; label: string; handler?: () => void | Promise<void> }
    ) => {
        report(error, 'critical', recoveryAction);
    }, [report]);

    const reportError = useCallback((error: ErrorInput) => {
        report(error, 'error');
    }, [report]);

    const reportWarning = useCallback((error: ErrorInput) => {
        report(error, 'warning');
    }, [report]);

    const reportInfo = useCallback((message: string) => {
        report(message, 'info');
    }, [report]);

    const dismissCritical = useCallback(() => {
        setCriticalError(null);
    }, []);

    const contextValue: ErrorContextType = {
        report,
        reportCritical,
        reportError,
        reportWarning,
        reportInfo,
        currentCriticalError: criticalError,
        dismissCritical
    };

    // Expose to window for ErrorBoundary (class component) access
    if (typeof window !== 'undefined') {
        (window as any).__errorService = contextValue;
    }

    return (
        <ErrorContext.Provider value={contextValue}>
            {children}
        </ErrorContext.Provider>
    );
}

export function useErrorService() {
    const context = useContext(ErrorContext);
    if (!context) {
        throw new Error('useErrorService must be used within ErrorProvider');
    }
    return context;
}
