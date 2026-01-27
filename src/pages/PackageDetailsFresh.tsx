import { useState, useEffect, useRef } from 'react';
import {
    ArrowLeft, Download, Play, Heart, Star, Code, X,
    AlertTriangle, Trash2, User, Globe, Calendar,
    Package as PackageIcon, ChevronRight,
    Loader2, ShieldCheck, MessageSquare, Cpu, ChevronDown
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import RepoSelector from '../components/RepoSelector';
import { Package } from '../components/PackageCard';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { clsx } from 'clsx';
import { resolveIconUrl } from '../utils/iconHelper';
import { useFavorites } from '../hooks/useFavorites';
import { submitReview } from '../services/reviewService';
import { trackEvent } from '@aptabase/tauri';
import { useToast } from '../context/ToastContext';
import { usePackageReviews } from '../hooks/useRatings';
import { usePackageMetadata, AppMetadata, metadataCache } from '../hooks/usePackageMetadata';

// --- Types ---
interface PackageDetailsProps {
    pkg: Package;
    onBack: () => void;
    preferredSource?: string;
    onInstall: (p: { name: string; source: string; repoName?: string }) => void;
    onUninstall: (p: { name: string; source: string; repoName?: string }) => void;
}

interface PackageVariant {
    source: 'chaotic' | 'aur' | 'official' | 'cachyos' | 'garuda' | 'endeavour' | 'manjaro';
    version: string;
    repo_name?: string;
    pkg_name?: string;
}

interface InstallStatus {
    installed: boolean;
    version?: string;
    repo?: string;
    source?: string;
    actual_package_name?: string;
}

export function prewarmMetadataCache(pkgName: string, meta: AppMetadata) {
    metadataCache.set(pkgName, { data: meta, timestamp: Date.now() });
}

// --- Helper Components ---
const Badge = ({ children, className }: { children: React.ReactNode, className?: string }) => (
    <span className={clsx("px-2.5 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider border", className)}>
        {children}
    </span>
);

const SourceBadge = ({ source }: { source: string }) => {
    const s = source.toLowerCase();
    const style =
        s === 'official' ? "bg-blue-500/10 text-blue-400 border-blue-500/20" :
            s === 'chaotic' ? "bg-purple-500/10 text-purple-400 border-purple-500/20" :
                s === 'aur' ? "bg-amber-500/10 text-amber-400 border-amber-500/20" :
                    "bg-zinc-500/10 text-zinc-400 border-zinc-500/20";

    return <Badge className={style}>{source}</Badge>;
};

// --- Main Component ---

export default function PackageDetails({ pkg, onBack, preferredSource, onInstall, onUninstall }: PackageDetailsProps) {
    // --- State & Hooks ---
    const { metadata: fullMeta } = usePackageMetadata(pkg.name);
    const { success, error } = useToast();

    const lookupId = pkg.app_id || fullMeta?.app_id || pkg.name;
    const { reviews, summary: rating, refresh: refreshReviews } = usePackageReviews(pkg.name, lookupId);

    const [variants, setVariants] = useState<PackageVariant[]>([]);
    const [selectedSource, setSelectedSource] = useState<string>(pkg.source);

    const [showReviewForm, setShowReviewForm] = useState(false);
    const [reviewTitle, setReviewTitle] = useState('');
    const [reviewBody, setReviewBody] = useState('');
    const [reviewRating, setReviewRating] = useState(5);

    // Pagination for reviews
    const [visibleReviewsCount, setVisibleReviewsCount] = useState(5);

    // Install/Status Logic
    const [installStatus, setInstallStatus] = useState<InstallStatus | null>(null);
    const [installedVariant, setInstalledVariant] = useState<InstallStatus | null>(null);
    const checkRequestId = useRef(0);

    // PKGBUILD Viewing
    const [showPkgbuild, setShowPkgbuild] = useState(false);
    const [pkgbuildContent, setPkgbuildContent] = useState<string | null>(null);
    const [pkgbuildLoading, setPkgbuildLoading] = useState(false);
    const [pkgbuildError, setPkgbuildError] = useState<string | null>(null);

    // Lightbox
    const [lightboxIndex, setLightboxIndex] = useState<number | null>(null);

    const { isFavorite, toggleFavorite } = useFavorites();
    const isFav = isFavorite(pkg.name);

    // --- Effects ---

    // 1. Fetch Variants & Initial Selection
    useEffect(() => {
        invoke<PackageVariant[]>('get_package_variants', { pkgName: pkg.name })
            .then(async (fetchedVars) => {
                const propAlternatives = (pkg.alternatives || []).map(a => ({
                    source: a.source,
                    version: a.version,
                    pkg_name: a.name,
                    repo_name: a.source === 'chaotic' ? 'chaotic-aur' : undefined
                } as PackageVariant));

                const combined = [...fetchedVars, ...propAlternatives];
                // Deduplicate
                const vars = combined.filter((v, index, self) =>
                    index === self.findIndex((t) => (
                        t.source === v.source && t.version === v.version && t.pkg_name === v.pkg_name
                    ))
                );
                setVariants(vars);

                // Check installed status to auto-select
                try {
                    const res = await invoke<InstallStatus>('check_installed_status', { name: pkg.name });
                    if (res.installed) {
                        setInstallStatus(res);
                        setInstalledVariant(res);
                        if (res.source) {
                            setSelectedSource(res.source);
                            return;
                        } else if (vars.some(v => v.source === 'aur')) {
                            setSelectedSource('aur'); // Default to AUR if source ambiguous but AUR present
                            return;
                        }
                    }
                } catch (e) { console.error(e); }

                // Fallback selection logic
                if (preferredSource && vars.some(v => v.source === preferredSource)) setSelectedSource(preferredSource);
                else if (vars.some(v => v.source === pkg.source)) setSelectedSource(pkg.source);
                else if (vars.some(v => v.source === 'chaotic')) setSelectedSource('chaotic');
                else if (vars.some(v => v.source === 'official')) setSelectedSource('official');
                else if (vars.length > 0) setSelectedSource(vars[0].source);
            });
    }, [pkg.name, preferredSource]);

    // 2. Status Checking Routine
    const checkStatus = (customName?: string) => {
        const reqId = ++checkRequestId.current;
        const nameToCheck = customName || installedVariant?.actual_package_name || installStatus?.actual_package_name || variants.find(v => v.source === selectedSource)?.pkg_name || pkg.name;

        invoke<InstallStatus>('check_installed_status', { name: nameToCheck })
            .then(res => {
                if (reqId !== checkRequestId.current) return;
                setInstallStatus(res);
                if (res.installed) setInstalledVariant(res);
            })
            .catch(console.error);
    };

    useEffect(() => {
        checkStatus();
        const unlisten = listen('install-complete', () => checkStatus());
        return () => { unlisten.then((f: UnlistenFn) => f()); };
    }, [pkg.name, selectedSource, variants]);


    // --- Actions ---

    const handleInstallClick = () => {
        onInstall({
            name: variants.find(v => v.source === selectedSource)?.pkg_name || pkg.name,
            source: selectedSource,
            repoName: variants.find(v => v.source === selectedSource)?.repo_name
        });
    };

    const handleLaunch = async () => {
        const nameToLaunch = installedVariant?.actual_package_name || installStatus?.actual_package_name || variants.find(v => v.source === selectedSource)?.pkg_name || pkg.name;
        try {
            await invoke('launch_app', { pkgName: nameToLaunch });
            success("App launched");
        } catch (e) { error("Could not launch app: " + String(e)); }
    };

    const handleReviewSubmit = async () => {
        try {
            await submitReview(pkg.name, reviewRating, reviewTitle + "\n\n" + reviewBody, "MonArch User");
            setShowReviewForm(false);
            setReviewTitle(''); setReviewBody('');
            refreshReviews();
            trackEvent('review_submitted', { package: pkg.name, rating: reviewRating });
            success("Review submitted!");
        } catch (e) { error("Failed to submit: " + String(e)); }
    };

    const fetchPkgbuild = async () => {
        setPkgbuildLoading(true);
        setPkgbuildError(null);
        try {
            const content = await invoke<string>('fetch_pkgbuild', { pkgName: pkg.name });
            setPkgbuildContent(content);
            setShowPkgbuild(true);
        } catch (e) {
            setPkgbuildError(String(e));
            setShowPkgbuild(true);
        } finally { setPkgbuildLoading(false); }
    };

    // --- Computed ---
    const isConflict = installedVariant?.installed && (
        !installStatus?.installed || installedVariant.source?.toLowerCase() !== selectedSource.toLowerCase()
    );

    const screenshots = (fullMeta?.screenshots && fullMeta.screenshots.length > 0)
        ? fullMeta.screenshots
        : (pkg.screenshots && pkg.screenshots.length > 0) ? pkg.screenshots : [];

    const displayedReviews = reviews.slice(0, visibleReviewsCount);
    const hasMoreReviews = reviews.length > visibleReviewsCount;

    return (
        <motion.div
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            className="h-full flex flex-col bg-app-bg text-app-fg overflow-y-auto"
        >
            {/* --- HERO SECTION --- */}
            <div className="relative min-h-[250px] lg:min-h-[300px] flex items-end">
                {/* Background Gradient / Image */}
                <div className="absolute inset-0 z-0 overflow-hidden">
                    <div className="absolute inset-0 bg-gradient-to-b from-blue-900/40 to-app-bg z-10" />
                    <div className="absolute inset-0 bg-[url('/grid.svg')] opacity-10" />
                    {screenshots.length > 0 && (
                        <div className="absolute inset-0 blur-3xl opacity-30 scale-110">
                            <img src={screenshots[0]} alt="" className="w-full h-full object-cover" />
                        </div>
                    )}
                </div>

                {/* Back Button */}
                <button
                    onClick={onBack}
                    className="absolute top-6 left-6 z-50 p-3 rounded-full bg-black/20 hover:bg-black/40 backdrop-blur-md text-white transition-all border border-white/10"
                >
                    <ArrowLeft size={24} />
                </button>

                {/* Content Container */}
                <div className="relative z-20 w-full max-w-7xl mx-auto px-6 pt-24 pb-8 lg:pb-12 flex flex-row items-end gap-8">

                    {/* Icon Card */}
                    <motion.div
                        initial={{ scale: 0.9, opacity: 0 }}
                        animate={{ scale: 1, opacity: 1 }}
                        transition={{ delay: 0.1 }}
                        className="w-32 h-32 lg:w-48 lg:h-48 rounded-4xl bg-app-card shadow-2xl shadow-black/50 border border-white/10 flex items-center justify-center p-6 shrink-0 backdrop-blur-xl"
                    >
                        {(pkg.icon || fullMeta?.icon_url) ? (
                            <img src={resolveIconUrl(pkg.icon || fullMeta?.icon_url)} alt={pkg.name} className="w-full h-full object-contain filter drop-shadow-xl" />
                        ) : (
                            <PackageIcon size={80} className="text-white/20" />
                        )}
                    </motion.div>

                    {/* Text & Actions */}
                    <div className="flex-1 min-w-0 mb-1">
                        <motion.h1
                            initial={{ y: 20, opacity: 0 }}
                            animate={{ y: 0, opacity: 1 }}
                            transition={{ delay: 0.2 }}
                            className="text-4xl lg:text-6xl font-black text-white tracking-tight leading-none mb-3 drop-shadow-2xl"
                        >
                            {pkg.display_name || fullMeta?.name || pkg.name}
                        </motion.h1>

                        <div className="flex flex-wrap items-center gap-4 mb-6 text-app-muted/80 font-medium">
                            {/* Restored Source Badge */}
                            <SourceBadge source={selectedSource} />

                            <div className="px-3 py-1 rounded-full bg-white/5 border border-white/10 text-sm flex items-center gap-2 text-white/80">
                                <Cpu size={14} /> <span>v{variants.find(v => v.source === selectedSource)?.version || pkg.version}</span>
                            </div>
                            <div className="px-3 py-1 rounded-full bg-white/5 border border-white/10 text-sm flex items-center gap-2 text-white/80">
                                <MessageSquare size={14} /> <span>{reviews.length} Reviews</span>
                            </div>
                            {pkg.out_of_date && <span className="text-amber-400 flex items-center gap-1 font-bold"><AlertTriangle size={14} /> Outdated</span>}
                        </div>

                        {/* WARNINGS BLOCK - Prominent */}
                        <div className="space-y-2 mb-6 max-w-2xl">
                            {selectedSource === 'aur' && (
                                <div className="flex items-start gap-4 p-4 rounded-xl bg-amber-500/10 border border-amber-500/20 backdrop-blur-sm">
                                    <div className="p-2 bg-amber-500/20 rounded-lg text-amber-500"><AlertTriangle size={20} /></div>
                                    <div>
                                        <h4 className="text-amber-500 font-bold text-sm">Community Package (AUR)</h4>
                                        <p className="text-amber-200/60 text-xs mt-1">
                                            This package comes from the Arch User Repository. It is not officially reviewed.
                                            Verify validity before installing.
                                        </p>
                                    </div>
                                </div>
                            )}
                            {isConflict && (
                                <div className="flex items-start gap-4 p-4 rounded-xl bg-red-500/10 border border-red-500/20 backdrop-blur-sm">
                                    <div className="p-2 bg-red-500/20 rounded-lg text-red-500"><AlertTriangle size={20} /></div>
                                    <div>
                                        <h4 className="text-red-500 font-bold text-sm">Dependency Conflict</h4>
                                        <p className="text-red-200/60 text-xs mt-1">
                                            You have the <b>{installedVariant?.source}</b> version active. Switch sources or uninstall first.
                                        </p>
                                    </div>
                                </div>
                            )}
                        </div>

                        {/* ACTION ROW */}
                        <motion.div
                            initial={{ y: 20, opacity: 0 }}
                            animate={{ y: 0, opacity: 1 }}
                            transition={{ delay: 0.3 }}
                            className="flex flex-wrap items-center gap-3"
                        >
                            {/* Variant Selector - MOVED UP */}
                            {variants.length > 1 && (
                                <div className="relative z-50 mr-2">
                                    <RepoSelector
                                        variants={variants}
                                        selectedSource={selectedSource}
                                        onChange={(s) => setSelectedSource(s as any)}
                                    />
                                </div>
                            )}

                            {installedVariant?.installed ? (
                                <>
                                    <button
                                        onClick={handleLaunch}
                                        className="h-14 px-8 bg-emerald-500 hover:bg-emerald-400 text-white rounded-2xl font-bold shadow-xl shadow-emerald-500/20 active:scale-95 transition-all flex items-center gap-3 text-lg"
                                    >
                                        <Play size={24} fill="currentColor" /> Launch
                                    </button>
                                    <button
                                        onClick={() => onUninstall({
                                            name: installedVariant?.actual_package_name || pkg.name,
                                            source: installedVariant?.source || 'official'
                                        })}
                                        className="h-14 px-6 bg-white/5 hover:bg-white/10 text-red-400 border border-white/10 rounded-2xl font-bold active:scale-95 transition-all flex items-center gap-2"
                                    >
                                        <Trash2 size={20} /> Uninstall
                                    </button>
                                </>
                            ) : (
                                <button
                                    onClick={handleInstallClick}
                                    className="h-14 px-10 bg-blue-600 hover:bg-blue-500 text-white rounded-2xl font-bold shadow-xl shadow-blue-600/20 active:scale-95 transition-all flex items-center gap-3 text-lg"
                                >
                                    <Download size={24} /> Install
                                </button>
                            )}

                            <button
                                onClick={() => toggleFavorite(pkg.name)}
                                className={clsx(
                                    "h-14 w-14 rounded-2xl border flex items-center justify-center transition-colors active:scale-95",
                                    isFav ? "bg-red-500/20 border-red-500/50 text-red-500" : "bg-white/5 border-white/10 text-white/50 hover:bg-white/10 hover:text-white"
                                )}
                            >
                                <Heart size={24} className={isFav ? "fill-current" : ""} />
                            </button>

                            {selectedSource === 'aur' && (
                                <button onClick={fetchPkgbuild} className="h-14 w-14 rounded-2xl border border-white/10 bg-white/5 flex items-center justify-center text-white/50 hover:text-white hover:bg-white/10 transition-colors" title="View PKGBUILD">
                                    {pkgbuildLoading ? <Loader2 size={24} className="animate-spin" /> : <Code size={24} />}
                                </button>
                            )}
                        </motion.div>
                    </div>
                </div>
            </div>

            {/* --- MAIN CONTENT GRID --- */}
            <div className="flex-1 bg-app-bg">
                <div className="max-w-7xl mx-auto p-6 lg:p-10 grid grid-cols-12 gap-8 lg:gap-16">

                    {/* LEFT COLUMN (Details) */}
                    <div className="col-span-8 space-y-12">
                        {/* SCREENSHOTS GALLERY */}
                        {screenshots.length > 0 && (
                            <section>
                                <h3 className="text-xl font-bold text-white mb-6 flex items-center gap-2">
                                    <Globe size={24} className="text-blue-500" /> Preview
                                </h3>
                                <div className="flex gap-4 overflow-x-auto pb-6 snap-x scrollbar-thin scrollbar-thumb-white/10 scrollbar-track-transparent">
                                    {screenshots.map((url, i) => (
                                        <motion.div
                                            key={i}
                                            whileHover={{ scale: 1.02 }}
                                            onClick={() => setLightboxIndex(i)}
                                            className="shrink-0 w-[400px] aspect-video rounded-2xl overflow-hidden bg-black/20 border border-white/10 cursor-pointer snap-center shadow-xl"
                                        >
                                            <img
                                                src={url}
                                                alt="Screenshot"
                                                className="w-full h-full object-cover"
                                                loading="lazy"
                                                onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }}
                                            />
                                        </motion.div>
                                    ))}
                                </div>
                            </section>
                        )}

                        {/* DESCRIPTION */}
                        <section>
                            <h3 className="text-xl font-bold text-white mb-6">About this App</h3>
                            <div className="bg-app-card/30 rounded-3xl p-8 border border-white/5 leading-loose">
                                <div
                                    className="prose prose-invert prose-lg prose-blue max-w-none text-app-muted/90 font-light"
                                    dangerouslySetInnerHTML={{ __html: fullMeta?.description || pkg.description || "No description available." }}
                                />
                                {pkg.keywords && (
                                    <div className="flex flex-wrap gap-2 mt-8 pt-6 border-t border-white/5">
                                        {pkg.keywords.map(k => (
                                            <span key={k} className="px-3 py-1 bg-white/5 rounded-lg text-xs font-mono text-blue-300 border border-blue-500/20">#{k}</span>
                                        ))}
                                    </div>
                                )}
                            </div>
                        </section>

                        {/* REVIEWS TAB */}
                        <section>
                            <div className="flex items-center justify-between mb-8">
                                <h3 className="text-xl font-bold text-white">User Reviews ({reviews.length})</h3>
                                <button onClick={() => setShowReviewForm(true)} className="px-4 py-2 bg-blue-600/10 text-blue-400 rounded-lg hover:bg-blue-600/20 font-bold transition-colors">Write a Review</button>
                            </div>

                            {/* Review Form */}
                            {showReviewForm && (
                                <div className="bg-app-card p-6 rounded-2xl border border-blue-500/30 mb-8 animate-in slide-in-from-top-4">
                                    <h4 className="font-bold text-white mb-4">Write your review</h4>
                                    <div className="flex gap-2 mb-4">
                                        {[1, 2, 3, 4, 5].map(s => <Star key={s} size={28} fill={s <= reviewRating ? "#fbbf24" : "none"} className={s <= reviewRating ? "text-amber-400" : "text-zinc-600 stroke-2"} onClick={() => setReviewRating(s)} />)}
                                    </div>
                                    <input value={reviewTitle} onChange={e => setReviewTitle(e.target.value)} placeholder="Title (e.g. Works great!)" className="w-full bg-black/20 border border-white/10 rounded-xl p-4 mb-3 text-white focus:border-blue-500 outline-none transition-colors" />
                                    <textarea value={reviewBody} onChange={e => setReviewBody(e.target.value)} placeholder="Share your experience..." className="w-full bg-black/20 border border-white/10 rounded-xl p-4 mb-3 text-white focus:border-blue-500 outline-none transition-colors" rows={4} />
                                    <div className="flex justify-end gap-3">
                                        <button onClick={() => setShowReviewForm(false)} className="px-6 py-2 text-zinc-400 hover:text-white transition-colors">Cancel</button>
                                        <button onClick={handleReviewSubmit} className="px-8 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-xl font-bold shadow-lg shadow-blue-600/20">Submit</button>
                                    </div>
                                </div>
                            )}

                            {/* Review List - PAGINATED */}
                            <div className="space-y-4">
                                {reviews.length === 0 ? (
                                    <div className="p-8 text-center bg-white/5 rounded-2xl border border-white/5 text-app-muted">
                                        No reviews yet. Be the first to share your thoughts!
                                    </div>
                                ) : (
                                    displayedReviews.map((review, idx) => (
                                        <div key={idx} className="p-6 bg-app-card rounded-2xl border border-white/5 hover:border-white/10 transition-colors">
                                            <div className="flex justify-between items-start mb-2">
                                                <div className="flex items-center gap-2">
                                                    <div className="w-8 h-8 rounded-full bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center text-xs font-bold text-white">
                                                        {review.userName.charAt(0)}
                                                    </div>
                                                    <span className="font-bold text-white">{review.userName}</span>
                                                    <span className="text-xs text-app-muted">• {review.date ? new Date(review.date).toLocaleDateString() : 'Unknown Date'}</span>
                                                </div>
                                                <div className="flex gap-0.5">
                                                    {[1, 2, 3, 4, 5].map(s => <Star key={s} size={14} fill={s <= review.rating ? "#fbbf24" : "none"} className={s <= review.rating ? "text-amber-400" : "text-zinc-700"} />)}
                                                </div>
                                            </div>
                                            {/* We don't have a distinct separate title field in the interface unless we parse it. For now, showing comment. */}
                                            <p className="text-app-muted text-sm leading-relaxed whitespace-pre-line mt-2">{review.comment}</p>
                                        </div>
                                    ))
                                )}

                                {/* Show More Button */}
                                {hasMoreReviews && (
                                    <div className="pt-4 flex justify-center">
                                        <button
                                            onClick={() => setVisibleReviewsCount(prev => prev + 5)}
                                            className="px-6 py-3 rounded-xl bg-white/5 border border-white/10 hover:bg-white/10 text-white font-medium flex items-center gap-2 transition-colors"
                                        >
                                            Show More Reviews <ChevronDown size={16} />
                                        </button>
                                    </div>
                                )}
                            </div>
                        </section>
                    </div>

                    {/* RIGHT COLUMN (Sidebar) */}
                    <div className="col-span-4 space-y-6">

                        {/* Ratings Card */}
                        <div className="bg-gradient-to-br from-yellow-500/10 to-transparent rounded-3xl p-8 border border-yellow-500/20 text-center">
                            <h4 className="text-sm font-bold text-yellow-500 uppercase tracking-wider mb-2">Community Rating</h4>
                            <div className="flex items-center justify-center gap-3">
                                <span className="text-6xl font-black text-white">{rating?.average.toFixed(1) || "0.0"}</span>
                                <div className="flex flex-col items-start">
                                    <div className="flex gap-1 mb-1">
                                        {[1, 2, 3, 4, 5].map(s => <Star key={s} size={16} fill={s <= Math.round(rating?.average || 0) ? "#EAB308" : "none"} className="text-yellow-500" />)}
                                    </div>
                                    <span className="text-xs text-white/50">{rating?.count || 0} reviews</span>
                                </div>
                            </div>
                        </div>

                        {/* Metadata Grid */}
                        <div className="bg-app-card rounded-3xl p-2 border border-white/5 overflow-hidden">
                            <div className="grid grid-cols-1 divide-y divide-white/5">
                                <div className="p-5 flex items-start justify-between gap-4 hover:bg-white/5 transition-colors">
                                    <span className="text-sm text-app-muted flex items-center gap-3 shrink-0"><User size={18} className="text-blue-500" /> Maintainer</span>
                                    <span className="text-sm text-white font-medium text-right break-words">{fullMeta?.maintainer || pkg.maintainer || "Community"}</span>
                                </div>
                                <div className="p-5 flex items-start justify-between gap-4 hover:bg-white/5 transition-colors">
                                    <span className="text-sm text-app-muted flex items-center gap-3 shrink-0"><ShieldCheck size={18} className="text-emerald-500" /> License</span>
                                    <span className="text-sm text-white font-medium text-right break-words">{fullMeta?.license || pkg.license || "Unknown"}</span>
                                </div>
                                <div className="p-5 flex items-start justify-between gap-4 hover:bg-white/5 transition-colors">
                                    <span className="text-sm text-app-muted flex items-center gap-3 shrink-0"><Calendar size={18} className="text-purple-500" /> Updated</span>
                                    <span className="text-sm text-white font-medium text-right">
                                        {fullMeta?.last_updated ? new Date(fullMeta.last_updated * 1000).toLocaleDateString() : 'Unknown'}
                                    </span>
                                </div>
                                {pkg.url && (
                                    <a href={pkg.url} target="_blank" className="p-5 flex items-center justify-between gap-4 hover:bg-white/10 transition-colors group">
                                        <span className="text-sm text-blue-400 flex items-center gap-3 shrink-0"><Globe size={18} /> Website</span>
                                        <ChevronRight size={18} className="text-white/30 group-hover:text-white transition-colors ml-auto" />
                                    </a>
                                )}
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            {/* LIGHTBOX PORTAL */}
            <AnimatePresence>
                {lightboxIndex !== null && (
                    <motion.div
                        initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
                        onClick={() => setLightboxIndex(null)}
                        className="fixed inset-0 z-[100] bg-black/95 backdrop-blur-xl flex items-center justify-center p-4"
                    >
                        <button className="absolute top-6 right-6 p-4 text-white/50 hover:text-white"><X size={32} /></button>
                        <img
                            src={screenshots[lightboxIndex]}
                            className="max-h-[90vh] max-w-[90vw] rounded-lg shadow-2xl"
                            onClick={e => e.stopPropagation()}
                        />
                    </motion.div>
                )}
            </AnimatePresence>

            {/* PKGBUILD Modal */}
            <AnimatePresence>
                {showPkgbuild && (
                    <motion.div
                        initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
                        className="fixed inset-0 z-[120] bg-black/80 backdrop-blur-sm flex items-center justify-center p-4"
                        onClick={() => setShowPkgbuild(false)}
                    >
                        <motion.div
                            initial={{ scale: 0.9 }} animate={{ scale: 1 }} exit={{ scale: 0.9 }}
                            onClick={e => e.stopPropagation()}
                            className="bg-app-card w-full max-w-4xl h-[80vh] rounded-2xl border border-white/10 flex flex-col overflow-hidden shadow-2xl"
                        >
                            <div className="p-4 border-b border-white/10 flex justify-between items-center bg-white/5">
                                <h3 className="font-bold text-white flex items-center gap-2"><Code size={20} className="text-blue-400" /> PKGBUILD Preview</h3>
                                <button onClick={() => setShowPkgbuild(false)}><X size={24} className="text-white/50 hover:text-white" /></button>
                            </div>
                            <div className="flex-1 overflow-auto p-4 bg-[#1e1e1e]">
                                {pkgbuildLoading ? (
                                    <div className="h-full flex flex-col items-center justify-center text-white/50 gap-4">
                                        <Loader2 size={40} className="animate-spin text-blue-500" />
                                        <p>Fetching PKGBUILD...</p>
                                    </div>
                                ) : pkgbuildError ? (
                                    <div className="h-full flex flex-col items-center justify-center text-red-400 gap-4 p-8 text-center">
                                        <AlertTriangle size={40} />
                                        <p>{pkgbuildError}</p>
                                    </div>
                                ) : (
                                    <pre className="font-mono text-sm text-gray-300 whitespace-pre-wrap">{pkgbuildContent}</pre>
                                )}
                            </div>
                        </motion.div>
                    </motion.div>
                )}
            </AnimatePresence>
        </motion.div>
    );
}
