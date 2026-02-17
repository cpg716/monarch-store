// Grand Unification: Top 25 Arch Distro Identity
// Maps specific repository names + distro ID to Friendly Labels.
// Used by search.rs (official results) and models.rs (PackageSource::from_repo_name).
//
// CachyOS clarification:
// - "core" | "extra" | "multilib" => "Arch Official" (Arch Linux official repos).
// - Any repo name starting with "cachyos" => "CachyOS (Optimized)" (v3/v4/core-v4/extra-v4/znver4, etc.).

pub fn get_friendly_label(db_name: &str, _distro_id: &str) -> &'static str {
    match db_name {
        // --- Arch official repos (core/extra/multilib). [community] merged into [extra] in 2023; keep for legacy. ---
        "core" | "extra" | "community" | "multilib" => "Arch Official",

        // --- SteamOS & Gaming Consoles ---
        "jupiter" | "jupiter-rel" | "jupiter-main" => "SteamOS (Jupiter)",
        "holo" | "holo-rel" | "holo-main" => "SteamOS (Holo)",
        "chimeraos" | "chimeraos-extra" => "ChimeraOS (Gaming)",
        "gamer-os" => "GamerOS",

        // --- Performance & Optimization; CachyOS uses best repo for CPU (v3/v4/znver4) ---
        n if n.starts_with("cachyos") => "CachyOS (Optimized)",
        "chaotic-aur" => "Chaotic-AUR (Pre-built)",
        n if n.starts_with("chaotic") => "Chaotic-AUR (Pre-built)",
        n if n.starts_with("manjaro") => "Manjaro",
        n if n.starts_with("garuda") => "Garuda Tools",
        n if n.starts_with("endeavour") => "EndeavourOS Tools",
        "arcolinux_repo" | "arcolinux_repo_3party" => "ArcoLinux Repo",
        "rebornos" => "RebornOS Repo",
        "blackarch" => "BlackArch (Security)",
        "xerolinux_repo" => "XeroLinux Repo",
        "mabox" => "Mabox Tools",
        "alg-repo" => "ArchLabs",
        "athena" => "Athena OS",
        "biglinux-stable" | "biglinux-testing" => "BigLinux Repo",
        "bluestar" => "Bluestar Linux",
        "obarun" => "Obarun",
        "parabola" => "Parabola (Libre)",
        "hyperbola" => "Hyperbola",
        "ctlos" => "CtlOS",
        "alci-repo" => "ALCI",

        // --- Universal ---
        "aur" => "AUR (Community)",
        "flatpak" => "Flatpak (Sandboxed)",

        // --- Store / unknown ---
        "monarch" => "MonARCH Store",
        "local" => "Installed (Local)",
        "" | "other" | "unknown" => "Other repository", // Never show "Unknown" in UI
        _ => "Other repository", // Catch-all; UI can show repo id (e.g. "Repository (repo-name)")
    }
}
