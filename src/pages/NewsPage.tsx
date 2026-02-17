import NewsFeed from '../components/NewsFeed';

export default function NewsPage() {
    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            <div className="p-8 pb-6 border-b border-black/5 dark:border-white/5 bg-app-bg/95 backdrop-blur-3xl z-10 transition-colors shadow-sm dark:shadow-2xl dark:shadow-black/20 sticky top-0">
                <h1 className="text-4xl lg:text-5xl font-black flex items-center gap-4 text-slate-900 dark:text-white tracking-tight leading-none mb-2">
                    News &amp; Announcements
                </h1>
                <p className="text-lg text-slate-500 dark:text-app-muted font-medium ml-1">
                    Distro and Flathub news. Read critical items before updating.
                </p>
            </div>
            <div className="flex-1 overflow-y-auto p-8 custom-scrollbar">
                <div className="max-w-2xl mx-auto">
                    <NewsFeed />
                </div>
            </div>
        </div>
    );
}
