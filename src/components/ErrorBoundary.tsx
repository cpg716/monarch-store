import { Component, ErrorInfo, ReactNode } from "react";

interface Props {
    children: ReactNode;
}

interface State {
    hasError: boolean;
    error: Error | null;
}

class ErrorBoundary extends Component<Props, State> {
    public state: State = {
        hasError: false,
        error: null,
    };

    public static getDerivedStateFromError(error: Error): State {
        return { hasError: true, error };
    }

    public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
        console.error("Uncaught error:", error, errorInfo);
    }

    public render() {
        if (this.state.hasError) {
            return (
                <div className="h-screen w-screen flex flex-col items-center justify-center bg-[#0f0f0f] text-white p-8 text-center">
                    <div className="w-16 h-16 bg-red-500/20 text-red-500 rounded-full flex items-center justify-center mb-6">
                        <span className="text-3xl">⚠️</span>
                    </div>
                    <h1 className="text-2xl font-bold mb-4">Something went wrong</h1>
                    <p className="text-app-muted mb-6 max-w-md">
                        A critical error occurred while rendering the interface.
                    </p>
                    <div className="bg-black/50 p-4 rounded-lg text-left text-xs font-mono w-full max-w-xl overflow-auto max-h-48 border border-white/10 mb-8">
                        <p className="text-red-400 font-bold mb-2">{this.state.error?.name}: {this.state.error?.message}</p>
                        <p className="text-white/40 whitespace-pre">{this.state.error?.stack}</p>
                    </div>
                    <button
                        onClick={() => window.location.reload()}
                        className="px-6 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg transition-colors font-bold"
                    >
                        Refresh App
                    </button>
                </div>
            );
        }

        return this.props.children;
    }
}

export default ErrorBoundary;
