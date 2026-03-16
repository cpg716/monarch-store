import { useState, useEffect } from 'react';
import { useErrorService } from '../context/ErrorContext';
import { API, ChaoticStatus } from '../services/api';
import { useAppStore } from '../store/internal_store';



/** Chaotic discovery visibility is controlled by user settings; system status is reported separately. */
export function useChaoticStatus(): {
    status: ChaoticStatus | null;
    enabled: boolean;
    loading: boolean;
    refresh: () => Promise<void>;
} {
    const [status, setStatus] = useState<ChaoticStatus | null>(null);
    const [loading, setLoading] = useState(true);
    const errorService = useErrorService();
    const advancedMode = useAppStore((s) => s.advancedMode);
    const discoveryEnabled = useAppStore((s) => s.isChaoticEnabled);

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

    // Discovery surfaces follow the explicit user toggle.
    // Safety guard: if host is blocked and expert mode is off, force disabled.
    const enabled = discoveryEnabled && (status ? (status.compatible || advancedMode) : true);

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
