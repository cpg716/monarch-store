// Grand Unification: Top 25 Arch Distro Identity
// Maps specific repository names + distro ID to Friendly Labels.
// Used by search.rs (official results) and models.rs (PackageSource::from_repo_name).

pub fn get_friendly_label(db_name: &str, distro_id: &str) -> &'static str {
    match db_name {
        // --- The Big Players (Core/Extra mapping) ---
        "core" | "extra" | "multilib" => match distro_id {
            "manjaro" => "Manjaro Official",
            "endeavouros" => "EndeavourOS (Arch)",
            "garuda" => "Garuda (Arch)",
            "cachyos" => "CachyOS (Arch)",
            "steamos" => "SteamOS (Arch)", // SteamOS often mirrors core/extra
            "chimeraos" => "ChimeraOS (Arch)",
            "arcolinux" => "ArcoLinux (Arch)",
            "rebornos" => "RebornOS (Arch)",
            "artix" => "Artix Linux",
            "biglinux" => "BigLinux (Arch)",
            "mabox" => "Mabox (Manjaro Base)",
            _ => "Arch Official", // Default fallback
        },

        // --- SteamOS & Gaming Consoles ---
        "jupiter" | "jupiter-rel" | "jupiter-main" => "SteamOS (Jupiter)",
        "holo" | "holo-rel" | "holo-main" => "SteamOS (Holo)",
        "chimeraos" | "chimeraos-extra" => "ChimeraOS (Gaming)",
        "gamer-os" => "GamerOS",

        // --- Performance & Optimization ---
        "cachyos" | "cachyos-v3" | "cachyos-v4" => "CachyOS (Optimized)",
        "chaotic-aur" => "Chaotic-AUR (Pre-built)",

        // --- Specialized Distro Repos ---
        "endeavouros" => "EndeavourOS Tools",
        "garuda" => "Garuda Tools",
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
        _ => "Custom Repository", // Catch-all for obscure distros
    }
}
