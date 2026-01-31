import { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getErrorService } from './getErrorService';

interface RepoStatusContextType {
    repos: { [key: string]: boolean };
    refreshRepos: () => Promise<void>;
    checkRepo: (name: string) => Promise<boolean>;
}

const RepoStatusContext = createContext<RepoStatusContextType | undefined>(undefined);

export function RepoStatusProvider({ children }: { children: ReactNode }) {
    const [repos, setRepos] = useState<{ [key: string]: boolean }>({});

    const checkRepo = async (name: string) => {
        try {
            const status = await invoke<boolean>('check_repo_status', { name });
            setRepos(prev => ({ ...prev, [name]: status }));
            return status;
        } catch (e) {
            getErrorService()?.reportError(e as Error | string);
            return false;
        }
    };

    const refreshRepos = async () => {
        await Promise.all([
            checkRepo('chaotic-aur'),
            checkRepo('cachyos'),
            checkRepo('garuda'),
            checkRepo('endeavouros'),
            checkRepo('manjaro')
        ]);
    };

    useEffect(() => {
        refreshRepos();
    }, []);

    return (
        <RepoStatusContext.Provider value={{ repos, refreshRepos, checkRepo }}>
            {children}
        </RepoStatusContext.Provider>
    );
}

export function useRepoStatus() {
    const context = useContext(RepoStatusContext);
    if (!context) throw new Error("useRepoStatus must be used within RepoStatusProvider");
    return context;
}
