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

/**
 * Global Batch Executor
 */


/**
 * Hook for fetching package metadata (icons, IDs, screens).
 * Reverted to direct individual fetching to ensure Upstream URL is passed for fallback lookups.
 * (Batching removed to fix regression where URL was lost)
 */
export function usePackageMetadata(pkgName: string, upstreamUrl?: string, skip = false) {
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

        // 2. Fetch Directly (No Batching)
        let isMounted = true;
        setIsLoading(true);

        invoke<AppMetadata>('get_metadata', {
            pkgName,
            upstreamUrl: upstreamUrl || null
        })
            .then(data => {
                if (isMounted && data) {
                    metadataCache.set(pkgName, data);
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
    metadataCache.set(pkgName, meta);
}
