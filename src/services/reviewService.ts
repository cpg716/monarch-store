import { createClient } from '@supabase/supabase-js';
import { invoke } from '@tauri-apps/api/core';

const SUPABASE_URL = "https://tcmbahxvwhcetfbtnxlj.supabase.co";
const SUPABASE_KEY = "sb_publishable_H5McpvxB2eoujN9LNzhy1w_lJup-ZcX";
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
 * Fetches reviews from ODRS (with ID probing) OR Supabase (fallback).
 */
export async function getPackageReviews(pkgName: string, appStreamId?: string): Promise<{ reviews: Review[], summary: RatingSummary }> {
    const probeIds = new Set<string>();
    if (appStreamId) {
        probeIds.add(appStreamId);
        if (!appStreamId.endsWith('.desktop')) probeIds.add(`${appStreamId}.desktop`);
    }
    if (pkgName) {
        probeIds.add(pkgName);
        if (!pkgName.endsWith('.desktop')) probeIds.add(`${pkgName}.desktop`);
    }

    // 0. Manual Patch for Popular Apps (Rapid Fix)
    // 0. Manual Patch Removed - Backend now handles this via Flathub Metadata Integration
    // if (pkgName in manualMap) probeIds.add(manualMap[pkgName]);

    const reviews: Review[] = [];
    const stars: { [key: number]: number } = { 1: 0, 2: 0, 3: 0, 4: 0, 5: 0 };
    let odrsCount = 0;
    let odrsSum = 0;

    const cutoffDate = new Date();
    cutoffDate.setDate(cutoffDate.getDate() - 365);

    // 1. Fetch ODRS (if available)
    for (const id of Array.from(probeIds)) {
        try {
            // We fetch reviews first now, as we need them for the date filter
            const odrsReviews: any[] = await invoke('get_app_reviews', { appId: id });

            if (odrsReviews && odrsReviews.length > 0) {
                // Filter ODRS reviews - Currency (365 days) AND Language (English)
                const recentOdrs = odrsReviews.filter(r => {
                    const d = new Date((r.date_created || 0) * 1000);
                    const isRecent = d >= cutoffDate;

                    // English Check: Allow if locale is missing OR starts with 'en' OR contains 'C'
                    // We assume missing locale = English/Generic to be safe
                    const locale = (r.locale || 'en_US').toLowerCase();
                    const isEnglish = locale.startsWith('en') || locale === 'c';

                    return isRecent && isEnglish;
                });

                if (recentOdrs.length > 0) {
                    odrsCount = recentOdrs.length;
                    // ODRS reviews have ratings 0-100. Normalize to 0-5.
                    const sum = recentOdrs.reduce((acc, r) => acc + (r.rating || 0), 0);
                    odrsSum = (sum / 20); // This is the total sum of "stars", ready for averaging later

                    // Re-build stats bucket for ODRS logic
                    recentOdrs.forEach(r => {
                        const starsVal = Math.round((r.rating || 0) / 20);
                        if (stars[starsVal] !== undefined) stars[starsVal]++;

                        reviews.push({
                            id: r.review_id || Math.random(),
                            userName: r.user_display || 'ODRS User',
                            rating: (r.rating || 0) / 20,
                            comment: r.description || r.summary || '',
                            date: new Date((r.date_created || 0) * 1000),
                            source: 'odrs'
                        });
                    });
                    break; // Found recent ODRS data
                }
            }
        } catch (e) {
            // Continue probing
        }
    }

    // 2. Fetch MonARCH/Supabase Reviews (Always!)
    let supabaseReviews: any[] = [];
    try {
        const { data } = await supabase
            .from('reviews')
            .select('*')
            .eq('package_name', pkgName)
            .gte('created_at', cutoffDate.toISOString()) // Database-level filter for efficiency
            .order('created_at', { ascending: false });


        if (data) {
            supabaseReviews = data;
            supabaseReviews.forEach(r => {
                reviews.push({
                    id: r.id,
                    userName: r.user_name || 'MonArch User',
                    rating: r.rating,
                    comment: r.comment,
                    date: new Date(r.created_at),
                    source: 'monarch'
                });
                // Add to stats
                const rounded = Math.round(r.rating) as 1 | 2 | 3 | 4 | 5;
                if (stars[rounded] !== undefined) stars[rounded]++;
            });
        }
    } catch (e) { console.error("Supabase fetch failed", e); }

    // 3. Calculate Composite Stats - purely from the filtered lists now
    const monarchCount = supabaseReviews.length;
    const finalCount = odrsCount + monarchCount;

    // Calculate weighted average
    // odrsSum is already the sum of stars (e.g. 4.5 + 5.0 + ...) from the loop above
    let finalSumOfStars = odrsSum;
    finalSumOfStars += supabaseReviews.reduce((acc: number, r: any) => acc + r.rating, 0);

    const finalAvg = finalCount > 0 ? finalSumOfStars / finalCount : 0;

    return {
        reviews: reviews.sort((a, b) => b.date.getTime() - a.date.getTime()),
        summary: {
            average: finalAvg,
            count: finalCount,
            stars
        }
    };
}

