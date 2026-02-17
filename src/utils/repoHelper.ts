import type { PackageSource } from '../services/bindings';

/** Normalize source ID for comparison (lowercase, alphanumeric only). matches backend `canonical_merge_key` logic roughly. */
export function normalizeSourceId(id: string): string {
    return id.toLowerCase().replace(/[^a-z0-9]/g, '');
}

/** Compare two sources (string or PackageSource); e.g. "flatpak" matches { id: "flathub", source_type: "flatpak" } but not flathub-beta. */
export function isSameSource(a: PackageSource | string, b: PackageSource | string): boolean {
    if (typeof a === 'string' && typeof b === 'string') return normalizeSourceId(a) === normalizeSourceId(b);
    if (typeof a !== 'string' && typeof b !== 'string') return normalizeSourceId(a.id) === normalizeSourceId(b.id) && normalizeSourceId(a.source_type) === normalizeSourceId(b.source_type);
    const str = typeof a === 'string' ? a : (b as string);
    const obj = typeof a === 'string' ? (b as PackageSource) : (a as PackageSource);
    if (typeof obj !== 'object' || obj === null) return false;

    const normStr = normalizeSourceId(str);
    const normObjId = normalizeSourceId(obj.id);
    const normObjType = normalizeSourceId(obj.source_type);

    if (normObjId === normStr) return true;
    if (normObjType === normStr) return normStr !== 'flatpak' || normObjId === 'flathub'; // "flatpak" string = stable only
    return false;
}

/** Normalize string (legacy) to PackageSource for tier/letter. */
export function toPackageSource(source: PackageSource | string): PackageSource {
    if (typeof source === 'object') return source;
    const s = (source as string).toLowerCase();
    const base = { package_name: null };
    if (s === 'chaotic' || s === 'chaotic-aur') return { ...base, source_type: 'repo', id: 'chaotic-aur', version: '', label: 'Chaotic-AUR' };
    if (s === 'aur') return { ...base, source_type: 'aur', id: 'aur', version: '', label: 'AUR' };
    if (s === 'flatpak' || s === 'flathub') return { ...base, source_type: 'flatpak', id: 'flathub', version: '', label: 'Flatpak' };
    if (s === 'flathub-beta') return { ...base, source_type: 'flatpak', id: 'flathub-beta', version: '', label: 'Flatpak (Beta)' };
    if (['core', 'extra', 'community', 'multilib', 'official'].includes(s)) return { ...base, source_type: 'repo', id: s === 'official' ? 'core' : s, version: '', label: 'Arch Official' };
    return { ...base, source_type: 'repo', id: s, version: '', label: s };
}

/**
 * Higher number = higher priority.
 * Hierarchy: Distro Repos > Arch Official > Chaotic-AUR > Flatpak > AUR.
 * Pass distroId (e.g. from useDistro().distro.id) to prefer current distro's repos.
 */
export function getSourceTier(source: PackageSource, distroId: string = ''): number {
    const s = source;
    const id = (s.id || '').toLowerCase();
    const type = (s.source_type || '').toLowerCase();
    const label = (s.label || '').toLowerCase();

    // 1. Distro-specific (CachyOS, EndeavourOS, Manjaro, etc.) — closest to the metal
    if (distroId && (id.includes(distroId) || id.startsWith('cachyos'))) return 100;
    if (id.includes('cachyos') || id.includes('garuda') || id.includes('endeavour') || id.includes('manjaro') ||
        label.includes('cachyos') || label.includes('garuda') || label.includes('endeavour') || label.includes('manjaro')) {
        return 100;
    }

    // 2. Official Arch (core, extra, community, multilib, or legacy "official")
    if (['core', 'extra', 'community', 'multilib', 'official'].includes(id)) return 90;

    // 3. Chaotic-AUR (pre-built binaries)
    if (id === 'chaotic-aur' || id === 'chaotic' || label.includes('chaotic')) return 80;

    // 4. Flatpak (sandboxed). Stable before Beta in sort order.
    if (type === 'flatpak' && id === 'flathub-beta') return 65;
    if (type === 'flatpak' || id === 'flathub') return 70;

    // 5. AUR (build from source — lowest for "instant" installs)
    if (type === 'aur' || id === 'aur') return 60;

    // Other repo
    if (type === 'repo') return 90;
    return 0;
}

