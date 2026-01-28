
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ESSENTIAL_IDS } from '../constants';

export function useSmartEssentials() {
    const [smartEssentials, setSmartEssentials] = useState<string[]>(ESSENTIAL_IDS);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        const fetchInstalled = async () => {
            try {
                // 1. Fetch Essentials Pool (Dynamic)
                const essentialsPool = await invoke<string[]>('get_essentials_list');

                // 2. Get raw list of installed packages (pacman -Qq)
                const installed = await invoke<string[]>('get_all_installed_names');
                const installedSet = new Set(installed);

                // 3. Filter out apps that are already installed
                // This implicitly handles "Hide Steam on Garuda" because Garuda has steam installed.
                const filtered = essentialsPool.filter(id => {
                    // Check direct match
                    if (installedSet.has(id)) return false;

                    // Simple heuristic: if package name sans "-bin" exists?
                    const baseName = id.replace(/-bin$/, "");
                    if (installedSet.has(baseName)) return false;

                    return true;
                });

                // Apply simple rotation if needed, or take top N?
                // For now, take top 40 effectively
                setSmartEssentials(filtered);
            } catch (err) {
                console.error("Failed to curate essentials:", err);
                // Fallback to static list on error (Logic kept in backend usually, but defensive here)
                setSmartEssentials(ESSENTIAL_IDS);
            } finally {
                setLoading(false);
            }
        };

        fetchInstalled();
    }, []);

    return { essentials: smartEssentials, loading };
}
