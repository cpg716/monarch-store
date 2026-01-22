import { useState, useEffect } from 'react';

type ThemeMode = 'system' | 'light' | 'dark';

export function useTheme() {
    const [themeMode, setThemeMode] = useState<ThemeMode>(() => {
        return (localStorage.getItem('theme-mode') as ThemeMode) || 'system';
    });

    const [accentColor, setAccentColor] = useState(() => {
        return localStorage.getItem('accent-color') || '#3b82f6';
    });

    useEffect(() => {
        localStorage.setItem('theme-mode', themeMode);
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

        // Update selection color too (optional but nice)
        root.style.setProperty('--tw-selection-bg', `${accentColor}4D`); // 30% opacity
    }, [themeMode, accentColor]);

    return {
        themeMode,
        setThemeMode,
        accentColor,
        setAccentColor
    };
}
