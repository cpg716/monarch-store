import { useState, useEffect } from 'react';
import { ArrowLeft, Download, Globe, Calendar, User, Zap, Package as PackageIcon, AlertTriangle, Star, MessageSquare, ChevronDown, X, ChevronLeft, ChevronRight, Code, Loader2, Heart } from 'lucide-react';
import { Package } from '../components/PackageCard';
import InstallMonitor from '../components/InstallMonitor';
import RepoSetupModal from '../components/RepoSetupModal';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import { useFavorites } from '../hooks/useFavorites';
import { getPackageReviews, submitReview, Review as ServiceReview, RatingSummary } from '../services/reviewService';
import { trackEvent } from '@aptabase/tauri';

interface AppMetadata {
    name: string;
    pkg_name?: string;
    icon_url?: string;
    app_id: string;
    summary?: string;
    screenshots: string[];
    version?: string;
    maintainer?: string;
    license?: string;
    last_updated?: number;
    description?: string;
}

// Define types locally if not in shared types
interface PackageDetailsProps {
    pkg: Package;
    onBack: () => void;
    preferredSource?: string;
}

interface ChaoticPackage {
    id: number;
    pkgname: string;
    lastUpdated?: string;
    version?: string;
    metadata?: {
        buildDate?: string;
        license?: string;
    }
}

