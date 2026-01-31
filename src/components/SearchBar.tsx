import React, { useRef } from 'react';
import { Search, X } from 'lucide-react';

interface SearchBarProps {
    value: string;
    onChange: (value: string) => void;
}

const SearchBar: React.FC<SearchBarProps> = ({ value, onChange }) => {
    const inputRef = useRef<HTMLInputElement>(null);

    const handleClear = () => {
        onChange('');
        inputRef.current?.focus();
    };

    return (
        <div className="relative w-full max-w-3xl group transition-all">
            {/* Gradient border effect on focus */}
            <div className="absolute -inset-0.5 bg-gradient-to-r from-blue-500 via-purple-500 to-pink-500 rounded-[2.5rem] blur opacity-25 group-hover:opacity-50 group-focus-within:opacity-75 transition-opacity duration-500" />

            <div className="relative">
                <div className="absolute inset-y-0 left-6 flex items-center pointer-events-none text-app-muted group-focus-within:text-blue-500 transition-colors">
                    <Search size={24} className="group-focus-within:scale-110 transition-transform" />
                </div>
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
                    placeholder="Search for apps (e.g. firefox, spotify, discord)"
                    className="w-full bg-app-card border-2 border-slate-200/80 dark:border-app-border/50 rounded-[2rem] py-5 pl-16 pr-12 text-xl text-app-fg placeholder-app-muted/60 focus:outline-none focus:border-blue-500/50 focus:bg-app-card transition-all shadow-lg dark:shadow-xl hover:shadow-xl hover:border-blue-300/30 dark:hover:border-app-border"
                />
                {value.length > 0 && (
                    <button
                        type="button"
                        onClick={handleClear}
                        className="absolute inset-y-0 right-4 flex items-center justify-center w-10 h-10 rounded-full text-app-muted hover:text-app-fg hover:bg-app-fg/10 transition-colors"
                        aria-label="Clear search"
                    >
                        <X size={20} />
                    </button>
                )}
            </div>
        </div>
    );
};

export default SearchBar;
