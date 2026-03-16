import NewsFeed from '../components/NewsFeed';

export default function NewsPage() {
    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            <div className="sticky top-0 z-10 border-b border-black/5 bg-app-bg/95 p-6 pb-4 backdrop-blur-3xl transition-colors dark:border-white/5">
                <h1 className="mb-2 text-2xl lg:text-3xl font-black tracking-tight text-slate-900 dark:text-white">
                    System Advisories
                </h1>
                <p className="text-sm font-medium text-slate-500 dark:text-app-muted">
                    Review critical distro and app-source announcements before updating.
                </p>
            </div>
            <div className="flex-1 overflow-y-auto p-6 custom-scrollbar">
                <div className="mx-auto max-w-3xl">
                    <NewsFeed />
                </div>
            </div>
        </div>
    );
}
