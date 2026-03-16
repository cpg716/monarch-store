/**
 * useUpdateChecker - Background update checking hook
 * Periodically checks for updates and sends desktop notifications
 */

import { useEffect, useRef } from 'react';
import { useAppStore } from '../store/internal_store';
import { notifyUpdatesAvailable } from '../services/notificationService';

// Check interval: 30 minutes
const CHECK_INTERVAL_MS = 30 * 60 * 1000;
// Minimum time between notifying (to avoid spam): 2 hours
const MIN_NOTIFY_INTERVAL_MS = 2 * 60 * 60 * 1000;

/**
 * Hook that performs background update checking.
 * Updates include installed packages from all supported sources (repo/AUR/Flatpak),
 * independent of discovery toggles.
 */
export function useUpdateChecker(enabled = true) {
    const refreshPendingUpdates = useAppStore((s) => s.refreshPendingUpdates);
    const pendingUpdates = useAppStore((s) => s.pendingUpdates);
    const updateNotificationsEnabled = useAppStore((s) => s.updateNotificationsEnabled);
    const isUpdating = useAppStore((s) => s.isUpdating);

    const lastNotifyRef = useRef(0);
    const previousTotalRef = useRef(0);

    const runRefresh = () => refreshPendingUpdates(true, true);

    // Initial check on mount
    useEffect(() => {
        if (!enabled) return;
        const timeout = setTimeout(runRefresh, 45000);
        return () => clearTimeout(timeout);
    }, [enabled, refreshPendingUpdates]);

    // Periodic background checks
    useEffect(() => {
        if (!enabled) return;
        const interval = setInterval(() => {
            if (!isUpdating) runRefresh();
        }, CHECK_INTERVAL_MS);
        return () => clearInterval(interval);
    }, [enabled, refreshPendingUpdates, isUpdating]);

    // Send notification when new updates are found
    useEffect(() => {
        if (!updateNotificationsEnabled) return;
        if (pendingUpdates.total === 0) return;

        const now = Date.now();
        const hasNewUpdates = pendingUpdates.total > previousTotalRef.current;
        const canNotify = now - lastNotifyRef.current > MIN_NOTIFY_INTERVAL_MS;

        if (hasNewUpdates && canNotify) {
            notifyUpdatesAvailable(pendingUpdates.total, {
                repo: pendingUpdates.repo,
                aur: pendingUpdates.aur,
                flatpak: pendingUpdates.flatpak,
            });
            lastNotifyRef.current = now;
        }

        previousTotalRef.current = pendingUpdates.total;
    }, [pendingUpdates, updateNotificationsEnabled]);
}