export async function submitReview(pkgName: string, rating: number, comment: string, userName: string) {
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
 * Batch fetch ratings for multiple packages (ODRS only).
 * Probes for both [name] and [name.desktop].
 */
export async function getRatingsBatch(pkgNames: string[]): Promise<Map<string, { average: number; count: number }>> {
    const probes: string[] = [];

    pkgNames.forEach(n => {
        probes.push(n);
        if (!n.endsWith('.desktop')) probes.push(`${n}.desktop`);
    });

    // 1. Fetch ODRS (All Time)
    let odrsResults: Record<string, any> = {};
    try {
        odrsResults = await invoke<Record<string, any>>('get_app_ratings_batch', { appIds: probes });
    } catch (e) { console.error("ODRS Batch failed", e); }

    // 2. Fetch Supabase (365 Days Filter)
    const cutoffDate = new Date();
    cutoffDate.setDate(cutoffDate.getDate() - 365);

    let supabaseMap = new Map<string, { sum: number; count: number }>();
    try {
        const { data } = await supabase
            .from('reviews')
            .select('package_name, rating')
            .in('package_name', pkgNames)
            .gte('created_at', cutoffDate.toISOString());

        if (data) {
            data.forEach(r => {
                const current = supabaseMap.get(r.package_name) || { sum: 0, count: 0 };
                current.sum += r.rating;
                current.count += 1;
                supabaseMap.set(r.package_name, current);
            });
        }
    } catch (e) { console.error("Supabase Batch failed", e); }

    const output = new Map();
    pkgNames.forEach(n => {
        // A. Resolve ODRS Score
        let odrsCount = 0;
        let odrsSum = 0;

        // Try Exact Name then .desktop
        let r = odrsResults[n];
        if (!r && !n.endsWith('.desktop')) r = odrsResults[`${n}.desktop`];

        if (r && r.total > 0) {
            odrsCount = r.total;
            // ODRS score is 0-100. Sum of stars = (Score/20) * Count.
            odrsSum = (r.score || 0) / 20 * odrsCount;
        }

        // B. Resolve Supabase Score
        const sb = supabaseMap.get(n) || { sum: 0, count: 0 };

        // C. Merge
        const totalCount = odrsCount + sb.count;
        const totalSum = odrsSum + sb.sum;

        if (totalCount > 0) {
            output.set(n, {
                average: totalSum / totalCount,
                count: totalCount
            });
        }
    });

    return output;
}

/**
 * Fetches just the rating summary (efficiently) for a package.
 * Hybrid Merge: ODRS (All Time) + Supabase (365 Days).
 */
export async function getCompositeRating(pkgName: string, appStreamId?: string): Promise<{ average: number; count: number } | null> {
    const probeIds = new Set<string>();
    if (appStreamId) {
        probeIds.add(appStreamId);
        if (!appStreamId.endsWith('.desktop')) probeIds.add(`${appStreamId}.desktop`);
    }
    if (pkgName) {
        probeIds.add(pkgName);
        if (!pkgName.endsWith('.desktop')) probeIds.add(`${pkgName}.desktop`);
    }

    let odrsCount = 0;
    let odrsSum = 0;

    // 1. Fetch ODRS Rating (Probe)
    // We strictly use get_app_rating here for speed. We accept "All Time" stats for the summary to avoid downloading 1000 reviews JSON.
    for (const id of Array.from(probeIds)) {
        try {
            const odrsRating: any = await invoke('get_app_rating', { appId: id });
            if (odrsRating && odrsRating.total > 0) {
                odrsCount = odrsRating.total;
                odrsSum = (odrsRating.score || 0) / 20 * odrsCount;
                break;
            }
        } catch (e) { }
    }

    // 2. Fetch Supabase (365 Days)
    let sbCount = 0;
    let sbSum = 0;
    try {
        const cutoffDate = new Date();
        cutoffDate.setDate(cutoffDate.getDate() - 365);

        const { data } = await supabase
            .from('reviews')
            .select('rating')
            .eq('package_name', pkgName)
            .gte('created_at', cutoffDate.toISOString());

        if (data && data.length > 0) {
            sbCount = data.length;
            sbSum = data.reduce((acc, r) => acc + r.rating, 0);
        }
    } catch (e) { /* ignore */ }

    const totalCount = odrsCount + sbCount;
    const totalSum = odrsSum + sbSum;

    if (totalCount > 0) {
        return {
            average: totalSum / totalCount,
            count: totalCount
        };
    }

    return null;
}
