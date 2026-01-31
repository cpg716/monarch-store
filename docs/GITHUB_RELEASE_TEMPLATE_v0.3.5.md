# MonARCH Store v0.3.5-alpha

**The first Distro-Aware App Manager for Arch, Manjaro, and CachyOS.**

This alpha release focuses on **1% polish**, **system integrity**, and **community distribution readiness**.

---

## Highlights

### Distro-Aware Intelligence
- **Manjaro Guard:** Hides Arch-native repos (e.g. Chaotic-AUR) where glibc/ABI differ to avoid partial upgrades.
- **CachyOS Performance:** Detects AVX2/AVX-512 and prioritizes v3/v4 repos for faster binaries.
- **Arch Power Mode:** Full repo access for vanilla Arch users.

### Instant Binaries (Chaotic-AUR & CachyOS)
- Pre-built AUR binaries where available — no compile wait.
- **CachyOS Optimized** and **Chaotic-AUR** badges with subtle hover feedback so pre-built value is clear at a glance.

### System Integrity
- **Atomic transactions only:** All repo sync and installs use `pacman -Syu` or `pacman -Syu --needed` — no naked `-Sy`.
- **Database lock handling:** Clear "Database Locked" message and one-click **Database Unlock** (with safe checks) when another pacman is running or a lock is stale.
- **Polkit integration:** Privileged operations use `monarch-helper` at `/usr/lib/monarch-store/monarch-helper`; passwordless when Polkit rules are installed.

### UX Polish
- **Modal sovereignty:** Every overlay closes with **Escape** and keeps focus trapped for accessibility.
- **Skeleton loading:** GPU-accelerated shimmer on package cards while ODRS and metadata load.
- **Badge micro-interactions:** Subtle scale on hover for CachyOS Optimized and Chaotic-AUR badges.

### Packaging & Security
- **Binary hardening:** Release builds use LTO and strip.
- **Input validation:** Package names validated with a strict regex to prevent shell-injection.
- **Zero telemetry before consent:** No external pings before onboarding acceptance.

---

## Installation

### Pre-built package (recommended)
Download `monarch-store-0.3.5_alpha-1-x86_64.pkg.tar.zst` (or current version) from the assets below, then:

```bash
sudo pacman -U monarch-store-0.3.5_alpha-1-x86_64.pkg.tar.zst
```

### From AUR
If the package is in the AUR, use your preferred AUR helper, e.g.:

```bash
yay -S monarch-store
# or
paru -S monarch-store
```

### Build from source
```bash
git clone https://github.com/cpg716/monarch-store.git
cd monarch-store
npm install
npm run tauri build
```

---

## Requirements
- Arch Linux (or compatible: Manjaro, CachyOS, EndeavourOS, Garuda)
- `webkit2gtk-4.1`, `gtk3`, `openssl`, `polkit`, `pacman-contrib`, `git`

---

## Full Changelog
See [RELEASE_NOTES.md](https://github.com/cpg716/monarch-store/blob/main/RELEASE_NOTES.md) and [docs/SECURITY_AUDIT_FORT_KNOX.md](https://github.com/cpg716/monarch-store/blob/main/docs/SECURITY_AUDIT_FORT_KNOX.md) for security and release context.
