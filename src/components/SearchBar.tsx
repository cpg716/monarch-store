import React, { useRef } from 'react';
import { Search, X, ArrowLeft } from 'lucide-react';

interface SearchBarProps {
    value: string;
    onChange: (value: string) => void;
    /** When set and there is a query, show a back button that calls this (e.g. clear search and return to previous view). */
    onBack?: () => void;
}

const SearchBar: React.FC<SearchBarProps> = ({ value, onChange, onBack }) => {
    const inputRef = useRef<HTMLInputElement>(null);

    const handleClear = () => {
        onChange('');
        inputRef.current?.focus();
    };

    const showBack = value.length > 0 && onBack != null;

    return (
        <div className="relative w-full max-w-2xl">
            <div className="flex items-center gap-2 bg-app-card border border-app-border rounded-xl shadow-sm dark:shadow-none overflow-hidden focus-within:ring-2 focus-within:ring-app-accent/40 focus-within:border-app-accent/50 transition-all">
                {showBack ? (
                    <button
                        type="button"
                        onClick={onBack}
                        className="flex-shrink-0 p-2.5 text-app-muted hover:text-app-fg hover:bg-app-fg/5 transition-colors"
                        aria-label="Back"
                    >
                        <ArrowLeft size={20} />
                    </button>
                ) : (
                    <div className="flex-shrink-0 pl-4 pr-1 py-3 text-app-muted pointer-events-none">
                        <Search size={20} />
                    </div>
                )}
                <input
                    ref={inputRef}
                    type="text"
                    value={value}
                    onChange={(e) => onChange(e.target.value)}
                    onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                            e.preventDefault();
                            (e.target as HTMLInputElement).blur();
                        }
                    }}
                    placeholder="Search apps by name or task (e.g. browser, video editor, Discord)"
                    data-monarch-search
                    className="flex-1 min-w-0 py-3 pr-3 text-base text-app-fg placeholder:text-app-muted/70 bg-transparent border-0 focus:outline-none focus:ring-0"
                />
                {value.length > 0 && (
                    <button
                        type="button"
                        onClick={handleClear}
                        className="flex-shrink-0 p-2.5 text-app-muted hover:text-app-fg hover:bg-app-fg/5 transition-colors"
                        aria-label="Clear search"
                    >
                        <X size={18} />
                    </button>
                )}
            </div>
        </div>
    );
};

export default SearchBar;
