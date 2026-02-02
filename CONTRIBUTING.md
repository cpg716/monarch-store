# Contributing to MonARCH Store 🦋

**Last updated:** 2026-02-02 (v0.4.0-alpha).
Project architecture features (**Iron Core**, **The Chameleon**, **Mission Control**, **Silent Guard**) are documented across the repository. Contributors should prioritize the [**Developer Guide**](docs/DEVELOPER.md) and [**Architecture**](ARCHITECTURE.md) for deep dives.

First off, thanks for taking the time to contribute! 🎉

The following is a set of guidelines for contributing to MonARCH Store. These are predominantly guidelines, not rules. Use your best judgment, and feel free to propose changes to this document in a pull request.

## Code of Conduct

This project and everyone participating in it is governed by our Code of Conduct. By participating, you are expected to uphold this code.

## How Can I Contribute?

### Reporting Bugs

This section guides you through submitting a bug report. Following these guidelines helps maintainers and the community understand your report, reproduce the behavior, and find related reports.

- **Use a clear and descriptive title** for the issue to identify the problem.
- **Describe the exact steps which reproduce the problem** in as much detail as possible.
- **Provide specific examples to demonstrate the steps**. Include links to files or GitHub projects, or copy/pasteable snippets, which you use in those examples.

### Suggesting Enhancements

This section guides you through submitting an enhancement suggestion, including completely new features and minor improvements to existing functionality.

- **Use a clear and descriptive title** for the issue to identify the suggestion.
- **Provide a step-by-step description of the suggested enhancement** in as much detail as possible.
- **Explain why this enhancement would be useful** to most MonARCH Store users.

### Pull Requests

- Fill in the required template
- Do not include issue numbers in the PR title
- Include screenshots and animated GIFs in your pull request whenever possible.
- End all files with a newline

### ⚠️ REPO SAFETY RULES (CRITICAL)

**Any modification to package management logic (pacman/makepkg wrappers, root command execution) requires MANDATORY Security Review.** See [AGENTS.md](AGENTS.md) for full rules.

- **No Arbitrary Command Execution:** Never construct shell commands from unsanitized user input. Validate package names with `utils::validate_package_name()` before shell ops.
- **Root Privileges:** Privileged operations go through **monarch-helper** via `pkexec`; command passed via temp file (path only in argv).
- **Partial Upgrades:** **NEVER** run `pacman -Sy` alone. All sync operations MUST use **SafeUpdateTransaction** (Iron Core) which enforces `-Syu` at the Rust level.

Violating these rules will result in immediate PR closure.

## Styleguides

### Git Commit Messages

- Use the present tense ("Add feature" not "Added feature")
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...")
- Limit the first line to 72 characters or less
- Reference issues and pull requests liberally after the first line

### Rust Styleguide

- Use `cargo fmt` before committing.
- Use `cargo clippy` to catch common mistakes.

### TypeScript/React Styleguide

- Use Functional Components with Hooks.
- Use strict type annotations.

## Development Setup

1.  **Prerequisites**:
    *   Rust (latest stable)
    *   Node.js (LTS) & NPM
    *   System dependencies: `webkit2gtk`, `base-devel`, `curl`, `wget`, `file`, `openssl`, `appmenu-gtk-module`, `gtk3`, `libappindicator-gtk3`, `librsvg`, `libvips`, `xdg-desktop-portal` (for native dark mode & dialogs)
    *   **Faster linking** (recommended): `mold` and `clang` for up to 7x faster development builds:
      ```bash
      sudo pacman -S mold clang
      ```
      The project is configured to use `mold` by default. If you encounter linker errors, see `src-tauri/.cargo/config.toml` for fallback options (`lld` or `gcc`).

2.  **Installation**:

    ```bash
    git clone https://github.com/cpg716/monarch-store.git
    cd monarch-store
    npm install
    ```

3.  **Running Locally**:

    ```bash
    npm run tauri dev
    ```

4.  **Building for Production**:

    ```bash
    npm run tauri build
    ```

## Architecture Overview

- [**USER_GUIDE.md**](USER_GUIDE.md) — Fundamental app usage and feature overview for end-users.
- [**docs/DEVELOPER.md**](docs/DEVELOPER.md) — **Developer documentation**: setup, project structure, code style, critical rules (single reference for contributors).
- [ARCHITECTURE.md](ARCHITECTURE.md) — Core philosophy, Host-Adaptive model, Butterfly engine, installer pipeline.
- [AGENTS.md](AGENTS.md) — Build commands, code style, package management rules.
- [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) — User-facing issues (GPG, db lock, etc.)
- [SECURITY.md](SECURITY.md) — Security policy and reporting.
