import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import {
    ArrowLeft, Download, Play, Heart, Star, Code, X,
    AlertTriangle, Trash2, User, Globe, Calendar,
    ChevronRight, ChevronLeft, CheckCircle2,
    Loader2, ShieldCheck, MessageSquare, Cpu, ChevronDown, RefreshCw,
    Lock, Zap, Info, Layers, Terminal, ExternalLink, Activity,
    Box, FileCode, Check, AlertCircle, Share2, StarHalf
} from 'lucide-react';
import { motion, AnimatePresence, LayoutGroup } from 'framer-motion';
import DOMPurify from 'dompurify';
import RepoSelector from '../components/RepoSelector';
import RepoBadge from '../components/RepoBadge';
import { PackageSource } from '../services/bindings';
import { commands, Package as BackendPackage, PackageInstallStatus as BackendInstallStatus } from '../services/bindings';
import { unwrap } from '../utils/specta';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { clsx } from 'clsx';
import { resolveIconUrl, resolveImageUrl } from '../utils/iconHelper';
import { getSourceTierForSort, isSameSource } from '../utils/repoHelper';
import { useFavorites } from '../hooks/useFavorites';
import { useToast } from '../context/ToastContext';
import { useErrorService } from '../context/ErrorContext';
import archLogo from '../assets/arch-logo.png';
import { getPackageReviews, RatingSummary, Review } from '../services/reviewService';
import { useDistro } from '../hooks/useDistro';
import { useChaoticStatus } from '../hooks/useChaoticStatus';
import { useSettings } from '../hooks/useSettings';
import { useEscapeKey } from '../hooks/useEscapeKey';
import { useFocusTrap } from '../hooks/useFocusTrap';
import { useAppStore } from '../store/internal_store';

// --- Types ---
interface PackageVariant {
    source: PackageSource | string;
    version: string;
    repo_name?: string;
    pkg_name?: string;
    match_source?: PackageSource | string;
}

interface PackageDetailsProps {
    pkg?: BackendPackage;
    onBack: () => void;
    preferredSource?: PackageSource | string;
    installInProgress?: boolean;
    activeInstallPackage?: { name: string; mode: 'install' | 'uninstall' } | null;
    onInstall: (p: { name: string; source: PackageSource | string; repoName?: string; displayName?: string }) => void;
    onUninstall: (p: { name: string; source: PackageSource | string; repoName?: string; displayName?: string }) => void;
    onOpenSettings?: () => void;
}

// --- Helpers ---

function DescriptionBlock({ html, keywords }: { html: string; keywords?: string[] }) {
    const contentRef = useRef<HTMLDivElement>(null);
    const [isExpanded, setIsExpanded] = useState(false);
    const [isOverflowing, setIsOverflowing] = useState(false);
    const COLLAPSED_HEIGHT = 180;

    useEffect(() => {
        if (contentRef.current) {
            setIsOverflowing(contentRef.current.scrollHeight > COLLAPSED_HEIGHT + 20);
        }
    }, [html]);

    return (
        <div className="bg-app-card/30 rounded-[2.5rem] p-10 border border-app-border leading-loose relative">
            <div
                ref={contentRef}
                className="prose prose-sm md:prose-base prose-invert max-w-none text-app-muted overflow-hidden transition-[max-height] duration-300"
                style={{ maxHeight: isExpanded || !isOverflowing ? 'none' : `${COLLAPSED_HEIGHT}px` }}
                dangerouslySetInnerHTML={{ __html: html }}
            />
            {isOverflowing && !isExpanded && (
                <div className="absolute bottom-14 left-0 right-0 h-16 bg-gradient-to-t from-app-card/80 to-transparent pointer-events-none rounded-b-3xl" />
            )}
            {isOverflowing && (
                <button
                    onClick={() => setIsExpanded(!isExpanded)}
                    className="mt-3 text-sm font-semibold text-blue-500 hover:text-blue-400 transition-colors flex items-center gap-1 group"
                >
                    {isExpanded ? 'Show less' : 'Show more'}
                    <ChevronDown size={16} className={clsx("transition-transform group-hover:translate-y-0.5", isExpanded && "rotate-180")} />
                </button>
            )}
            {keywords && keywords.length > 0 && (
                <div className="flex flex-wrap gap-2 mt-8 pt-6 border-t border-app-border">
                    {keywords.map(k => (
                        <span key={k} className="px-3 py-1 bg-blue-500/10 rounded-lg text-xs font-mono text-blue-400 border border-blue-500/20">#{k}</span>
                    ))}
                </div>
            )}
        </div>
    );
}

function ImageWithFallback({ src, alt, className }: { src: string; alt: string; className?: string }) {
    const [displaySrc, setDisplaySrc] = useState(src);
    const [isTransitioning, setIsTransitioning] = useState(false);
    const [hasError, setHasError] = useState(false);

    useEffect(() => {
        if (!src) return;
        if (src === displaySrc) return;

        setIsTransitioning(true);
        const img = new Image();
        img.src = src;
        img.onload = () => {
            setDisplaySrc(src);
            setIsTransitioning(false);
            setHasError(false);
        };
        img.onerror = () => {
            setDisplaySrc(archLogo);
            setIsTransitioning(false);
            setHasError(true);
        };
    }, [src, displaySrc]);

    return (
        <div className={clsx("relative overflow-hidden", className)}>
            <img
                src={displaySrc}
                alt={alt}
                className={clsx(
                    "w-full h-full object-contain transition-all duration-500",
                    isTransitioning ? 'opacity-0 scale-95' : 'opacity-100 scale-100',
                    hasError && "p-4 animate-pulse opacity-50"
                )}
            />
        </div>
    );
}

function SecurityNotice({ source }: { source: string }) {
    if (source === 'flatpak') {
        return (
            <div className="p-4 bg-emerald-500/5 rounded-2xl border border-emerald-500/10 space-y-2">
                <div className="flex items-center gap-2 text-emerald-500">
                    <Lock size={16} />
                    <span className="text-[11px] font-black uppercase tracking-widest">Sandboxed</span>
                </div>
                <p className="text-[10px] text-app-muted leading-relaxed">
                    This application runs in a restricted environment (sandbox) with specific permissions.
                </p>
            </div>
        );
    }

    return (
        <div className="p-4 bg-amber-500/5 rounded-2xl border border-amber-500/10 space-y-2">
            <div className="flex items-center gap-2 text-amber-500">
                <AlertCircle size={16} />
                <span className="text-[11px] font-black uppercase tracking-widest">System Access</span>
            </div>
            <p className="text-[10px] text-app-muted leading-relaxed">
                This is a native package and has full access to your system resources. Ensure you trust the package maintainer.
            </p>
        </div>
    );
}

