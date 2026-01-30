/// Error classification for pacman operations.
/// Provides structured error types for the UI to display appropriate recovery actions.
use serde::{Deserialize, Serialize};

/// Classified error types that the UI can act upon
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PacmanErrorKind {
    /// Database is locked by another process
    DatabaseLocked,
    /// PGP signature verification failed
    KeyringError,
    /// Package not found in any repository
    PackageNotFound,
    /// Mirror/network issues
    MirrorFailure,
    /// Disk space insufficient
    DiskFull,
    /// Dependency conflict
    DependencyConflict,
    /// File conflict with existing package
    FileConflict,
    /// Corrupted package download
    CorruptedPackage,
    /// Permission denied
    PermissionDenied,
    /// Generic/unknown error
    Unknown,
}

/// Structured error with classification and suggested action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedError {
    pub kind: PacmanErrorKind,
    pub title: String,
    pub description: String,
    pub recovery_action: Option<RecoveryAction>,
    pub raw_message: String,
}

/// Actions that can be taken to recover from an error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    /// Remove the database lock file
    UnlockDatabase,
    /// Reset and repopulate the keyring
    RepairKeyring,
    /// Refresh mirrors with reflector or manual intervention
    RefreshMirrors,
    /// Force refresh sync databases
    ForceRefreshDb,
    /// Free up disk space
    CleanCache,
    /// Retry the operation
    Retry,
    /// Show manual resolution steps
    ShowManualSteps(String),
}

