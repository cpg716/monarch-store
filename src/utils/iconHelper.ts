import { convertFileSrc } from '@tauri-apps/api/core';

/** True if string looks like raw base64 image data (no data: prefix). */
function isRawBase64(s: string): boolean {
    const stripped = s.replace(/\s/g, '');
    if (stripped.length < 50) return false;
    return /^[A-Za-z0-9+/=]+$/.test(stripped);
}

/**
 * Resolves an icon string (URL or path) to a displayable Source URL.
 * Handles:
 * 1. file:// absolute paths -> convertFileSrc (asset://)
 * 2. https:// remote URLs -> pass through
 * 3. data: URLs -> pass through
 * 4. File paths without protocol -> treat as local absolute, convertFileSrc
 * 5. Raw base64 strings (with or without whitespace) -> wrap as data:image/png;base64,...
 */
export function resolveIconUrl(icon: string | null | undefined): string | undefined {
    if (!icon || typeof icon !== 'string') return undefined;
    const s = icon.trim();

    // Already a data URL
    if (s.startsWith('data:')) return s;

    // Handle file:// protocol
    if (s.startsWith('file://')) {
        const path = s.replace('file://', '');
        return convertFileSrc(path);
    }

    // Handle local absolute paths (Linux/macOS) that might miss the protocol
    if (s.startsWith('/')) {
        return convertFileSrc(s);
    }

    // Remote URLs (https/http) pass through
    if (s.startsWith('http://') || s.startsWith('https://')) return s;

    // Backend sometimes returns raw base64 (with or without newlines); browser 400s if used as src
    if (isRawBase64(s)) return `data:image/png;base64,${s.replace(/\s/g, '')}`;

    // Final guard: never return a base64-looking string without the data: prefix (catches any missed path)
    if (s.length >= 50 && /^[A-Za-z0-9+/=\s]+$/.test(s)) {
        return `data:image/png;base64,${s.replace(/\s/g, '')}`;
    }

    return s;
}

/**
 * Resolves a screenshot/image URL for display in the webview.
 * Same logic as resolveIconUrl: file:// and / paths become asset:// via convertFileSrc; https passes through.
 */
export function resolveImageUrl(url: string | null | undefined): string | undefined {
    return resolveIconUrl(url);
}