function FlatpakPermissions({ permissions }: { permissions: string[] }) {
    if (!permissions || permissions.length === 0) return null;

    const getPermissionIcon = (p: string) => {
        if (p.includes('network')) return <Globe size={13} className="text-blue-400" />;
        if (p.includes('filesystem')) return <Box size={13} className="text-amber-400" />;
        if (p.includes('device')) return <Cpu size={13} className="text-purple-400" />;
        if (p.includes('socket')) return <Terminal size={13} className="text-emerald-400" />;
        return <ShieldCheck size={13} className="text-slate-400" />;
    };

    return (
        <div className="space-y-3">
            <h4 className="text-[11px] font-black text-app-muted uppercase tracking-widest flex items-center gap-2">
                <Lock size={14} className="text-emerald-500" /> Permissions
            </h4>
            <div className="grid grid-cols-1 gap-2">
                {permissions.map((p, i) => (
                    <div key={i} className="flex items-center gap-2 px-3 py-1.5 bg-white/5 rounded-lg border border-white/5 group hover:border-white/10 transition-colors">
                        {getPermissionIcon(p)}
                        <span className="text-[10px] text-app-fg/70 font-mono truncate" title={p}>{p}</span>
                    </div>
                ))}
            </div>
        </div>
    );
}

function DependencyTree({ deps, type }: { deps: string[]; type: 'runtime' | 'build' }) {
    if (!deps || deps.length === 0) return null;

    const [isExpanded, setIsExpanded] = useState(false);

    return (
        <div className="space-y-3">
            <h4 className="text-[11px] font-black text-app-muted uppercase tracking-widest flex items-center gap-2">
                <Layers size={14} className={type === 'runtime' ? "text-blue-500" : "text-amber-500"} />
                {type === 'runtime' ? 'Runtime' : 'Build'} Dependencies
                <span className="ml-auto bg-white/5 px-2 py-0.5 rounded-md text-[9px] font-mono text-app-muted">{deps.length}</span>
            </h4>
            <div className={clsx(
                "flex flex-wrap gap-1.5 transition-all duration-300 overflow-hidden",
                !isExpanded && deps.length > 10 ? "max-h-[85px]" : "max-h-[1000px]"
            )}>
                {deps.map((d, i) => (
                    <div key={i} className="px-2 py-1 bg-white/5 rounded-md border border-white/5 text-[10px] font-mono text-app-fg/60 hover:bg-white/10 hover:border-white/20 transition-all cursor-default flex items-center gap-1.5">
                        <Box size={9} className="shrink-0 opacity-40" />
                        {d}
                    </div>
                ))}
            </div>
            {deps.length > 10 && (
                <button
                    onClick={() => setIsExpanded(!isExpanded)}
                    className="w-full py-1.5 text-[10px] font-bold text-app-muted hover:text-white transition-colors border-t border-white/5 mt-1"
                >
                    {isExpanded ? 'Collapse' : `Show all ${deps.length}`}
                </button>
            )}
        </div>
    );
}

