/**
 * Simple implementation of `vercmp` logic for Arch Linux versions.
 * Format: [epoch:]version[-release]
 * Returns: > 0 if v1 > v2, < 0 if v1 < v2, 0 if equal.
 */
export function compareVersions(v1: string, v2: string): number {
    if (v1 === v2) return 0;

    // 1. Parse Epoch
    const parseEpoch = (v: string): [number, string] => {
        const parts = v.split(':');
        if (parts.length > 1 && /^\d+$/.test(parts[0])) {
            return [parseInt(parts[0], 10), parts.slice(1).join(':')];
        }
        return [0, v];
    };

    const [e1, s1] = parseEpoch(v1);
    const [e2, s2] = parseEpoch(v2);

    if (e1 !== e2) return e1 - e2;

    // 2. Split into segments (version vs release)
    const parseRelease = (v: string): [string, string | null] => {
        const parts = v.split('-');
        if (parts.length > 1) {
            return [parts.slice(0, -1).join('-'), parts[parts.length - 1]];
        }
        return [v, null];
    };

    const [ver1, rel1] = parseRelease(s1);
    const [ver2, rel2] = parseRelease(s2);

    // 3. Compare Version Segments
    const cmpVer = compareSegments(ver1, ver2);
    if (cmpVer !== 0) return cmpVer;

    // 4. Compare Release Segments (if both present)
    if (rel1 && rel2) {
        return compareSegments(rel1, rel2);
    } else if (rel1 && !rel2) {
        return 1;
    } else if (!rel1 && rel2) {
        return -1;
    }

    return 0;
}

function compareSegments(s1: string, s2: string): number {
    // Regex to split into alternating alpha and numeric groups
    // e.g. "1.0b" -> ["1", ".", "0", "b"]
    // Simplified: split by non-alphanumeric, but keep sequences.
    // Standard `vercmp` is complex. We will use a "smart" alphanumeric split.

    // Split into chunks of digits or non-digits
    const chunker = /(\d+|\D+)/g;
    const parts1 = s1.match(chunker) || [];
    const parts2 = s2.match(chunker) || [];

    for (let i = 0; i < Math.max(parts1.length, parts2.length); i++) {
        const p1 = parts1[i] || "";
        const p2 = parts2[i] || "";

        if (p1 === p2) continue;

        const isNum1 = /^\d+$/.test(p1);
        const isNum2 = /^\d+$/.test(p2);

        if (isNum1 && isNum2) {
            const n1 = parseInt(p1, 10);
            const n2 = parseInt(p2, 10);
            if (n1 !== n2) return n1 - n2;
        } else if (!isNum1 && !isNum2) {
            return p1.localeCompare(p2);
        } else {
            // Numeric is always "newer" than alpha in some contexts, but `vercmp` says:
            // 1.0 > 1.0b ? Yes.
            // 1.0 < 1.0.1 ? Yes.
            // If one is empty?
            if (p1 === "") return -1; // s2 has extra
            if (p2 === "") return 1;  // s1 has extra

            // If one is number and other is dot?
            // This naive split is risky.
            // Let's stick to Node's localeCompare with numeric: true for simplicity in JS
            return p1.localeCompare(p2, undefined, { numeric: true, sensitivity: 'base' });
        }
    }
    return 0;
}
