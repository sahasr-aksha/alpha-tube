import { MediaPlayer, MediaProvider } from '@vidstack/react';
import { DefaultVideoLayout, defaultLayoutIcons } from '@vidstack/react/player/layouts/default';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEffect } from 'react';
import { createPortal } from 'react-dom';
import '@vidstack/react/player/styles/default/theme.css';
import '@vidstack/react/player/styles/default/layouts/video.css';

interface CyberPlayerProps {
    videoPath: string;
    videoName: string;
    onClose: () => void;
}

export default function CyberPlayer({ videoPath, videoName, onClose }: CyberPlayerProps) {
    const nativeVideoSrc = convertFileSrc(videoPath);

    // Handle Escape key to close
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') onClose();
        };
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [onClose]);

    return createPortal(
        <div
            className="fixed inset-0 z-[9999] bg-black/95 backdrop-blur-md flex items-center justify-center animate-in fade-in duration-300"
            style={{
                '--brand': '#E0BBE4',
                '--media-slider-track-fill-bg': '#FF2E63',
                '--media-slider-thumb-bg': '#FF2E63',
                '--media-slider-thumb-border': '2px solid #fff',
                '--media-focus-ring-color': '#00F0FF',
            } as React.CSSProperties}
        >
            {/* Header */}
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

            {/* Main Player Container */}
            <div className="w-full max-w-[90vw] aspect-video rounded-2xl overflow-hidden shadow-[0_0_50px_rgba(224,187,228,0.2)] border border-white/10 relative group">

                {/* Visual Borders */}
                <div className="absolute top-0 left-0 w-8 h-8 border-t-2 border-l-2 border-[#E0BBE4] rounded-tl-lg z-20 pointer-events-none opacity-50"></div>
                <div className="absolute top-0 right-0 w-8 h-8 border-t-2 border-r-2 border-[#E0BBE4] rounded-tr-lg z-20 pointer-events-none opacity-50"></div>
                <div className="absolute bottom-0 left-0 w-8 h-8 border-b-2 border-l-2 border-[#E0BBE4] rounded-bl-lg z-20 pointer-events-none opacity-50"></div>
                <div className="absolute bottom-0 right-0 w-8 h-8 border-b-2 border-r-2 border-[#E0BBE4] rounded-br-lg z-20 pointer-events-none opacity-50"></div>

                <MediaPlayer
                    src={nativeVideoSrc}
                    viewType="video"
                    streamType="on-demand"
                    logLevel="warn"
                    className="w-full h-full"
                    autoPlay
                >
                    <MediaProvider />
                    <DefaultVideoLayout icons={defaultLayoutIcons} thumbnails={null} />
                </MediaPlayer>
            </div>
        </div>,
        document.body
    );
}
