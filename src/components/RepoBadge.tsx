import { clsx } from 'clsx';

export type RepoSource = string;

interface RepoBadgeProps {
    repo: RepoSource;
    className?: string;
}

/**
 * Trust Signal pill: maps repository/source names to color-coded badges.
 * Official → Blue, Optimized (Chaotic/CachyOS) → Purple, AUR → Orange, Fallback → Gray.
 */
export default function RepoBadge({ repo, className }: RepoBadgeProps) {
    const s = (repo || '').toLowerCase();

    const isOfficial =
        s === 'official' || s === 'core' || s === 'extra' || s === 'multilib' || s === 'manjaro';
    const isOptimized =
        s === 'chaotic' || s === 'chaotic-aur' || s.startsWith('cachyos');
    const isCommunity = s === 'aur';

    const [label, pillClass] = isOfficial
        ? ['Official', 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200']
        : isOptimized
            ? ['Optimized', 'bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-200']
            : isCommunity
                ? ['AUR', 'bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-200']
                : [s || 'Repo', 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300'];

    return (
        <span
            className={clsx(
                'inline-flex items-center px-2.5 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider shrink-0 whitespace-nowrap',
                pillClass,
                className
            )}
            title={repo}
        >
            {label}
        </span>
    );
}
