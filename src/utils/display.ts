export function parseNumericString(value: string | number | null | undefined): number | null {
    if (typeof value === 'number' && Number.isFinite(value)) return value;
    if (typeof value !== 'string') return null;
    const trimmed = value.trim();
    if (!trimmed) return null;
    const parsed = Number(trimmed);
    return Number.isFinite(parsed) ? parsed : null;
}

export function formatBytes(value: string | number | null | undefined): string | null {
    const bytes = parseNumericString(value);
    if (bytes == null || bytes < 0) return null;
    if (bytes === 0) return '0 B';

    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let size = bytes;
    let unitIndex = 0;

    while (size >= 1024 && unitIndex < units.length - 1) {
        size /= 1024;
        unitIndex += 1;
    }

    const rounded = size >= 100 || unitIndex === 0 ? size.toFixed(0) : size.toFixed(1);
    return `${rounded} ${units[unitIndex]}`;
}

export function formatUnixDate(value: string | number | null | undefined): string | null {
    const raw = parseNumericString(value);
    if (raw == null || raw <= 0) return null;
    const millis = raw > 9999999999 ? raw : raw * 1000;
    const date = new Date(millis);
    if (Number.isNaN(date.getTime())) return null;
    return date.toLocaleDateString(undefined, {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
    });
}
