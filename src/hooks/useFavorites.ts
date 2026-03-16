import { useCallback } from 'react';
import { useAppStore } from '../store/internal_store';
import { normalizeCanonicalId } from '../utils/packageKey';

/** Thin wrapper over global store: favorites live in internal_store; persistence in store actions. */
export function useFavorites() {
    const favorites = useAppStore((s) => s.favorites);
    const toggleFavorite = useAppStore((s) => s.toggleFavorite);

    const isFavorite = useCallback(
        (pkgNameOrId: string | undefined | null) => {
            if (!pkgNameOrId) return false;
            const norm = normalizeCanonicalId(pkgNameOrId);
            return !!norm && favorites.some((f) => normalizeCanonicalId(f) === norm);
        },
        [favorites]
    );

    return { favorites, toggleFavorite, isFavorite };
}