/** Best source by order: Distro → Arch Official → Chaotic → Flatpak → AUR. */
export function getBestSource(availableSources?: PackageSource[], distroId: string = ''): PackageSource | null {
    if (!availableSources?.length) return null;
    const sorted = [...availableSources].sort((a, b) => getSourceTier(b, distroId) - getSourceTier(a, distroId));
    return sorted[0];
}

/** Tier for sorting (accepts PackageSource or legacy string). Higher = higher priority. */
export function getSourceTierForSort(source: PackageSource | string, distroId: string = ''): number {
    return getSourceTier(toPackageSource(source), distroId);
}

/** Other sources (excluding primary) in same priority order, for letter badges. */
export function getOtherSourcesInOrder(
    availableSources?: PackageSource[],
    primary?: PackageSource | null,
    distroId: string = ''
): PackageSource[] {
    if (!availableSources?.length) return [];
    const primaryKey = primary ? `${primary.source_type}:${primary.id}` : null;
    const others = availableSources.filter(s => `${s.source_type}:${s.id}` !== primaryKey);
    return others.sort((a, b) => getSourceTier(b, distroId) - getSourceTier(a, distroId));
}

/**
 * Canonical filter "family" id for Search and Category filters.
 * Maps repo/source to a single id so UI and backend stay in sync (e.g. core + extra -> "official").
 */
export function getSourceFamilyId(source: PackageSource | string): string {
    const s = typeof source === 'object' ? source : toPackageSource(source);
    const id = (s.id || '').toLowerCase();
    const st = (s.source_type || '').toLowerCase();
    if (['core', 'extra', 'community', 'multilib', 'official'].includes(id)) return 'official';
    if (id === 'chaotic-aur' || id === 'chaotic') return 'chaotic-aur';
    if (st === 'aur' || id === 'aur') return 'aur';
    if (st === 'flatpak' || id === 'flathub' || id === 'flathub-beta') return 'flatpak';
    if (id.includes('cachyos')) return 'cachyos';
    if (id.includes('manjaro')) return 'manjaro';
    if (id.includes('garuda')) return 'garuda';
    if (id.includes('endeavour')) return 'endeavour';
    return id || st || 'other';
}

/** Display label for a filter family id (matches Search and Category chip labels). */
export function getSourceFamilyLabel(familyId: string): string {
    const id = familyId.toLowerCase();
    if (id === 'official') return 'Official';
    if (id === 'chaotic-aur') return 'AUR-Binaries';
    if (id === 'aur') return 'AUR-Source';
    if (id === 'flatpak') return 'Flatpak';
    if (id === 'cachyos') return 'CachyOS';
    if (id === 'manjaro') return 'Manjaro';
    if (id === 'garuda') return 'Garuda';
    if (id === 'endeavour') return 'EndeavourOS';
    return familyId.charAt(0).toUpperCase() + familyId.slice(1);
}

/** Two-letter code for source (for "other available" badges). */
export function getSourceLetter(source: PackageSource): string {
    const st = (source.source_type || '').toLowerCase();
    const id = (source.id || '').toLowerCase();
    const label = (source.label || '').toLowerCase();
    if (id.includes('cachyos')) return 'Ca';
    if (id.includes('garuda')) return 'Ga';
    if (id.includes('endeavour')) return 'En';
    if (id.includes('manjaro')) return 'Ma';
    if (['core', 'extra', 'community', 'multilib', 'official'].includes(id) || (st === 'repo' && !id.includes('chaotic'))) return 'Ar';
    if (id === 'chaotic-aur' || label.includes('chaotic')) return 'Ch';
    if (st === 'flatpak' || id === 'flathub') return 'Fl';
    if (st === 'aur') return 'Au';
    return 'Re';
}

