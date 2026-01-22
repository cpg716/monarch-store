import { useRef, useCallback } from 'react';

export function useInfiniteScroll(
    callback: () => void,
    hasMore: boolean,
    loading: boolean
) {
    const observer = useRef<IntersectionObserver | null>(null);

    const lastElementRef = useCallback(
        (node: HTMLElement | null) => {
            if (loading) return;
            if (observer.current) observer.current.disconnect();

            observer.current = new IntersectionObserver((entries) => {
                if (entries[0].isIntersecting && hasMore) {
                    callback();
                }
            });

            if (node) observer.current.observe(node);
        },
        [loading, hasMore, callback]
    );

    return lastElementRef;
}
