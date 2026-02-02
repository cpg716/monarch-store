use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlpmClassifiedError {
    pub kind: String,
    pub title: String,
    pub description: String,
    pub recovery_action: Option<String>,
    pub raw_message: String,
}

pub fn classify_alpm_error(error_msg: &str) -> AlpmClassifiedError {
    let msg_lower = error_msg.to_lowercase();

    // Database Lock
    if msg_lower.contains("database is locked")
        || msg_lower.contains("unable to lock database")
        || msg_lower.contains("db.lck")
    {
        return AlpmClassifiedError {
            kind: "DatabaseLocked".to_string(),
            title: "Database Locked".to_string(),
            description:
                "Another package manager is running or a previous operation was interrupted."
                    .to_string(),
            recovery_action: Some("UnlockDatabase".to_string()),
            raw_message: error_msg.to_string(),
        };
    }

    // Corrupt / failed to open DB (ALPM_ERR_DB_OPEN style)
    if msg_lower.contains("failed to init")
        || msg_lower.contains("could not open")
        || msg_lower.contains("failed to open")
        || msg_lower.contains("database not found")
    {
        return AlpmClassifiedError {
            kind: "DbOpen".to_string(),
            title: "Package Database Issue".to_string(),
            description: crate::self_healer::db_open_message().to_string(),
            recovery_action: Some("RemoveLockAndSync".to_string()),
            raw_message: error_msg.to_string(),
        };
    }

    // Keyring/PGP Error
    if msg_lower.contains("gpgme error")
        || msg_lower.contains("pgp signature")
        || msg_lower.contains("invalid or corrupted package")
        || msg_lower.contains("key could not be looked up")
        || msg_lower.contains("unknown public key")
        || msg_lower.contains("signature from")
    {
        return AlpmClassifiedError {
            kind: "KeyringError".to_string(),
            title: "Security Key Issue".to_string(),
            description:
                "Package signatures could not be verified. Your keyring may need to be refreshed."
                    .to_string(),
            recovery_action: Some("RepairKeyring".to_string()),
            raw_message: error_msg.to_string(),
        };
    }

    // Package Not Found
    if msg_lower.contains("target not found")
        || msg_lower.contains("no results found")
        || msg_lower.contains("package not found")
        || msg_lower.contains("not found in any enabled repository")
    {
        return AlpmClassifiedError {
            kind: "PackageNotFound".to_string(),
            title: "Package Not Found".to_string(),
            description: "The package could not be found. It may have been renamed, removed, or your repositories need syncing. Ensure the repository is enabled in /etc/pacman.conf.".to_string(),
            recovery_action: Some("ForceRefreshDb".to_string()),
            raw_message: error_msg.to_string(),
        };
    }

    // Mirror/Network Issues
    if msg_lower.contains("failed retrieving file")
        || msg_lower.contains("failed to synchronize")
        || msg_lower.contains("could not resolve host")
        || msg_lower.contains("connection timed out")
        || msg_lower.contains("error downloading")
    {
        return AlpmClassifiedError {
            kind: "MirrorFailure".to_string(),
            title: "Download Failed".to_string(),
            description: "Could not download packages from mirrors. Check your internet connection or try refreshing your mirror list.".to_string(),
            recovery_action: Some("RefreshMirrors".to_string()),
            raw_message: error_msg.to_string(),
        };
    }

    // Dependency Conflicts
    if msg_lower.contains("conflicting dependencies")
        || msg_lower.contains("breaks dependency")
        || msg_lower.contains("unresolvable package conflicts")
    {
        return AlpmClassifiedError {
            kind: "DependencyConflict".to_string(),
            title: "Dependency Conflict".to_string(),
            description: "Package dependencies conflict with installed packages. Manual intervention may be required.".to_string(),
            recovery_action: None,
            raw_message: error_msg.to_string(),
        };
    }

    // makepkg "An unknown error has occurred" — toolchain, permissions, or stale build dir
    if msg_lower.contains("unknown error has occurred")
        || msg_lower.contains("an unknown error has occurred")
    {
        return AlpmClassifiedError {
            kind: "MakepkgUnknownError".to_string(),
            title: "AUR Build Failed (Unknown Error)".to_string(),
            description: "makepkg reported an unknown error. Ensure base-devel and git are installed; fix permissions on /tmp/monarch-install and user cache; do not run makepkg as root.".to_string(),
            recovery_action: Some("RunPermissionSanitizer".to_string()),
            raw_message: error_msg.to_string(),
        };
    }

    // Generic error
    AlpmClassifiedError {
        kind: "Unknown".to_string(),
        title: "Installation Failed".to_string(),
        description: error_msg.to_string(),
        recovery_action: None,
        raw_message: error_msg.to_string(),
    }
}
