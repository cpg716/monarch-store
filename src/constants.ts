// Full pool of "Essentials" - Popular proprietary/chaotic apps
export const ESSENTIALS_POOL = [
    "google-chrome", "visual-studio-code-bin", "spotify", "discord", "slack-desktop", "zoom", "sublime-text-4",
    "obsidian", "telegram-desktop-bin", "brave-bin", "edge-bin", "vlc", "gimp", "steam", "minecraft-launcher",
    "teams-for-linux", "notion-app", "postman-bin", "figma-linux-bin", "anydesk-bin"
];

export const getRotatedEssentials = () => {
    const now = new Date();
    const start = new Date(now.getFullYear(), 0, 0);
    const diff = (now.getTime() - start.getTime()) + ((start.getTimezoneOffset() - now.getTimezoneOffset()) * 60 * 1000);
    const oneDay = 1000 * 60 * 60 * 24;
    const day = Math.floor(diff / oneDay);
    const week = Math.floor(day / 7);

    const poolSize = ESSENTIALS_POOL.length;
    const subsetSize = 12;
    const startIndex = (week * 3) % poolSize;

    let result: string[] = [];
    for (let i = 0; i < subsetSize; i++) {
        result.push(ESSENTIALS_POOL[(startIndex + i) % poolSize]);
    }
    return result;
};

export const ESSENTIAL_IDS = getRotatedEssentials();
