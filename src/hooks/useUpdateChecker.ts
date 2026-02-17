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
 * Pass includeAur/includeFlatpak from Settings so the update count matches what "Update All" would run.
 */
export function useUpdateChecker(includeAur?: boolean, includeFlatpak?: boolean) {
    const refreshPendingUpdates = useAppStore((s) => s.refreshPendingUpdates);
    const pendingUpdates = useAppStore((s) => s.pendingUpdates);
    const updateNotificationsEnabled = useAppStore((s) => s.updateNotificationsEnabled);
    const isUpdating = useAppStore((s) => s.isUpdating);

    const lastNotifyRef = useRef(0);
    const previousTotalRef = useRef(0);

    const runRefresh = () => refreshPendingUpdates(includeAur, includeFlatpak);

    // Initial check on mount
    useEffect(() => {
        const timeout = setTimeout(runRefresh, 10000);
        return () => clearTimeout(timeout);
    }, [refreshPendingUpdates, includeAur, includeFlatpak]);

    // Periodic background checks
    useEffect(() => {
        const interval = setInterval(() => {
            if (!isUpdating) runRefresh();
        }, CHECK_INTERVAL_MS);
        return () => clearInterval(interval);
    }, [refreshPendingUpdates, isUpdating, includeAur, includeFlatpak]);

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