/** Tailwind classes for source letter badge (compact, colored). */
export function getSourceLetterColor(source: PackageSource): string {
    const st = (source.source_type || '').toLowerCase();
    const id = (source.id || '').toLowerCase();
    const label = (source.label || '').toLowerCase();
    if (id.includes('cachyos')) return 'bg-green-600/90 text-white border-green-500/50';
    if (id.includes('garuda') || label.includes('garuda')) return 'bg-purple-600/90 text-white border-purple-500/50';
    if (id.includes('endeavour')) return 'bg-violet-600/90 text-white border-violet-500/50';
    if (id.includes('manjaro')) return 'bg-teal-600/90 text-white border-teal-500/50';
    if (['core', 'extra', 'community', 'multilib'].includes(id) || (st === 'repo' && !id.includes('chaotic'))) return 'bg-blue-600/90 text-white border-blue-500/50';
    if (id === 'chaotic-aur' || label.includes('chaotic')) return 'bg-purple-500/90 text-white border-purple-400/50';
    if (st === 'flatpak' || id === 'flathub') return 'bg-slate-500/90 text-white border-slate-400/50';
    if (st === 'aur') return 'bg-orange-500/90 text-white border-orange-400/50';
    return 'bg-gray-500/90 text-white border-gray-400/50';
}

/** Number of additional sources beyond the best (for "+N sources" indicator). */
export function getAdditionalSourceCount(availableSources?: PackageSource[]): number {
    return Math.max(0, (availableSources?.length ?? 1) - 1);
}

/**
 * Grand Unification: Top 25 Arch Distro Badge Colors
 * Distinct visual identity per distro family.
 */
export const getRepoColor = (labelRaw: string): string => {
    const label = labelRaw.toLowerCase();

    // SteamOS / Chimera / GamerOS (Gaming Vibe - Indigo)
    if (label.includes('steamos') || label.includes('chimeraos') || label.includes('gameros') || label.includes('jupiter') || label.includes('holo')) {
        return 'bg-indigo-600 border-indigo-500/50 text-white shadow-indigo-500/20';
    }

    // Chaotic / Garuda (Performance/Gaming - Purple)
    if (label.includes('chaotic') || label.includes('garuda') || label.includes('dragonized')) {
        return 'bg-purple-600 border-purple-500/50 text-white shadow-purple-500/20';
    }

    // CachyOS (Green Optimization)
    if (label.includes('cachyos')) {
        return 'bg-green-600 border-green-500/50 text-white shadow-green-500/20';
    }

    // EndeavourOS (Violet)
    if (label.includes('endeavour')) {
        return 'bg-violet-600 border-violet-500/50 text-white shadow-violet-500/20';
    }

    // Manjaro / Mabox (Teal)
    if (label.includes('manjaro') || label.includes('mabox')) {
        return 'bg-teal-600 border-teal-500/50 text-white shadow-teal-500/20';
    }

    // BlackArch / Parabola / Hyperbola (Black/Gray - Security/Libre)
    if (label.includes('blackarch') || label.includes('parabola') || label.includes('hyperbola') || label.includes('security')) {
        return 'bg-gray-800 border-gray-600/50 text-white shadow-black/40';
    }

    // Arch Official (Classic Blue)
    if (label.includes('arch') || label.includes('official') || label === 'core' || label === 'extra' || label === 'multilib') {
        return 'bg-blue-600 border-blue-500/50 text-white shadow-blue-500/20';
    }

    // AUR (Orange Community)
    if (label.includes('aur')) {
        return 'bg-orange-500 border-orange-400/50 text-white shadow-orange-500/20';
    }

    // Flatpak (Slate/Sandboxed)
    if (label.includes('flatpak')) {
        return 'bg-slate-500 border-slate-400/50 text-white shadow-slate-500/20';
    }

    // Fallback
    return 'bg-gray-500 border-gray-400/50 text-white';
};
