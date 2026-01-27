export interface FriendlyError {
    title: string;
    description: string;
    isTechnical: boolean;
}

export function friendlyError(raw: string): FriendlyError {
    const r = raw.toLowerCase();

    // 1. Connection / Retrieval Errors
    if (r.includes("target not found") || r.includes("failed retrieving file") || r.includes("404 not found")) {
        return {
            title: "Download Failed",
            description: "We couldn't reach the server or the file is missing. The repository might be syncing.",
            isTechnical: false
        };
    }

    // 2. Security / Keyring Errors
    if (r.includes("signature is unknown trust") || r.includes("invalid or corrupted package") || r.includes("gpgme error") || r.includes("pgp signature")) {
        return {
            title: "Security Verification Failed",
            description: "The package signature is invalid. This usually means your local keys are outdated. The app will attempt to auto-repair this.",
            isTechnical: false
        };
    }

    // 3. Conflict Errors
    if (r.includes("failed to commit transaction") || r.includes("exists in filesystem")) {
        return {
            title: "File Conflict",
            description: "This app or one of its files is already installed and conflicts with the new version.",
            isTechnical: true
        };
    }

    if (r.includes("conflicting dependencies")) {
        return {
            title: "Dependency Conflict",
            description: "This app requires a different version of a library than what you have installed.",
            isTechnical: true
        };
    }

    // 4. Lock Errors
    if (r.includes("unable to lock database") || r.includes("pacman.db.lck")) {
        return {
            title: "Installer Busy",
            description: "Another installation or update is currently running. Please wait for it to finish.",
            isTechnical: false
        };
    }

    // 5. System Errors
    if (r.includes("no such file or directory")) {
        return {
            title: "System Error",
            description: "A required system file is missing. This might require manual intervention.",
            isTechnical: true
        };
    }

    // 6. Generic/Unknown (The Switch to Transparency)
    return {
        title: "Installation Failed",
        description: "An unexpected error occurred. See details below.",
        isTechnical: true
    };
}

