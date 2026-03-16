import { debugWarn } from './debugLog';

/**
 * Frontend identity is backend-owned. The only valid registry key is backend canonical_id.
 */
export function normalizeCanonicalId(raw: string): string {
    return raw.trim().toLowerCase().replace(/[^a-z0-9]/g, '');
}

export function getPackageListKey(pkg: {
    canonical_id?: unknown;
    name?: unknown;
    app_id?: unknown;
}): string {
    if (typeof pkg.canonical_id === "string") {
        const canonical = normalizeCanonicalId(pkg.canonical_id);
        if (canonical) return canonical;
    }

    // Emergency render/storage key when backend violates the canonical_id contract.
    // We still warn loudly so this remains visible during development.
    const name =
        typeof pkg.name === 'string' && pkg.name.trim().length > 0
            ? normalizeCanonicalId(pkg.name)
            : '';
    const appId =
        typeof pkg.app_id === 'string' && pkg.app_id.trim().length > 0
            ? normalizeCanonicalId(pkg.app_id)
            : '';

    const fallback = name || appId;
    if (fallback) {
        debugWarn('[IRON-CORE] Backend package missing canonical_id, using emergency key:', {
            name: typeof pkg.name === 'string' ? pkg.name : null,
            app_id: typeof pkg.app_id === 'string' ? pkg.app_id : null,
            fallback,
        });
        return fallback;
    }

    return "";
}

export function getKnownAppIdsForPackage(_pkg: {
    canonical_id?: unknown;
    app_id?: string | null;
    name: string;
}): string[] {
    return [];
}

export function expandRatingLookupIds(ids: string[]): string[] {
    return Array.from(new Set(ids.map((value) => String(value ?? '').trim()).filter(Boolean)));
}

/** Safe string key for PackageSource. Backend may return source_type/id as non-strings. */
export function getSourceKey(
    source: { source_type?: unknown; id?: unknown } | string,
    index?: number
): string {
    if (typeof source === "string") return source;
    const st = typeof source.source_type === "string" ? source.source_type : "";
    const id = typeof source.id === "string" ? source.id : "";
    const base = `${st}:${id}`;
    return index !== undefined ? `${base}-${index}` : base;
}

/** Ensures key is always a valid string; use index fallback when value is object/non-primitive. */
export function safeKey(value: unknown, index: number): string {
    if (value === null || value === undefined) return `key-${index}`;
    if (typeof value === "string" || typeof value === "number") return String(value);
    return `key-${index}`;
}
