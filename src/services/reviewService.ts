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
    const manualMap: Record<string, string> = {
        'vlc': 'org.videolan.VLC',
        'gimp': 'org.gimp.GIMP',
        'lutris': 'net.lutris.Lutris',
        'upscayl-bin': 'org.upscayl.Upscayl',
        'heroic-games-launcher-bin': 'com.heroicgameslauncher.hgl',
        'firefox': 'org.mozilla.firefox',
        'discord': 'com.discordapp.Discord',
        'steam': 'com.valvesoftware.Steam',
        'visual-studio-code-bin': 'com.visualstudio.code',
        'obsidian': 'md.obsidian.Obsidian',
        'spotify': 'com.spotify.Client'
    };
    if (pkgName in manualMap) probeIds.add(manualMap[pkgName]);

    // 1. Try ODRS with probing
    for (const id of Array.from(probeIds)) {
        try {
            const odrsRating: any = await invoke('get_app_rating', { appId: id });
            const odrsReviews: any[] = await invoke('get_app_reviews', { appId: id });

            const hasOdrsRatings = odrsRating && odrsRating.total > 0;
            const hasOdrsReviews = odrsReviews && odrsReviews.length > 0;

            if (hasOdrsRatings || hasOdrsReviews) {
                let average = odrsRating?.score ? odrsRating.score / 20 : 0;
                let count = odrsRating?.total || 0;

                if (count === 0 && odrsReviews.length > 0) {
                    count = odrsReviews.length;
                    const sum = odrsReviews.reduce((acc, r) => acc + (r.rating || 0), 0);
                    average = (sum / count) / 20;
                }

                return {
                    reviews: odrsReviews.map(r => ({
                        id: r.review_id || Math.random(),
                        userName: r.user_display || 'Anonymous',
                        rating: (r.rating || 0) / 20,
                        comment: r.description || r.summary || '',
                        date: new Date((r.date_created || 0) * 1000),
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
        } catch (e) {
            // Continue probing
        }
    }

    // 2. Fallback to Supabase
    try {
        const { data: reviews, error } = await supabase
            .from('reviews')
            .select('*')
            .eq('package_name', pkgName)
            .order('created_at', { ascending: false });

        if (error) throw error;

        const typedReviews = (reviews || []).map(r => ({
            id: r.id,
            userName: r.user_name || 'MonArch User',
            rating: r.rating,
            comment: r.comment,
            date: new Date(r.created_at),
            source: 'monarch' as const
        }));

        const total = typedReviews.length;
        const sum = typedReviews.reduce((acc, r) => acc + r.rating, 0);
        const avg = total > 0 ? sum / total : 0;

        const stars = { 1: 0, 2: 0, 3: 0, 4: 0, 5: 0 };
        typedReviews.forEach(r => {
            const rounded = Math.round(r.rating) as 1 | 2 | 3 | 4 | 5;
            if (stars[rounded] !== undefined) stars[rounded]++;
        });

        return {
            reviews: typedReviews,
            summary: { average: avg, count: total, stars }
        };
    } catch (e) {
        return { reviews: [], summary: { average: 0, count: 0, stars: { 1: 0, 2: 0, 3: 0, 4: 0, 5: 0 } } };
    }
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

    // Shared Manual Map (Duplicate of getCompositeRating for now to be safe)
    const manualMap: Record<string, string> = {
        'vlc': 'org.videolan.VLC',
        'gimp': 'org.gimp.GIMP',
        'lutris': 'net.lutris.Lutris',
        'upscayl-bin': 'org.upscayl.Upscayl',
        'heroic-games-launcher-bin': 'com.heroicgameslauncher.hgl',
        'firefox': 'org.mozilla.firefox',
        'discord': 'com.discordapp.Discord',
        'steam': 'com.valvesoftware.Steam',
        'visual-studio-code-bin': 'com.visualstudio.code',
        'obsidian': 'md.obsidian.Obsidian',
        'spotify': 'com.spotify.Client'
    };

    pkgNames.forEach(n => {
        probes.push(n);
        if (!n.endsWith('.desktop')) probes.push(`${n}.desktop`);
        if (n in manualMap) probes.push(manualMap[n]);
    });

    try {
        // Rust returns HashMap<String, OdrsRating>
        const results = await invoke<Record<string, any>>('get_app_ratings_batch', { appIds: probes });

        const output = new Map();
        pkgNames.forEach(n => {
            // 1. Try manual map match first (highest quality)
            let r = (n in manualMap) ? results[manualMap[n]] : null;

            // 2. Try Exact Name
            if (!r) r = results[n];

            // 3. Try .desktop
            if (!r && !n.endsWith('.desktop')) r = results[`${n}.desktop`];

            if (r && r.total > 0) {
                output.set(n, {
                    average: r.score ? r.score / 20 : 0,
                    count: r.total
                });
            }
        });
        return output;
    } catch (e) {
        console.error("Batch rating fetch failed", e);
        return new Map();
    }
}

/**
 * Fetches just the rating summary (efficiently) for a package.
 * Tries ODRS first (probing), then Supabase.
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

    // 0. Manual Patch for Popular Apps (Rapid Fix)
    const manualMap: Record<string, string> = {
        'vlc': 'org.videolan.VLC',
        'gimp': 'org.gimp.GIMP',
        'lutris': 'net.lutris.Lutris',
        'upscayl-bin': 'org.upscayl.Upscayl',
        'heroic-games-launcher-bin': 'com.heroicgameslauncher.hgl',
        'firefox': 'org.mozilla.firefox',
        'discord': 'com.discordapp.Discord',
        'steam': 'com.valvesoftware.Steam',
        'visual-studio-code-bin': 'com.visualstudio.code',
        'obsidian': 'md.obsidian.Obsidian',
        'spotify': 'com.spotify.Client'
    };
    if (pkgName in manualMap) probeIds.add(manualMap[pkgName]);

    // 1. Try ODRS with probing
    for (const id of Array.from(probeIds)) {
        try {
            const odrsRating: any = await invoke('get_app_rating', { appId: id });
            if (odrsRating && odrsRating.total > 0) {
                return {
                    average: odrsRating.score ? odrsRating.score / 20 : 0,
                    count: odrsRating.total
                };
            }

            const odrsReviews: any[] = await invoke('get_app_reviews', { appId: id });
            if (odrsReviews && odrsReviews.length > 0) {
                const count = odrsReviews.length;
                const sum = odrsReviews.reduce((acc, r) => acc + (r.rating || 0), 0);
                const average = (sum / count) / 20;
                return { average, count };
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
