import { Component, ErrorInfo, ReactNode } from "react";

interface Props {
    children: ReactNode;
}

interface State {
    hasError: boolean;
    error: Error | null;
    showDetails: boolean;
}

// Note: ErrorBoundary cannot use hooks, so we'll use a workaround for ErrorService
// We'll create a wrapper component that uses ErrorService
class ErrorBoundary extends Component<Props, State> {
    public state: State = {
        hasError: false,
        error: null,
        showDetails: false,
    };

    public static getDerivedStateFromError(error: Error): Partial<State> {
        return { hasError: true, error };
    }

    public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
        console.error("Uncaught error:", error, errorInfo);
        // Try to report to ErrorService if available (via window for class components)
        if (typeof window !== 'undefined' && (window as any).__errorService) {
            (window as any).__errorService.reportCritical(error);
        }
    }

    public render() {
        if (this.state.hasError) {
            const { error, showDetails } = this.state;
            return (
                <div className="h-screen w-screen flex flex-col items-center justify-center bg-[#0f0f0f] text-white p-8 text-center">
                    <div className="w-16 h-16 bg-red-500/20 text-red-500 rounded-full flex items-center justify-center mb-6">
                        <span className="text-3xl">⚠️</span>
                    </div>
                    <h1 className="text-2xl font-bold mb-4">Something went wrong</h1>
                    <p className="text-app-muted mb-6 max-w-md">
                        A critical error occurred. Refreshing the app usually fixes this. If it keeps happening, try restarting MonARCH Store.
                    </p>
                    {showDetails && error && (
                        <div className="bg-black/50 p-4 rounded-lg text-left text-xs font-mono w-full max-w-xl overflow-auto max-h-48 border border-white/10 mb-6">
                            <p className="text-red-400 font-bold mb-2">{error.name}: {error.message}</p>
                            <p className="text-white/40 whitespace-pre">{error.stack}</p>
                        </div>
                    )}
                    <div className="flex flex-wrap items-center justify-center gap-3">
                        <button
                            onClick={() => window.location.reload()}
                            className="px-6 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg transition-colors font-bold"
                        >
                            Refresh App
                        </button>
                        <button
                            onClick={() => this.setState((s) => ({ showDetails: !s.showDetails }))}
                            className="px-4 py-2 bg-white/10 hover:bg-white/20 text-white rounded-lg text-sm transition-colors"
                        >
                            {showDetails ? "Hide details" : "Show details"}
                        </button>
                    </div>
                </div>
            );
        }

        return this.props.children;
    }
}

export default ErrorBoundary;
