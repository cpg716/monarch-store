
import { Gamepad2, Briefcase, Globe, Music, Cpu, Terminal, PenTool, LayoutGrid, LucideIcon } from 'lucide-react';
import { motion } from 'framer-motion';

interface CategoryGridProps {
    onSelectCategory: (category: string) => void;
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
        color: 'text-violet-600',
        borderColor: 'border-l-violet-600',
        iconBg: 'bg-violet-600/15'
    },
    {
        id: 'Office',
        label: 'Productivity',
        description: 'Office suites, note-taking, and professional tools.',
        popular: ['LibreOffice', 'Obsidian', 'Thunderbird', 'OnlyOffice'],
        icon: Briefcase,
        color: 'text-blue-500',
        borderColor: 'border-l-blue-500',
        iconBg: 'bg-blue-500/10'
    },
    {
        id: 'Network',
        label: 'Internet',
        description: 'Web browsers, messengers, and cloud clients.',
        popular: ['Firefox', 'Brave', 'Discord', 'Telegram'],
        icon: Globe,
        color: 'text-emerald-500',
        borderColor: 'border-l-emerald-500',
        iconBg: 'bg-emerald-500/10'
    },
    {
        id: 'AudioVideo',
        label: 'Multimedia',
        description: 'Video editors, music players, and streaming tools.',
        popular: ['VLC', 'Spotify', 'OBS Studio', 'Kdenlive'],
        icon: Music,
        color: 'text-rose-500',
        borderColor: 'border-l-rose-500',
        iconBg: 'bg-rose-500/10'
    },
    {
        id: 'Development',
        label: 'Development',
        description: 'IDEs, compilers, and programming environments.',
        popular: ['VS Code', 'Neovim', 'Postman', 'Docker'],
        icon: Terminal,
        color: 'text-amber-500',
        borderColor: 'border-l-amber-500',
        iconBg: 'bg-amber-500/10'
    },
    {
        id: 'Graphics',
        label: 'Graphics',
        description: 'Digital art, modeling, and photo editing software.',
        popular: ['GIMP', 'Blender', 'Inkscape', 'Krita'],
        icon: PenTool,
        color: 'text-fuchsia-500',
        borderColor: 'border-l-fuchsia-500',
        iconBg: 'bg-fuchsia-500/10'
    },
    {
        id: 'System',
        label: 'System',
        description: 'System utilities, file managers, and hardware tools.',
        popular: ['GParted', 'BleachBit', 'Htop', 'Timeshift'],
        icon: Cpu,
        color: 'text-slate-500',
        borderColor: 'border-l-slate-500',
        iconBg: 'bg-slate-500/10'
    },
    {
        id: 'Utility',
        label: 'Utilities',
        description: 'Essential small tools for everyday tasks.',
        popular: ['Calculator', 'Archive Mgr', 'Screenshot', 'Notes'],
        icon: LayoutGrid,
        color: 'text-lime-500',
        borderColor: 'border-l-lime-500',
        iconBg: 'bg-lime-500/10'
    },
];

export default function CategoryGrid({ onSelectCategory }: CategoryGridProps) {
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
            <h2 className="text-2xl font-bold mb-6 flex items-center gap-2 text-app-fg">
                <LayoutGrid className="text-blue-500" size={24} />
                Browse by Category
            </h2>

            <motion.div
                variants={container}
                initial="hidden"
                animate="show"
                className="grid grid-cols-1 md:grid-cols-2 gap-4"
            >
                {CATEGORIES.map((cat) => (
                    <motion.div
                        key={cat.id}
                        variants={item}
                        whileHover={{ scale: 1.02 }}
                        whileTap={{ scale: 0.98 }}
                        onClick={() => onSelectCategory(cat.id)}
                        className={`group relative p-6 rounded-2xl cursor-pointer border border-app-border bg-app-card hover:shadow-lg transition-all overflow-hidden border-l-4 ${cat.borderColor}`}
                    >
                        <div className="flex items-start justify-between">
                            <div className="flex flex-col gap-2">
                                <div className="flex items-center gap-3 mb-1">
                                    <div className={`p-2 rounded-lg ${cat.iconBg} ${cat.color}`}>
                                        <cat.icon size={24} />
                                    </div>
                                    <h3 className={`text-xl font-bold text-app-fg group-hover:${cat.color} transition-colors`}>
                                        {cat.label}
                                    </h3>
                                </div>
                                <p className="text-app-muted text-sm leading-relaxed mb-3">
                                    {cat.description}
                                </p>

                                {/* Popular Examples */}
                                <div className="flex flex-wrap gap-2 mt-auto">
                                    {cat.popular.map(app => (
                                        <span key={app} className="text-[10px] uppercase tracking-wider font-semibold px-2 py-1 rounded bg-app-subtle text-app-muted border border-app-border">
                                            {app}
                                        </span>
                                    ))}
                                </div>
                            </div>

                            {/* Decorative huge icon in background */}
                            <cat.icon className={`absolute -right-6 -bottom-6 w-32 h-32 opacity-5 rotate-12 group-hover:rotate-0 group-hover:scale-110 transition-transform duration-500 ${cat.color} pointer-events-none`} />
                        </div>
                    </motion.div>
                ))}
            </motion.div>
        </section>
    );
}
