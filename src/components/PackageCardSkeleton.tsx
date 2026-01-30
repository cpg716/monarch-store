

const PackageCardSkeleton = () => {
    return (
        <div className="bg-app-card/40 border border-app-border rounded-3xl p-6 h-full flex flex-col gap-4 animate-pulse card-gpu">
            {/* Header: Icon + Title */}
            <div className="flex items-start gap-3">
                <div className="w-14 h-14 rounded-2xl skeleton-shimmer shrink-0" />
                <div className="flex-1 min-w-0 space-y-2 py-1">
                    <div className="h-4 skeleton-shimmer rounded w-3/4" />
                    <div className="h-3 skeleton-shimmer rounded w-1/4" />
                </div>
            </div>

            {/* Description Skeleton (2 lines) */}
            <div className="space-y-2">
                <div className="h-3 skeleton-shimmer rounded w-full" />
                <div className="h-3 skeleton-shimmer rounded w-2/3" />
            </div>

            {/* Footer Skeleton */}
            <div className="mt-auto pt-2 flex items-center justify-between">
                <div className="h-5 w-16 skeleton-shimmer rounded-full" />
                <div className="w-8 h-8 skeleton-shimmer rounded-lg" />
            </div>
        </div>
    );
};

export default PackageCardSkeleton;
