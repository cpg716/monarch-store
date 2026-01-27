import { useState, useEffect, useCallback } from 'react';
import { LazyStore } from '@tauri-apps/plugin-store';

const STORE_PATH = 'favorites.json';
const STORAGE_KEY = 'favorites';

// Create a singleton store instance
const store = new LazyStore(STORE_PATH);

export function useFavorites() {
    const [favorites, setFavorites] = useState<string[]>([]);

    // Load initial data and subscribe to changes
    useEffect(() => {
        let isMounted = true;

        const syncStore = async () => {
            try {
                const saved = await store.get<string[]>(STORAGE_KEY);
                if (isMounted) {
                    setFavorites(saved || []);
                }
            } catch (e) {
                console.error("Failed to load favorites from store", e);
            }
        };

        syncStore();

        // Listen for changes from other windows/parts of the app
        let unlisten: (() => void) | undefined;

        store.onKeyChange<string[]>(STORAGE_KEY, (value) => {
            if (isMounted) {
                setFavorites(value || []);
            }
        }).then(u => unlisten = u);

        return () => {
            isMounted = false;
            if (unlisten) unlisten();
        };
    }, []);

    const toggleFavorite = useCallback(async (pkgName: string) => {
        try {
            const current = await store.get<string[]>(STORAGE_KEY) || [];
            const newFavorites = current.includes(pkgName)
                ? current.filter(p => p !== pkgName)
                : [...current, pkgName];

            await store.set(STORAGE_KEY, newFavorites);
            await store.save(); // Ensure persistence to disk
            setFavorites(newFavorites);
        } catch (e) {
            console.error("Failed to toggle favorite", e);
        }
    }, []);

    const isFavorite = useCallback((pkgName: string) => favorites.includes(pkgName), [favorites]);

    return { favorites, toggleFavorite, isFavorite };
}
