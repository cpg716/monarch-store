import { useState, useEffect } from 'react';

const MAX_HISTORY = 10;
const STORAGE_KEY = 'monarch_search_history';

export function useSearchHistory() {
    const [history, setHistory] = useState<string[]>([]);

    // Initial Load
    useEffect(() => {
        const saved = localStorage.getItem(STORAGE_KEY);
        if (saved) {
            try {
                setHistory(JSON.parse(saved));
            } catch (e) {
                console.error("Failed to parse search history", e);
            }
        }
    }, []);

    const addSearch = (query: string) => {
        if (!query || query.trim().length === 0) return;

        setHistory(prev => {
            const filtered = prev.filter(q => q !== query);
            const next = [query, ...filtered].slice(0, MAX_HISTORY);
            localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
            return next;
        });
    };

    const removeSearch = (query: string) => {
        setHistory(prev => {
            const next = prev.filter(q => q !== query);
            localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
            return next;
        });
    };

    const clearHistory = () => {
        setHistory([]);
        localStorage.removeItem(STORAGE_KEY);
    };

    return {
        history,
        addSearch,
        removeSearch,
        clearHistory
    };
}
