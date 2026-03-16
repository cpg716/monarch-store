import type { PackageSource } from '../services/bindings';
import archOfficialLogo from '../assets/source-logos/arch-official.svg';
import flatpakLogo from '../assets/source-logos/flatpak.svg';
import aurLogo from '../assets/source-logos/aur.svg';
import chaoticLogo from '../assets/source-logos/chaotic-aur.svg';
import cachyosLogo from '../assets/source-logos/cachyos.svg';
import manjaroLogo from '../assets/source-logos/manjaro.svg';
import garudaLogo from '../assets/source-logos/garuda.svg';
import endeavourLogo from '../assets/source-logos/endeavouros.svg';

export interface SourceBrand {
    familyId: string;
    label: string;
    shortLabel: string;
    logoAsset: string | null;
    altText: string;
    colorClass: string;
    bgClass: string;
    hint: string;
    recommended: boolean;
}

export function getSourceBrand(
    source: PackageSource | string,
    distroId: string,
    pkgName?: string | null
): SourceBrand {
    const normalizedDistro = String(distroId ?? '').toLowerCase();
    const sourceObj = typeof source === 'string'
        ? { source_type: source, id: source, label: source, version: '', package_name: pkgName ?? null }
        : source;
    const sourceType = String(sourceObj.source_type ?? '').toLowerCase();
    const id = String(sourceObj.id ?? '').toLowerCase();
    const label = String(sourceObj.label ?? '').trim();

    const addChannel = (base: string) => {
        const lower = String(pkgName ?? sourceObj.package_name ?? '').toLowerCase();
        if (lower.includes('-nightly')) return `${base} Nightly`;
        if (lower.includes('-beta')) return `${base} Beta`;
        if (lower.includes('-developer-edition')) return `${base} Developer Edition`;
        if (lower.includes('-esr')) return `${base} ESR`;
        if (lower.includes('-canary')) return `${base} Canary`;
        if (lower.includes('-ptb')) return `${base} PTB`;
        return base;
    };

    if (id.startsWith('cachyos')) {
        return {
            familyId: 'cachyos',
            label: addChannel('CachyOS'),
            shortLabel: 'CachyOS',
            logoAsset: cachyosLogo,
            altText: 'CachyOS',
            colorClass: 'text-cyan-500',
            bgClass: 'bg-cyan-500/10 border-cyan-500/20',
            hint: 'Optimized packages for CachyOS and compatible systems.',
            recommended: normalizedDistro === 'cachyos',
        };
    }

    const isArchOfficialRepo = ['core', 'extra', 'community', 'multilib', 'official'].includes(id);
    if (isArchOfficialRepo && normalizedDistro === 'manjaro') {
        return {
            familyId: 'manjaro',
            label: addChannel('Manjaro Official'),
            shortLabel: 'Manjaro',
            logoAsset: manjaroLogo,
            altText: 'Manjaro',
            colorClass: 'text-teal-500',
            bgClass: 'bg-teal-500/10 border-teal-500/20',
            hint: 'Packages from your Manjaro repositories.',
            recommended: true,
        };
    }

    if (id === 'chaotic-aur') {
        return {
            familyId: 'chaotic-aur',
            label: addChannel('Chaotic-AUR'),
            shortLabel: 'Chaotic',
            logoAsset: chaoticLogo,
            altText: 'Chaotic-AUR',
            colorClass: 'text-purple-500',
            bgClass: 'bg-purple-500/10 border-purple-500/20',
            hint: 'Pre-built community packages with fast installs.',
            recommended: normalizedDistro === 'arch' || normalizedDistro === 'garuda',
        };
    }

    if (isArchOfficialRepo) {
        return {
            familyId: 'official',
            label: addChannel('Arch Official'),
            shortLabel: 'Arch',
            logoAsset: archOfficialLogo,
            altText: 'Arch Official',
            colorClass: 'text-emerald-500',
            bgClass: 'bg-emerald-500/10 border-emerald-500/20',
            hint: 'Stable packages from the official system repositories.',
            recommended: normalizedDistro === 'arch' || normalizedDistro === 'endeavouros',
        };
    }

    if (sourceType === 'flatpak' || id === 'flathub' || id === 'flathub-beta' || id === 'flatpak') {
        return {
            familyId: 'flatpak',
            label: addChannel(id === 'flathub-beta' ? 'Flatpak Beta' : 'Flatpak'),
            shortLabel: 'Flatpak',
            logoAsset: flatpakLogo,
            altText: 'Flatpak',
            colorClass: 'text-blue-500',
            bgClass: 'bg-blue-500/10 border-blue-500/20',
            hint: 'Sandboxed app that works across many Linux systems.',
            recommended: false,
        };
    }

    if (sourceType === 'aur' || id === 'aur') {
        return {
            familyId: 'aur',
            label: addChannel('AUR'),
            shortLabel: 'AUR',
            logoAsset: aurLogo,
            altText: 'Arch User Repository',
            colorClass: 'text-orange-500',
            bgClass: 'bg-orange-500/10 border-orange-500/20',
            hint: 'Community packaging scripts built on your machine.',
            recommended: false,
        };
    }

    if (id.includes('garuda')) {
        return {
            familyId: 'garuda',
            label: addChannel('Garuda'),
            shortLabel: 'Garuda',
            logoAsset: garudaLogo,
            altText: 'Garuda',
            colorClass: 'text-orange-500',
            bgClass: 'bg-orange-500/10 border-orange-500/20',
            hint: 'Packages from Garuda Linux repositories.',
            recommended: normalizedDistro === 'garuda',
        };
    }

    if (id.includes('endeavour')) {
        return {
            familyId: 'endeavouros',
            label: addChannel('EndeavourOS'),
            shortLabel: 'EndeavourOS',
            logoAsset: endeavourLogo,
            altText: 'EndeavourOS',
            colorClass: 'text-violet-500',
            bgClass: 'bg-violet-500/10 border-violet-500/20',
            hint: 'Packages from EndeavourOS repositories.',
            recommended: normalizedDistro === 'endeavouros',
        };
    }

    if (id.includes('manjaro')) {
        return {
            familyId: 'manjaro',
            label: addChannel('Manjaro'),
            shortLabel: 'Manjaro',
            logoAsset: manjaroLogo,
            altText: 'Manjaro',
            colorClass: 'text-teal-500',
            bgClass: 'bg-teal-500/10 border-teal-500/20',
            hint: 'Packages from Manjaro repositories.',
            recommended: normalizedDistro === 'manjaro',
        };
    }

    return {
        familyId: 'other',
        label: addChannel(label || sourceObj.id || 'Other Source'),
        shortLabel: label || sourceObj.id || 'Other',
        logoAsset: null,
        altText: label || sourceObj.id || 'Other source',
        colorClass: 'text-slate-500',
        bgClass: 'bg-slate-500/10 border-slate-500/20',
        hint: 'Available from a configured source.',
        recommended: false,
    };
}
