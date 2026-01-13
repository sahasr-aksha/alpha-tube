import { MediaPlayer, MediaProvider, MediaPlayerInstance } from '@vidstack/react';
import { DefaultVideoLayout, defaultLayoutIcons } from '@vidstack/react/player/layouts/default';
import { useEffect, useState, useRef } from 'react';
import { createPortal } from 'react-dom';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { Download, ChevronDown, Check, Loader2 } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import '@vidstack/react/player/styles/default/theme.css';
import '@vidstack/react/player/styles/default/layouts/video.css';

interface StreamPlayerProps {
    videoUrl: string;
    videoTitle: string;
    onClose: () => void;
    onDownload: () => void;
}

interface StreamResponse {
    proxy_url: string;
    current_quality: string;
    available_qualities: string[];
}

interface QualityReady {
    quality: string;
}

export default function StreamPlayer({ videoUrl, videoTitle, onClose, onDownload }: StreamPlayerProps) {
    const [streamUrl, setStreamUrl] = useState<string>("");
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    // Quality state - simplified: just names, no URLs
    const [availableQualities, setAvailableQualities] = useState<string[]>([]);
    const [currentQuality, setCurrentQuality] = useState<string>("360p");
    const [qualityMenuOpen, setQualityMenuOpen] = useState(false);
    const [switchingQuality, setSwitchingQuality] = useState(false);
    const [fetchingQualities, setFetchingQualities] = useState(false);

    // Volume persistence
    const [volume, setVolume] = useState(() => {
        const saved = localStorage.getItem("playerVolume");
        return saved ? parseFloat(saved) : 1;
    });

    // Player ref for timestamp preservation
    const playerRef = useRef<MediaPlayerInstance>(null);
    const savedTimeRef = useRef<number>(0);

    // Handle volume change and persist to localStorage
    const handleVolumeChange = (newVolume: number) => {
        setVolume(newVolume);
        localStorage.setItem("playerVolume", String(newVolume));
    };

    // Start streaming with unified command
    useEffect(() => {
        let unlistenQualityReady: UnlistenFn | null = null;

        const startPlayback = async () => {
            setLoading(true);
            setError(null);

            try {
                console.log("[StreamPlayer] Starting playback with stream_video...");

                // Use unified stream_video command - starts with 360p default
                const result = await invoke<StreamResponse>("stream_video", {
                    videoUrl: videoUrl,
                    quality: null, // null = default 360p
                });

                console.log("[StreamPlayer] Got response:", result);
                setCurrentQuality(result.current_quality);
                setAvailableQualities(result.available_qualities);
                setStreamUrl(`${result.proxy_url}?t=${Date.now()}`);
                setLoading(false);

                // Listen for additional qualities becoming ready
                unlistenQualityReady = await listen<QualityReady>("quality-ready", (event) => {
                    console.log(`[StreamPlayer] Quality ready: ${event.payload.quality}`);
                    setAvailableQualities(prev => {
                        if (prev.includes(event.payload.quality)) return prev;
                        const updated = [...prev, event.payload.quality];
                        // Sort by resolution
                        return updated.sort((a, b) => {
                            const aNum = parseInt(a.replace("p", ""));
                            const bNum = parseInt(b.replace("p", ""));
                            return aNum - bNum;
                        });
                    });
                });

                // Fetch all qualities in background
                setFetchingQualities(true);
                try {
                    const allQualities = await invoke<string[]>("fetch_all_qualities", {
                        videoUrl: videoUrl,
                    });
                    console.log("[StreamPlayer] All available qualities:", allQualities);
                    setAvailableQualities(allQualities);
                } catch (e) {
                    console.error("[StreamPlayer] Error fetching all qualities:", e);
                } finally {
                    setFetchingQualities(false);
                }

            } catch (err) {
                console.error("[StreamPlayer] Failed to start streaming:", err);
                setError(String(err));
                setLoading(false);
            }
        };

        startPlayback();

        return () => {
            if (unlistenQualityReady) unlistenQualityReady();
        };
    }, [videoUrl]);

    // Handle quality change - uses unified stream_video command
    const handleQualityChange = async (quality: string) => {
        if (quality === currentQuality || switchingQuality) return;

        console.log(`[StreamPlayer] === QUALITY SWITCH START ===`);
        console.log(`[StreamPlayer] From: ${currentQuality} -> To: ${quality}`);
        setSwitchingQuality(true);
        setQualityMenuOpen(false);

        // Save current playback time
        if (playerRef.current) {
            savedTimeRef.current = playerRef.current.currentTime || 0;
            console.log(`[StreamPlayer] Saved time: ${savedTimeRef.current}s`);
        }

        try {
            // Use unified stream_video command with specific quality
            // Backend handles caching + freshness automatically
            console.log(`[StreamPlayer] Calling stream_video with quality: ${quality}`);
            const result = await invoke<StreamResponse>("stream_video", {
                videoUrl: videoUrl,
                quality: quality,
            });

            console.log(`[StreamPlayer] Got response:`, result);
            console.log(`[StreamPlayer] Proxy URL: ${result.proxy_url}`);
            console.log(`[StreamPlayer] Current quality confirmed: ${result.current_quality}`);

            // Update player with cache-busting URL
            const newUrl = `${result.proxy_url}?q=${quality}&t=${Date.now()}`;
            console.log(`[StreamPlayer] Setting streamUrl to: ${newUrl}`);
            setStreamUrl(newUrl);
            setCurrentQuality(quality);
            setAvailableQualities(result.available_qualities);
            console.log(`[StreamPlayer] === QUALITY SWITCH COMPLETE ===`);
        } catch (err) {
            console.error("[StreamPlayer] Failed to switch quality:", err);
        } finally {
            setSwitchingQuality(false);
        }
    };

    // Restore playback position after quality switch
    const handleCanPlay = () => {
        if (savedTimeRef.current > 0 && playerRef.current) {
            console.log(`[StreamPlayer] Restoring time: ${savedTimeRef.current}s`);
            playerRef.current.currentTime = savedTimeRef.current;
            savedTimeRef.current = 0;
        }
    };

    // Handle Escape key
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') onClose();
        };
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [onClose]);

    // Close quality menu when clicking outside
    useEffect(() => {
        const handleClickOutside = () => setQualityMenuOpen(false);
        if (qualityMenuOpen) {
            document.addEventListener('click', handleClickOutside);
            return () => document.removeEventListener('click', handleClickOutside);
        }
    }, [qualityMenuOpen]);

    return createPortal(
        <div
            className="fixed inset-0 z-[9999] bg-black/95 backdrop-blur-md flex items-center justify-center animate-in fade-in duration-300"
            style={{
                '--brand': '#00FFA3',
                '--media-slider-track-fill-bg': '#00FFA3',
                '--media-slider-thumb-bg': '#00FFA3',
                '--media-slider-thumb-border': '2px solid #fff',
                '--media-focus-ring-color': '#00F0FF',
            } as React.CSSProperties}
        >
            {/* Header */}
            <div className="absolute top-0 left-0 right-0 p-4 flex justify-between items-center bg-gradient-to-b from-black/80 to-transparent z-50">
                <h2 className="text-white font-mono text-lg truncate drop-shadow-[0_0_10px_rgba(0,255,163,0.5)] pl-4 max-w-[50%]">
                    {videoTitle}
                </h2>
                <div className="flex items-center gap-3">
                    {/* Quality Selector */}
                    {availableQualities.length > 0 && (
                        <div className="relative">
                            <motion.button
                                whileHover={{ scale: 1.02 }}
                                whileTap={{ scale: 0.98 }}
                                onClick={(e) => {
                                    e.stopPropagation();
                                    setQualityMenuOpen(!qualityMenuOpen);
                                }}
                                className={`flex items-center gap-2 px-3 py-2 rounded-lg font-mono text-sm transition-all border ${switchingQuality
                                    ? 'bg-neo-mint/20 border-neo-mint/50 text-neo-mint'
                                    : 'bg-black/50 border-white/20 text-white hover:border-neo-mint/50'
                                    }`}
                            >
                                {switchingQuality ? (
                                    <Loader2 size={14} className="animate-spin text-neo-mint" />
                                ) : (
                                    <span className="text-neo-mint font-bold">{currentQuality}</span>
                                )}
                                <ChevronDown size={14} className={`transition-transform ${qualityMenuOpen ? 'rotate-180' : ''}`} />
                            </motion.button>

                            <AnimatePresence>
                                {qualityMenuOpen && (
                                    <motion.div
                                        initial={{ opacity: 0, y: -10 }}
                                        animate={{ opacity: 1, y: 0 }}
                                        exit={{ opacity: 0, y: -10 }}
                                        className="absolute top-full right-0 mt-2 bg-black/90 backdrop-blur-md border border-white/20 rounded-lg overflow-hidden min-w-[120px] shadow-xl"
                                        onClick={(e) => e.stopPropagation()}
                                    >
                                        {availableQualities.map((quality) => (
                                            <button
                                                key={quality}
                                                onClick={() => handleQualityChange(quality)}
                                                className={`w-full px-4 py-2 text-left font-mono text-sm flex items-center justify-between gap-3 transition-all ${quality === currentQuality
                                                    ? 'bg-neo-mint/20 text-neo-mint'
                                                    : 'text-white/80 hover:bg-white/10 hover:text-white cursor-pointer'
                                                    }`}
                                            >
                                                <span>{quality}</span>
                                                {quality === currentQuality && <Check size={14} />}
                                            </button>
                                        ))}
                                        {fetchingQualities && (
                                            <div className="px-4 py-2 text-white/40 text-xs flex items-center gap-2">
                                                <Loader2 size={12} className="animate-spin" />
                                                Loading more...
                                            </div>
                                        )}
                                    </motion.div>
                                )}
                            </AnimatePresence>
                        </div>
                    )}

                    {/* Download Button */}
                    <motion.button
                        whileHover={{ scale: 1.05 }}
                        whileTap={{ scale: 0.95 }}
                        onClick={onDownload}
                        className="flex items-center gap-2 px-4 py-2 bg-neo-mint text-black font-bold rounded-lg transition-all hover:shadow-[0_0_15px_rgba(0,255,163,0.5)]"
                    >
                        <Download size={16} />
                        DOWNLOAD
                    </motion.button>

                    {/* Close Button */}
                    <button
                        onClick={onClose}
                        className="bg-black/50 hover:bg-red-500/80 text-white border border-white/20 hover:border-red-500 rounded-lg px-4 py-2 font-mono text-sm transition-all duration-300"
                    >
                        [ CLOSE ]
                    </button>
                </div>
            </div>

            {/* Main Player Container */}
            <div
                className="w-full max-w-[90vw] aspect-video rounded-2xl overflow-hidden shadow-[0_0_50px_rgba(0,255,163,0.2)] border border-white/10 relative"
            >
                {/* Visual Corners */}
                <div className="absolute top-0 left-0 w-8 h-8 border-t-2 border-l-2 border-neo-mint rounded-tl-lg z-20 pointer-events-none opacity-50" />
                <div className="absolute top-0 right-0 w-8 h-8 border-t-2 border-r-2 border-neo-mint rounded-tr-lg z-20 pointer-events-none opacity-50" />
                <div className="absolute bottom-0 left-0 w-8 h-8 border-b-2 border-l-2 border-neo-mint rounded-bl-lg z-20 pointer-events-none opacity-50" />
                <div className="absolute bottom-0 right-0 w-8 h-8 border-b-2 border-r-2 border-neo-mint rounded-br-lg z-20 pointer-events-none opacity-50" />

                {loading && (
                    <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/80 z-30">
                        <div className="w-12 h-12 border-4 border-neo-mint/20 border-t-neo-mint rounded-full animate-spin mb-4" />
                        <p className="text-white/60 font-mono text-sm">Starting stream...</p>
                    </div>
                )}

                {error && (
                    <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/80 z-30">
                        <p className="text-red-400 font-mono text-sm mb-4">Failed to load stream</p>
                        <p className="text-white/40 font-mono text-xs max-w-md text-center">{error}</p>
                    </div>
                )}

                {/* Buffering overlay during quality switch */}
                {switchingQuality && (
                    <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/60 z-30 backdrop-blur-sm">
                        <div className="w-16 h-16 border-4 border-neo-mint/20 border-t-neo-mint rounded-full animate-spin mb-4" />
                        <p className="text-neo-mint font-mono text-sm font-bold">Switching Quality...</p>
                        <p className="text-white/50 font-mono text-xs mt-2">Please wait</p>
                    </div>
                )}

                {streamUrl && !loading && (
                    <MediaPlayer
                        key={streamUrl}
                        ref={playerRef}
                        src={streamUrl}
                        viewType="video"
                        streamType="on-demand"
                        logLevel="warn"
                        className="w-full h-full"
                        autoPlay
                        volume={volume}
                        onVolumeChange={(detail) => handleVolumeChange(detail.volume)}
                        onCanPlay={handleCanPlay}
                    >
                        <MediaProvider />
                        <DefaultVideoLayout icons={defaultLayoutIcons} />
                    </MediaPlayer>
                )}
            </div>
        </div>,
        document.body
    );
}
