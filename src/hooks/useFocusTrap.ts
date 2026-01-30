import { useEffect, useRef } from 'react';

/**
 * Hook to trap focus within a modal/dialog container.
 * Prevents Tab from escaping to background content.
 * 
 * @param isActive - Whether the focus trap should be active
 * @returns Ref to attach to the container element
 * 
 * @example
 * ```tsx
 * const modalRef = useFocusTrap(isOpen);
 * return <div ref={modalRef} className="modal">...</div>;
 * ```
 */
export function useFocusTrap(isActive: boolean) {
  const containerRef = useRef<HTMLDivElement>(null);
  
  useEffect(() => {
    if (!isActive || !containerRef.current) return;
    
    const container = containerRef.current;
    
    // Find all focusable elements
    const focusable = container.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    
    if (focusable.length === 0) return;
    
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    
    // Focus the first element when trap activates
    first.focus();
    
    const handleTab = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return;
      
      // Shift+Tab: if on first, wrap to last
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } 
      // Tab: if on last, wrap to first
      else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };
    
    container.addEventListener('keydown', handleTab);
    return () => container.removeEventListener('keydown', handleTab);
  }, [isActive]);
  
  return containerRef;
}
