import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import ErrorBoundary from "./components/ErrorBoundary";
import LoadingScreen from "./components/LoadingScreen";

import { ToastProvider } from './context/ToastContext';
import { ErrorProvider } from './context/ErrorContext';
import { getErrorService } from './context/getErrorService';
import { SessionPasswordProvider } from './context/SessionPasswordContext';

// Global uncaught errors: report to ErrorService when available (after ErrorProvider mounts)
if (typeof window !== 'undefined') {
  window.onerror = (message, _source, _lineno, _colno, error) => {
    const msg = error?.message ?? String(message);
    getErrorService()?.reportCritical(msg);
    return false;
  };
}

// Screenshot mode: show only loading screen for README capture (no Tauri required)
const isScreenshotLoading =
  typeof window !== "undefined" &&
  new URLSearchParams(window.location.search).get("screenshot") === "loading";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <ToastProvider>
        <ErrorProvider>
          <SessionPasswordProvider>
            {isScreenshotLoading ? <LoadingScreen /> : <App />}
          </SessionPasswordProvider>
        </ErrorProvider>
      </ToastProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);
