import { useEffect } from 'react';
import { useAppStore } from '../store/internal_store';

type ThemeMode = 'system' | 'light' | 'dark';

export function useTheme() {
    const themeMode = useAppStore(state => state.themeMode);
    const setThemeMode = useAppStore(state => state.setThemeMode);
    const accentColor = useAppStore(state => state.accentColor);
    const setAccentColor = useAppStore(state => state.setAccentColor);

    useEffect(() => {
        const applyTheme = () => {
            const root = window.document.documentElement;

            // Handle Theme Mode
            root.classList.remove('theme-light', 'theme-dark', 'dark');

            // Determine effective theme (resolving 'system' to actual preference)
            let effectiveTheme: 'light' | 'dark' = 'light';
            if (themeMode === 'system') {
                effectiveTheme = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
            } else {
                effectiveTheme = themeMode;
            }

            // Apply theme classes
            root.classList.add(`theme-${effectiveTheme}`);

            // Add 'dark' class for Tailwind's dark: modifier
            if (effectiveTheme === 'dark') {
                root.classList.add('dark');
            }

            // Handle Accent Color
            root.style.setProperty('--app-accent', accentColor);

            // Update selection color
            root.style.setProperty('--tw-selection-bg', `${accentColor}4D`); // 30% opacity
        };

        applyTheme();

        // Add real-time listener for system theme changes if we are in 'system' mode
        if (themeMode === 'system') {
            const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
            const handleChange = () => applyTheme();
            mediaQuery.addEventListener('change', handleChange);
            return () => mediaQuery.removeEventListener('change', handleChange);
        }
    }, [themeMode, accentColor]);

    return {
        themeMode,
        setThemeMode,
        accentColor,
        setAccentColor
    };
}
