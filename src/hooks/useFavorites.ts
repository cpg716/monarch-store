import { useCallback } from 'react';
import { useAppStore } from '../store/internal_store';

/** Thin wrapper over global store: favorites live in internal_store; persistence in store actions. */
export function useFavorites() {
    const favorites = useAppStore((s) => s.favorites);
    const toggleFavorite = useAppStore((s) => s.toggleFavorite);

    const isFavorite = useCallback(
        (pkgNameOrId: string) => {
            const norm = pkgNameOrId.toLowerCase().trim();
            return favorites.some((f) => f.toLowerCase() === norm);
        },
        [favorites]
    );

    return { favorites, toggleFavorite, isFavorite };
}
