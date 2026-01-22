import { createClient } from '@supabase/supabase-js';
import { invoke } from '@tauri-apps/api/core';

// Initialize Supabase Client (Placeholder Credentials)
// User must update these in production or use env vars
const SUPABASE_URL = "https://tcmbahxvwhcetfbtnxlj.supabase.co";
const SUPABASE_KEY = "sb_publishable_H5McpvxB2eoujN9LNzhylw_lJup-ZcX";
const supabase = createClient(SUPABASE_URL, SUPABASE_KEY);

export interface Review {
    id: number | string;
    userName: string;
    rating: number; // 0-5
    comment: string;
    date: Date;
    source: 'odrs' | 'monarch';
}

export interface RatingSummary {
    average: number;
    count: number;
    stars: { [key: number]: number }; // 1: count, 2: count...
}

/**
 * Fetches reviews from ODRS (if AppStream ID exists) OR Supabase (fallback).
 */
export async function getPackageReviews(pkgName: string, appStreamId?: string): Promise<{ reviews: Review[], summary: RatingSummary }> {
    // 1. Try ODRS if AppStream ID is available
    if (appStreamId) {
        try {
            const odrsRating: any = await invoke('get_app_rating', { appId: appStreamId });
            const odrsReviews: any[] = await invoke('get_app_reviews', { appId: appStreamId });

            if (odrsRating || (odrsReviews && odrsReviews.length > 0)) {

                // Fallback: Calculate summary from reviews if rating API returned null
                let average = odrsRating?.score ? odrsRating.score / 20 : 0;
                let count = odrsRating?.total || 0;

                if (count === 0 && odrsReviews.length > 0) {
                    count = odrsReviews.length;
                    const sum = odrsReviews.reduce((acc, r) => acc + (r.rating || 0), 0);
                    // ODRS ratings in reviews are usually 0-100 too, check backend 'rating' field
                    // In backend Review struct: pub rating: Option<u32>
                    // In frontend map: rating: (r.rating || 0) / 20
                    // So here we sum r.rating (0-100) and divide by count, then divide by 20 for 0-5 scale
                    average = (sum / count) / 20;
                }

                return {
                    reviews: odrsReviews.map(r => ({
                        id: r.review_id,
                        userName: r.user_display || 'Anonymous',
                        rating: (r.rating || 0) / 20,
                        comment: r.description || r.summary,
                        date: new Date(r.date_created * 1000),
                        source: 'odrs'
                    })),
                    summary: {
                        average: average,
                        count: count,
                        stars: {
                            1: odrsRating?.star1 || 0,
                            2: odrsRating?.star2 || 0,
                            3: odrsRating?.star3 || 0,
                            4: odrsRating?.star4 || 0,
                            5: odrsRating?.star5 || 0,
                        }
                    }
                };
            }
        };
    }
} catch (e) {
    console.warn("[ReviewService] ODRS Fetch failed", e);
}
    }

// 1b. Fallback: Try ODRS with pkgName if appStreamId didn't work (or wasn't provided, though logic below handles that)
// Only try if pkgName is different from appStreamId to avoid redundant call
if (pkgName && pkgName !== appStreamId) {
    try {
        const odrsReviews: any[] = await invoke('get_app_reviews', { appId: pkgName });
        if (odrsReviews && odrsReviews.length > 0) {
            const count = odrsReviews.length;
            const sum = odrsReviews.reduce((acc: any, r: any) => acc + (r.rating || 0), 0);
            const average = (sum / count) / 20;

            return {
                reviews: odrsReviews.map((r: any) => ({
                    id: r.review_id,
                    userName: r.user_display || 'Anonymous',
                    rating: (r.rating || 0) / 20,
                    comment: r.description || r.summary,
                    date: new Date(r.date_created * 1000),
                    source: 'odrs'
                })),
                summary: {
                    average,
                    count,
                    stars: {} // ODRS doesn't return stars breakdown for reviews-only fetch easily without rating api
                }
            }
        }
    } catch (e) { /* ignore */ }
}

