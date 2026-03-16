/**
 * Notification Service for MonARCH Store
 * Uses the backend show_desktop_notification command (runs off async runtime) to avoid
 * "Cannot start a runtime from within a runtime" on Linux (notify-rust/zbus).
 */

import { isPermissionGranted, requestPermission } from '@tauri-apps/plugin-notification';
import { commands } from './bindings';
import { debugWarn } from '../utils/debugLog';

let permissionGranted = false;
let permissionChecked = false;

/**
 * Ensure notification permissions are granted
 */
async function ensurePermission(): Promise<boolean> {
    if (permissionChecked) return permissionGranted;

    try {
        permissionGranted = await isPermissionGranted();
        if (!permissionGranted) {
            const result = await requestPermission();
            permissionGranted = result === 'granted';
        }
        permissionChecked = true;
    } catch (e) {
        debugWarn('[MonARCH] Notifications not available:', e);
        permissionChecked = true;
        permissionGranted = false;
    }

    return permissionGranted;
}

/**
 * Show notification via backend so it runs in spawn_blocking (avoids Tokio nested runtime panic).
 */
async function showNotification(title: string, body: string): Promise<void> {
    try {
        const res = await commands.showDesktopNotification(title, body);
        if (res.status === 'error') throw res.error;
    } catch (e) {
        debugWarn('[MonARCH] Failed to send notification:', e);
    }
}

/**
 * Send a notification about available updates
 */
export async function notifyUpdatesAvailable(count: number, breakdown?: { repo: number; aur: number; flatpak: number }): Promise<void> {
    if (count <= 0) return;
    if (!(await ensurePermission())) return;

    let body = `${count} update${count === 1 ? '' : 's'} available`;

    // Add breakdown if multiple sources
    if (breakdown) {
        const parts: string[] = [];
        if (breakdown.repo > 0) parts.push(`${breakdown.repo} system`);
        if (breakdown.aur > 0) parts.push(`${breakdown.aur} AUR`);
        if (breakdown.flatpak > 0) parts.push(`${breakdown.flatpak} Flatpak`);
        if (parts.length > 1) {
            body = `${count} updates available (${parts.join(', ')})`;
        }
    }

    await showNotification('MonARCH Store', body);
}

/**
 * Send a notification about update completion
 */
export async function notifyUpdateComplete(success: boolean, summary?: string): Promise<void> {
    if (!(await ensurePermission())) return;

    const body = success
        ? (summary || 'System updated successfully!')
        : 'Update completed with some issues';

    await showNotification('MonARCH Store', body);
}

/**
 * Send a generic notification
 */
export async function notify(title: string, body: string): Promise<void> {
    if (!(await ensurePermission())) return;

    await showNotification(title, body);
}
