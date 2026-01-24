export function friendlyError(raw: string): string {
    const r = raw.toLowerCase();

    if (r.includes("target not found")) {
        return "We couldn't find this file on the server. Refreshing catalog...";
    }
    if (r.includes("signature is unknown trust") || r.includes("invalid or corrupted package")) {
        return "Security Update Required. Verifying keys... (Auto-repairing)";
    }
    if (r.includes("failed to commit transaction")) {
        return "Installation conflicted with another file. Please try again.";
    }
    if (r.includes("conflicting dependencies")) {
        return "This app conflicts with an existing package on your system.";
    }
    if (r.includes("failed to retrieve some files")) {
        return "Download failed. Please check your internet connection and try again.";
    }
    if (r.includes("unable to lock database") || r.includes("pacman.db.lck")) {
        return "Another installer is currently running. Please wait.";
    }
    if (r.includes("no such file or directory")) {
        return "System file missing. Trying to auto-repair...";
    }

    return raw; // Fallback to raw error if no match
}