// 2. Fallback to Supabase (MonArch Community Reviews)
try {
    const { data: reviews, error } = await supabase
        .from('reviews')
        .select('*')
        .eq('package_name', pkgName)
        .order('created_at', { ascending: false });

    if (error) {
        // console.warn("[ReviewService] Supabase error:", error); // Suppress generic errors for now
        throw error;
    }

    const typedReviews = (reviews || []).map(r => ({
        id: r.id,
        userName: r.user_name || 'MonArch User',
        rating: r.rating,
        comment: r.comment,
        date: new Date(r.created_at),
        source: 'monarch' as const
    }));

    // Calculate generic summary
    const total = typedReviews.length;
    const sum = typedReviews.reduce((acc, r) => acc + r.rating, 0);
    const avg = total > 0 ? sum / total : 0;

    // Simple star count
    const stars = { 1: 0, 2: 0, 3: 0, 4: 0, 5: 0 };
    typedReviews.forEach(r => {
        const rounded = Math.round(r.rating) as 1 | 2 | 3 | 4 | 5;
        if (stars[rounded] !== undefined) stars[rounded]++;
    });

    return {
        reviews: typedReviews,
        summary: {
            average: avg,
            count: total,
            stars
        }
    };

} catch (e) {
    console.warn("Supabase Fetch failed", e);
    return { reviews: [], summary: { average: 0, count: 0, stars: { 1: 0, 2: 0, 3: 0, 4: 0, 5: 0 } } };
}
}

export async function submitReview(pkgName: string, rating: number, comment: string, userName: string) {
    // Only submitting to Supabase for now to avoid ODRS auth complexity
    const { error } = await supabase
        .from('reviews')
        .insert({
            package_name: pkgName,
            rating,
            comment,
            user_name: userName
        });

    if (error) throw error;
}

/**
 * Fetches just the rating summary (efficiently) for a package.
 * Tries ODRS first, then Supabase.
 */
export async function getCompositeRating(pkgName: string, appStreamId?: string): Promise<{ average: number; count: number } | null> {

    // 1. Try ODRS
    if (appStreamId) {
        try {
            // Try lightweight rating API first
            const odrsRating: any = await invoke('get_app_rating', { appId: appStreamId });

            if (odrsRating && odrsRating.total > 0) {
                return {
                    average: odrsRating.score ? odrsRating.score / 20 : 0,
                    count: odrsRating.total
                };
            }

            // Fallback: Fetch reviews if rating API failed but reviews might exist (e.g. VLC case)
            const odrsReviews: any[] = await invoke('get_app_reviews', { appId: appStreamId });
            if (odrsReviews && odrsReviews.length > 0) {
                const count = odrsReviews.length;
                const sum = odrsReviews.reduce((acc, r) => acc + (r.rating || 0), 0);
                const average = (sum / count) / 20; // 0-100 -> 0-5
                return { average, count };
            }

        } catch (e) {
            // console.warn("ODRS Rating fetch failed", e);
        }
    }

    // 1b. Fallback ODRS (Try pkgName if different)
    if (pkgName && pkgName !== appStreamId) {
        try {
            const odrsRating: any = await invoke('get_app_rating', { appId: pkgName });
            if (odrsRating && odrsRating.total > 0) {
                return {
                    average: odrsRating.score ? odrsRating.score / 20 : 0,
                    count: odrsRating.total
                };
            }
        } catch (e) { }
    }

    // 2. Fallback to Supabase
    try {
        const { data, error } = await supabase
            .from('reviews')
            .select('rating')
            .eq('package_name', pkgName);

        if (error) throw error;

        if (data && data.length > 0) {
            const count = data.length;
            const sum = data.reduce((acc, r) => acc + r.rating, 0);
            return {
                average: sum / count,
                count
            };
        }
    } catch (e) { /* ignore */ }

    return null;
}
