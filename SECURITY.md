# Security Policy

## Supported Versions

| Version   | Supported          |
| --------- | ------------------ |
| 0.3.5.x   | :white_check_mark: |
| 0.3.x     | :white_check_mark: |
| 1.0.x     | :white_check_mark: (when released) |
| &lt; 0.3   | :x:                |

## Reporting a Vulnerability

We take the security of MonARCH Store users extremely seriously, as this application manages system packages and root privileges.

**DO NOT report security vulnerabilities through public GitHub issues.**

If you believe you have found a security vulnerability in MonARCH Store, please report it to us as described below.

### Disclosure Process

1.  **Email:** Please email `security@monarch.store` with a detailed summary.
2.  **Encryption:** If possible, encrypt your email using our PGP key (Key ID: contact maintainers for current key).
3.  **Response:** You will receive a response within 48 hours acknowledging receipt.
4.  **Timeline:** We ask for a 90-day embargo period before public disclosure to allow us to release a patch.

### Scope

Examples of vulnerabilities we are interested in:
- Privilege Escalation (e.g., bypassing Polkit or invoking helper without authorization)
- Command Injection via package names or search queries (package names are validated before shell/helper)
- Repo Database Spoofing
- Arbitrary Code Execution during installation (privileged operations go through monarch-helper; command is passed via temp file)

**Current architecture:** Privileged operations use **monarch-helper** (invoked via `pkexec`); the GUI writes the JSON command to a temp file and passes only the file path. See [Install & Update Audit](docs/INSTALL_UPDATE_AUDIT.md) and [Architecture](docs/ARCHITECTURE.md).

### Third-Party Repository Keys (Smart Repair / Bootstrap)

MonARCH’s “Smart Repair” and bootstrap flows import the following **hardcoded** GPG key IDs from `keyserver.ubuntu.com` for trusted third-party repos. Users may verify these keys independently.

| Repository   | Key ID (long)   | Usage                    |
|-------------|-----------------|--------------------------|
| Chaotic-AUR | `3056513887B78AEB` | Keyring repair, bootstrap, repo setup |
| CachyOS     | `F3B607488DB35A47` | Keyring repair, bootstrap, repo setup |
| Garuda      | `349BC7808577C592` | Keyring repair, bootstrap |
| Manjaro     | `279E7CF5D8D56EC8` | Repo setup (Manjaro only) |

Import is performed only after **user-initiated** actions (e.g. “Initialize Keyring”, “Keyring Repair”, “Enable” a repo). No silent background key import occurs. Key IDs are defined in `src-tauri/monarch-gui/src/repair.rs` and `src-tauri/monarch-gui/src/repo_setup.rs`.

Thank you for helping keep the Arch ecosystem safe.
