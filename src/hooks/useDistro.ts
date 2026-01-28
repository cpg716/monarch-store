import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface DistroCapabilities {
    repo_management: 'unlocked' | 'locked' | 'managed';
    chaotic_aur_support: 'allowed' | 'blocked' | 'native';
    default_search_sort: 'binary_first' | 'source_first';
    description: string;
    icon_key: string;
}

export type DistroId = 'arch' | 'manjaro' | 'endeavouros' | 'garuda' | 'cachyos' | string;

export interface DistroContext {
    id: DistroId;
    pretty_name: string;
    capabilities: DistroCapabilities;
}

const DEFAULT_CONTEXT: DistroContext = {
    id: 'arch',
    pretty_name: 'Arch Linux',
    capabilities: {
        repo_management: 'unlocked',
        chaotic_aur_support: 'allowed',
        default_search_sort: 'binary_first',
        description: 'Standard Arch System.',
        icon_key: 'arch'
    }
};

export function useDistro() {
    const [distro, setDistro] = useState<DistroContext>(DEFAULT_CONTEXT);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        // In a real app, we might check a cache first
        invoke<DistroContext>('get_distro_context')
            .then(ctx => {
                setDistro(ctx);
                setLoading(false);
            })
            .catch(err => {
                console.error("Failed to detect distro:", err);
                setLoading(false);
            });
    }, []);

    return { distro, loading, isManjaro: distro.id === 'manjaro' };
}
