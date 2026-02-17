import { useState, useEffect } from 'react';
import { useErrorService } from '../context/ErrorContext';
import { API, ChaoticStatus } from '../services/api';



/** Chaotic-AUR is "enabled" only when host is compatible and repo is in ALPM syncdbs. */
export function useChaoticStatus(): {
    status: ChaoticStatus | null;
    enabled: boolean;
    loading: boolean;
    refresh: () => Promise<void>;
} {
    const [status, setStatus] = useState<ChaoticStatus | null>(null);
    const [loading, setLoading] = useState(true);
    const errorService = useErrorService();

    const fetchStatus = async () => {
        try {
            const s = await API.system.checkChaoticStatus();
            setStatus(s);
            return s;
        } catch (e) {
            errorService.reportError(e as Error | string);
            return null;
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        fetchStatus();
    }, []);

    const enabled = status ? status.compatible && status.chaotic_in_alpm : false;

    const refresh = async () => {
        await fetchStatus();
    };

    return { status, enabled, loading, refresh };
}

/** True if the package's only source(s) are Chaotic-AUR (no flatpak/AUR/other repo). */
export function isOnlyChaoticSource(pkg: { source: { id?: string } | string; alternatives?: { source: { id?: string } | string }[] | null }): boolean {
    const variants = [pkg, ...(pkg.alternatives ?? [])];
    const allChaotic = variants.every(
        (v) => typeof v.source === 'string' ? v.source === 'chaotic' : v.source?.id === 'chaotic-aur'
    );
    return allChaotic && variants.length > 0;
}
