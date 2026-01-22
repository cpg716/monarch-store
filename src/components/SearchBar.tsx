import React from 'react';
import { Search } from 'lucide-react';

interface SearchBarProps {
    value: string;
    onChange: (value: string) => void;
}

const SearchBar: React.FC<SearchBarProps> = ({ value, onChange }) => {
    return (
        <div className="relative w-full max-w-3xl group transition-all">
            {/* Gradient border effect on focus */}
            <div className="absolute -inset-0.5 bg-gradient-to-r from-blue-500 via-purple-500 to-pink-500 rounded-[2.5rem] blur opacity-25 group-hover:opacity-50 group-focus-within:opacity-75 transition-opacity duration-500" />

            <div className="relative">
                <div className="absolute inset-y-0 left-6 flex items-center pointer-events-none text-app-muted group-focus-within:text-blue-500 transition-colors">
                    <Search size={24} className="group-focus-within:scale-110 transition-transform" />
                </div>
                <input
                    type="text"
                    value={value}
                    onChange={(e) => onChange(e.target.value)}
                    placeholder="Search for apps (e.g. firefox, spotify, discord)"
                    className="w-full bg-app-card border-2 border-app-border/50 rounded-[2rem] py-5 pl-16 pr-6 text-xl text-app-fg placeholder-app-muted/60 focus:outline-none focus:border-blue-500/50 focus:bg-app-card transition-all shadow-xl hover:shadow-2xl hover:border-app-border"
                />
            </div>
        </div>
    );
};

export default SearchBar;
