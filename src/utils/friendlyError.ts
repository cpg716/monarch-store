export interface FriendlyError {
    title: string;
    description: string;
    isTechnical: boolean;
    recoveryAction?: 'unlock' | 'repair_keyring' | 'refresh_mirrors' | 'clean_cache' | 'retry' | 'manual';
    recoveryLabel?: string;
    /** Raw message for Expert / "View Log" display */
    expertMessage?: string;
}

/**
 * Classifies pacman/makepkg error output into user-friendly messages with recovery suggestions.
 * This mirrors the backend error_classifier.rs for consistent UX.
 */
export function friendlyError(raw: string): FriendlyError {
    const r = raw.toLowerCase();

    // 1. Database Lock Errors (Most common, very recoverable — auto-unlock)
    // ALPM_ERR_DB_WRITE: show friendly message; expert view shows raw [ALPM] error
    if (r.includes("unable to lock database") || r.includes("database is locked") || r.includes("db.lck")
        || r.includes("alpm_err_db_write") || r.includes("db_write")
        || (r.includes("could not remove") && r.includes("db.lck"))) {
        return {
            title: "Database Locked",
            description: "Another package manager is running or a previous operation was interrupted. Auto-unlocking…",
            isTechnical: false,
            recoveryAction: 'unlock',
            recoveryLabel: 'Unlock & Retry',
            expertMessage: raw.trim()
        };
    }

    // 2. Security / Keyring Errors (Common, auto-recoverable)
    if (r.includes("gpgme error") || r.includes("pgp signature") || r.includes("invalid or corrupted package")
        || r.includes("key could not be looked up") || r.includes("unknown public key")
        || r.includes("signature from") || r.includes("trust database")) {
        return {
            title: "Updating Security Certificates",
            description: "We are refreshing the system's security keys to ensure this package is safe. Please wait...",
            isTechnical: false,
            recoveryAction: 'repair_keyring',
            recoveryLabel: 'Retry'
        };
    }

    // 3. Package Not Found
    if (r.includes("target not found") || r.includes("no results found") || r.includes("package not found")) {
        return {
            title: "Package Not Found",
            description: "The package could not be found. It may have been renamed, removed, or is not available for your system.",
            isTechnical: false,
            recoveryAction: 'retry',
            recoveryLabel: 'Try Again'
        };
    }

    // 4. Mirror/Network Issues (sync DB or package download)
    if (r.includes("failed retrieving file") || r.includes("failed to retrieve")
        || r.includes("failed to synchronize") || r.includes("could not resolve host")
        || r.includes("connection timed out") || r.includes("error downloading") || r.includes("404")) {
        return {
            title: "Download Failed",
            description: "Could not download repository databases or packages. Check your internet connection; if it persists, try running 'pacman -Syy' in a terminal to see which mirror fails.",
            isTechnical: false,
            recoveryAction: 'refresh_mirrors',
            recoveryLabel: 'Retry Download'
        };
    }

    // 5. Disk Space Issues
    if (r.includes("no space left on device") || r.includes("not enough free disk space")) {
        return {
            title: "Disk Full",
            description: "Not enough disk space. Try clearing the package cache to free up space.",
            isTechnical: false,
            recoveryAction: 'clean_cache',
            recoveryLabel: 'Clear Cache'
        };
    }

    // 6. Dependency Conflicts
    if (r.includes("conflicting dependencies") || r.includes("breaks dependency")
        || r.includes("unresolvable package conflicts")) {
        return {
            title: "Dependency Conflict",
            description: "This package conflicts with something already installed. You may need to remove the conflicting package first.",
            isTechnical: true,
            recoveryAction: 'manual',
            recoveryLabel: 'View Details'
        };
    }

    // 7. File Conflicts
    if (r.includes("exists in filesystem") || r.includes("file conflict")) {
        return {
            title: "File Conflict",
            description: "A file on your system conflicts with this package. This sometimes happens with manually installed software.",
            isTechnical: true,
            recoveryAction: 'manual',
            recoveryLabel: 'View Details'
        };
    }

    // 8. Corrupted Package
    if (r.includes("corrupted package") || r.includes("failed integrity")) {
        return {
            title: "Corrupted Download",
            description: "A package download was corrupted. This usually fixes itself when you try again.",
            isTechnical: false,
            recoveryAction: 'retry',
            recoveryLabel: 'Retry'
        };
    }

    // 9. Permission Denied
    if (r.includes("permission denied") || r.includes("operation not permitted")) {
        return {
            title: "Permission Denied",
            description: "Administrator privileges are required for this operation.",
            isTechnical: false,
            recoveryAction: 'retry',
            recoveryLabel: 'Try Again'
        };
    }

    // 10. AUR-specific: Missing dependencies for build
    if (r.includes("missing dependencies") && r.includes("makepkg")) {
        return {
            title: "Build Dependencies Missing",
            description: "Some packages needed to build this AUR package are not installed.",
            isTechnical: true,
            recoveryAction: 'manual',
            recoveryLabel: 'View Details'
        };
    }

    // 11. AUR-specific: PGP key for source verification
    if (r.includes("pgp key") && r.includes("could not be verified")) {
        return {
            title: "Source Verification Failed",
            description: "The package source code could not be verified. The developer's PGP key may need to be imported.",
            isTechnical: true,
            recoveryAction: 'manual',
            recoveryLabel: 'View Details'
        };
    }

    // 12. System Errors
    if (r.includes("no such file or directory")) {
        return {
            title: "System Error",
            description: "A required file or directory is missing. This might require manual intervention.",
            isTechnical: true,
            recoveryAction: 'manual',
            recoveryLabel: 'View Details'
        };
    }

    // 13. makepkg "An unknown error has occurred" — toolchain, permissions, or stale build cache
    if (r.includes("unknown error has occurred") || r.includes("makepkg reported an unknown error") || r.includes("permission sanitizer")) {
        return {
            title: "AUR Build Failed (Unknown Error)",
            description: "makepkg reported an unknown error. Ensure base-devel and git are installed; run scripts/monarch-permission-sanitizer.sh to fix build cache permissions.",
            isTechnical: false,
            recoveryAction: 'manual',
            recoveryLabel: 'Run Permission Sanitizer'
        };
    }

    // 14. Tauri/backend not available or invoke/connection failures
    if ((r.includes("tauri") && (r.includes("not available") || r.includes("undefined") || r.includes("could not find")))
        || r.includes("__tauri__") || (r.includes("invoke") && r.includes("not available"))
        || r.includes("failed to fetch") || r.includes("fetch failed") || r.includes("load failed")
        || r.includes("connection refused") || r.includes("econnrefused") || r.includes("networkerror")
        || r.includes("unable to connect") || r.includes("could not connect") || r.includes("connection reset")
        || r.includes("the operation was aborted") || r.includes("network request failed")) {
        return {
            title: "App Backend Unavailable",
            description: "MonARCH must be run as a desktop app (npm run tauri dev). The backend is not connected.",
            isTechnical: false,
            recoveryAction: 'manual',
            recoveryLabel: 'Run as Desktop App'
        };
    }

    // 15. Invalid response from backend (decode/serialization failure)
    if (r.includes("error decoding response body") || r.includes("decoding response body")
        || r.includes("invalid json") || r.includes("unexpected end of") || r.includes("expected value")) {
        const rawTrim = raw.trim();
        return {
            title: "Backend Response Error",
            description: "The app received an invalid response from the backend. Try restarting the app; if it persists, a recent update may have broken compatibility. See Technical Details for the raw error.",
            isTechnical: true,
            recoveryAction: 'retry',
            recoveryLabel: 'Restart App',
            expertMessage: rawTrim
        };
    }

    // 16. Generic "unknown" / "unexpected" from backend or JS
    if (r.includes("unknown error") || r.includes("unexpected error") || r.includes("an error occurred")) {
        return {
            title: "Operation Failed",
            description: raw.trim().length > 120 ? raw.trim().slice(0, 120) + "…" : raw.trim(),
            isTechnical: true,
            recoveryAction: 'retry',
            recoveryLabel: 'Try Again',
            expertMessage: raw.trim()
        };
    }

    // Fallback: Generic/Unknown
    return {
        title: "Operation Failed",
        description: "An unexpected error occurred. Check the logs for details.",
        isTechnical: true,
        recoveryAction: 'retry',
        recoveryLabel: 'Try Again'
    };
}

/**
 * Scans an array of log lines for errors and returns the most relevant one
 */
export function findErrorInLogs(logs: string[]): FriendlyError | null {
    // Search from the end (most recent) to find the most relevant error
    for (let i = logs.length - 1; i >= 0; i--) {
        const line = logs[i];
        if (line.toLowerCase().includes('error') || line.toLowerCase().includes('failed')) {
            return friendlyError(line);
        }
    }
    return null;
}

