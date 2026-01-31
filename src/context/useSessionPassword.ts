import { useContext } from 'react';
import { SessionPasswordContext } from './SessionPasswordContext';

export function useSessionPassword() {
    const ctx = useContext(SessionPasswordContext);
    if (!ctx) throw new Error('useSessionPassword must be used within SessionPasswordProvider');
    return ctx;
}
