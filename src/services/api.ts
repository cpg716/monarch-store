import {
    commands,
    Result,
    SystemInfo,
    ChaoticStatus,
    CacheSizeResult,
    OrphansWithSizeResult,
    Package,
    PackageVariant,
    ChaoticPackage,
    UpdateItem,
    SystemUpdateCommandPayload,
    PackageSource,
    RepoConfig,
    InfraStats,
    SearchOptions
} from "./bindings";

// Helper to unwrap the Result type for cleaner async/await usage
async function unwrap<T>(promise: Promise<Result<T, string>>): Promise<T> {
    const result = await promise;
    if (result.status === "error") {
        throw new Error(result.error);
    }
    return result.data;
}

export const API = {
    system: {
        getInfo: () => unwrap(commands.getSystemInfo()),
        checkChaoticStatus: () => unwrap(commands.checkChaoticStatus()),
        getCacheSize: () => unwrap(commands.getCacheSize()),
        getOrphansWithSize: () => unwrap(commands.getOrphansWithSize()),
        getAllInstalledNames: () => unwrap(commands.getAllInstalledNames()),
        getInfraStats: () => unwrap(commands.getInfraStats()),
        getRepoCounts: () => unwrap(commands.getRepoCounts()),
        getRepoStates: () => unwrap(commands.getRepoStates()),
        isAurEnabled: () => unwrap(commands.isAurEnabled()),
        toggleRepo: (name: string, enabled: boolean, password: string | null) => unwrap(commands.toggleRepo(name, enabled, password)),
        toggleRepoFamily: (family: string, enabled: boolean, skipOsSync: boolean | null, password: string | null) => unwrap(commands.toggleRepoFamily(family, enabled, skipOsSync, password)),
        setAurEnabled: (enabled: boolean) => unwrap(commands.setAurEnabled(enabled)),
        isOneClickEnabled: () => unwrap(commands.isOneClickEnabled()),
        setOneClickEnabled: (enabled: boolean) => unwrap(commands.setOneClickEnabled(enabled)),
        getMissingRequiredBins: () => unwrap(commands.getMissingRequiredBins()),
        checkSecurityPolicy: () => unwrap(commands.checkSecurityPolicy()),
        installMonarchPolicy: (password: string | null) => unwrap(commands.installMonarchPolicy(password)),
        optimizeSystem: (password: string | null) => unwrap(commands.optimizeSystem(password)),
        triggerRepoSync: (interval: number | null, password: string | null) => unwrap(commands.triggerRepoSync(interval, password)),
        updateAndInstallPackage: (name: string, repoName: string | null, password: string | null) => unwrap(commands.updateAndInstallPackage(name, repoName, password)),
        isAdvancedMode: () => unwrap(commands.isAdvancedMode()),
        setAdvancedMode: (enabled: boolean) => unwrap(commands.setAdvancedMode(enabled)),
        checkAppUpdate: () => unwrap(commands.checkAppUpdate()),
        isTelemetryEnabled: () => unwrap(commands.isTelemetryEnabled()),
        setTelemetryEnabled: (enabled: boolean) => unwrap(commands.setTelemetryEnabled(enabled)),
        isNotificationsEnabled: () => unwrap(commands.isNotificationsEnabled()),
        setNotificationsEnabled: (enabled: boolean) => unwrap(commands.setNotificationsEnabled(enabled)),
        showDesktopNotification: (title: string, body: string) => unwrap(commands.showDesktopNotification(title, body)),
        getInstallModeCommand: () => commands.getInstallModeCommand(), // Direct return string
        isSyncOnStartupEnabled: () => unwrap(commands.isSyncOnStartupEnabled()),
        setSyncOnStartupEnabled: (enabled: boolean) => unwrap(commands.setSyncOnStartupEnabled(enabled)),
        checkAndClearRefreshRequested: () => unwrap(commands.checkAndClearRefreshRequested()),
        prepareChaoticComponents: (password: string | null) => unwrap(commands.prepareChaoticComponents(password)),
        prepareFlatpak: (password: string | null) => unwrap(commands.prepareFlatpak(password)),
        ensureFlathubRemote: () => unwrap(commands.ensureFlathubRemote()),
    },
    search: {
        searchPackages: (query: string, options: SearchOptions | null) => unwrap(commands.searchPackages(query, options)),
        searchAur: (query: string) => unwrap(commands.searchAur(query)),
        getPackagesByNames: (names: string[], options: SearchOptions | null, cacheContext: string | null) => unwrap(commands.getPackagesByNames(names, options, cacheContext)),
        getChaoticPackageInfo: (name: string) => unwrap(commands.getChaoticPackageInfo(name)),
        getChaoticPackagesBatch: (names: string[]) => unwrap(commands.getChaoticPackagesBatch(names)),
        getTrending: (options: SearchOptions | null) => unwrap(commands.getTrending(options)),
        getPackageVariants: (pkgName: string, options: SearchOptions | null) => unwrap(commands.getPackageVariants(pkgName, options)),
    },
    package: {
        abortInstallation: () => unwrap(commands.abortInstallation()),
        installPackage: (name: string, source: PackageSource, password: string | null, repoName: string | null) => unwrap(commands.installPackage(name, source, password, repoName)),
        uninstallPackage: (name: string, source: PackageSource | null, password: string | null) => unwrap(commands.uninstallPackage(name, source, password)),
    },
    update: {
        getSystemUpdateCommand: () => commands.getSystemUpdateCommand(), // Direct return payload
        performSystemUpdate: (password: string | null, includeAur: boolean | null, includeFlatpak: boolean | null) => unwrap(commands.performSystemUpdate(password, includeAur, includeFlatpak)),
        checkUpdates: (includeAur: boolean | null, includeFlatpak: boolean | null) => unwrap(commands.checkUpdates(includeAur, includeFlatpak)),
        applyUpdates: (targets: UpdateItem[], password: string | null) => unwrap(commands.applyUpdates(targets, password)),
    }
};

export type {
    SystemInfo,
    ChaoticStatus,
    CacheSizeResult,
    OrphansWithSizeResult,
    Package,
    PackageVariant,
    ChaoticPackage,
    UpdateItem,
    SystemUpdateCommandPayload,
    PackageSource,
    RepoConfig,
    InfraStats,
    SearchOptions
};
