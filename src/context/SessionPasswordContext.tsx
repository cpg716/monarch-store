import { createContext, useState, useCallback, useRef, ReactNode } from 'react';
import { Lock } from 'lucide-react';
import { useAppStore } from '../store/internal_store';

const SESSION_TTL_MS = 15 * 60 * 1000; // 15 minutes

// Module-level cache so password is not in React state; cleared on use or expiry
let cachedPassword: string | null = null;
let cacheExpiry = 0;

function getCachedPassword(): string | null {
    if (cachedPassword && Date.now() < cacheExpiry) return cachedPassword;
    cachedPassword = null;
    return null;
}

function setCachedPassword(p: string | null) {
    cachedPassword = p;
    cacheExpiry = p ? Date.now() + SESSION_TTL_MS : 0;
}

interface SessionPasswordContextType {
    /** Returns password for privileged action when "Reduce password prompts" is on; else null. Shows one dialog per session. */
    requestSessionPassword: () => Promise<string | null>;
}

export const SessionPasswordContext = createContext<SessionPasswordContextType | undefined>(undefined);

export function SessionPasswordProvider({ children }: { children: ReactNode }) {
    const reducePasswordPrompts = useAppStore((s) => s.reducePasswordPrompts);
    const [showModal, setShowModal] = useState(false);
    const [inputValue, setInputValue] = useState('');
    const resolveRef = useRef<((p: string | null) => void) | null>(null);

    const requestSessionPassword = useCallback((): Promise<string | null> => {
        if (!reducePasswordPrompts) return Promise.resolve(null);
        const cached = getCachedPassword();
        if (cached) return Promise.resolve(cached);
        return new Promise((resolve) => {
            resolveRef.current = resolve;
            setInputValue('');
            setShowModal(true);
        });
    }, [reducePasswordPrompts]);

    const submit = useCallback((usePassword: boolean) => {
        const p = usePassword ? inputValue.trim() || null : null;
        if (p) setCachedPassword(p);
        resolveRef.current?.(p);
        resolveRef.current = null;
        setInputValue('');
        setShowModal(false);
    }, [inputValue]);

    return (
        <SessionPasswordContext.Provider value={{ requestSessionPassword }}>
            {children}
            {showModal && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm p-4" role="dialog" aria-modal="true" aria-label="Session password">
                    <div className="bg-app-card border border-app-border rounded-2xl shadow-2xl p-6 max-w-md w-full space-y-4">
                        <div className="flex items-center gap-3">
                            <div className="p-2 bg-amber-500/20 rounded-xl">
                                <Lock size={24} className="text-amber-500" />
                            </div>
                            <div>
                                <h3 className="font-bold text-app-fg text-lg">Password for this session</h3>
                                <p className="text-xs text-app-muted mt-0.5">Enter once; used for installs and repairs for about 15 minutes. Not stored. Less secure than system prompt each time.</p>
                            </div>
                        </div>
                        <input
                            type="password"
                            placeholder="System password"
                            value={inputValue}
                            onChange={(e) => setInputValue(e.target.value)}
                            onKeyDown={(e) => { if (e.key === 'Enter') submit(true); if (e.key === 'Escape') submit(false); }}
                            className="w-full bg-app-bg border border-app-border rounded-xl px-4 py-3 text-app-fg placeholder:text-app-muted focus:outline-none focus:ring-2 focus:ring-amber-500/50"
                            autoFocus
                        />
                        <div className="flex gap-3">
                            <button
                                type="button"
                                onClick={() => submit(false)}
                                className="flex-1 py-2.5 rounded-xl font-bold text-sm text-app-muted hover:text-app-fg hover:bg-app-fg/5 transition-colors"
                            >
                                Use system prompt
                            </button>
                            <button
                                type="button"
                                onClick={() => submit(true)}
                                className="flex-1 py-2.5 rounded-xl font-bold text-sm text-white bg-amber-500 hover:bg-amber-600 transition-colors"
                            >
                                Use for session
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </SessionPasswordContext.Provider>
    );
}
