import logoFull from '../assets/logo_full.png';

export default function HeroSection() {

    return (
        <div className="relative w-full rounded-3xl overflow-hidden mb-8 group select-none shadow-lg">
            {/* Light Background */}
            <div className="absolute inset-0 bg-gradient-to-br from-purple-300 via-blue-300 to-cyan-200 transition-all duration-500 group-hover:scale-105" />

            {/* Animated Shapes */}
            <div className="absolute top-[-50%] left-[-20%] w-[800px] h-[800px] rounded-full bg-white/30 blur-3xl animate-pulse" />
            <div className="absolute bottom-[-20%] right-[-10%] w-[600px] h-[600px] rounded-full bg-purple-400/20 blur-3xl" />

            {/* Glass Overlay */}
            <div className="absolute inset-0 bg-white/20 backdrop-blur-[2px]" />

            {/* Content Container - NOW HORIZONTAL FOR SLIMMER PROFILE */}
            <div className="relative z-10 flex flex-row items-center justify-center gap-8 px-10 py-6 text-slate-800">

                {/* Left: Impactful Logo */}
                <div className="flex-shrink-0 animate-fade-in-up" style={{ animationDelay: '0.1s' }}>
                    <img
                        src={logoFull}
                        alt="MonARCH Store"
                        className="h-24 object-contain"
                        style={{
                            filter: 'drop-shadow(0 4px 6px rgba(0,0,0,0.2))'
                        }}
                    />
                </div>

                {/* Right: Messaging */}
                <div className="flex flex-col text-left max-w-2xl">
                    {/* Tagline */}
                    <h2 className="text-3xl font-black text-slate-800 mb-2 tracking-tight animate-fade-in-up leading-tight" style={{ animationDelay: '0.2s' }}>
                        Order from <span className="text-transparent bg-clip-text bg-gradient-to-r from-purple-600 to-indigo-600">Chaos</span>.
                    </h2>

                    {/* Description */}
                    <div className="text-sm text-slate-700 leading-relaxed animate-fade-in-up font-medium" style={{ animationDelay: '0.25s' }}>
                        <p>
                            The ultimate <strong>Chaotic-AUR</strong> interface. Pre-built binaries, fast downloads, and easy installation.
                        </p>
                        <p className="mt-1 text-slate-600 font-normal text-xs">
                            Supports <strong>Arch Official</strong>, <strong>AUR</strong> (no terminal needed), <strong>CachyOS</strong>, <strong>Garuda</strong>, <strong>Manjaro</strong>, & <strong>EndeavourOS</strong>.
                        </p>
                    </div>
                </div>
            </div>

            {/* Decor image (Arch Logo Abstract) */}
            <div className="absolute right-0 top-1/2 -translate-y-1/2 w-96 h-96 opacity-15 pointer-events-none">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="0.5" className="w-full h-full text-purple-600 rotate-12">
                    <path d="M12 2L2 22h20L12 2zm0 3.5L18.5 20h-13L12 5.5z" />
                </svg>
            </div>
        </div>
    );
}
