import { useState, useRef, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { ChevronDown, Globe } from "lucide-react";

export interface PlatformOption {
    id: string;
    name: string;
    icon: string;
    color: string;
}

// Supported search platforms (matching backend)
export const PLATFORMS: PlatformOption[] = [
    { id: "ytsearch", name: "YouTube", icon: "🔴", color: "#FF0000" },
    { id: "scsearch", name: "SoundCloud", icon: "🟠", color: "#FF5500" },
    { id: "bilisearch", name: "Bilibili", icon: "🔵", color: "#00A1D6" },
    { id: "nicosearch", name: "Niconico", icon: "⚪", color: "#CCCCCC" },
    { id: "dailymotion", name: "Dailymotion", icon: "🔷", color: "#0066DC" },
    { id: "gvsearch", name: "Google Video", icon: "🟢", color: "#4285F4" },
    { id: "yvsearch", name: "Yahoo Screen", icon: "🟣", color: "#720E9E" },
];

interface PlatformSelectorProps {
    selectedPlatform: string;
    onSelect: (platformId: string) => void;
    excludeShorts?: boolean;
    onExcludeShortsChange?: (checked: boolean) => void;
}


export default function PlatformSelector({
    selectedPlatform,
    onSelect,
    excludeShorts = true,
    onExcludeShortsChange
}: PlatformSelectorProps) {
    const [isOpen, setIsOpen] = useState(false);
    const dropdownRef = useRef<HTMLDivElement>(null);

    // Check if YouTube is selected
    const isYouTubeSelected = selectedPlatform === "ytsearch";

    // Get current platform details
    const currentPlatform = PLATFORMS.find(p => p.id === selectedPlatform) || PLATFORMS[0];

    // Close dropdown when clicking outside
    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
                setIsOpen(false);
            }
        };

        document.addEventListener("mousedown", handleClickOutside);
        return () => document.removeEventListener("mousedown", handleClickOutside);
    }, []);

    const handleSelect = (platformId: string) => {
        onSelect(platformId);
        setIsOpen(false);
    };

    return (
        <div className="flex flex-col gap-2">
            <div ref={dropdownRef} className="relative">
                {/* Trigger Button */}
                <button
                    onClick={() => setIsOpen(!isOpen)}
                    className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-white/5 border border-white/10 hover:border-neo-mint/50 hover:bg-white/10 transition-all duration-200 group"
                >
                    <Globe size={16} className="text-gray-400 group-hover:text-neo-mint transition-colors" />
                    <span className="text-lg">{currentPlatform.icon}</span>
                    <span className="text-sm text-white font-medium hidden sm:inline">{currentPlatform.name}</span>
                    <ChevronDown
                        size={14}
                        className={`text-gray-400 transition-transform duration-200 ${isOpen ? 'rotate-180' : ''}`}
                    />
                </button>

                {/* Dropdown Menu */}
                <AnimatePresence>
                    {isOpen && (
                        <motion.div
                            initial={{ opacity: 0, y: -10, scale: 0.95 }}
                            animate={{ opacity: 1, y: 0, scale: 1 }}
                            exit={{ opacity: 0, y: -10, scale: 0.95 }}
                            transition={{ duration: 0.15 }}
                            className="absolute right-0 top-full mt-2 w-56 py-2 z-50 rounded-xl border border-white/10 bg-[#1E1E22]/95 backdrop-blur-xl shadow-2xl shadow-black/50"
                        >
                            {/* Header */}
                            <div className="px-3 py-2 border-b border-white/5">
                                <span className="text-xs text-gray-500 uppercase tracking-wider font-semibold">
                                    Search Source
                                </span>
                            </div>

                            {/* Platform Options */}
                            <div className="py-1 max-h-64 overflow-y-auto">
                                {PLATFORMS.map((platform) => {
                                    const isSelected = platform.id === selectedPlatform;
                                    return (
                                        <button
                                            key={platform.id}
                                            onClick={() => handleSelect(platform.id)}
                                            className={`w-full flex items-center gap-3 px-3 py-2.5 text-left transition-all duration-150 ${isSelected
                                                ? 'bg-neo-mint/20 text-neo-mint'
                                                : 'text-white hover:bg-white/5'
                                                }`}
                                        >
                                            <span className="text-lg">{platform.icon}</span>
                                            <span className="text-sm font-medium flex-1">{platform.name}</span>
                                            {isSelected && (
                                                <motion.div
                                                    layoutId="platform-check"
                                                    className="w-2 h-2 rounded-full bg-neo-mint"
                                                />
                                            )}
                                        </button>
                                    );
                                })}
                            </div>

                            {/* Footer Hint */}
                            <div className="px-3 py-2 border-t border-white/5">
                                <span className="text-[10px] text-gray-500 leading-tight block">
                                    Source only applies to text search.<br />
                                    Direct URLs auto-detect the source.
                                </span>
                            </div>
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>

            {/* Exclude Shorts Checkbox - Only visible when YouTube is selected */}
            <AnimatePresence>
                {isYouTubeSelected && onExcludeShortsChange && (
                    <motion.label
                        initial={{ opacity: 0, height: 0 }}
                        animate={{ opacity: 1, height: 'auto' }}
                        exit={{ opacity: 0, height: 0 }}
                        transition={{ duration: 0.15 }}
                        className="flex items-center gap-2 px-2 cursor-pointer group"
                    >
                        <div className="relative">
                            <input
                                type="checkbox"
                                checked={excludeShorts}
                                onChange={(e) => onExcludeShortsChange(e.target.checked)}
                                className="sr-only peer"
                            />
                            <div className="w-4 h-4 rounded border border-white/20 bg-white/5 peer-checked:bg-neo-mint peer-checked:border-neo-mint transition-all duration-150 flex items-center justify-center">
                                {excludeShorts && (
                                    <svg className="w-3 h-3 text-black" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                                    </svg>
                                )}
                            </div>
                        </div>
                        <span className="text-xs text-gray-400 group-hover:text-white transition-colors">
                            Exclude Shorts
                        </span>
                    </motion.label>
                )}
            </AnimatePresence>
        </div>
    );
}

