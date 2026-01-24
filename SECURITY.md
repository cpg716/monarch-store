# Security Policy

## Supported Versions

Use this section to tell people about which versions of your project are
currently being supported with security updates.

| Version | Supported          |
| ------- | ------------------ |
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting a Vulnerability

We take the security of MonARCH Store users extremely seriously, as this application manages system packages and root privileges.

**DO NOT report security vulnerabilities through public GitHub issues.**

If you believe you have found a security vulnerability in MonARCH Store, please report it to us as described below.

### Disclosure Process

1.  **Email:** Please email `security@monarch.store` with a detailed summary.
2.  **Encryption:** If possible, encrypt your email using our PGP key (Key ID: `0xDEADBEEF`).
3.  **Response:** You will receive a response within 48 hours acknowledging receipt.
4.  **Timeline:** We ask for a 90-day embargo period before public disclosure to allow us to release a patch.

### Scope

Examples of vulnerabilities we are interested in:
- Privilege Escalation (e.g., bypassing Polkit)
- Command Injection via package names or search queries
- Repo Database Spoofing
- Arbitrary Code Execution during installation

Thank you for helping keep the Arch ecosystem safe.
