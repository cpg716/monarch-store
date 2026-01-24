import { convertFileSrc } from '@tauri-apps/api/core';

/**
 * Resolves an icon string (URL or path) to a displayable Source URL.
 * Handles:
 * 1. file:// absolute paths -> convertFileSrc (asset://)
 * 2. https:// remote URLs -> pass through
 * 3. File paths without protocol -> treat as local absolute, add convertFileSrc
 */
export function resolveIconUrl(icon: string | null | undefined): string | undefined {
    if (!icon) return undefined;

    // Handle file:// protocol
    if (icon.startsWith('file://')) {
        const path = icon.replace('file://', '');
        const assetUrl = convertFileSrc(path);
        console.log(`[IconHelper] Converting: ${icon} -> ${assetUrl}`);
        return assetUrl;
    }

    // Handle local absolute paths (Linux/macOS) that might miss the protocol
    if (icon.startsWith('/')) {
        return convertFileSrc(icon);
    }

    // Remote URLs (https/http) pass through
    return icon;
}
