/**
 * Skeleton card for loading states: pulsing blocks (icon, title, description).
 * Use in grids instead of spinners for a more polished, instant feel.
 */
export default function SkeletonCard() {
    return (
        <div className="rounded-xl border border-app-border bg-app-card dark:bg-black/20 p-6 flex flex-col h-full min-h-[200px] animate-pulse">
            <div className="flex justify-between items-start mb-4 gap-4">
                <div className="flex items-center gap-4 min-w-0 flex-1">
                    <div className="w-14 h-14 rounded-xl bg-gray-200 dark:bg-gray-700 shrink-0" />
                    <div className="flex-1 min-w-0 space-y-2">
                        <div className="h-4 w-[70%] rounded bg-gray-200 dark:bg-gray-700" />
                        <div className="h-3 w-[50%] rounded bg-gray-200 dark:bg-gray-700" />
                    </div>
                </div>
            </div>
            <div className="h-10 w-full rounded bg-gray-200 dark:bg-gray-700 mb-6" />
            <div className="mt-auto flex items-center gap-2">
                <div className="h-6 w-16 rounded-full bg-gray-200 dark:bg-gray-700" />
                <div className="h-6 w-12 rounded bg-gray-200 dark:bg-gray-700" />
            </div>
        </div>
    );
}
