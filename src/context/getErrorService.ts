import type { ErrorContextType } from './ErrorContext';

/**
 * Get the error service from window (for use outside React tree: hooks, store, main.tsx).
 * Returns undefined until ErrorProvider has mounted. Use for optional reporting in catch blocks.
 */
export function getErrorService(): ErrorContextType | undefined {
    if (typeof window === 'undefined') return undefined;
    return (window as Window & { __errorService?: ErrorContextType }).__errorService;
}
