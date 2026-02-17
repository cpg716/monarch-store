import { ESSENTIAL_IDS } from '../constants';

/**
 * Returns the essentials list for the home section.
 * @param initialFromApp - When set (from App startup), use this. When null, returns the curated fallback (35 apps). App is the single source of the fetched list at startup, so the hook does not fetch and there is no duplicate request.
 */
export function useSmartEssentials(initialFromApp: string[] | null = null) {
    return {
        essentials: initialFromApp ?? ESSENTIAL_IDS,
        loading: false,
    };
}
