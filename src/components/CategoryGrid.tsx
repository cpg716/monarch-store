import { Gamepad2, Briefcase, Globe, Music, Cpu, Terminal, PenTool, LayoutGrid, LucideIcon } from 'lucide-react';
import { motion } from 'framer-motion';

interface CategoryGridProps {
    onSelectCategory: (category: string) => void;
    selectedCategoryId?: string | null;
}

export interface CategoryData {
    id: string;
    label: string;
    description: string;
    popular: string[];
    icon: LucideIcon;
    color: string;
    borderColor: string;
    iconBg: string;
}

export const CATEGORIES: CategoryData[] = [
    {
        id: 'Game',
        label: 'Games',
        description: 'Immersive worlds, retro emulators, and competitive gaming tools.',
        popular: ['Steam', 'Lutris', 'Minecraft', 'Heroic'],
        icon: Gamepad2,
        color: 'text-violet-600 dark:text-violet-400',
        borderColor: 'border-l-violet-600',
        iconBg: 'bg-violet-100 dark:bg-violet-500/15'
    },
    {
        id: 'Office',
        label: 'Productivity',
        description: 'Office suites, note-taking, and professional tools.',
        popular: ['LibreOffice', 'Obsidian', 'Thunderbird', 'OnlyOffice'],
        icon: Briefcase,
        color: 'text-blue-600 dark:text-blue-400',
        borderColor: 'border-l-blue-500',
        iconBg: 'bg-blue-100 dark:bg-blue-500/10'
    },
    {
        id: 'Network',
        label: 'Internet',
        description: 'Web browsers, messengers, and cloud clients.',
        popular: ['Firefox', 'Brave', 'Discord', 'Telegram'],
        icon: Globe,
        color: 'text-emerald-600 dark:text-emerald-400',
        borderColor: 'border-l-emerald-500',
        iconBg: 'bg-emerald-100 dark:bg-emerald-500/10'
    },
    {
        id: 'AudioVideo',
        label: 'Multimedia',
        description: 'Video editors, music players, and streaming tools.',
        popular: ['VLC', 'Spotify', 'OBS Studio', 'Kdenlive'],
        icon: Music,
        color: 'text-rose-600 dark:text-rose-400',
        borderColor: 'border-l-rose-500',
        iconBg: 'bg-rose-100 dark:bg-rose-500/10'
    },
    {
        id: 'Development',
        label: 'Development',
        description: 'IDEs, compilers, and programming environments.',
        popular: ['VS Code', 'Neovim', 'Postman', 'Docker'],
        icon: Terminal,
        color: 'text-amber-600 dark:text-amber-400',
        borderColor: 'border-l-amber-500',
        iconBg: 'bg-amber-100 dark:bg-amber-500/10'
    },
    {
        id: 'Graphics',
        label: 'Graphics',
        description: 'Digital art, modeling, and photo editing software.',
        popular: ['GIMP', 'Blender', 'Inkscape', 'Krita'],
        icon: PenTool,
        color: 'text-fuchsia-600 dark:text-fuchsia-400',
        borderColor: 'border-l-fuchsia-500',
        iconBg: 'bg-fuchsia-100 dark:bg-fuchsia-500/10'
    },
    {
        id: 'System',
        label: 'System',
        description: 'System utilities, file managers, and hardware tools.',
        popular: ['GParted', 'BleachBit', 'Htop', 'Timeshift'],
        icon: Cpu,
        color: 'text-slate-600 dark:text-slate-400',
        borderColor: 'border-l-slate-500',
        iconBg: 'bg-slate-100 dark:bg-slate-500/10'
    },
    {
        id: 'Utilities',
        label: 'Utilities',
        description: 'Essential small tools for everyday tasks.',
        popular: ['Calculator', 'Archive Mgr', 'Screenshot', 'Notes'],
        icon: LayoutGrid,
        color: 'text-lime-600 dark:text-lime-400',
        borderColor: 'border-l-lime-500',
        iconBg: 'bg-lime-100 dark:bg-lime-500/10'
    },
];

export default function CategoryGrid({ onSelectCategory, selectedCategoryId }: CategoryGridProps) {
    const container = {
        hidden: { opacity: 0 },
        show: {
            opacity: 1,
            transition: {
                staggerChildren: 0.1
            }
        }
    };

    const item = {
        hidden: { opacity: 0, y: 20 },
        show: { opacity: 1, y: 0 }
    };

    return (
        <section className="w-full">
            <h2 className="text-2xl font-bold mb-6 flex items-center gap-2 text-slate-800 dark:text-white">
                <LayoutGrid className="text-blue-500" size={24} />
                Browse by Category
            </h2>

            <motion.div
                variants={container}
                initial="hidden"
                animate="show"
                className="grid grid-cols-2 gap-4"
            >
                {CATEGORIES.map((cat) => (
                    <motion.div
                        key={cat.id}
                        variants={item}
                        whileHover={{ scale: 1.02 }}
                        whileTap={{ scale: 0.98 }}
                        onClick={() => onSelectCategory(cat.id)}
                        className={`group relative p-6 rounded-3xl cursor-pointer border border-black/5 dark:border-white/5 bg-white/60 dark:bg-black/20 hover:bg-white/80 dark:hover:bg-black/40 hover:shadow-xl transition-all overflow-hidden backdrop-blur-sm ${selectedCategoryId === cat.id ? 'ring-2 ring-blue-500' : ''}`}
                    >
                        <div className="flex items-start justify-between relative z-10">
                            <div className="flex flex-col gap-3">
                                <div className="flex items-center gap-4 mb-2">
                                    <div className={`p-3 rounded-2xl ${cat.iconBg} ${cat.color} border border-black/5 dark:border-white/5 shadow-inner`}>
                                        <cat.icon size={26} />
                                    </div>
                                    <h3 className={`text-2xl font-bold text-slate-800 dark:text-white group-hover:${cat.color} transition-colors tracking-tight`}>
                                        {cat.label}
                                    </h3>
                                </div>
                                <p className="text-slate-600 dark:text-indigo-100/60 text-sm leading-relaxed mb-4 font-medium max-w-[80%]">
                                    {cat.description}
                                </p>

                                {/* Popular Examples */}
                                <div className="flex flex-wrap gap-2 mt-auto">
                                    {cat.popular.map(app => (
                                        <span
                                            key={app}
                                            onClick={(e) => { e.stopPropagation(); onSelectCategory(cat.id); }}
                                            className="text-[10px] uppercase tracking-wider font-bold px-2.5 py-1 rounded-lg bg-black/5 dark:bg-white/5 text-slate-500 dark:text-white/50 border border-black/5 dark:border-white/5 hover:bg-black/10 dark:hover:bg-white/10 hover:text-slate-700 dark:hover:text-white hover:border-black/10 dark:hover:border-white/20 transition-all cursor-default"
                                        >
                                            {app}
                                        </span>
                                    ))}
                                </div>
                            </div>
                        </div>

                        {/* Decorative huge icon in background */}
                        <cat.icon className={`absolute -right-8 -bottom-8 w-48 h-48 opacity-[0.03] rotate-12 group-hover:rotate-0 group-hover:scale-110 transition-transform duration-700 ease-out ${cat.color} pointer-events-none`} />

                        {/* Gradient Glow */}
                        <div className={`absolute inset-0 bg-gradient-to-br ${cat.iconBg.replace('bg-', 'from-').replace('/10', '/0').replace('/15', '/0')} to-transparent opacity-0 group-hover:opacity-20 transition-opacity duration-500`} />
                    </motion.div>
                ))}
            </motion.div>
        </section>
    );
}
