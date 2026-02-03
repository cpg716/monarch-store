import { useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Minus, Square, X, Copy } from 'lucide-react';
import { clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: (string | undefined | null | false)[]) {
    return twMerge(clsx(inputs));
}

export default function TitleBar() {
    const [isMaximized, setIsMaximized] = useState(false);
    const appWindow = getCurrentWindow();

    useEffect(() => {
        const updateState = async () => {
            setIsMaximized(await appWindow.isMaximized());
        };
        updateState();

        // Listen for resize events to update maximized state icon
        const unlisten = appWindow.onResized(() => {
            updateState();
        });

        return () => {
            unlisten.then(f => f());
        }
    }, []);

    const minimize = () => appWindow.minimize();
    const toggleMaximize = async () => {
        const max = await appWindow.isMaximized();
        await appWindow.toggleMaximize();
        setIsMaximized(!max);
    };
    const close = () => appWindow.close();

    return (
        <div
            className="h-10 bg-app-bg/95 backdrop-blur-md flex items-center justify-between px-4 select-none border-b border-white/5 fixed top-0 left-0 right-0 z-[100]"
        >
            {/* Left: Branding */}
            <div className="flex items-center gap-3 pointer-events-none" data-tauri-drag-region>
                {/* Placeholder for App Icon - assuming one exists or using a generic one */}
                <div className="w-5 h-5 bg-gradient-to-br from-blue-500 to-purple-600 rounded-md shadow-inner flex items-center justify-center">
                    <span className="font-bold text-[10px] text-white">M</span>
                </div>
                <span className="text-xs font-semibold tracking-wide opacity-80">MonARCH Store</span>
            </div>

            {/* Center: Drag Region - Flex 1 fills remaining space */}
            <div className="flex-1 h-full" data-tauri-drag-region />

            {/* Right: Window Controls - Explicit Z-index to float above drag layer */}
            <div className="flex items-center gap-1.5 relative z-50">
                <TitleBarButton onClick={minimize} aria-label="Minimize">
                    <Minus size={14} strokeWidth={3} />
                </TitleBarButton>
                <TitleBarButton onClick={toggleMaximize} aria-label="Maximize">
                    {isMaximized ? (
                        <Copy size={12} strokeWidth={3} className="rotate-180" />
                    ) : (
                        <Square size={12} strokeWidth={3} />
                    )}
                </TitleBarButton>
                <TitleBarButton onClick={close} variant="danger" aria-label="Close">
                    <X size={14} strokeWidth={3} />
                </TitleBarButton>
            </div>
        </div>
    );
}

function TitleBarButton({
    children,
    onClick,
    variant = 'default',
    ...props
}: React.ComponentProps<'button'> & { variant?: 'default' | 'danger' }) {
    return (
        <button
            type="button"
            onClick={onClick}
            className={cn(
                "w-7 h-7 flex items-center justify-center rounded-lg transition-all duration-200 focus:outline-none focus-visible:ring-2 focus-visible:ring-white/60 focus-visible:ring-offset-2 focus-visible:ring-offset-transparent",
                variant === 'default'
                    ? "hover:bg-white/10 active:bg-white/20 text-white/70 hover:text-white"
                    : "hover:bg-red-500 active:bg-red-600 text-white/70 hover:text-white"
            )}
            {...props}
        >
            {children}
        </button>
    );
}
