import { motion } from "framer-motion";
import {
    Terminal,
    Battery,
    Clock,
} from "lucide-react";
import { useEffect, useState } from "react";

function AboutUs() {
    const [time, setTime] = useState(new Date());

    useEffect(() => {
        const timer = setInterval(() => setTime(new Date()), 1000);
        return () => clearInterval(timer);
    }, []);

    const [batteryLevel, setBatteryLevel] = useState(100);
    const [isCharging, setIsCharging] = useState(false);

    useEffect(() => {
        // Battery API
        // @ts-ignore - Navigator.getBattery is standard in Chromium but TS might complain
        if (navigator.getBattery) {
            // @ts-ignore
            navigator.getBattery().then(battery => {
                const updateBattery = () => {
                    setBatteryLevel(Math.round(battery.level * 100));
                    setIsCharging(battery.charging);
                };
                updateBattery();
                battery.addEventListener('levelchange', updateBattery);
                battery.addEventListener('chargingchange', updateBattery);
                return () => {
                    battery.removeEventListener('levelchange', updateBattery);
                    battery.removeEventListener('chargingchange', updateBattery);
                };
            });
        }
    }, []);

    return (
        <div className="relative h-full w-full bg-transparent text-[#a7a7a7] font-mono overflow-hidden select-none p-4">
            {/* Background Grid - Hyperland Style */}
            <div className="absolute inset-0 grid grid-cols-[repeat(40,minmax(0,1fr))] opacity-[0.03] pointer-events-none">
                {[...Array(1600)].map((_, i) => (
                    <div key={i} className="border-[0.5px] border-white/20" />
                ))}
            </div>

            <div className="relative z-10 flex flex-col h-full gap-4">
                {/* Top Polybar */}
                <motion.div
                    initial={{ y: -50, opacity: 0 }}
                    animate={{ y: 0, opacity: 1 }}
                    className="h-10 bg-[#11111b]/60 backdrop-blur-[2px] rounded-lg flex items-center justify-between px-4 border border-white/20 shadow-lg"
                >
                    {/* Left: Window Title equivalent */}
                    <div className="text-xs tracking-widest uppercase text-cyan-400 font-bold flex items-center gap-2">
                        <Terminal size={12} />
                        <span>Alpha Tube v0.1.0-nightly</span>
                    </div>

                    {/* Right: Modules (Only Battery & Time) */}
                    <div className="flex items-center gap-4 text-xs font-semibold">
                        <div className={`flex items-center gap-2 px-2 py-1 rounded border ${isCharging ? 'text-green-400 border-green-500/20 bg-green-500/10' : 'text-pink-400 border-pink-500/20 bg-pink-500/10'}`}>
                            <Battery size={12} className={isCharging ? "animate-pulse" : ""} />
                            <span>{batteryLevel}%</span>
                        </div>
                        <div className="flex items-center gap-2 text-yellow-400 bg-yellow-500/10 px-2 py-1 rounded border border-yellow-500/20">
                            <Clock size={12} />
                            <span>{time.toLocaleTimeString([], { hour12: false })}</span>
                        </div>
                    </div>
                </motion.div>

                {/* Main Tiling Grid */}
                <div className="flex-1 grid grid-cols-12 grid-rows-6 gap-4">

                    {/* Hero Window (Top Left) - KEPT AS IS (Main Visual) */}
                    <motion.div
                        initial={{ scale: 0.9, opacity: 0 }}
                        animate={{ scale: 1, opacity: 1 }}
                        transition={{ delay: 0.1 }}
                        className="col-span-8 row-span-4 bg-[#0a0a0f]/20 backdrop-blur-md border border-cyan-500/20 rounded-xl relative overflow-hidden group flex flex-col items-center justify-center shadow-[0_4px_30px_rgba(0,0,0,0.1)]"
                    >
                        {/* Window Decoration */}
                        <div className="absolute top-0 inset-x-0 h-6 bg-white/5 flex items-center justify-between px-3 border-b border-white/10">
                            <span className="text-[10px] text-cyan-500 tracking-wider">~/alpha-tube/core</span>
                            <div className="flex gap-1.5">
                                <div className="w-2 h-2 rounded-full bg-red-500/50" />
                                <div className="w-2 h-2 rounded-full bg-yellow-500/50" />
                                <div className="w-2 h-2 rounded-full bg-green-500/50" />
                            </div>
                        </div>

                        {/* Content */}
                        <div className="relative z-10 flex flex-col items-center">
                            {/* Divine Flying Alpha Logo */}
                            <motion.div
                                animate={{
                                    y: [0, -10, 0],
                                    filter: [
                                        "drop-shadow(0 0 20px rgba(6,182,212,0.3))",
                                        "drop-shadow(0 0 40px rgba(236,72,153,0.5))",
                                        "drop-shadow(0 0 20px rgba(6,182,212,0.3))"
                                    ]
                                }}
                                transition={{
                                    y: { duration: 4, repeat: Infinity, ease: "easeInOut" },
                                    filter: { duration: 3, repeat: Infinity, ease: "easeInOut" }
                                }}
                                className="mb-6 relative"
                            >
                                <motion.span
                                    animate={{
                                        color: ["#22d3ee", "#e879f9", "#d946ef", "#22d3ee"],
                                        textShadow: [
                                            "0 0 20px rgba(34,211,238,0.8)",
                                            "0 0 40px rgba(232,121,249,0.8)",
                                            "0 0 20px rgba(34,211,238,0.8)"
                                        ]
                                    }}
                                    transition={{ duration: 4, repeat: Infinity, ease: "easeInOut" }}
                                    className="text-8xl font-black"
                                >
                                    α
                                </motion.span>
                            </motion.div>

                            <h1 className="text-4xl md:text-5xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-cyan-400 via-purple-500 to-pink-500 tracking-tighter mb-2 drop-shadow-md">
                                ALPHA TUBE
                            </h1>
                            <p className="text-sm font-medium text-white/50 uppercase tracking-[0.3em]">
                                Ultrafast Video Downloader
                            </p>
                        </div>
                    </motion.div>

                    {/* Terminal Info Window (Top Right) - KEPT AS IS (User liked it?) */}
                    {/* User didn't complain about this one specifically, but complained about 'lead engineer' and 'corporation'. 
                       I'll remove the border/bg from others but keep this one as a "Terminal" which makes sense to have a window. */}
                    <motion.div
                        initial={{ x: 50, opacity: 0 }}
                        animate={{ x: 0, opacity: 1 }}
                        transition={{ delay: 0.2 }}
                        className="col-span-4 row-span-4 bg-[#0a0a0f]/20 backdrop-blur-md border border-pink-500/20 rounded-xl relative overflow-hidden flex flex-col font-mono text-xs shadow-[0_4px_30px_rgba(0,0,0,0.1)]"
                    >
                        <div className="h-6 bg-pink-500/10 border-b border-pink-500/20 flex items-center px-3 gap-2">
                            <Terminal size={10} className="text-pink-400" />
                            <span className="text-pink-400/80">user@alpha-tube:~$</span>
                        </div>
                        <div className="p-4 space-y-2 text-white/70 overflow-y-auto custom-scrollbar">
                            <div className="flex gap-2">
                                <span className="text-green-500">➜</span>
                                <span className="text-blue-400">neofetch</span>
                            </div>
                            <div className="grid grid-cols-[80px_1fr] gap-x-2 gap-y-1 mt-2 content-start">
                                <span className="text-pink-500 font-bold">OS</span> <span>Alpha OS v1.0</span>
                                <span className="text-pink-500 font-bold">Host</span> <span>Tauri Engine</span>
                                <span className="text-pink-500 font-bold">Kernel</span> <span>Rust 1.75.0</span>
                                <span className="text-pink-500 font-bold">Uptime</span> <span>Forever</span>
                                <span className="text-pink-500 font-bold">Shell</span> <span>React 19</span>
                                <span className="text-pink-500 font-bold">Theme</span> <span>Cyber-Glass</span>
                                <span className="text-pink-500 font-bold">CPU</span> <span>Quantum Core</span>
                            </div>
                            <div className="flex gap-2 animate-pulse mt-4">
                                <span className="text-green-500">➜</span>
                                <span className="w-2 h-4 bg-white/50 block"></span>
                            </div>
                        </div>
                    </motion.div>


                    {/* Engineered By - REDESIGNED: Minimal, floating, visual */}
                    <motion.div
                        initial={{ y: 50, opacity: 0 }}
                        animate={{ y: 0, opacity: 1 }}
                        transition={{ delay: 0.4 }}
                        className="col-span-4 row-span-2 flex flex-col justify-center items-center text-center p-4 relative"
                    >
                        {/* Subtle glowing backdrop instead of a box */}
                        <div className="absolute inset-0 bg-gradient-to-t from-yellow-500/5 to-transparent opacity-50 rounded-xl" />

                        <h3 className="text-yellow-500/80 text-xs font-bold tracking-[0.2em] uppercase mb-2">Architect</h3>
                        <div className="relative">
                            <h2 className="text-3xl md:text-4xl font-black text-white tracking-tighter mix-blend-overlay">
                                ARYAN SINGH
                            </h2>
                            {/* Glitchy underline */}
                            <motion.div
                                animate={{ width: ["0%", "100%", "0%"], left: ["0%", "0%", "100%"] }}
                                transition={{ duration: 3, repeat: Infinity, ease: "easeInOut" }}
                                className="h-0.5 bg-yellow-500 absolute bottom-0"
                            />
                        </div>
                    </motion.div>

                    {/* Corporation/Footer - REDESIGNED: Minimal footer text */}
                    <motion.div
                        initial={{ x: 50, opacity: 0 }}
                        animate={{ x: 0, opacity: 1 }}
                        transition={{ delay: 0.5 }}
                        className="col-span-8 row-span-2 flex flex-col justify-center items-center text-center p-6 relative"
                    >
                        {/* Subtle backdrop */}
                        <div className="absolute inset-0 bg-white/[0.02] rounded-xl border border-white/5" />

                        <p className="relative z-10 text-white/60 italic font-light text-sm max-w-lg leading-relaxed mb-4">
                            "You are what your deep, driving desire is. As your desire is, so is your will. As your will is, so is your deed. As your deed is, so is your destiny."
                        </p>

                        <div className="relative z-10 flex items-center gap-2 mb-6">
                            <span className="text-sm shadow-none filter-none">🌸</span>
                            <span className="text-xs uppercase tracking-widest font-bold text-transparent bg-clip-text bg-gradient-to-r from-yellow-200 via-yellow-100 to-amber-200 drop-shadow-[0_0_8px_rgba(253,224,71,0.6)] animate-pulse">
                                Brihadaranyaka Upanishad 4.4.5
                            </span>
                        </div>
                        <div className="relative z-10 mt-6 flex items-center gap-6 text-xs uppercase tracking-[0.3em] font-bold">
                            <span className="text-cyan-400 drop-shadow-[0_0_15px_rgba(34,211,238,0.8)] animate-pulse">© Aryan Singh</span>
                            <div className="w-1 h-1 bg-white/20 rounded-full" />
                            <span className="text-white/30">Est. 2026</span>
                        </div>
                    </motion.div>

                </div>
            </div>
        </div>
    );
}

export default AboutUs;

