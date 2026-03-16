export function debugInfo(...args: unknown[]): void {
    if (import.meta.env.DEV) {
        console.info(...args);
    }
}

export function debugWarn(...args: unknown[]): void {
    if (import.meta.env.DEV) {
        console.warn(...args);
    }
}

export function debugError(...args: unknown[]): void {
    if (import.meta.env.DEV) {
        console.error(...args);
    }
}

