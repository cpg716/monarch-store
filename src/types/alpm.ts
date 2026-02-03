export interface PackageSource {
    source_type: string; // 'repo', 'aur', 'flatpak', 'local'
    id: string;          // "core", "chaotic-aur", "flathub", etc.
    version: string;
    label: string;       // "Arch Official", "Chaotic-AUR", etc.
}

/** Unified view of a package with merged sources (Official + Flatpak + AUR in one object). */
export interface UnifiedPackageSource {
    type: 'official' | 'flatpak' | 'aur';
    label: string;
    version: string;
    packageId: string; // Real ID (e.g. org.videolan.VLC or package name)
}

export interface UnifiedPackage {
    id: string;
    name: string;
    display_name?: string;
    currentSource: 'official' | 'flatpak' | 'aur';
    version: string;
    description: string;
    screenshots: string[];
    icon?: string;
    availableSources: UnifiedPackageSource[];
}

export interface AlpmProgressEvent {
    event_type: string;
    package?: string;
    percent?: number;
    downloaded?: number;
    total?: number;
    message: string;
}

export type AlpmEventType =
    | 'progress'
    | 'download_progress'
    | 'download_start'
    | 'download_complete'
    | 'extract_start'
    | 'extract_progress'
    | 'extract_complete'
    | 'install_start'
    | 'install_progress'
    | 'install_complete'
    | 'package_found'
    | 'package_marked'
    | 'file_added'
    | 'transaction_complete'
    | 'error';

export interface UpdateItem {
    name: string;
    current_version: string;
    new_version: string;
    source: PackageSource;
    size?: number;
    icon?: string;
}
