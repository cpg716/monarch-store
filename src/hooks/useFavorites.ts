import { useState, useEffect } from 'react';

// Shared event emitter for syncing favorites across components
const FAVORITES_UPDATED_EVENT = 'favorites-updated';

export function useFavorites() {
    const [favorites, setFavorites] = useState<string[]>(() => {
        try {
            const saved = localStorage.getItem('monarch_favorites');
            return saved ? JSON.parse(saved) : [];
        } catch (e) {
            console.error("Failed to parse favorites", e);
            return [];
        }
    });

    // Listen for updates from other instances of the hook
    useEffect(() => {
        const handleStorageChange = () => {
            try {
                const saved = localStorage.getItem('monarch_favorites');
                if (saved) {
                    setFavorites(JSON.parse(saved));
                }
            } catch (e) {
                console.error("Failed to sync favorites", e);
            }
        };

        window.addEventListener(FAVORITES_UPDATED_EVENT, handleStorageChange);
        return () => window.removeEventListener(FAVORITES_UPDATED_EVENT, handleStorageChange);
    }, []);

    const toggleFavorite = (pkgName: string) => {
        setFavorites(prev => {
            const newFavorites = prev.includes(pkgName)
                ? prev.filter(p => p !== pkgName)
                : [...prev, pkgName];

            localStorage.setItem('monarch_favorites', JSON.stringify(newFavorites));
            window.dispatchEvent(new Event(FAVORITES_UPDATED_EVENT));
            return newFavorites;
        });
    };

    const isFavorite = (pkgName: string) => favorites.includes(pkgName);

    return { favorites, toggleFavorite, isFavorite };
}