export default function PackageDetails({ pkg: pkgProp, onBack, preferredSource, installInProgress = false, activeInstallPackage = null, onInstall, onUninstall }: PackageDetailsProps) {
    // Note: installInProgress is used to provide visual feedback for the current selection's action button.
    const containerRef = useRef<HTMLDivElement>(null);
    const activePackageId = useAppStore((s) => s.activePackageId);
    const packageRegistry = useAppStore((s) => s.packageRegistry);
    const registryPkg = activePackageId ? (packageRegistry[activePackageId] as any) : undefined;

    const pkg = useMemo(() => {
        const base = registryPkg ?? pkgProp;
        if (registryPkg && pkgProp) {
            const merged = { ...base };
            const baseName = base.display_name || base.name;
            const propName = pkgProp.display_name || pkgProp.name;
            if (propName && baseName && /[A-Z]/.test(propName) && !/[A-Z]/.test(baseName)) {
                merged.display_name = propName;
            }
            const baseIcon = base.icon || '';
            const propIcon = pkgProp.icon || '';
            if (propIcon.startsWith('http') && (!baseIcon || baseIcon.startsWith('/'))) {
                merged.icon = propIcon;
            }
            if (!base.rating && pkgProp.rating) {
                merged.rating = pkgProp.rating;
            }
            return merged as BackendPackage;
        }
        return (base || {}) as BackendPackage;
    }, [pkgProp, registryPkg, activePackageId]);

    const smartDisplayName = useMemo(() => {
        if (!pkg?.name) return '';
        const dn = pkg.display_name || pkg.name;
        if (dn === pkg.name && /^[a-z0-9\-\.]+$/.test(dn)) {
            return dn.split(/[-.]/).map(s => s.charAt(0).toUpperCase() + s.slice(1)).join(' ');
        }
        return dn;
    }, [pkg]);

    const upsertPackages = useAppStore((s) => s.upsertPackages);
    const fetchAttempted = useRef<string | null>(null);

    useEffect(() => {
        if (!pkg?.name) return;
        const pkgKey = pkg.name;
        if (fetchAttempted.current === pkgKey) return;

        commands.getMetadata(pkg.name)
            .then(res => {
                if (res.status === 'ok' && res.data) {
                    const meta = res.data;
                    fetchAttempted.current = pkgKey;

                    const currentDN = pkg.display_name || pkg.name;
                    const incomingDN = meta.name || currentDN;

                    const pickBetterName = (oldN: string, newN: string) => {
                        if (!newN) return oldN;
                        if (!oldN) return newN;
                        const oldHasUpper = /[A-Z]/.test(oldN);
                        const newHasUpper = /[A-Z]/.test(newN);
                        if (oldHasUpper && !newHasUpper) return oldN;
                        return newN;
                    };

                    const finalDisplayName = pickBetterName(currentDN, incomingDN);

                    // Type casting for metadata fields that might not exist yet but we want to use
                    const anyMeta = meta as any;
                    const enrichedPkg: BackendPackage = {
                        ...pkg,
                        display_name: finalDisplayName,
                        app_id: meta.app_id || pkg.app_id,
                        version: meta.version || pkg.version,
                        description: meta.summary || pkg.description,
                        long_description: meta.description || pkg.long_description,
                        icon: (meta.icon_url && !meta.icon_url.startsWith('/usr/')) ? meta.icon_url : pkg.icon,
                        screenshots: meta.screenshots && meta.screenshots.length > 0 ? meta.screenshots : pkg.screenshots,
                        maintainer: meta.maintainer || pkg.maintainer,
                        license: meta.license ? [meta.license] : pkg.license,
                        available_sources: meta.available_sources && meta.available_sources.length > 0 ? meta.available_sources : pkg.available_sources,
                        installed: meta.installed ?? pkg.installed,
                        rating: pkg.rating,
                        depends: (anyMeta.depends && anyMeta.depends.length > 0) ? anyMeta.depends : (pkg as any).depends,
                    } as any;

                    upsertPackages([enrichedPkg]);
                }
            })
            .catch(err => console.error(err));
    }, [pkg?.name, upsertPackages]);

    const [reviews, setReviews] = useState<Review[]>([]);
    const [rating, setRating] = useState<RatingSummary | null>(() => {
        if (pkg?.rating) {
            return {
                average: pkg.rating.score || 0,
                count: pkg.rating.total,
                stars: {
                    1: pkg.rating.star1,
                    2: pkg.rating.star2,
                    3: pkg.rating.star3,
                    4: pkg.rating.star4,
                    5: pkg.rating.star5,
                }
            };
        }
        return null;
    });

    const lookupId = pkg ? (pkg.app_id ?? pkg.name) : '';
    const refreshReviews = useCallback(() => {
        if (!pkg?.name) return;
        getPackageReviews(pkg.name, lookupId).then(res => {
            setReviews(res.reviews);
            setRating(res.summary);
            if (res.summary.count > 0 && pkg) {
                const odrsRating = {
                    total: res.summary.count,
                    star1: res.summary.stars[1] || 0,
                    star2: res.summary.stars[2] || 0,
                    star3: res.summary.stars[3] || 0,
                    star4: res.summary.stars[4] || 0,
                    star5: res.summary.stars[5] || 0,
                    score: res.summary.average
                };
                upsertPackages([{ ...pkg, rating: odrsRating as any }]);
            }
        }).catch(err => console.error(err));
    }, [pkg?.name, lookupId, upsertPackages]);

    useEffect(() => {
        refreshReviews();
    }, [refreshReviews]);

    const { success } = useToast();
    const errorService = useErrorService();
    const { distro: distroFull } = useDistro();
    const distroId = typeof distroFull.id === 'string' ? distroFull.id : 'unknown';

    const { isFlatpakEnabled, isAurEnabled, isChaoticEnabled } = useSettings();

    const [variants, setVariants] = useState<PackageVariant[]>([]);
    const [selectedSource, setSelectedSource] = useState<PackageSource | string>(pkg?.source ?? ({} as PackageSource));

    const [showReviewForm, setShowReviewForm] = useState(false);
    const [reviewTitle, setReviewTitle] = useState('');
    const [reviewBody, setReviewBody] = useState('');
    const [reviewRating, setReviewRating] = useState(0);
    const [isSubmittingReview, setIsSubmittingReview] = useState(false);
    const [visibleReviewsCount, setVisibleReviewsCount] = useState(5);
    const [sortOrder, setSortOrder] = useState<'newest' | 'oldest' | 'highest' | 'lowest'>('newest');
    const [filterRating, setFilterRating] = useState<number | null>(null);

    const [installStatus, setInstallStatus] = useState<BackendInstallStatus | null>(null);
    const [installedVariant, setInstalledVariant] = useState<BackendInstallStatus | null>(null);
    const [allInstalledVariants, setAllInstalledVariants] = useState<BackendInstallStatus[]>([]);
    const [flatpakPermissions, setFlatpakPermissions] = useState<string[]>([]);

    const [showPkgbuild, setShowPkgbuild] = useState(false);
    useEscapeKey(() => setShowPkgbuild(false), showPkgbuild);
    const pkgbuildModalRef = useFocusTrap(showPkgbuild);
    const [pkgbuildContent, setPkgbuildContent] = useState<string | null>(null);
    const [pkgbuildLoading, setPkgbuildLoading] = useState(false);
    const [pkgbuildError, setPkgbuildError] = useState<string | null>(null);

    const [lightboxIndex, setLightboxIndex] = useState<number | null>(null);
    useEscapeKey(() => setLightboxIndex(null), lightboxIndex !== null);

    const { isFavorite, toggleFavorite } = useFavorites();
    const isFav = isFavorite(pkg?.name ?? '');
    const reviewsRef = useRef<HTMLDivElement>(null);

    const scrollToReviews = () => {
        reviewsRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    };

    const checkRequestId = useRef(0);
    const checkStatus = useCallback((customName?: string) => {
        const reqId = ++checkRequestId.current;

        // --- SELECTION LOGIC FIX ---
        // If we have a selected source, we MUST check only THAT specific source's status
        // to determine if the Install/Launch button should change for that repo.
        const selectedVariantAtCall = variants.find(v => isSameSource(v.source, selectedSource));
        const nameToCheck = customName || selectedVariantAtCall?.pkg_name || pkg?.name || '';

        if (!nameToCheck) return;

        commands.checkInstalledStatus(nameToCheck)
            .then(unwrap)
            .then(res => {
                if (reqId !== checkRequestId.current) return;

                // If the backend returns a source, it's installed. 
                // We only mark it as 'the' installed variant if it matches the current selection or we are initializing.
                if (res.installed) {
                    setInstallStatus(res);
                    if (isSameSource(res.source as any, selectedSource)) {
                        setInstalledVariant(res);
                    }
                } else {
                    setInstallStatus(res);
                    if (installedVariant && isSameSource(installedVariant.source as any, selectedSource)) {
                        setInstalledVariant(null);
                    }
                }
            })
            .catch((e) => errorService.reportError(e));
    }, [installedVariant, selectedSource, variants, pkg, errorService]);

    const checkAllVariants = useCallback(async () => {
        if (!variants || variants.length === 0 || !pkg) return;
        try {
            const uniqueNames = Array.from(new Set(variants.map(v => v.pkg_name || pkg.name)));
            const statuses = await Promise.all(uniqueNames.map(n => commands.checkInstalledStatus(n).then(unwrap)));
            setAllInstalledVariants(statuses.filter(s => s.installed));
        } catch (e) { console.error("Conflict check failed:", e); }
    }, [variants, pkg]);

    useEffect(() => {
        if (!pkg?.name) return;
        checkStatus();
        checkAllVariants();
        const unlisten = listen('install-complete', () => {
            checkStatus();
            checkAllVariants();
        });
        return () => { unlisten.then((f: UnlistenFn) => f()); };
    }, [pkg?.name, selectedSource, variants, checkStatus, checkAllVariants]);

    useEffect(() => {
        if (!pkg?.name) return;

        const toVariant = (s: PackageSource): PackageVariant => ({
            source: s,
            version: s.version || 'latest',
            pkg_name: s.package_name || pkg.name,
            repo_name: s.id === 'chaotic-aur' ? 'chaotic-aur' : undefined,
        });

        const rawSources = (pkg.available_sources && pkg.available_sources.length > 0)
            ? pkg.available_sources
            : [pkg.source];

        const nextVariants = rawSources.map(toVariant);
        nextVariants.sort((a, b) => getSourceTierForSort(b.source, distroId) - getSourceTierForSort(a.source, distroId));

        // Filter based on user settings
        const filteredVariants = nextVariants.filter(v => {
            const s = v.source;
            if (typeof s === 'string') {
                if (s === 'aur' && !isAurEnabled) return false;
                if (s === 'flatpak' && !isFlatpakEnabled) return false;
                if (s === 'chaotic' && !isChaoticEnabled) return false;
            } else {
                if (s.source_type === 'aur' && !isAurEnabled) return false;
                if (s.source_type === 'flatpak' && !isFlatpakEnabled) return false;
                if (s.id === 'chaotic-aur' && !isChaoticEnabled) return false;
            }
            return true;
        });

        setVariants(prev => JSON.stringify(prev) === JSON.stringify(filteredVariants) ? prev : filteredVariants);

        async function applyInstallStatusAndSelection(p: BackendPackage, vars: PackageVariant[]) {
            try {
                // If anything is installed, we should probably prefer that variant initially
                const res = unwrap(await commands.checkInstalledStatus(p.name));
                if (res.installed && res.source) {
                    setInstallStatus(res);
                    setInstalledVariant(res);
                    const match = vars.find(v => isSameSource(v.source, res.source as any));
                    if (match) {
                        setSelectedSource(match.source);
                        return;
                    }
                }
            } catch (e) { console.error(e); }

            if (preferredSource) {
                const pref = vars.find(v => isSameSource(v.source, preferredSource));
                if (pref) {
                    setSelectedSource(pref.source);
                    return;
                }
            }
            if (vars.length > 0) setSelectedSource(vars[0].source);
        }

        applyInstallStatusAndSelection(pkg, filteredVariants);
    }, [pkg?.name, distroId, preferredSource, isAurEnabled, isFlatpakEnabled, isChaoticEnabled]);

    useEffect(() => {
        if (!selectedSource || typeof selectedSource === 'string') {
            setFlatpakPermissions([]);
            return;
        }

        if (selectedSource.source_type === 'flatpak') {
            const appId = selectedSource.id;
            // Since we added this command in flathub_api.rs and registered it, we call it here
            (commands as any).getFlatpakPermissions(appId)
                .then((res: any) => {
                    if (res.status === 'ok') setFlatpakPermissions(res.data);
                })
                .catch(console.error);
        } else {
            setFlatpakPermissions([]);
        }
    }, [selectedSource]);

    const handleInstallClick = () => {
        onInstall({
            name: variants.find(v => isSameSource(v.source, selectedSource))?.pkg_name || pkg!.name,
            source: selectedSource,
            repoName: variants.find(v => isSameSource(v.source, selectedSource))?.repo_name,
            displayName: pkg?.display_name ?? undefined
        });
    };

    const handleLaunch = async () => {
        const nameToLaunch = installedVariant?.actual_package_name || installStatus?.actual_package_name || variants.find(v => isSameSource(v.source, selectedSource))?.pkg_name || pkg!.name;
        if (!nameToLaunch?.trim()) {
            errorService.reportError('Cannot launch: no package name');
            return;
        }
        try {
            unwrap(await commands.launchApp({ pkg_name: nameToLaunch.trim() }));
            success('App launched');
        } catch (e) {
            errorService.reportError(e as Error | string);
        }
    };

    const handleReviewSubmit = async () => {
        if (!reviewBody.trim() || reviewRating === 0) return;
        setIsSubmittingReview(true);
        try {
            unwrap(await commands.submitReview(
                lookupId,
                reviewRating,
                reviewTitle || "MonArch User Review",
                reviewBody,
                "MonArch User"
            ));
            success("Review submitted! Thank you.");
            setShowReviewForm(false);
            setReviewTitle("");
            setReviewBody("");
            setReviewRating(0);
            refreshReviews();
        } catch (e) {
            errorService.reportError(e as Error | string);
        } finally {
            setIsSubmittingReview(false);
        }
    };

    const togglePkgbuild = async () => {
        if (!showPkgbuild) {
            setShowPkgbuild(true);
            setPkgbuildLoading(true);
            setPkgbuildError(null);
            try {
                const content = unwrap(await commands.fetchPkgbuild(pkg?.name ?? ''));
                setPkgbuildContent(content);
            } catch (e) {
                setPkgbuildError(typeof e === 'string' ? e : "Failed to fetch PKGBUILD");
            } finally {
                setPkgbuildLoading(false);
            }
        } else {
            setShowPkgbuild(false);
        }
    };

    const screenshots = useMemo(() => {
        const raw = (pkg?.screenshots && pkg.screenshots.length > 0) ? pkg.screenshots : [];
        return raw.map((url) => resolveImageUrl(url) ?? url);
    }, [pkg?.screenshots]);

    useEffect(() => {
        if (lightboxIndex === null) return;
        const handleKey = (e: KeyboardEvent) => {
            if (e.key === 'ArrowRight') setLightboxIndex(prev => prev !== null ? Math.min(prev + 1, screenshots.length - 1) : null);
            if (e.key === 'ArrowLeft') setLightboxIndex(prev => prev !== null ? Math.max(prev - 1, 0) : null);
        };
        window.addEventListener('keydown', handleKey);
        return () => window.removeEventListener('keydown', handleKey);
    }, [lightboxIndex, screenshots.length]);

    const processedReviews = useMemo(() => {
        return [...reviews]
            .filter(r => filterRating === null || Math.round(r.rating) === filterRating)
            .sort((a, b) => {
                if (sortOrder === 'newest') return b.date.getTime() - a.date.getTime();
                if (sortOrder === 'oldest') return a.date.getTime() - b.date.getTime();
                if (sortOrder === 'highest') return b.rating - a.rating;
                if (sortOrder === 'lowest') return a.rating - b.rating;
                return 0;
            });
    }, [reviews, filterRating, sortOrder]);

    const displayedReviews = processedReviews.slice(0, visibleReviewsCount);
    const hasMoreReviews = processedReviews.length > visibleReviewsCount;

    const selectedVariant = variants.find((v) => isSameSource(v.source, selectedSource));
    const displayedVersion = selectedVariant?.version?.trim() ? selectedVariant.version : (pkg.version?.trim() || 'latest');

    const conflictVariant = useMemo(() => {
        if (installedVariant?.installed) return null;
        return allInstalledVariants.find(v => !isSameSource(v.source as any, selectedSource));
    }, [allInstalledVariants, installedVariant, selectedSource]);

    const activeInstall = activeInstallPackage;

    if (!pkg?.name) {
        return (
            <div className="flex-1 flex items-center justify-center bg-app-bg">
                <div className="flex flex-col items-center gap-4">
                    <Loader2 className="w-10 h-10 text-app-muted animate-spin" />
                    <p className="text-app-muted">Loading package details…</p>
                </div>
            </div>
        );
    }

    return (
        <div
            ref={containerRef}
            className="flex-1 flex flex-col bg-[#0a0a0a] text-app-fg overflow-y-auto overflow-x-hidden pt-6 pb-20"
        >
            <div className="max-w-7xl mx-auto w-full px-6 lg:px-10">
                {/* --- Top Navbar --- */}
                <nav className="flex items-center justify-between mb-10">
                    <button
                        onClick={onBack}
                        className="group flex items-center gap-2 px-3 py-1.5 rounded-xl hover:bg-white/5 text-app-muted hover:text-white transition-all active:scale-95"
                    >
                        <ArrowLeft size={16} className="transition-transform group-hover:-translate-x-1" />
                        <span className="text-[11px] font-black uppercase tracking-widest">Store</span>
                    </button>
                    <div className="flex gap-2">
                        <button
                            onClick={() => { navigator.clipboard.writeText(window.location.href); success("Link copied!"); }}
                            className="p-2.5 rounded-xl bg-white/5 border border-white/5 text-white/40 hover:text-white transition-all"
                        >
                            <Share2 size={16} />
                        </button>
                        <button
                            onClick={() => toggleFavorite(pkg.name)}
                            className={clsx(
                                "p-2.5 rounded-xl border transition-all active:scale-95",
                                isFav ? "bg-red-500/10 border-red-500/10 text-red-500" : "bg-white/5 border-white/5 text-white/40"
                            )}
                        >
                            <Heart size={16} className={isFav ? "fill-current" : ""} />
                        </button>
                    </div>
                </nav>

                {/* --- Header Section --- */}
                <div className="flex flex-col md:flex-row gap-10 items-start md:items-center mb-12">
                    <div className="relative shrink-0 w-32 h-32 lg:w-44 lg:h-44 flex items-center justify-center p-2">
                        <ImageWithFallback
                            src={resolveIconUrl(pkg.icon) || archLogo}
                            alt={pkg.name}
                            className="w-full h-full drop-shadow-[0_25px_50px_rgba(0,0,0,0.5)] object-contain"
                        />
                    </div>

                    <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-3 mb-3">
                            <RepoBadge source={selectedSource} />
                            {installedVariant?.installed && (
                                <span className="px-2.5 py-1 rounded-lg bg-emerald-500/10 border border-emerald-500/10 text-[9px] font-black uppercase tracking-widest text-emerald-400">Installed</span>
                            )}
                        </div>
                        <h1 className="text-4xl lg:text-7xl font-black text-white tracking-tighter mb-4 leading-none">
                            {smartDisplayName}
                        </h1>

                        {rating && rating.count > 0 && (
                            <div className="flex items-center gap-3 mb-6">
                                <div className="flex gap-0.5">
                                    {[1, 2, 3, 4, 5].map((s) => (
                                        <Star
                                            key={s}
                                            size={16}
                                            fill={s <= Math.round(rating.average) ? "#fbbf24" : "none"}
                                            className={s <= Math.round(rating.average) ? "text-amber-400" : "text-white/10"}
                                        />
                                    ))}
                                </div>
                                <div className="flex items-baseline gap-1.5">
                                    <span className="text-lg font-black text-white leading-none">{rating.average.toFixed(1)}</span>
                                    <span className="text-[10px] font-bold text-app-muted uppercase tracking-widest">({rating.count} Users)</span>
                                </div>
                            </div>
                        )}

                        <div className="flex flex-wrap items-center gap-4 mb-8">
                            <div className="w-full sm:w-auto sm:min-w-[200px]">
                                {variants.length > 1 ? (
                                    <RepoSelector variants={variants} selectedSource={selectedSource} onChange={setSelectedSource} />
                                ) : (
                                    <div className="px-4 py-2 bg-white/5 rounded-xl border border-white/5 text-[10px] font-black text-app-muted uppercase tracking-widest">
                                        Community Repository
                                    </div>
                                )}
                            </div>

                            <div className="flex gap-3">
                                {installedVariant?.installed ? (
                                    <>
                                        <button onClick={handleLaunch} className="px-10 h-12 bg-emerald-600 hover:bg-emerald-500 text-white rounded-xl font-black transition-all flex items-center gap-2 text-xs uppercase tracking-widest active:scale-95 shadow-lg shadow-emerald-900/20">
                                            <Play size={16} fill="currentColor" /> Launch
                                        </button>
                                        <button
                                            onClick={() => onUninstall({
                                                name: variants.find(v => isSameSource(v.source, selectedSource))?.pkg_name || pkg.name,
                                                source: selectedSource,
                                                repoName: variants.find(v => isSameSource(v.source, selectedSource))?.repo_name,
                                                displayName: pkg.display_name || undefined
                                            })}
                                            disabled={installInProgress}
                                            className={clsx(
                                                "px-6 h-12 flex items-center justify-center rounded-xl transition-all font-black gap-2 select-none text-xs uppercase tracking-widest active:scale-95",
                                                installInProgress ? "bg-white/5 text-white/10 cursor-not-allowed" : "bg-red-600/10 border border-red-600/20 text-red-500 hover:bg-red-600/20"
                                            )}
                                        >
                                            {installInProgress && activeInstall?.name === (variants.find(v => isSameSource(v.source, selectedSource))?.pkg_name || pkg.name) ? <Loader2 size={16} className="animate-spin" /> : <Trash2 size={16} />}
                                            <span>Uninstall</span>
                                        </button>
                                    </>
                                ) : (
                                    <button
                                        onClick={handleInstallClick}
                                        disabled={installInProgress}
                                        className={clsx(
                                            "px-12 h-12 rounded-xl font-black transition-all flex items-center gap-2 text-xs uppercase tracking-widest active:scale-95",
                                            installInProgress ? "bg-white/5 text-white/10 cursor-not-allowed" : "bg-blue-600 hover:bg-blue-500 text-white shadow-xl shadow-blue-900/30"
                                        )}
                                    >
                                        {installInProgress && activeInstall?.name === (variants.find(v => isSameSource(v.source, selectedSource))?.pkg_name || pkg.name) ? <Loader2 size={16} className="animate-spin" /> : <Download size={16} />}
                                        {installInProgress && activeInstall?.name === (variants.find(v => isSameSource(v.source, selectedSource))?.pkg_name || pkg.name) ? "Installing..." : "Install"}
                                    </button>
                                )}
                            </div>
                        </div>

                        {/* Quick Stats Row */}
                        <div className="flex flex-wrap gap-x-8 gap-y-4 pt-6 border-t border-white/5">
                            <div className="space-y-1">
                                <span className="block text-[9px] font-black text-app-muted uppercase tracking-widest">Version</span>
                                <span className="block text-xs font-bold text-white/90">{displayedVersion}</span>
                            </div>
                            <div className="space-y-1">
                                <span className="block text-[9px] font-black text-app-muted uppercase tracking-widest">Weight</span>
                                <span className="block text-xs font-bold text-white/90">{pkg.installed_size ? `${Math.round(Number(pkg.installed_size) / 1024 / 1024)} MB` : "N/A"}</span>
                            </div>
                            <div className="space-y-1">
                                <span className="block text-[9px] font-black text-app-muted uppercase tracking-widest">License</span>
                                <span className="block text-xs font-bold text-white/90 truncate max-w-[120px]">{pkg.license?.[0] || "Open Source"}</span>
                            </div>
                            <div className="space-y-1">
                                <span className="block text-[9px] font-black text-app-muted uppercase tracking-widest">Maintainer</span>
                                <span className="block text-xs font-bold text-white/90">{pkg.maintainer || "Community"}</span>
                            </div>
                        </div>
                    </div>
                </div>

                {/* --- Conflict / AUR Notices --- */}
                <div className="space-y-3 mb-12">
                    <AnimatePresence>
                        {conflictVariant && (
                            <motion.div initial={{ opacity: 0, y: -10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -10 }}>
                                <div className="p-4 bg-amber-500/5 border border-amber-500/10 rounded-2xl flex items-start gap-4">
                                    <AlertCircle size={18} className="text-amber-500 shrink-0 mt-0.5" />
                                    <div>
                                        <h5 className="text-[10px] font-black text-amber-500 mb-1 uppercase tracking-widest">Notice: Different Source Installed</h5>
                                        <p className="text-[11px] text-amber-200/50 leading-relaxed font-medium">
                                            This application is already installed via <span className="text-amber-400 font-bold">{conflictVariant.source?.label || conflictVariant.source?.id}</span>.
                                            Switching sources will not share application data.
                                        </p>
                                    </div>
                                </div>
                            </motion.div>
                        )}
                    </AnimatePresence>
                    <AnimatePresence>
                        {((typeof selectedSource === 'string' ? selectedSource === 'aur' : selectedSource.source_type === 'aur')) && (
                            <motion.div initial={{ opacity: 0, y: -10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -10 }}>
                                <div className="p-4 bg-amber-500/5 border border-amber-500/10 rounded-2xl flex items-center justify-between gap-4">
                                    <div className="flex items-start gap-4">
                                        <AlertTriangle size={18} className="text-amber-500 shrink-0 mt-0.5" />
                                        <div>
                                            <h5 className="text-[10px] font-black text-amber-500 mb-1 uppercase tracking-widest">Community Package</h5>
                                            <p className="text-[11px] text-amber-200/50 leading-relaxed font-medium">This package is from the AUR. Review the build script before installing.</p>
                                        </div>
                                    </div>
                                    <button onClick={togglePkgbuild} className="px-4 py-2 bg-amber-500/10 hover:bg-amber-500/20 text-amber-500 rounded-xl text-[9px] font-black uppercase tracking-widest border border-amber-500/10 transition-all">Review Source</button>
                                </div>
                            </motion.div>
                        )}
                    </AnimatePresence>
                </div>

                {/* --- Main Content Flow --- */}
                <div className="space-y-16">
                    {/* Screenshots */}
                    {screenshots.length > 0 && (
                        <section className="space-y-6">
                            <h3 className="text-[11px] font-black text-app-muted uppercase tracking-[0.2em] flex items-center gap-2">
                                <Box size={14} className="text-blue-500" /> Screenshots
                            </h3>
                            <div className="flex gap-4 overflow-x-auto pb-4 scrollbar-hide snap-x">
                                {screenshots.map((s, i) => (
                                    <div
                                        key={i}
                                        onClick={() => setLightboxIndex(i)}
                                        className="shrink-0 w-[min(420px,75vw)] aspect-video rounded-2xl overflow-hidden bg-white/5 border border-white/10 cursor-zoom-in snap-center shadow-xl group"
                                    >
                                        <img src={s} alt="" className="w-full h-full object-contain bg-black/20 group-hover:scale-[1.05] transition-transform duration-500" />
                                    </div>
                                ))}
                            </div>
                        </section>
                    )}

                    {/* Description */}
                    <section className="space-y-8">
                        <h3 className="text-[11px] font-black text-app-muted uppercase tracking-[0.2em] flex items-center gap-2">
                            <Info size={14} className="text-blue-500" /> Description
                        </h3>
                        <DescriptionBlock
                            html={DOMPurify.sanitize(pkg.long_description || pkg.description || "")}
                            keywords={pkg.keywords || []}
                        />
                    </section>

                    {/* Technical Extras Grid */}
                    <section className="grid grid-cols-1 md:grid-cols-2 gap-8">
                        {/* Links Section */}
                        <div className="space-y-6">
                            <h3 className="text-[11px] font-black text-app-muted uppercase tracking-[0.2em] flex items-center gap-2">
                                <Globe size={14} className="text-blue-500" /> Links
                            </h3>
                            <div className="flex flex-col gap-2">
                                {pkg.url && (
                                    <a href={pkg.url} target="_blank" rel="noreferrer" className="flex items-center justify-between p-4 bg-white/5 rounded-2xl border border-white/5 hover:border-blue-500/30 group transition-all">
                                        <div className="flex items-center gap-3">
                                            <Globe size={16} className="text-app-muted group-hover:text-blue-400" />
                                            <span className="text-xs font-bold text-app-fg/70">Official Website</span>
                                        </div>
                                        <ExternalLink size={14} className="text-app-muted group-hover:text-blue-400" />
                                    </a>
                                )}
                                <button onClick={() => { navigator.clipboard.writeText(pkg.name); success("CLI Name Copied!"); }} className="flex items-center justify-between p-4 bg-white/5 rounded-2xl border border-white/5 hover:border-purple-500/30 group transition-all text-left">
                                    <div className="flex items-center gap-3">
                                        <Terminal size={16} className="text-app-muted group-hover:text-purple-400" />
                                        <span className="text-xs font-bold text-app-fg/70 truncate">CLI: {pkg.name}</span>
                                    </div>
                                    <Code size={14} className="text-app-muted group-hover:text-purple-400" />
                                </button>
                                <button onClick={togglePkgbuild} className="flex items-center justify-between p-4 bg-white/5 rounded-2xl border border-white/5 hover:border-emerald-500/30 group transition-all text-left">
                                    <div className="flex items-center gap-3">
                                        <FileCode size={16} className="text-app-muted group-hover:text-emerald-400" />
                                        <span className="text-xs font-bold text-app-fg/70">View Source</span>
                                    </div>
                                    <ChevronRight size={14} className="text-app-muted group-hover:text-emerald-400" />
                                </button>
                            </div>
                        </div>

                        {/* Security / Permissions */}
                        <div className="space-y-6">
                            <h3 className="text-[11px] font-black text-app-muted uppercase tracking-[0.2em] flex items-center gap-2">
                                <ShieldCheck size={14} className="text-blue-500" /> Security
                            </h3>
                            <div className="space-y-6">
                                <SecurityNotice source={(selectedSource as any)?.source_type || 'repo'} />

                                <FlatpakPermissions permissions={flatpakPermissions} />

                                <div className="space-y-4">
                                    <DependencyTree deps={(pkg as any).depends || []} type="runtime" />
                                    <DependencyTree deps={(pkg as any).make_depends || []} type="build" />
                                </div>
                            </div>
                        </div>
                    </section>

                    {/* Reviews */}
                    <section ref={reviewsRef} className="pt-12 border-t border-white/5 space-y-12">
                        <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
                            <div>
                                <h3 className="text-xl font-black text-white px-2 mb-1">Feedback</h3>
                                <p className="text-[10px] text-app-muted font-black uppercase tracking-widest px-2">Community Rating & Experience</p>
                            </div>
                            <button
                                onClick={() => setShowReviewForm(true)}
                                className="px-5 py-2.5 bg-blue-600 hover:bg-blue-500 text-white rounded-xl text-xs font-black uppercase tracking-widest transition-all shadow-lg shadow-blue-900/20 active:scale-95"
                            >
                                Write a Review
                            </button>
                        </div>

                        {/* Filters Bar */}
                        <div className="flex flex-wrap items-center justify-between gap-6 py-8 border-y border-white/5">
                            <div className="flex flex-wrap items-center gap-2">
                                <span className="text-[10px] font-black text-app-muted uppercase tracking-widest mr-3">Filter</span>
                                <button
                                    onClick={() => setFilterRating(null)}
                                    className={clsx(
                                        "px-4 py-2 rounded-xl text-[10px] font-black uppercase tracking-widest transition-all",
                                        filterRating === null ? "bg-white/10 text-white shadow-lg shadow-black/20" : "text-app-muted hover:text-white hover:bg-white/5"
                                    )}
                                >
                                    All
                                </button>
                                {[5, 4, 3, 2, 1].map(r => (
                                    <button
                                        key={r}
                                        onClick={() => setFilterRating(r)}
                                        className={clsx(
                                            "flex items-center gap-1.5 px-4 py-2 rounded-xl text-[10px] font-black uppercase tracking-widest transition-all",
                                            filterRating === r ? "bg-amber-500/10 text-amber-500 shadow-lg shadow-amber-900/10" : "text-app-muted hover:text-white hover:bg-white/5"
                                        )}
                                    >
                                        {r} <Star size={10} fill="currentColor" />
                                    </button>
                                ))}
                            </div>
                            <div className="flex items-center gap-4 bg-white/5 pl-4 pr-1 py-1 rounded-xl border border-white/5">
                                <span className="text-[10px] font-black text-app-muted uppercase tracking-widest">Sort By</span>
                                <select
                                    value={sortOrder}
                                    onChange={(e) => setSortOrder(e.target.value as any)}
                                    className="bg-transparent text-white text-[10px] font-black uppercase tracking-widest px-3 py-1.5 rounded-lg outline-none cursor-pointer"
                                >
                                    <option value="newest">Newest</option>
                                    <option value="oldest">Oldest</option>
                                    <option value="highest">Highest</option>
                                    <option value="lowest">Lowest</option>
                                </select>
                            </div>
                        </div>

                        <div className="grid grid-cols-1 md:grid-cols-2 gap-6 pb-20">
                            {displayedReviews.length > 0 ? (
                                displayedReviews.map((review, i) => (
                                    <motion.div
                                        key={review.id || i}
                                        initial={{ opacity: 0, y: 10 }}
                                        animate={{ opacity: 1, y: 0 }}
                                        className="p-8 bg-white/[0.03] border border-white/5 rounded-[2rem] space-y-6"
                                    >
                                        <div className="flex items-start justify-between">
                                            <div className="flex items-center gap-4">
                                                <div className="w-12 h-12 rounded-2xl bg-white/5 flex items-center justify-center text-blue-400">
                                                    <User size={20} />
                                                </div>
                                                <div>
                                                    <div className="text-sm font-black text-white">{review.userName || 'User'}</div>
                                                    <div className="text-[10px] font-bold text-app-muted uppercase tracking-widest">{review.date instanceof Date ? review.date.toLocaleDateString() : 'Recent'}</div>
                                                </div>
                                            </div>
                                            <div className="flex gap-0.5">
                                                {[1, 2, 3, 4, 5].map(s => <Star key={s} size={12} fill={s <= review.rating ? "#fbbf24" : "none"} className={s <= review.rating ? "text-amber-400" : "text-white/5"} />)}
                                            </div>
                                        </div>
                                        <p className="text-sm text-app-muted leading-relaxed italic">"{review.comment}"</p>
                                    </motion.div>
                                ))
                            ) : (
                                <div className="md:col-span-2 text-center py-24 bg-white/[0.01] border border-dashed border-white/10 rounded-[3rem]">
                                    <MessageSquare size={48} className="mx-auto text-white/5 mb-6" />
                                    <p className="text-sm font-black text-app-muted uppercase tracking-[0.2em]">Be the first to share your experience</p>
                                </div>
                            )}

                            {hasMoreReviews && (
                                <div className="md:col-span-2 flex justify-center pt-8">
                                    <button
                                        onClick={() => setVisibleReviewsCount(prev => prev + 6)}
                                        className="px-8 py-3 bg-white/5 hover:bg-white/10 text-white text-[11px] font-black uppercase tracking-widest rounded-xl transition-all"
                                    >
                                        Load more reviews
                                    </button>
                                </div>
                            )}
                        </div>
                    </section>
                </div>
            </div>

            {/* --- Modals --- */}
            <AnimatePresence>
                {showReviewForm && (
                    <motion.div
                        initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
                        className="fixed inset-0 z-[1000] bg-black/80 backdrop-blur-sm flex items-center justify-center p-6"
                        onClick={() => setShowReviewForm(false)}
                    >
                        <motion.div
                            initial={{ scale: 0.9, y: 20 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.9, y: 20 }}
                            className="bg-[#111] border border-white/10 w-full max-w-md rounded-[2.5rem] p-10 space-y-8 shadow-2xl"
                            onClick={e => e.stopPropagation()}
                        >
                            <div className="flex items-center justify-between">
                                <div>
                                    <h3 className="text-2xl font-black text-white mb-1 tracking-tight">Write a Review</h3>
                                    <p className="text-[10px] text-app-muted font-black uppercase tracking-widest">Share with the community</p>
                                </div>
                                <button onClick={() => setShowReviewForm(false)} className="p-2 text-app-muted hover:text-white transition-colors">
                                    <X size={24} />
                                </button>
                            </div>

                            <div className="space-y-6">
                                <div className="flex gap-3 justify-center py-4 bg-white/[0.02] rounded-3xl">
                                    {[1, 2, 3, 4, 5].map((s) => (
                                        <button key={s} onClick={() => setReviewRating(s)} className="p-1 transition-transform active:scale-95">
                                            <Star size={32} fill={s <= reviewRating ? "#fbbf24" : "none"} className={s <= reviewRating ? "text-amber-400" : "text-white/10"} />
                                        </button>
                                    ))}
                                </div>
                                <div className="space-y-3">
                                    <label className="text-[10px] font-black text-app-muted uppercase tracking-widest pl-2">Your Experience</label>
                                    <textarea
                                        value={reviewBody}
                                        onChange={(e) => setReviewBody(e.target.value)}
                                        placeholder="How is this app working for you?"
                                        className="w-full h-40 bg-white/[0.03] border border-white/5 rounded-3xl p-6 text-sm text-white placeholder:text-white/20 focus:border-blue-500/50 outline-none transition-all resize-none shadow-inner"
                                    />
                                </div>
                            </div>

                            <div className="flex gap-4">
                                <button onClick={() => setShowReviewForm(false)} className="flex-1 h-14 rounded-2xl bg-white/5 text-[11px] font-black uppercase tracking-widest hover:bg-white/10 transition-all">Cancel</button>
                                <button
                                    onClick={handleReviewSubmit}
                                    disabled={isSubmittingReview || !reviewBody.trim() || reviewRating === 0}
                                    className="flex-1 h-14 rounded-2xl bg-blue-600 text-[11px] font-black uppercase tracking-widest hover:bg-blue-500 transition-all flex items-center justify-center gap-2 disabled:opacity-50 shadow-xl shadow-blue-900/30"
                                >
                                    {isSubmittingReview ? <Loader2 size={18} className="animate-spin" /> : <Check size={18} />}
                                    Submit
                                </button>
                            </div>
                        </motion.div>
                    </motion.div>
                )}
            </AnimatePresence>

            <AnimatePresence>
                {lightboxIndex !== null && (
                    <motion.div
                        initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
                        onClick={() => setLightboxIndex(null)}
                        className="fixed inset-0 z-[1000] bg-black/98 backdrop-blur-2xl flex items-center justify-center p-4 sm:p-12 cursor-zoom-out"
                    >
                        <button onClick={() => setLightboxIndex(null)} className="absolute top-8 right-8 p-4 text-white/50 hover:text-white transition-colors"><X size={32} /></button>
                        {lightboxIndex > 0 && (
                            <button onClick={(e) => { e.stopPropagation(); setLightboxIndex(lightboxIndex - 1); }} className="absolute left-12 top-1/2 -translate-y-1/2 p-4 rounded-full bg-white/5 hover:bg-white/10 text-white transition-all"><ChevronLeft size={48} /></button>
                        )}
                        {lightboxIndex < (screenshots?.length || 0) - 1 && (
                            <button onClick={(e) => { e.stopPropagation(); setLightboxIndex(lightboxIndex + 1); }} className="absolute right-12 top-1/2 -translate-y-1/2 p-4 rounded-full bg-white/5 hover:bg-white/10 text-white transition-all"><ChevronRight size={48} /></button>
                        )}
                        {screenshots && screenshots[lightboxIndex] && (
                            <motion.img
                                key={lightboxIndex}
                                initial={{ opacity: 0, scale: 0.9 }}
                                animate={{ opacity: 1, scale: 1 }}
                                src={screenshots[lightboxIndex]}
                                className="max-h-full max-w-full rounded-2xl shadow-2xl object-contain"
                                onClick={e => e.stopPropagation()}
                            />
                        )}
                    </motion.div>
                )}
            </AnimatePresence>

            <AnimatePresence>
                {showPkgbuild && (
                    <motion.div
                        initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
                        className="fixed inset-0 z-[1000] bg-black/95 backdrop-blur-md flex items-center justify-center p-4 sm:p-10"
                        onClick={() => setShowPkgbuild(false)}
                    >
                        <motion.div
                            ref={pkgbuildModalRef}
                            initial={{ scale: 0.95, y: 40 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.95, y: 40 }}
                            onClick={e => e.stopPropagation()}
                            className="bg-[#050505] w-full max-w-5xl h-full max-h-[90vh] rounded-[3rem] border border-white/10 flex flex-col overflow-hidden shadow-2xl"
                        >
                            <div className="p-10 border-b border-white/5 flex justify-between items-center bg-white/[0.02]">
                                <div className="flex items-center gap-5">
                                    <div className="p-4 rounded-2xl bg-amber-500/10 text-amber-500">
                                        <FileCode size={28} />
                                    </div>
                                    <div>
                                        <h3 className="text-2xl font-black text-white leading-tight">Build Source</h3>
                                        <p className="text-[10px] text-app-muted font-black uppercase tracking-widest">Inspecting PKGBUILD for {pkg.name}</p>
                                    </div>
                                </div>
                                <button onClick={() => setShowPkgbuild(false)} className="p-3 rounded-full hover:bg-white/10 text-white transition-all"><X size={28} /></button>
                            </div>
                            <div className="flex-1 overflow-auto p-12 font-mono text-sm leading-relaxed">
                                {pkgbuildLoading ? (
                                    <div className="h-full flex flex-col items-center justify-center text-app-muted gap-6 animate-pulse">
                                        <RefreshCw size={48} className="animate-spin text-blue-500" />
                                        <p className="font-black tracking-[0.2em] uppercase text-xs">Fetching source data...</p>
                                    </div>
                                ) : (
                                    <pre className="text-white/70 whitespace-pre scrollbar-thin scrollbar-thumb-white/10 pr-4">
                                        {pkgbuildContent}
                                    </pre>
                                )}
                            </div>
                        </motion.div>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
}