export default function PackageDetails({ pkg, onBack, preferredSource }: PackageDetailsProps) {
    // State
    interface PackageVariant {
        source: 'chaotic' | 'aur' | 'official' | 'cachyos' | 'garuda' | 'endeavour' | 'manjaro';
        version: string;
        repo_name?: string;
        pkg_name?: string;
    }

    // Explicitly typed state
    const [fullMeta, setFullMeta] = useState<AppMetadata | null>(null);
    const [chaoticInfo, setChaoticInfo] = useState<ChaoticPackage | null>(null);
    const [rating, setRating] = useState<RatingSummary | null>(null);
    const [reviews, setReviews] = useState<ServiceReview[]>([]);

    // Removed localReviews state as it is merged in reviewService

    const [activeTab, setActiveTab] = useState<'details' | 'reviews'>('details');
    const [showReviewForm, setShowReviewForm] = useState(false);
    const [reviewTitle, setReviewTitle] = useState('');
    const [reviewBody, setReviewBody] = useState('');
    const [reviewRating, setReviewRating] = useState(5);

    // New State for Multi-Source
    const [variants, setVariants] = useState<PackageVariant[]>([]);
    const [selectedSource, setSelectedSource] = useState<string>(pkg.source);

    // Install Flow State
    // Install Flow State
    const [showInstallMonitor, setShowInstallMonitor] = useState(false);
    const [showInstallConfirm, setShowInstallConfirm] = useState(false);
    const [showRepoSetup, setShowRepoSetup] = useState(false);
    const [missingRepoId, setMissingRepoId] = useState<string>("");

    // PKGBUILD Viewer State
    const [showPkgbuild, setShowPkgbuild] = useState(false);
    const { isFavorite, toggleFavorite } = useFavorites();
    const isFav = isFavorite(pkg.name);
    const [pkgbuildContent, setPkgbuildContent] = useState<string | null>(null);
    const [pkgbuildLoading, setPkgbuildLoading] = useState(false);
    const [pkgbuildError, setPkgbuildError] = useState<string | null>(null);

    // Lightbox State for Screenshots
    const [lightboxIndex, setLightboxIndex] = useState<number | null>(null);

    // Fetch Variants and Auto-Select Priority AND Meta/Reviews
    useEffect(() => {
        // 1. Fetch Variants
        invoke<PackageVariant[]>('get_package_variants', { pkgName: pkg.name })
            .then(vars => {
                // We trust the backend: If AUR is returned here, it means the user ENABLED it in settings.
                // We should NOT hide it, just prioritize selecting others first.

                setVariants(vars);

                // If preferredSource is provided and exists in variants, use it
                if (preferredSource && vars.some(v => v.source === preferredSource)) {
                    setSelectedSource(preferredSource);
                    return;
                }

                // Default Selection Logic: Chaotic -> Official -> CachyOS/Garuda/Endeavour -> Manjaro -> AUR
                if (vars.some(v => v.source === 'chaotic')) {
                    setSelectedSource('chaotic');
                } else if (vars.some(v => v.source === 'official')) {
                    setSelectedSource('official');
                } else if (vars.some(v => v.source === 'cachyos')) {
                    setSelectedSource('cachyos');
                } else if (vars.some(v => v.source === 'garuda')) {
                    setSelectedSource('garuda');
                } else if (vars.some(v => v.source === 'endeavour')) {
                    setSelectedSource('endeavour');
                } else if (vars.some(v => v.source === 'manjaro')) {
                    setSelectedSource('manjaro');
                } else if (vars.some(v => v.source === 'aur')) {
                    setSelectedSource('aur');
                } else if (vars.length > 0) {
                    setSelectedSource(vars[0].source);
                }
            })
            .catch(console.error);

        // 2. Fetch Metadata
        const loadInitialMeta = async () => {
            if (pkg.icon || pkg.screenshots) {
                setFullMeta({
                    name: pkg.display_name || pkg.name,
                    pkg_name: pkg.name,
                    icon_url: pkg.icon,
                    screenshots: pkg.screenshots || [],
                    app_id: pkg.name
                });
            }

            try {
                const meta = await invoke<AppMetadata>('get_metadata', { pkgName: pkg.name, upstreamUrl: pkg.url });
                setFullMeta(meta);
            } catch (e) {
                console.error(e);
            }
        };

        loadInitialMeta();

        // ... (rest of effects combined for clarity/lifecycle)
    }, [pkg.name, pkg.url, pkg.icon, pkg.screenshots]); // Removed isChaotic dep to avoid loops if source changes

    // Derived state for UI consistency
    // const isChaotic = selectedSource === 'chaotic';
    // const isAur = selectedSource === 'aur'; // Use selectedSource directly in JSX for clarity

    useEffect(() => {
        if (selectedSource === 'chaotic') {
            invoke<ChaoticPackage>('get_chaotic_package_info', { name: pkg.name })
                .then(setChaoticInfo)
                .catch(console.error);
        }
    }, [selectedSource, pkg.name]);


    useEffect(() => {
        const fetchReviews = async () => {
            // setIsLoadingReviews(true);
            try {
                // Determine ID to use (prefer AppStream ID if available, else pkg name)
                const lookupId = fullMeta?.app_id || pkg.app_id || pkg.name;
                console.log(`[PackageDetails] Loading reviews for ${pkg.name}. Lookup ID: ${lookupId}`);
                console.log("[PackageDetails] FullMeta:", fullMeta);

                const { reviews: fetchedReviews, summary } = await getPackageReviews(pkg.name, lookupId);
                setReviews(fetchedReviews);
                setRating(summary);
            } catch (e) {
                console.error("Failed to load reviews", e);
            } finally {
                // setIsLoadingReviews(false);
            }
        };

        fetchReviews();
    }, [pkg.name, fullMeta]);

    const handleReviewSubmit = async () => {
        try {
            // Need user name, for now hardcoded "You" or from auth if we had it
            await submitReview(pkg.name, reviewRating, reviewTitle + "\n\n" + reviewBody, "MonArch User");

            setShowReviewForm(false);
            setReviewTitle('');
            setReviewBody('');

            // Refresh reviews
            const lookupId = fullMeta?.app_id || pkg.app_id || pkg.name;
            const { reviews: fetchedReviews, summary } = await getPackageReviews(pkg.name, lookupId);
            setReviews(fetchedReviews);
            setRating(summary);

            trackEvent('review_submitted', { package: pkg.name, rating: reviewRating });
            alert("Review submitted!");
        } catch (e) {
            alert("Failed to submit review: " + String(e));
        }
    };

    // Fetch PKGBUILD for AUR packages
    const fetchPkgbuild = async () => {
        setPkgbuildLoading(true);
        setPkgbuildError(null);
        try {
            const content = await invoke<string>('fetch_pkgbuild', { pkgName: pkg.name });
            setPkgbuildContent(content);
            setShowPkgbuild(true);
        } catch (e) {
            setPkgbuildError(String(e));
            setShowPkgbuild(true); // Show modal with error
        } finally {
            setPkgbuildLoading(false);
        }
    };

    // Handle install button click - shows confirmation for AUR packages
    const handleInstallClick = async () => {
        // Pre-flight check: Is the repo actually backend-enabled?
        try {
            const isSetup = await invoke<boolean>('check_repo_status', { name: selectedSource });
            if (!isSetup) {
                setMissingRepoId(selectedSource);
                setShowRepoSetup(true);
                return;
            }
        } catch (e) {
            console.error("Failed to check repo status:", e);
        }

        if (selectedSource === 'aur') {
            setShowInstallConfirm(true);
        } else {
            trackEvent('install_clicked', { package: pkg.name, source: selectedSource });
            setShowInstallMonitor(true);
        }
    };



    return (
        <div className="h-full flex flex-col bg-app-bg animate-in slide-in-from-right duration-300 transition-colors">
            {/* Header / Hero */}
            <div className="relative">
                <div className="absolute inset-0 h-48 bg-gradient-to-b from-app-sidebar to-app-bg -z-10" />

                {/* Out of Date Warning */}
                {pkg.out_of_date && (
                    <div className="mx-8 mt-4 mb-2 p-4 rounded-xl bg-amber-500/10 border border-amber-500/20 flex items-center gap-4 animate-in slide-in-from-top-2">
                        <div className="w-10 h-10 rounded-full bg-amber-500/20 flex items-center justify-center text-amber-500 shrink-0">
                            <AlertTriangle size={20} />
                        </div>
                        <div>
                            <h3 className="text-amber-500 font-bold text-sm">Review Recommended</h3>
                            <p className="text-amber-200/60 text-xs">
                                This package has been flagged as out-of-date by the community. It might be unstable or missing features.
                            </p>
                        </div>
                    </div>
                )}

                <div className="p-6 pb-6 flex flex-col lg:flex-row items-start gap-6 relative">
                    {/* Back Button - Absolute on desktop to save space, or flex? Flex is safer for alignment */}
                    <button
                        onClick={onBack}
                        className="absolute top-8 left-8 p-3 rounded-full bg-white/5 hover:bg-white/10 transition-colors z-10 hidden lg:block"
                    >
                        <ArrowLeft size={24} />
                    </button>

                    {/* Mobile Back Button */}
                    <button
                        onClick={onBack}
                        className="lg:hidden mb-4 p-2 rounded-full bg-white/5 hover:bg-white/10 transition-colors"
                    >
                        <ArrowLeft size={24} />
                    </button>

                    {/* Main Icon - Centered on mobile, left on desktop */}
                    <div className="w-28 h-28 lg:w-32 lg:h-32 lg:ml-16 shrink-0 rounded-2xl bg-app-card shadow-2xl flex items-center justify-center p-5 border border-app-border overflow-hidden mx-auto lg:mx-0">
                        {pkg.icon || fullMeta?.icon_url ? (
                            <img src={pkg.icon || fullMeta?.icon_url} alt={pkg.name} className="w-full h-full object-contain drop-shadow-lg" />
                        ) : selectedSource === 'chaotic' ? (
                            <Zap size={64} className="text-yellow-400" />
                        ) : selectedSource === 'official' ? (
                            <PackageIcon size={64} className="text-blue-400" />
                        ) : (
                            <PackageIcon size={64} className="text-zinc-500" />
                        )}
                    </div>

                    {/* Center Info */}
                    <div className="flex-1 min-w-0 pt-2 text-center lg:text-left">
                        <div className="mb-2 flex items-center justify-center lg:justify-start gap-3 flex-wrap">
                            <h1 className="text-3xl lg:text-4xl font-black text-app-fg truncate">
                                {pkg.display_name || fullMeta?.name || pkg.name}
                            </h1>
                            <button
                                onClick={() => toggleFavorite(pkg.name)}
                                className={clsx(
                                    "p-2 rounded-full transition-all active:scale-95",
                                    isFav ? "bg-red-500/10 text-red-500" : "bg-app-subtle text-app-muted hover:bg-app-hover hover:text-red-400"
                                )}
                                title={isFav ? "Remove from Favorites" : "Add to Favorites"}
                            >
                                <Heart size={24} className={clsx(isFav && "fill-current")} />
                            </button>
                            <span className={clsx(
                                "px-3 py-1 rounded-full text-xs font-bold uppercase tracking-wider",
                                pkg.source === 'aur' ? "bg-amber-600/20 text-amber-600 border border-amber-600/30" :
                                    pkg.source === 'chaotic' ? "bg-violet-600/20 text-violet-600 border border-violet-600/30" :
                                        pkg.source === 'official' ? "bg-teal-600/20 text-teal-600 border border-teal-600/30" :
                                            "bg-sky-600/20 text-sky-600 border border-sky-600/30"
                            )}>
                                {pkg.source}
                            </span>

                        </div>

                        {(pkg.display_name || fullMeta?.name) && (pkg.display_name || fullMeta?.name)?.toLowerCase() !== pkg.name.toLowerCase() && (
                            <div className="text-sm font-mono text-app-muted opacity-60 mb-2 truncate">
                                {pkg.name}
                            </div>
                        )}

                        <p className="text-lg text-app-muted font-light leading-relaxed max-w-3xl mx-auto lg:mx-0">
                            {fullMeta?.summary || pkg.description}
                        </p>

                        {/* Rating Summary moved here */}
                        {rating && rating.count > 0 && (
                            <div className="flex items-center justify-center lg:justify-start gap-2 mt-3">
                                <div className="flex text-yellow-500 gap-0.5">
                                    {[1, 2, 3, 4, 5].map(s => (
                                        <Star key={s} size={18} fill={s <= rating.average ? "currentColor" : "none"} />
                                    ))}
                                </div>
                                <span className="text-sm text-app-muted">
                                    ({rating.count} reviews)
                                </span>
                            </div>
                        )}

                        <div className="flex flex-col sm:flex-row items-center justify-center lg:justify-start gap-3 mt-6">
                            {/* Install Button */}
                            <button
                                onClick={handleInstallClick}
                                className="w-full sm:w-auto min-w-[180px] bg-blue-600 hover:bg-blue-500 text-white px-6 py-3 rounded-xl font-bold text-lg shadow-xl shadow-blue-900/20 active:scale-95 transition-all flex items-center justify-center gap-2"
                            >
                                <Download size={20} /> Install
                            </button>

                            {/* View PKGBUILD Button - only for AUR */}
                            {selectedSource === 'aur' && (
                                <button
                                    onClick={fetchPkgbuild}
                                    disabled={pkgbuildLoading}
                                    className="px-4 py-3 rounded-xl border border-app-border bg-app-card/50 hover:bg-app-card text-app-fg font-medium flex items-center gap-2 transition-all disabled:opacity-50"
                                >
                                    {pkgbuildLoading ? <Loader2 size={18} className="animate-spin" /> : <Code size={18} />}
                                    View PKGBUILD
                                </button>
                            )}

                            {variants.length > 1 && (
                                <div className="flex flex-col items-start text-left">
                                    <span className="text-xs text-app-muted mb-1 font-medium">Install from:</span>
                                    <div className="relative z-10">
                                        <select
                                            value={selectedSource}
                                            onChange={(e) => setSelectedSource(e.target.value as any)}
                                            className="appearance-none bg-app-card/50 border border-app-border rounded-xl px-4 py-3 pr-10 text-app-fg font-medium focus:outline-none focus:ring-2 focus:ring-app-accent/50 cursor-pointer text-sm"
                                        >
                                            {variants.map(v => {
                                                const label = v.source === 'chaotic' ? 'Chaotic (Prebuilt)' :
                                                    v.source === 'official' ? 'Official' :
                                                        v.source === 'aur' ? 'AUR (Source)' :
                                                            v.source === 'cachyos' ? 'CachyOS' :
                                                                v.source.charAt(0).toUpperCase() + v.source.slice(1);
                                                return (
                                                    <option key={v.source} value={v.source}>
                                                        {label} - {v.version}
                                                    </option>
                                                );
                                            })}
                                        </select>
                                        <ChevronDown size={16} className="absolute right-3 top-1/2 -translate-y-1/2 text-app-muted pointer-events-none" />
                                    </div>
                                    {/* Helper text under dropdown */}
                                    <p className="text-xs text-app-muted mt-2 max-w-[320px]">
                                        {selectedSource === 'chaotic' && 'Pre-compiled AUR binary. Installs instantly without building from source.'}
                                        {selectedSource === 'aur' && 'Built from source. Always the latest version, but takes longer to install.'}
                                        {selectedSource === 'official' && 'Official Arch repository. Most stable and well-tested option.'}
                                        {selectedSource === 'cachyos' && 'Performance-optimized build with CPU-specific tuning for speed.'}
                                        {selectedSource === 'garuda' && 'Gaming-focused build with extra performance tweaks.'}
                                        {selectedSource === 'endeavour' && 'Community-maintained build, often with additional patches.'}
                                        {selectedSource === 'manjaro' && 'Tested and delayed release for extra stability over Arch.'}
                                    </p>
                                </div>
                            )}
                        </div>

                        {/* AUR Safety Warning */}
                        {selectedSource === 'aur' && (
                            <div className="mt-4 p-4 rounded-xl bg-amber-500/10 border border-amber-500/30 flex items-start gap-3">
                                <AlertTriangle size={20} className="text-amber-500 shrink-0 mt-0.5" />
                                <div>
                                    <p className="text-amber-500 font-bold text-sm">AUR Package Warning</p>
                                    <p className="text-amber-500/80 text-xs mt-1">
                                        This package is from the Arch User Repository and has not been reviewed by Arch maintainers.
                                        Always inspect the PKGBUILD before installing. Use at your own risk.
                                    </p>
                                </div>
                            </div>
                        )}

                        {/* Horizontal Package Info Bar */}
                        <div className="mt-6 w-full bg-white/5 backdrop-blur-md rounded-xl p-4 border border-white/10">
                            <div className="flex flex-wrap items-center justify-start gap-6 text-sm">
                                <div className="flex items-center gap-2">
                                    <User size={14} className="text-app-muted" />
                                    <span className="text-app-muted">Maintainer:</span>
                                    <span className="text-app-fg font-medium">
                                        {fullMeta?.maintainer || pkg.maintainer || (selectedSource === 'chaotic' ? 'Chaotic-AUR Team' : selectedSource === 'aur' ? 'AUR Contributor' : 'Arch Linux')}
                                    </span>
                                </div>
                                <div className="flex items-center gap-2">
                                    <Globe size={14} className="text-app-muted" />
                                    <span className="text-app-muted">License:</span>
                                    <span className="text-app-fg font-medium">
                                        {fullMeta?.license || chaoticInfo?.metadata?.license || pkg.license?.join(', ') || 'Open Source'}
                                    </span>
                                </div>
                                <div className="flex items-center gap-2">
                                    <Calendar size={14} className="text-app-muted" />
                                    <span className="text-app-muted">Updated:</span>
                                    <span className="text-app-fg font-medium">
                                        {(() => {
                                            const selectedVariant = variants.find(v => v.source === selectedSource);
                                            if (selectedVariant?.version) {
                                                // Try to use chaotic build date if available
                                                if (selectedSource === 'chaotic' && chaoticInfo?.lastUpdated) {
                                                    return new Date(chaoticInfo.lastUpdated).toLocaleDateString();
                                                }
                                            }
                                            if (fullMeta?.last_updated) {
                                                return new Date(fullMeta.last_updated * 1000).toLocaleDateString();
                                            }
                                            if (pkg.last_modified) {
                                                return new Date(pkg.last_modified * 1000).toLocaleDateString();
                                            }
                                            return 'Recently';
                                        })()}
                                    </span>
                                </div>
                                <div className="flex items-center gap-2">
                                    <PackageIcon size={14} className="text-app-muted" />
                                    <span className="text-app-muted">Version:</span>
                                    <span className="text-app-fg font-medium font-mono text-xs">
                                        {variants.find(v => v.source === selectedSource)?.version || pkg.version}
                                    </span>
                                </div>
                                {pkg.url && (
                                    <a href={pkg.url} target="_blank" rel="noreferrer" className="flex items-center gap-2 text-blue-400 hover:text-blue-300 transition-colors ml-auto">
                                        <Globe size={14} /> Website
                                    </a>
                                )}
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            {/* Scrollable Content Area */}
            <div className="flex-1 overflow-y-auto min-h-0 relative">
                {/* Screenshots Carousel */}
                {((fullMeta?.screenshots && fullMeta.screenshots.length > 0) || (pkg.screenshots && pkg.screenshots.length > 0)) && (
                    <div className="py-6 px-4 md:px-8 border-b border-app-border/30">
                        <div className="overflow-x-auto scrollbar-hide snap-x snap-mandatory pb-4">
                            <div className="flex gap-4 w-max">
                                {(fullMeta?.screenshots?.length ? fullMeta.screenshots : pkg.screenshots || []).map((url: string, i: number) => (
                                    <div
                                        key={i}
                                        onClick={(e) => {
                                            e.stopPropagation();
                                            setLightboxIndex(i);
                                        }}
                                        className="flex-shrink-0 w-[280px] sm:w-[320px] md:w-[400px] lg:w-[480px] aspect-video bg-app-card rounded-xl overflow-hidden shadow-lg border border-app-border snap-center cursor-pointer group relative"
                                    >
                                        <img
                                            src={url}
                                            alt={`Screenshot ${i + 1} `}
                                            className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500"
                                        />
                                        <div className="absolute inset-0 bg-black/0 group-hover:bg-black/10 transition-colors flex items-center justify-center opacity-0 group-hover:opacity-100 poineter-events-none">
                                            <span className="bg-black/60 px-3 py-1 rounded-full text-xs text-white font-medium">Click to enlarge</span>
                                        </div>
                                    </div>
                                ))}
                            </div>
                        </div>
                    </div>
                )}

                {/* Lightbox Modal - Portal recommended but inline works if fixed properly */}
                {lightboxIndex !== null && (
                    <div
                        className="fixed inset-0 z-[100] bg-black/95 backdrop-blur-md flex items-center justify-center"
                        onClick={(e) => {
                            e.stopPropagation();
                            setLightboxIndex(null);
                        }}
                    >
                        {/* Close Button */}
                        <button
                            onClick={(e) => {
                                e.stopPropagation();
                                setLightboxIndex(null);
                            }}
                            className="absolute top-6 right-6 p-3 rounded-full bg-white/10 hover:bg-white/20 transition-colors text-white z-[101]"
                        >
                            <X size={28} />
                        </button>

                        {/* Previous Button */}
                        {lightboxIndex > 0 && (
                            <button
                                onClick={(e) => {
                                    e.stopPropagation();
                                    setLightboxIndex(lightboxIndex - 1);
                                }}
                                className="absolute left-6 p-4 rounded-full bg-white/10 hover:bg-white/20 transition-colors text-white z-[101]"
                            >
                                <ChevronLeft size={48} />
                            </button>
                        )}

                        {/* Next Button */}
                        {lightboxIndex < (fullMeta?.screenshots?.length || pkg.screenshots?.length || 1) - 1 && (
                            <button
                                onClick={(e) => {
                                    e.stopPropagation();
                                    setLightboxIndex(lightboxIndex + 1);
                                }}
                                className="absolute right-6 p-4 rounded-full bg-white/10 hover:bg-white/20 transition-colors text-white z-[101]"
                            >
                                <ChevronRight size={48} />
                            </button>
                        )}

                        {/* Image */}
                        <div className="relative max-w-[90vw] max-h-[90vh]" onClick={(e) => e.stopPropagation()}>
                            <img
                                src={(fullMeta?.screenshots?.length ? fullMeta.screenshots : pkg.screenshots || [])[lightboxIndex]}
                                alt={`Screenshot ${lightboxIndex + 1} `}
                                className="max-w-full max-h-[85vh] object-contain rounded-lg shadow-2xl"
                            />
                        </div>

                        {/* Counter */}
                        <div className="absolute bottom-8 left-1/2 -translate-x-1/2 bg-black/60 px-6 py-2 rounded-full text-white text-sm font-medium backdrop-blur-sm pointer-events-none">
                            {lightboxIndex + 1} / {(fullMeta?.screenshots?.length || pkg.screenshots?.length || 0)}
                        </div>
                    </div>
                )}

                {/* Content Tabs */}
                <div className="p-8 pt-6 max-w-5xl mx-auto">
                    <div className="flex items-center gap-8 border-b border-app-border mb-8">
                        <button
                            onClick={() => setActiveTab('details')}
                            className={clsx("pb-4 text-sm font-bold uppercase tracking-wider transition-all border-b-2", activeTab === 'details' ? "border-blue-500 text-app-fg" : "border-transparent text-app-muted hover:text-app-fg")}
                        >
                            Details
                        </button>
                        <button
                            onClick={() => setActiveTab('reviews')}
                            className={clsx("pb-4 text-sm font-bold uppercase tracking-wider transition-all border-b-2", activeTab === 'reviews' ? "border-blue-500 text-app-fg" : "border-transparent text-app-muted hover:text-app-fg")}
                        >
                            Reviews ({rating?.count || 0})
                        </button>
                    </div>

                    {activeTab === 'details' ? (
                        <div className="space-y-8">
                            {/* Description */}
                            <div className="bg-app-card/30 rounded-2xl p-8 border border-app-border/50">
                                <h3 className="text-lg font-bold mb-6 flex items-center gap-2">
                                    <Globe size={18} className="text-blue-500" /> Description
                                </h3>
                                {fullMeta?.description ? (
                                    <div
                                        className="prose prose-invert prose-lg max-w-none text-app-muted leading-relaxed"
                                        dangerouslySetInnerHTML={{ __html: fullMeta.description }}
                                    />
                                ) : (
                                    <p className="text-app-muted leading-relaxed text-lg">
                                        {fullMeta?.summary || pkg.description || "No description available."}
                                    </p>
                                )}
                            </div>

                            {/* Keywords/Tags */}
                            {pkg.keywords && (
                                <div className="flex flex-wrap gap-2">
                                    {pkg.keywords.map(k => (
                                        <span key={k} className="px-3 py-1.5 rounded-lg bg-app-card border border-app-border text-sm text-app-muted">
                                            #{k}
                                        </span>
                                    ))}
                                </div>
                            )}
                        </div>
                    ) : (
                        <div className="space-y-8 max-w-4xl">
                            <div className="flex justify-between items-center bg-blue-600/10 border border-blue-500/20 p-6 rounded-2xl">
                                <div>
                                    <h3 className="font-bold text-lg text-blue-400">What do you think?</h3>
                                    <p className="text-sm text-app-muted">Share your experience with the community.</p>
                                </div>
                                <button
                                    onClick={() => setShowReviewForm(true)}
                                    className="px-6 py-2 bg-blue-600 text-white rounded-lg font-bold hover:bg-blue-500 transition-colors"
                                >
                                    Write a Review
                                </button>
                            </div>

                            {showReviewForm && (
                                <div className="bg-app-card p-6 rounded-2xl border border-blue-500/30 animate-in zoom-in-95 duration-200">
                                    <h3 className="font-bold mb-4 text-app-fg">New Review</h3>
                                    <div className="space-y-4">
                                        <div className="flex gap-2 mb-4">
                                            {[1, 2, 3, 4, 5].map(s => (
                                                <Star
                                                    key={s}
                                                    size={24}
                                                    className="cursor-pointer transition-colors"
                                                    fill={s <= reviewRating ? "#EAB308" : "none"}
                                                    color={s <= reviewRating ? "#EAB308" : "currentColor"}
                                                    onClick={() => setReviewRating(s)}
                                                />
                                            ))}
                                        </div>
                                        <input
                                            type="text"
                                            placeholder="Summary (e.g. Amazing app!)"
                                            className="w-full bg-app-bg border border-app-border rounded-lg p-3 text-app-fg focus:border-blue-500 outline-none placeholder:text-app-muted/50"
                                            value={reviewTitle}
                                            onChange={(e) => setReviewTitle(e.target.value)}
                                        />
                                        <textarea
                                            placeholder="Tell us more about your experience..."
                                            rows={4}
                                            className="w-full bg-app-bg border border-app-border rounded-lg p-3 text-app-fg focus:border-blue-500 outline-none placeholder:text-app-muted/50"
                                            value={reviewBody}
                                            onChange={(e) => setReviewBody(e.target.value)}
                                        />
                                        <div className="flex justify-end gap-3">
                                            <button onClick={() => setShowReviewForm(false)} className="px-6 py-2 text-app-muted hover:text-app-fg transition-colors">Cancel</button>
                                            <button
                                                onClick={handleReviewSubmit}
                                                disabled={!reviewTitle || !reviewBody}
                                                className="px-6 py-2 bg-blue-600 text-white rounded-lg font-bold disabled:opacity-50"
                                            >
                                                Submit Review
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            )}

                            <div className="space-y-6">
                                <div className="space-y-6">
                                    {/* Unified Reviews List */}
                                    {reviews.map(review => (
                                        <div key={review.id} className={clsx(
                                            "p-6 rounded-2xl border relative overflow-hidden",
                                            review.source === 'monarch' ? "bg-blue-600/5 border-blue-500/20" : "bg-app-card/50 border-app-border"
                                        )}>
                                            {review.source === 'monarch' && (
                                                <div className="absolute top-2 right-4 text-[9px] font-bold text-blue-500/50 uppercase">Community Review</div>
                                            )}
                                            {review.source === 'odrs' && (
                                                <div className="absolute top-2 right-4 text-[9px] font-bold text-app-muted/50 uppercase">ODRS</div>
                                            )}

                                            <div className="flex justify-between items-start mb-2">
                                                <div className="flex items-center gap-2">
                                                    <span className="font-bold text-app-fg">{review.userName || 'Anonymous'}</span>
                                                </div>
                                                <div className="flex text-yellow-500 gap-0.5">
                                                    {[...Array(5)].map((_, i) => (
                                                        <Star key={i} size={14} fill={i < review.rating ? "currentColor" : "none"} />
                                                    ))}
                                                </div>
                                            </div>
                                            <p className="text-app-muted text-sm leading-relaxed whitespace-pre-line">{review.comment}</p>
                                            <p className="text-xs text-app-muted opacity-60 mt-4">Posted {new Date(review.date).toLocaleDateString()}</p>
                                        </div>
                                    ))}

                                    {reviews.length === 0 && (
                                        <div className="text-center py-12 text-app-muted bg-app-card/30 rounded-2xl border border-app-border border-dashed">
                                            <MessageSquare size={32} className="mx-auto mb-2 opacity-50" />
                                            <p>No reviews available yet.</p>
                                        </div>
                                    )}
                                </div>
                            </div>
                        </div>
                    )}
                </div>

                {/* Install Monitor Overlay */}
                {showInstallMonitor && (
                    <InstallMonitor
                        pkg={{
                            name: variants.find(v => v.source === selectedSource)?.pkg_name || pkg.name,
                            source: selectedSource
                        }}
                        onClose={() => setShowInstallMonitor(false)}
                    />
                )}

                {/* PKGBUILD Viewer Modal */}
                {showPkgbuild && (
                    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4">
                        <div className="bg-app-card border border-app-border rounded-2xl w-full max-w-4xl max-h-[80vh] flex flex-col shadow-2xl">
                            <div className="flex items-center justify-between p-4 border-b border-app-border">
                                <div className="flex items-center gap-3">
                                    <Code size={20} className="text-app-accent" />
                                    <h3 className="font-bold text-app-fg">PKGBUILD - {pkg.name}</h3>
                                </div>
                                <button
                                    onClick={() => setShowPkgbuild(false)}
                                    className="p-2 hover:bg-app-fg/10 rounded-lg transition-colors"
                                >
                                    <X size={20} className="text-app-muted" />
                                </button>
                            </div>
                            <div className="flex-1 overflow-auto p-4">
                                {pkgbuildError ? (
                                    <div className="flex flex-col items-center justify-center py-12 text-center">
                                        <AlertTriangle size={48} className="text-amber-500 mb-4" />
                                        <p className="text-app-fg font-bold">Failed to load PKGBUILD</p>
                                        <p className="text-app-muted text-sm mt-2">{pkgbuildError}</p>
                                    </div>
                                ) : (
                                    <pre className="text-xs font-mono text-app-fg bg-app-bg p-4 rounded-lg overflow-x-auto whitespace-pre-wrap">
                                        {pkgbuildContent}
                                    </pre>
                                )}
                            </div>
                            <div className="p-4 border-t border-app-border flex justify-between items-center">
                                <p className="text-xs text-app-muted">
                                    Review the build script carefully before installing. Look for suspicious commands like curl piped to bash.
                                </p>
                                <button
                                    onClick={() => setShowPkgbuild(false)}
                                    className="px-4 py-2 bg-app-accent text-white rounded-lg font-medium hover:opacity-90 transition-all"
                                >
                                    Close
                                </button>
                            </div>
                        </div>
                    </div>
                )}

                {/* AUR Install Confirmation Modal */}
                {showInstallConfirm && (
                    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4">
                        <div className="bg-app-card border border-app-border rounded-2xl w-full max-w-md shadow-2xl">
                            <div className="p-6">
                                <div className="flex items-center gap-3 mb-4">
                                    <div className="p-3 bg-amber-500/20 rounded-xl">
                                        <AlertTriangle size={24} className="text-amber-500" />
                                    </div>
                                    <div>
                                        <h3 className="font-bold text-app-fg text-lg">Install from AUR?</h3>
                                        <p className="text-app-muted text-sm">Unreviewed community package</p>
                                    </div>
                                </div>

                                <div className="bg-app-bg rounded-xl p-4 mb-4 space-y-2">
                                    <div className="flex justify-between text-sm">
                                        <span className="text-app-muted">Package:</span>
                                        <span className="text-app-fg font-medium">{pkg.name}</span>
                                    </div>
                                    <div className="flex justify-between text-sm">
                                        <span className="text-app-muted">Version:</span>
                                        <span className="text-app-fg font-medium">{variants.find(v => v.source === 'aur')?.version || pkg.version}</span>
                                    </div>
                                    <div className="flex justify-between text-sm">
                                        <span className="text-app-muted">Source:</span>
                                        <span className="text-amber-500 font-medium">Arch User Repository</span>
                                    </div>
                                </div>

                                <p className="text-xs text-app-muted mb-4">
                                    AUR packages are user-submitted and not officially reviewed. They may contain malicious code.
                                    We recommend viewing the PKGBUILD first.
                                </p>

                                <div className="flex gap-3">
                                    <button
                                        onClick={() => setShowInstallConfirm(false)}
                                        className="flex-1 px-4 py-3 rounded-xl border border-app-border bg-app-card/50 hover:bg-app-card text-app-fg font-medium transition-all"
                                    >
                                        Cancel
                                    </button>
                                    <button
                                        onClick={() => {
                                            trackEvent('install_clicked', { package: pkg.name, source: 'aur' });
                                            setShowInstallConfirm(false);
                                            setShowInstallMonitor(true);
                                        }}
                                        className="flex-1 px-4 py-3 rounded-xl bg-amber-500 text-white font-bold shadow-lg shadow-amber-900/20 active:scale-95 transition-all"
                                    >
                                        Install Now
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                )}

                {/* Repo Setup Modal (Fallback) */}
                <RepoSetupModal
                    repoName={missingRepoId.charAt(0).toUpperCase() + missingRepoId.slice(1)}
                    repoId={missingRepoId}
                    isOpen={showRepoSetup}
                    onClose={() => setShowRepoSetup(false)}
                    onSuccess={() => {
                        setShowRepoSetup(false);
                        // Re-trigger install after success
                        handleInstallClick();
                    }}
                />
            </div>
        </div>
    );
}
