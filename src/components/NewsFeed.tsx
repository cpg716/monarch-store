import { useState, useEffect, useCallback, useMemo } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ExternalLink, Rss, ChevronDown, ChevronUp, ShieldAlert, Newspaper, Sparkles, Loader2 } from 'lucide-react';
import { clsx } from 'clsx';
import DOMPurify from 'dompurify';
import { openUrl } from '@tauri-apps/plugin-opener';
import { commands, NewsItem, NewsCategory } from '../services/bindings';
import { unwrap } from '../utils/specta';

import { useAppStore } from '../store/internal_store';

const MAX_READ_IDS = 500;

/** Get Read IDs from store. */
export function getReadNewsIds(): string[] {
    return useAppStore.getState().readNewsIds;
}

/** Mark the given news item IDs as read. */
export function markNewsItemsAsRead(ids: string[]): void {
    if (ids.length === 0) return;
    const store = useAppStore.getState();
    const prev = new Set(store.readNewsIds);
    ids.forEach((id) => prev.add(id));
    store.setReadNewsIds(Array.from(prev).slice(-MAX_READ_IDS));
}

interface NewsFeedProps {
    limit?: number;
    compact?: boolean;
    onItemOpen?: (item: NewsItem) => void;
}

export default function NewsFeed({ limit, compact = false, onItemOpen }: NewsFeedProps) {
    const [items, setItems] = useState<NewsItem[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const fetchNewsData = useCallback(async () => {
        setLoading(true);
        setError(null);
        try {
            const list = unwrap(await commands.fetchNews());
            setItems(list || []);
        } catch (e) {
            console.error('[MonARCH] Failed to fetch news:', e);
            setError(String(e));
            setItems([]);
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        fetchNewsData();
    }, [fetchNewsData]);

    const categorizedItems = useMemo(() => {
        const groups: Record<NewsCategory, NewsItem[]> = {
            critical: [],
            system: [],
            discovery: []
        };

        items.forEach(item => {
            groups[item.category].push(item);
        });

        return groups;
    }, [items]);

    const [expandedId, setExpandedId] = useState<string | null>(null);

    const readNewsIds = useAppStore(s => s.readNewsIds);
    const setReadNewsIds = useAppStore(s => s.setReadNewsIds);

    const markRead = useCallback((id: string) => {
        if (readNewsIds.includes(id)) return;
        const next = [...readNewsIds, id].slice(-MAX_READ_IDS);
        setReadNewsIds(next);
    }, [readNewsIds, setReadNewsIds]);

    const handleCardClick = useCallback(
        (item: NewsItem) => {
            markRead(item.id);
            onItemOpen?.(item);
            setExpandedId((prev) => (prev === item.id ? null : item.id));
        },
        [markRead, onItemOpen]
    );

    const handleOpenInBrowser = useCallback(async (link: string) => {
        if (!link) return;
        try {
            await openUrl(link);
        } catch {
            window.open(link, '_blank', 'noopener');
        }
    }, []);

    if (loading) {
        return (
            <div className="flex flex-col items-center justify-center py-20 gap-4 text-app-muted">
                <Loader2 size={32} className="text-blue-500 animate-spin" />
                <p className="text-sm font-medium">Gathering announcements...</p>
            </div>
        );
    }

    if (error) {
        return (
            <div className="py-12 text-center">
                <p className="text-red-500 font-bold mb-2">Failed to load news</p>
                <p className="text-xs text-app-muted mb-4">{error}</p>
                <button
                    onClick={fetchNewsData}
                    className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-xl text-sm font-bold transition-all"
                >
                    Try Again
                </button>
            </div>
        );
    }

    if (items.length === 0) {
        return (
            <div className="py-12 text-center text-app-muted">
                <Rss size={48} className="mx-auto mb-4 opacity-20" />
                <p className="text-lg font-bold">Quiet on the front lines</p>
                <p className="text-sm opacity-60 mt-1 text-balance">No announcements for your distro right now.</p>
            </div>
        );
    }

    const renderSection = (category: NewsCategory, title: string, icon: React.ReactNode, count: number) => {
        let sectionItems = categorizedItems[category];
        if (sectionItems.length === 0) return null;

        if (limit && limit > 0) {
            sectionItems = sectionItems.slice(0, limit);
        }

        return (
            <div className="mb-10 last:mb-0">
                <div className="flex items-center gap-3 mb-4 px-1">
                    <div className={clsx(
                        "p-2 rounded-xl",
                        category === 'critical' ? "bg-red-500/10 text-red-500" :
                            category === 'system' ? "bg-blue-500/10 text-blue-500" :
                                "bg-purple-500/10 text-purple-500"
                    )}>
                        {icon}
                    </div>
                    <div>
                        <h2 className="text-lg font-bold text-slate-900 dark:text-white leading-none">{title}</h2>
                        <p className="text-xs text-app-muted mt-1 font-medium">{limit ? `Showing top ${sectionItems.length} of ${count}` : `${count} total items`}</p>
                    </div>
                </div>
                <div className="space-y-3">
                    {sectionItems.map((item, idx) => (
                        <NewsCard
                            key={typeof item.id === 'string' || typeof item.id === 'number' ? String(item.id) : `news-${idx}`}
                            item={item}
                            idx={idx}
                            read={readNewsIds.includes(item.id)}
                            expanded={expandedId === item.id}
                            compact={compact}
                            onToggle={handleCardClick}
                            onOpenBrowser={handleOpenInBrowser}
                        />
                    ))}
                </div>
            </div>
        );
    };

    return (
        <div className="pb-12">
            {renderSection('critical', 'Critical Alerts', <ShieldAlert size={20} />, categorizedItems.critical.length)}
            {renderSection('system', 'System Announcements', <Newspaper size={20} />, categorizedItems.system.length)}
            {renderSection('discovery', 'App Discovery', <Sparkles size={20} />, categorizedItems.discovery.length)}
        </div>
    );
}

function NewsCard({
    item,
    idx,
    read,
    expanded,
    compact,
    onToggle,
    onOpenBrowser
}: {
    item: NewsItem,
    idx: number,
    read: boolean,
    expanded: boolean,
    compact: boolean,
    onToggle: (item: NewsItem) => void,
    onOpenBrowser: (link: string) => void
}) {
    const hasContent = item.content && item.content.trim().length > 0;
    const isFlathub = item.source_label.toLowerCase() === 'flathub';
    const critical = item.category === 'critical';

    return (
        <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: idx * 0.05 }}
            className={clsx(
                'group rounded-2xl border transition-all duration-300',
                expanded ? 'ring-2 ring-blue-500/50 scale-[1.01] shadow-xl shadow-blue-500/10' : 'hover:scale-[1.005] hover:shadow-md',
                critical
                    ? 'border-red-500/30 bg-red-500/5 dark:bg-red-500/10'
                    : 'border-black/5 dark:border-white/5 bg-white dark:bg-app-card/60'
            )}
        >
            <button
                type="button"
                onClick={() => onToggle(item)}
                className={clsx(
                    'w-full text-left p-4 sm:p-5',
                    expanded && 'pb-0'
                )}
            >
                <div className="flex items-start gap-4">
                    {!read && (
                        <span
                            className={clsx(
                                'shrink-0 mt-2 w-2.5 h-2.5 rounded-full',
                                critical ? 'bg-red-500 animate-pulse ring-4 ring-red-500/20' : 'bg-blue-500 ring-4 ring-blue-500/20'
                            )}
                        />
                    )}
                    <div className="flex-1 min-w-0">
                        <div className="flex flex-wrap items-center gap-2 mb-2">
                            <span
                                className={clsx(
                                    'text-[10px] font-black uppercase tracking-widest px-2 py-0.5 rounded-md',
                                    isFlathub
                                        ? 'bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-500/20'
                                        : 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20'
                                )}
                            >
                                {item.source_label}
                            </span>
                            {critical && (
                                <span className="inline-flex items-center gap-1 text-[10px] font-black uppercase tracking-widest text-red-600 dark:text-red-400 bg-red-500/10 px-2 py-0.5 rounded-md border border-red-500/20">
                                    <ShieldAlert size={10} />
                                    Manual Intervention
                                </span>
                            )}
                        </div>
                        <h3
                            className={clsx(
                                'text-slate-900 dark:text-white leading-tight',
                                compact ? 'text-sm' : 'text-lg',
                                !read ? 'font-black' : 'font-semibold'
                            )}
                        >
                            {item.title}
                        </h3>
                        {item.pub_date && (
                            <p className="text-[11px] font-medium text-app-muted mt-2 uppercase tracking-wide opacity-50">{item.pub_date}</p>
                        )}
                    </div>
                    <div className="shrink-0 text-slate-400 dark:text-white/30 pt-1 group-hover:text-blue-500 transition-colors">
                        {expanded ? <ChevronUp size={20} /> : <ChevronDown size={20} />}
                    </div>
                </div>
            </button>
            <AnimatePresence>
                {expanded && (
                    <motion.div
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: 'auto', opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        className="overflow-hidden"
                    >
                        <div className="p-5 pt-4">
                            <div className="h-px bg-black/5 dark:bg-white/5 mb-5" />
                            {hasContent ? (
                                <div
                                    className="prose prose-sm dark:prose-invert max-w-none text-slate-700 dark:text-slate-300 text-sm leading-relaxed prose-p:my-3 prose-strong:text-slate-900 dark:prose-strong:text-white prose-a:text-blue-500 hover:prose-a:underline"
                                    dangerouslySetInnerHTML={{
                                        __html: DOMPurify.sanitize(item.content!, {
                                            ALLOWED_TAGS: ['p', 'br', 'ul', 'ol', 'li', 'strong', 'em', 'a', 'img', 'h2', 'h3', 'h4', 'code', 'pre'],
                                            ALLOWED_ATTR: ['href', 'src', 'alt']
                                        }),
                                    }}
                                />
                            ) : (
                                <p className="text-app-muted text-sm italic">No extended content available for this announcement.</p>
                            )}
                            {item.link && (
                                <button
                                    type="button"
                                    onClick={() => onOpenBrowser(item.link)}
                                    className="mt-6 inline-flex items-center gap-2 text-sm font-bold text-blue-600 dark:text-blue-400 hover:underline px-4 py-2 bg-blue-500/5 dark:bg-blue-500/10 rounded-xl transition-all"
                                >
                                    <ExternalLink size={14} />
                                    Read Full Article
                                </button>
                            )}
                        </div>
                    </motion.div>
                )}
            </AnimatePresence>
        </motion.div>
    );
}

