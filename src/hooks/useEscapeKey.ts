import { useEffect } from 'react';

/**
 * Hook to handle Escape key press for closing modals/dialogs.
 * 
 * @param onEscape - Callback to execute when Escape is pressed
 * @param isActive - Whether the listener should be active (default: true)
 * 
 * @example
 * ```tsx
 * useEscapeKey(onClose, isOpen);
 * ```
 */
export function useEscapeKey(onEscape: () => void, isActive: boolean = true) {
  useEffect(() => {
    if (!isActive) return;
    
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onEscape();
      }
    };
    
    window.addEventListener('keydown', handleEscape);
    return () => window.removeEventListener('keydown', handleEscape);
  }, [onEscape, isActive]);
}