impl ClassifiedError {
    /// Analyze pacman output and classify the error
    pub fn from_output(output: &str) -> Option<Self> {
        let output_lower = output.to_lowercase();

        // Database Lock Detection
        if output_lower.contains("database is locked")
            || output_lower.contains("unable to lock database")
            || output_lower.contains("db.lck")
        {
            return Some(Self {
                kind: PacmanErrorKind::DatabaseLocked,
                title: "Database Locked".to_string(),
                description:
                    "Another package manager is running or a previous operation was interrupted."
                        .to_string(),
                recovery_action: Some(RecoveryAction::UnlockDatabase),
                raw_message: output.to_string(),
            });
        }

        // Keyring/PGP Error Detection
        if output_lower.contains("gpgme error")
            || output_lower.contains("pgp signature")
            || output_lower.contains("invalid or corrupted package")
            || output_lower.contains("key could not be looked up")
            || output_lower.contains("unknown public key")
            || output_lower.contains("signature from")
            || output_lower.contains("trust database")
        {
            return Some(Self {
                kind: PacmanErrorKind::KeyringError,
                title: "Security Key Issue".to_string(),
                description: "Package signatures could not be verified. Your keyring may need to be refreshed.".to_string(),
                recovery_action: Some(RecoveryAction::RepairKeyring),
                raw_message: output.to_string(),
            });
        }

        // Package Not Found
        if output_lower.contains("target not found")
            || output_lower.contains("no results found")
            || output_lower.contains("package not found")
        {
            return Some(Self {
                kind: PacmanErrorKind::PackageNotFound,
                title: "Package Not Found".to_string(),
                description: "The package could not be found. It may have been renamed, removed, or your repositories need syncing.".to_string(),
                recovery_action: Some(RecoveryAction::ForceRefreshDb),
                raw_message: output.to_string(),
            });
        }

        // Mirror/Network Issues
        if output_lower.contains("failed retrieving file")
            || output_lower.contains("failed to synchronize")
            || output_lower.contains("could not resolve host")
            || output_lower.contains("connection timed out")
            || output_lower.contains("error downloading")
            || output_lower.contains("404")
        {
            return Some(Self {
                kind: PacmanErrorKind::MirrorFailure,
                title: "Download Failed".to_string(),
                description: "Could not download packages from mirrors. Check your internet connection or try refreshing your mirror list.".to_string(),
                recovery_action: Some(RecoveryAction::RefreshMirrors),
                raw_message: output.to_string(),
            });
        }

        // Disk Full
        if output_lower.contains("no space left on device")
            || output_lower.contains("not enough free disk space")
        {
            return Some(Self {
                kind: PacmanErrorKind::DiskFull,
                title: "Disk Full".to_string(),
                description: "Not enough disk space to complete the operation. Try clearing the package cache.".to_string(),
                recovery_action: Some(RecoveryAction::CleanCache),
                raw_message: output.to_string(),
            });
        }

        // Dependency Conflicts
        if output_lower.contains("conflicting dependencies")
            || output_lower.contains("breaks dependency")
            || output_lower.contains("satisfies dependency")
            || output_lower.contains("unresolvable package conflicts")
        {
            return Some(Self {
                kind: PacmanErrorKind::DependencyConflict,
                title: "Dependency Conflict".to_string(),
                description: "Package dependencies conflict with installed packages. Manual intervention may be required.".to_string(),
                recovery_action: Some(RecoveryAction::ShowManualSteps(
                    "Review the conflicting packages and decide which to keep. You may need to remove one before installing the other.".to_string()
                )),
                raw_message: output.to_string(),
            });
        }

        // File Conflicts
        if output_lower.contains("exists in filesystem") || output_lower.contains("file conflict") {
            return Some(Self {
                kind: PacmanErrorKind::FileConflict,
                title: "File Conflict".to_string(),
                description: "A file already exists on your system that would be overwritten. This usually happens when files were installed outside of pacman.".to_string(),
                recovery_action: Some(RecoveryAction::ShowManualSteps(
                    "You can either: 1) Remove the conflicting file manually, or 2) Use --overwrite flag (advanced users only).".to_string()
                )),
                raw_message: output.to_string(),
            });
        }

        // Corrupted Package
        if output_lower.contains("corrupted package") || output_lower.contains("failed integrity") {
            return Some(Self {
                kind: PacmanErrorKind::CorruptedPackage,
                title: "Corrupted Download".to_string(),
                description:
                    "A downloaded package was corrupted. This usually resolves by retrying."
                        .to_string(),
                recovery_action: Some(RecoveryAction::Retry),
                raw_message: output.to_string(),
            });
        }

        // Permission Denied
        if output_lower.contains("permission denied")
            || output_lower.contains("operation not permitted")
        {
            return Some(Self {
                kind: PacmanErrorKind::PermissionDenied,
                title: "Permission Denied".to_string(),
                description: "The operation requires administrator privileges.".to_string(),
                recovery_action: Some(RecoveryAction::Retry),
                raw_message: output.to_string(),
            });
        }

        None
    }

    /// Check if this error is recoverable automatically
    #[allow(dead_code)]
    pub fn is_auto_recoverable(&self) -> bool {
        matches!(
            self.kind,
            PacmanErrorKind::DatabaseLocked
                | PacmanErrorKind::KeyringError
                | PacmanErrorKind::CorruptedPackage
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_lock_detection() {
        let output = "error: failed to init transaction (unable to lock database)";
        let err = ClassifiedError::from_output(output).unwrap();
        assert_eq!(err.kind, PacmanErrorKind::DatabaseLocked);
    }

    #[test]
    fn test_keyring_error_detection() {
        let output = "error: package: signature from \"Developer\" is invalid";
        let err = ClassifiedError::from_output(output).unwrap();
        assert_eq!(err.kind, PacmanErrorKind::KeyringError);
    }

    #[test]
    fn test_package_not_found_detection() {
        let output = "error: target not found: nonexistent-package";
        let err = ClassifiedError::from_output(output).unwrap();
        assert_eq!(err.kind, PacmanErrorKind::PackageNotFound);
    }

    #[test]
    fn test_mirror_failure_detection() {
        let output = "error: failed retrieving file 'extra.db' from mirror.example.com : The requested URL returned error: 404";
        let err = ClassifiedError::from_output(output).unwrap();
        assert_eq!(err.kind, PacmanErrorKind::MirrorFailure);
    }
}
