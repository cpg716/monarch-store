import { useState, useEffect } from 'react';
import { commands, DistroContext } from '../services/bindings';

const DEFAULT_CONTEXT: DistroContext = {
    id: 'arch',
    pretty_name: 'Arch Linux',
    capabilities: {
        repo_management: 'unlocked',
        chaotic_aur_support: 'allowed',
        default_search_sort: 'binary_first',
        description: 'Standard Arch System.',
        icon_key: 'arch'
    },
    cpu_tier: 'v1',
    active_repos: ['core', 'extra']
};

export function useDistro() {
    const [distro, setDistro] = useState<DistroContext>(DEFAULT_CONTEXT);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        // In a real app, we might check a cache first
        commands.getDistroContext()
            .then(ctx => {
                setDistro(ctx);
                setLoading(false);
            })
            .catch(() => {
                setLoading(false);
            });
    }, []);

    return { distro, loading, isManjaro: distro.id === 'manjaro' };
}
