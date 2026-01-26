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

// Global Singleton Cache with TTL
export const metadataCache = new Map<string, { data: AppMetadata, timestamp: number }>();
const CACHE_TTL = 5 * 60 * 1000; // 5 minutes

/**
 * Global Batch Executor
 */


/**
 * Hook for fetching package metadata (icons, IDs, screens).
 * Reverted to direct individual fetching to ensure Upstream URL is passed for fallback lookups.
 * (Batching removed to fix regression where URL was lost)
 */
export function usePackageMetadata(pkgName: string, upstreamUrl?: string, skip = false) {
    const [metadata, setMetadata] = useState<AppMetadata | null>(() => {
        const cached = metadataCache.get(pkgName);
        if (cached && (Date.now() - cached.timestamp < CACHE_TTL)) {
            return cached.data;
        }
        return null;
    });
    const [isLoading, setIsLoading] = useState(false);

    useEffect(() => {
        if (skip || !pkgName) return;

        // 1. Check Cache
        const cached = metadataCache.get(pkgName);
        if (cached && (Date.now() - cached.timestamp < CACHE_TTL)) {
            setMetadata(cached.data);
            return;
        }

        // 2. Fetch Directly (No Batching)
        let isMounted = true;
        setIsLoading(true);

        invoke<AppMetadata>('get_metadata', {
            pkgName,
            upstreamUrl: upstreamUrl || null
        })
            .then(data => {
                if (isMounted && data) {
                    metadataCache.set(pkgName, { data, timestamp: Date.now() });
                    setMetadata(data);
                }
            })
            .catch(err => {
                console.warn(`[Metadata] Failed for ${pkgName}:`, err);
            })
            .finally(() => {
                if (isMounted) setIsLoading(false);
            });

        return () => {
            isMounted = false;
        };
    }, [pkgName, upstreamUrl, skip]);

    return { metadata, isLoading };
}

/**
 * Force update the cache for a specific package (e.g. from search results)
 */
export function prewarmMetadataCache(pkgName: string, meta: AppMetadata) {
    metadataCache.set(pkgName, { data: meta, timestamp: Date.now() });
}
