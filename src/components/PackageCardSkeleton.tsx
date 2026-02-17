

const PackageCardSkeleton = () => {
    return (
        <div className="bg-app-card/40 dark:bg-black/10 border border-app-border rounded-2xl p-5 h-[180px] min-w-[260px] max-w-full flex flex-col gap-3 animate-pulse card-gpu">
            {/* Header: Icon + Title + Badge */}
            <div className="flex items-start gap-3">
                <div className="w-12 h-12 rounded-xl bg-slate-200/50 dark:bg-white/5 shrink-0" />
                <div className="flex-1 min-w-0 space-y-2 py-0.5">
                    <div className="h-4 bg-slate-200/50 dark:bg-white/5 rounded w-3/4" />
                    <div className="h-3 bg-slate-200/50 dark:bg-white/5 rounded w-1/3" />
                </div>
                <div className="h-5 w-10 bg-slate-200/50 dark:bg-white/5 rounded-lg shrink-0" />
            </div>

            {/* Description Skeleton (2 lines) */}
            <div className="space-y-2 mb-auto">
                <div className="h-3 bg-slate-200/50 dark:bg-white/5 rounded w-full" />
                <div className="h-3 bg-slate-200/50 dark:bg-white/5 rounded w-[90%]" />
            </div>

            {/* Footer Skeleton */}
            <div className="pt-3 border-t border-app-border/50 flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <div className="h-5 w-12 bg-slate-200/50 dark:bg-white/5 rounded-lg" />
                    <div className="h-5 w-16 bg-slate-200/50 dark:bg-white/5 rounded-full" />
                </div>
                <div className="flex items-center gap-1.5">
                    <div className="w-8 h-8 bg-slate-200/50 dark:bg-white/5 rounded-xl" />
                    <div className="w-8 h-8 bg-slate-200/50 dark:bg-white/5 rounded-xl" />
                </div>
            </div>
        </div>
    );
};

export default PackageCardSkeleton;
