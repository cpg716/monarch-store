export const getRepoColor = (labelRaw: string): string => {
    const label = labelRaw.toLowerCase();

    // SteamOS / Chimera (Gaming Vibe)
    if (label.includes('steamos') || label.includes('chimeraos') || label.includes('gameros') || label.includes('jupiter') || label.includes('holo')) {
        return 'bg-indigo-600 border-indigo-500/50 text-white shadow-indigo-500/20';
    }

    // Chaotic / Garuda (Performance/Gaming)
    if (label.includes('chaotic') || label.includes('garuda') || label.includes('dragonized')) {
        return 'bg-purple-600 border-purple-500/50 text-white shadow-purple-500/20';
    }

    // CachyOS (Green Optimization)
    if (label.includes('cachyos')) {
        return 'bg-green-600 border-green-500/50 text-white shadow-green-500/20';
    }

    // EndeavourOS (Violet)
    if (label.includes('endeavour')) {
        return 'bg-violet-600 border-violet-500/50 text-white shadow-violet-500/20';
    }

    // Manjaro / Mabox (Teal)
    if (label.includes('manjaro') || label.includes('mabox')) {
        return 'bg-teal-600 border-teal-500/50 text-white shadow-teal-500/20';
    }

    // Arch Official (Classic Blue)
    if (label.includes('arch') || label.includes('official') || label === 'core' || label === 'extra' || label === 'multilib') {
        return 'bg-blue-600 border-blue-500/50 text-white shadow-blue-500/20';
    }

    // AUR (Orange Community)
    if (label.includes('aur')) {
        return 'bg-orange-500 border-orange-400/50 text-white shadow-orange-500/20';
    }

    // Flatpak (Slate/Sandboxed)
    if (label.includes('flatpak')) {
        return 'bg-slate-500 border-slate-400/50 text-white shadow-slate-500/20';
    }

    // Specialized / Security (Black/Gray)
    if (label.includes('blackarch') || label.includes('parabola') || label.includes('hyperbola') || label.includes('security')) {
        return 'bg-gray-800 border-gray-600/50 text-white shadow-black/40';
    }

    // Fallback
    return 'bg-gray-500 border-gray-400/50 text-white';
};
