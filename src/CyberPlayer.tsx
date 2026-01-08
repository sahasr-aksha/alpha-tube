import { MediaPlayer, MediaProvider, useAudioOptions } from '@vidstack/react';
import { DefaultVideoLayout, defaultLayoutIcons } from '@vidstack/react/player/layouts/default';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import '@vidstack/react/player/styles/default/theme.css';
import '@vidstack/react/player/styles/default/layouts/video.css';

// Audio Track Selector Component (renders inside MediaPlayer context)
function AudioTrackSelector() {
    const options = useAudioOptions();
    const [isOpen, setIsOpen] = useState(false);

    if (options.length <= 1) return null; // No selector needed for single track

    const selectedTrack = options.find(opt => opt.selected);

    return (
        <div className="absolute top-16 right-4 z-[60]">
            <button
                onClick={() => setIsOpen(!isOpen)}
                className="bg-black/70 hover:bg-[#FF2E63] text-white border border-white/30 rounded-lg px-3 py-1.5 font-mono text-xs transition-all backdrop-blur-sm flex items-center gap-2"
            >
                🔊 {selectedTrack?.label || 'Audio'}
            </button>
            {isOpen && (
                <div className="absolute right-0 mt-2 bg-black/95 border border-white/20 rounded-lg overflow-hidden min-w-[180px] shadow-xl">
                    {options.map((opt, i) => (
                        <button
                            key={i}
                            onClick={() => { opt.select(); setIsOpen(false); }}
                            className={`w-full text-left px-4 py-2 font-mono text-sm hover:bg-white/10 transition-colors ${opt.selected ? 'text-[#E0BBE4] bg-white/5' : 'text-white'}`}
                        >
                            {opt.selected && '✓ '}{opt.label}
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}

interface CyberPlayerProps {
    videoPath: string;
    videoName: string;
    onClose: () => void;
}

export default function CyberPlayer({ videoPath, videoName, onClose }: CyberPlayerProps) {
    // State for video source and loading status
    const [videoSrc, setVideoSrc] = useState<string>('');
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        const prepareStream = async () => {
            setLoading(true);
            try {
                console.log('Preparing stream for:', videoPath);
                // This command checks for multi-audio and creates HLS if needed
                const path: string = await invoke('prepare_hls_stream', { videoPath });

                console.log('Stream prepared, path:', path);
                const assetUrl = convertFileSrc(path);
                setVideoSrc(assetUrl);
            } catch (error) {
                console.error('Failed to prepare stream:', error);
                // Fallback to direct file playback
                setVideoSrc(convertFileSrc(videoPath));
            } finally {
                setLoading(false);
            }
        };

        prepareStream();
    }, [videoPath]);

    // Handle Escape key to close
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') {
                onClose();
            }
        };
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [onClose]);

    return createPortal(
        <div
            className="fixed inset-0 z-[9999] bg-black/95 backdrop-blur-md flex items-center justify-center animate-in fade-in duration-300"
            style={{
                // Cyber-Pastel Theme Overrides
                '--brand': '#E0BBE4', // Electric Lavender
                '--media-slider-track-fill-bg': '#FF2E63', // Cyber-Pink
                '--media-slider-thumb-bg': '#FF2E63',
                '--media-slider-thumb-border': '2px solid #fff',
                '--media-focus-ring-color': '#00F0FF',
            } as React.CSSProperties}
        >
            {/* Header / Top Bar */}
            <div className="absolute top-0 left-0 right-0 p-4 flex justify-between items-center bg-gradient-to-b from-black/80 to-transparent z-50 pointer-events-none">
                <h2 className="text-white font-mono text-lg truncate drop-shadow-[0_0_10px_rgba(224,187,228,0.5)] pl-4 pointer-events-auto">
                    {videoName}
                </h2>
                <button
                    onClick={onClose}
                    className="pointer-events-auto bg-black/50 hover:bg-[#FF2E63] text-white border border-white/20 hover:border-[#FF2E63] rounded-lg px-4 py-2 font-mono text-sm transition-all duration-300 backdrop-blur-sm group"
                >
                    [ CLOSE ]
                </button>
            </div>

            {/* Player Container */}
            <div className="w-full max-w-[90vw] aspect-video rounded-2xl overflow-hidden shadow-[0_0_50px_rgba(224,187,228,0.2)] border border-white/10 relative group">
                {/* Corner Accents */}
                <div className="absolute top-0 left-0 w-8 h-8 border-t-2 border-l-2 border-[#E0BBE4] rounded-tl-lg z-20 pointer-events-none opacity-50"></div>
                <div className="absolute top-0 right-0 w-8 h-8 border-t-2 border-r-2 border-[#E0BBE4] rounded-tr-lg z-20 pointer-events-none opacity-50"></div>
                <div className="absolute bottom-0 left-0 w-8 h-8 border-b-2 border-l-2 border-[#E0BBE4] rounded-bl-lg z-20 pointer-events-none opacity-50"></div>
                <div className="absolute bottom-0 right-0 w-8 h-8 border-b-2 border-r-2 border-[#E0BBE4] rounded-br-lg z-20 pointer-events-none opacity-50"></div>

                {loading ? (
                    <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/90 z-20">
                        <div className="w-12 h-12 border-4 border-[#FF2E63] border-t-transparent rounded-full animate-spin mb-4"></div>
                        <p className="text-[#E0BBE4] font-mono animate-pulse">OPTIMIZING PLAYBACK STREAM...</p>
                    </div>
                ) : (
                    <MediaPlayer
                        src={videoSrc}
                        viewType="video"
                        streamType="on-demand"
                        logLevel="warn"
                        className="w-full h-full"
                        autoPlay
                    >
                        <MediaProvider />
                        <AudioTrackSelector />
                        <DefaultVideoLayout
                            icons={defaultLayoutIcons}
                            thumbnails={null}
                        />
                    </MediaPlayer>
                )}
            </div>
        </div>,
        document.body
    );
}
