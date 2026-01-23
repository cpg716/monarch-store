// Global error boundary - MUST BE FIRST
window.onerror = (message, source, lineno, colno, error) => {
  console.error("GLOBAL ERROR:", { message, source, lineno, colno, error });
  return false;
};
console.log("main.tsx: Starting execution...");

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import ErrorBoundary from "./components/ErrorBoundary";

import { ToastProvider } from './context/ToastContext';
import { RepoStatusProvider } from './context/RepoStatusContext';

console.log("main.tsx: Mounting App...");
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <RepoStatusProvider>
        <ToastProvider>
          <App />
        </ToastProvider>
      </RepoStatusProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);
