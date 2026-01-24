import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface AppMetadata {
    name: string;
    pkg_name?: string;
    icon_url?: string;
    app_id: string;
    summary?: string;
    screenshots: string[];
    version?: string;
    maintainer?: string;
    license?: string;
    last_updated?: number;
    description?: string;
}

// Global Singleton Cache
const metadataCache = new Map<string, AppMetadata>();

// Batching Queue
let pendingQueue: string[] = [];
let batchTimeout: ReturnType<typeof setTimeout> | null = null;
const BATCH_WINDOW_MS = 100; // Collect requests for 100ms

// Callbacks for subscribers
const subscribers = new Map<string, ((meta: AppMetadata) => void)[]>();

/**
 * Global Batch Executor
 */
async function processBatch() {
    const pkgNames = Array.from(new Set(pendingQueue));
    pendingQueue = [];
    batchTimeout = null;

    if (pkgNames.length === 0) return;

    try {
        const results = await invoke<Record<string, AppMetadata>>('get_metadata_batch', { pkgNames });

        Object.entries(results).forEach(([name, meta]) => {
            metadataCache.set(name, meta);
            // Notify all hooks waiting for this package
            const packageSubs = subscribers.get(name);
            if (packageSubs) {
                packageSubs.forEach(cb => cb(meta));
                subscribers.delete(name);
            }
        });

        // CRITICAL: Notify anyone left in the queue that no metadata was found
        // otherwise they stay in isLoading: true forever
        pkgNames.forEach(name => {
            const leftOver = subscribers.get(name);
            if (leftOver) {
                // We pass null to indicate "no metadata found"
                leftOver.forEach(cb => cb(null as any));
                subscribers.delete(name);
            }
        });
    } catch (e) {
        console.error("[MetadataHook] Batch fetch failed", e);
        // Clear entire queue on hard failure
        pkgNames.forEach(name => {
            const subs = subscribers.get(name);
            if (subs) {
                subs.forEach(cb => cb(null as any));
                subscribers.delete(name);
            }
        });
    }
}

/**
 * Hook for fetching package metadata (icons, IDs, screens).
 * Automatically batches requests from multiple components.
 */
export function usePackageMetadata(pkgName: string, skip = false) {
    const [metadata, setMetadata] = useState<AppMetadata | null>(metadataCache.get(pkgName) || null);
    const [isLoading, setIsLoading] = useState(false);

    useEffect(() => {
        if (skip || !pkgName) return;

        // 1. Check Cache
        const cached = metadataCache.get(pkgName);
        if (cached) {
            setMetadata(cached);
            return;
        }

        // 2. Already loading? Add callback
        const currentSubs = subscribers.get(pkgName) || [];
        const isAlreadyLoading = currentSubs.length > 0;

        const onDataReceived = (data: AppMetadata) => {
            setMetadata(data);
            setIsLoading(false);
        };

        subscribers.set(pkgName, [...currentSubs, onDataReceived]);

        if (isAlreadyLoading) {
            setIsLoading(true);
            return;
        }

        // 3. Queue for batching
        setIsLoading(true);
        pendingQueue.push(pkgName);

        if (!batchTimeout) {
            batchTimeout = setTimeout(processBatch, BATCH_WINDOW_MS);
        }

    }, [pkgName, skip]);

    return { metadata, isLoading };
}

/**
 * Force update the cache for a specific package (e.g. from search results)
 */
export function prewarmMetadataCache(pkgName: string, meta: AppMetadata) {
    metadataCache.set(pkgName, meta);
}
