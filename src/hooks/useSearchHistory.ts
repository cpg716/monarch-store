import { useCallback } from 'react';
import { useAppStore } from '../store/internal_store';

const MAX_HISTORY = 10;

export function useSearchHistory() {
    const history = useAppStore(s => s.searchHistory);
    const setHistory = useAppStore(s => s.setSearchHistory);

    const addSearch = useCallback((query: string) => {
        if (!query || query.trim().length === 0) return;

        const filtered = history.filter(q => q !== query);
        const next = [query, ...filtered].slice(0, MAX_HISTORY);
        setHistory(next);
    }, [history, setHistory]);

    const removeSearch = useCallback((query: string) => {
        const next = history.filter(q => q !== query);
        setHistory(next);
    }, [history, setHistory]);

    const clearHistory = useCallback(() => {
        setHistory([]);
    }, [setHistory]);

    return {
        history,
        addSearch,
        removeSearch,
        clearHistory
    };
}
