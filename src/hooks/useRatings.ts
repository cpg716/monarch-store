import { useState, useEffect } from 'react';
import * as reviewService from '../services/reviewService';

// Global Cache to prevent redundant fetches within a session
const ratingCache = new Map<string, { average: number; count: number }>();
const reviewCache = new Map<string, { reviews: reviewService.Review[], summary: reviewService.RatingSummary }>();

export function prewarmRatings(pkgNames: string[]) {
    const toFetch = pkgNames.filter(n => !ratingCache.has(`${n}-`));
    if (toFetch.length === 0) return;

    reviewService.getRatingsBatch(toFetch).then(map => {
        map.forEach((val, key) => {
            // Match the cache key format in hooks: "pkgName-"
            ratingCache.set(`${key}-`, val);
        });
    });
}

/**
 * Hook for fetching simple rating summary (used in Cards).
 */
export function usePackageRating(pkgName: string, initialAppId?: string) {
    const cacheKey = `${pkgName}-${initialAppId || ''}`;
    const [rating, setRating] = useState<{ average: number; count: number } | null>(ratingCache.get(cacheKey) || null);
    const [isLoading, setIsLoading] = useState(false);

    useEffect(() => {
        // Skip if already cached
        if (ratingCache.has(cacheKey)) return;

        let isMounted = true;
        const fetchRating = async () => {
            setIsLoading(true);
            try {
                const summary = await reviewService.getCompositeRating(pkgName, initialAppId);
                if (isMounted && summary) {
                    ratingCache.set(cacheKey, summary);
                    setRating(summary);
                }
            } catch (e) {
            } finally {
                if (isMounted) setIsLoading(false);
            }
        };

        fetchRating();
        return () => { isMounted = false; };
    }, [pkgName, initialAppId, cacheKey]);

    return { rating, isLoading };
}

/**
 * Hook for fetching full reviews and stars (used in Details).
 */
export function usePackageReviews(pkgName: string, initialAppId?: string) {
    const cacheKey = `${pkgName}-${initialAppId || ''}`;
    const cached = reviewCache.get(cacheKey);

    const [reviews, setReviews] = useState<reviewService.Review[]>(cached?.reviews || []);
    const [summary, setSummary] = useState<reviewService.RatingSummary | null>(cached?.summary || null);
    const [isLoading, setIsLoading] = useState(false);

    const refresh = async (force = false) => {
        if (!force && reviewCache.has(cacheKey)) return;

        setIsLoading(true);
        try {
            const data = await reviewService.getPackageReviews(pkgName, initialAppId);
            reviewCache.set(cacheKey, data);
            setReviews(data.reviews);
            setSummary(data.summary);

            // Also update the simple rating cache for consistency
            ratingCache.set(cacheKey, data.summary);
        } catch (e) {
            console.warn("[usePackageReviews] Fetch failed", e);
        } finally {
            setIsLoading(false);
        }
    };

    useEffect(() => {
        refresh();
    }, [pkgName, initialAppId, cacheKey]);

    return { reviews, summary, isLoading, refresh: () => refresh(true) };
}
