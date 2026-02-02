# Frequently Asked Questions (FAQ) ❓

### 1. What does "Host-Adaptive" mean?
Unlike other app stores that might try to force a specific configuration or enable dozens of repositories by default, MonARCH respects your system's existing state. We read your `/etc/pacman.conf` and only show you what your system is actually set up to handle. We adapt to YOU, not the other way around.

### 2. Why does MonARCH ask for my password?
MonARCH manages system packages, which requires root privileges. We use the standard Linux `Polkit` (PolicyKit) mechanism to securely perform these actions. Your password is never stored by the app unless you explicitly enable the "Reduce password prompts" session feature.

### 3. Why are some updates labeled "Built from Source"?
Those are **AUR (Arch User Repository)** packages. Unlike standard packages that come pre-compiled, these are built from scratch on your machine. This takes significantly more time and CPU power. We label them so you aren't surprised by the extra compilation time.

### 4. Why can't I enable Chaotic-AUR on Manjaro?
Manjaro uses slightly different versions of core system libraries (like `glibc`) than vanilla Arch. Using the pre-built binaries from Chaotic-AUR on Manjaro is a common cause of system breakage. MonARCH includes a "Manjaro Guard" to prevent these incompatible configurations for your own safety.

### 5. Does MonARCH Store replace Pacman?
No. MonARCH is a companion to your system tools. It uses the same library (`libalpm`) as Pacman, meaning they share the same database. Anything you do in MonARCH is visible to Pacman, and vice-versa.

### 6. Where are the AUR build files stored?
By default, MonARCH clones and builds AUR packages in `~/.cache/monarch/aur/`. You can change this or enable automatic cleaning in **Settings → AUR Builder**.

### 7. How do I report a bug?
Please open an issue on our [GitHub repository](https://github.com/cpg716/monarch-store/issues). Provide as much detail as possible, including your distribution and any logs from the Detailed Console.

---

*Still have questions? Check out the [User Guide](USER_GUIDE.md) or join our community discussions.*
