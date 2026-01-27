

const PackageCardSkeleton = () => {
    return (
        <div className="bg-app-card/40 border border-app-border rounded-3xl p-6 h-full flex flex-col gap-4 animate-pulse">
            {/* Header: Icon + Title */}
            <div className="flex items-start gap-3">
                {/* Icon Skeleton */}
                <div className="w-10 h-10 rounded-lg bg-app-subtle shrink-0" />

                {/* Content Skeleton */}
                <div className="flex-1 min-w-0 space-y-2 py-1">
                    <div className="h-4 bg-app-subtle rounded w-3/4" />
                    <div className="h-3 bg-app-subtle rounded w-1/4" />
                </div>
            </div>

            {/* Description Skeleton (2 lines) */}
            <div className="space-y-2">
                <div className="h-3 bg-app-subtle rounded w-full" />
                <div className="h-3 bg-app-subtle rounded w-2/3" />
            </div>

            {/* Footer Skeleton */}
            <div className="mt-auto pt-2 flex items-center justify-between">
                {/* Chip */}
                <div className="h-5 w-16 bg-app-subtle rounded-full" />
                {/* Download Button */}
                <div className="w-8 h-8 bg-app-subtle rounded-lg" />
            </div>
        </div>
    );
};

export default PackageCardSkeleton;
