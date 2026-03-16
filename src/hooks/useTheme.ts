import { useEffect, useState } from 'react';
import { useAppStore } from '../store/internal_store';
import { commands } from '../services/bindings';
import { unwrap } from '../utils/specta';
import { listen } from '@tauri-apps/api/event';

type ThemeMode = 'system' | 'light' | 'dark';

export function useTheme() {
    const themeMode = useAppStore(state => state.themeMode);
    const setThemeMode = useAppStore(state => state.setThemeMode);
    const accentColor = useAppStore(state => state.accentColor);
    const setAccentColor = useAppStore(state => state.setAccentColor);
    const [portalTheme, setPortalTheme] = useState<'light' | 'dark' | null>(null);
    const [portalAccent, setPortalAccent] = useState<string | null>(null);

    useEffect(() => {
        const applyTheme = () => {
            const root = window.document.documentElement;

            // Handle Theme Mode
            root.classList.remove('theme-light', 'theme-dark', 'dark');

            // Determine effective theme (resolving 'system' to actual preference)
            let effectiveTheme: 'light' | 'dark' = 'light';
            if (themeMode === 'system') {
                effectiveTheme = portalTheme
                    ? portalTheme
                    : window.matchMedia('(prefers-color-scheme: dark)').matches
                        ? 'dark'
                        : 'light';
            } else {
                effectiveTheme = themeMode;
            }

            // Apply theme classes
            root.classList.add(`theme-${effectiveTheme}`);

            // Add 'dark' class for Tailwind's dark: modifier
            if (effectiveTheme === 'dark') {
                root.classList.add('dark');
            }

            const systemAccent = portalAccent || accentColor;
            const effectiveAccent = themeMode === 'system' ? systemAccent : accentColor;
            const systemBg = effectiveTheme === 'dark' ? '#0f0f0f' : '#f8fafc';

            root.style.setProperty('--system-accent', systemAccent);
            root.style.setProperty('--system-bg', systemBg);
            root.style.setProperty('--app-accent', effectiveAccent);

            // Update selection color
            root.style.setProperty('--tw-selection-bg', `${effectiveAccent}4D`); // 30% opacity
        };

        applyTheme();

        // Add real-time listener for system theme changes if we are in 'system' mode
        if (themeMode === 'system') {
            const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
            const handleChange = () => applyTheme();
            mediaQuery.addEventListener('change', handleChange);
            return () => mediaQuery.removeEventListener('change', handleChange);
        }
    }, [themeMode, accentColor, portalTheme, portalAccent]);

    useEffect(() => {
        commands.getHostAppearance()
            .then(unwrap)
            .then((appearance) => {
                if (appearance.color_scheme === 'dark') setPortalTheme('dark');
                if (appearance.color_scheme === 'light') setPortalTheme('light');
                if (appearance.accent_color) setPortalAccent(appearance.accent_color);
            })
            .catch(() => { });

        let unlistenTheme: (() => void) | null = null;
        let unlistenAccent: (() => void) | null = null;

        listen<string>('system-theme-changed', (event) => {
            const mode = (event.payload || '').toLowerCase();
            if (mode === 'dark') setPortalTheme('dark');
            if (mode === 'light') setPortalTheme('light');
        }).then((fn) => {
            unlistenTheme = fn;
        }).catch(() => { });

        listen<string>('system-accent-changed', (event) => {
            const color = String(event.payload || '').trim();
            if (/^#[0-9a-f]{6}$/i.test(color)) {
                setPortalAccent(color);
            }
        }).then((fn) => {
            unlistenAccent = fn;
        }).catch(() => { });

        return () => {
            unlistenTheme?.();
            unlistenAccent?.();
        };
    }, []);

    const prefersDark =
        typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches;
    const resolvedTheme: 'light' | 'dark' =
        themeMode === 'system'
            ? (portalTheme ?? (prefersDark ? 'dark' : 'light'))
            : themeMode;
    const hostAccentColor = portalAccent;
    const effectiveAccentColor = themeMode === 'system' ? (portalAccent || accentColor) : accentColor;

    return {
        themeMode,
        setThemeMode,
        accentColor,
        setAccentColor,
        hostAccentColor,
        hostThemePreference: portalTheme,
        resolvedTheme,
        effectiveAccentColor,
        isFollowingSystemTheme: themeMode === 'system',
    };
}
