import React from 'react';
import { motion } from 'framer-motion';

export interface VideoFormat {
    format_id: string;
    ext: string;
    resolution: string;
    fps: number;
    filesize: number;
    vcodec: string;
    acodec: string;
    note: string;
}

export interface VideoMetadataResponse {
    title: string;
    thumbnail_url: string;
    duration: number;
    formats: VideoFormat[];
    is_playlist?: boolean;
    video_count?: number;
}

interface VideoMetadataCardProps {
    metadata: VideoMetadataResponse;
    formats: VideoFormat[];
    onDownload: (formatId: string) => void;
}

const VideoMetadataCard: React.FC<VideoMetadataCardProps> = ({ metadata, formats, onDownload }) => {
    const formatBytes = (bytes: number) => {
        if (bytes === 0) return 'Unknown size';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    };

    const formatDuration = (seconds: number) => {
        const min = Math.floor(seconds / 60);
        const sec = Math.floor(seconds % 60);
        return `${min}:${sec < 10 ? '0' : ''}${sec}`;
    };

    const getQualityLabel = (fmt: VideoFormat) => {
        // Audio Check
        if (fmt.vcodec === 'none' || fmt.resolution === 'audio only') {
            // Simple heuristic based on filesize/note if available
            if (fmt.note && fmt.note.includes('High')) return "Audio (High)";
            if (fmt.filesize > 5 * 1024 * 1024) return "Audio (High)"; // > 5MB
            if (fmt.filesize > 2 * 1024 * 1024) return "Audio (Standard)"; // > 2MB
            return "Audio (Low)";
        }

        const res = fmt.resolution || "";
        const dimKey = Math.min(parseInt(res.split('x')[0] || "0"), parseInt(res.split('x')[1] || "0"));

        // Check string includes first for common formats
        if (res.includes('2160') || (dimKey >= 2160)) return "4K Ultra HD";
        if (res.includes('1440') || (dimKey >= 1440)) return "2K QHD";
        if (res.includes('1080') || (dimKey >= 1080)) return "Full HD";
        if (res.includes('720') || (dimKey >= 720)) return "HD";
        if (res.includes('480') || (dimKey >= 480)) return "Standard (480p)";
        if (res.includes('360') || (dimKey >= 360)) return "Low";
        if (res.includes('240') || res.includes('244') || (dimKey >= 240)) return "Very Low";
        if (res.includes('144') || (dimKey >= 144) || (dimKey > 0 && dimKey < 144)) return "Xtream Low";

        return res || "Unknown";
    };

    return (
        <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className="w-full max-w-4xl mx-auto bg-card-dark rounded-3xl p-8 overflow-hidden relative border border-white/10 shadow-card-float"
        >
            {/* Glossy overlay */}
            <div className="absolute top-0 right-0 w-full h-full bg-gradient-to-bl from-white/5 to-transparent pointer-events-none" />

            <div className="flex flex-col md:flex-row gap-8 relative z-10">
                {/* Left: Thumbnail */}
                <div className="flex-shrink-0">
                    <div className="relative group overflow-hidden rounded-2xl shadow-lg border border-white/10">
                        <img
                            src={metadata.thumbnail_url}
                            alt={metadata.title}
                            className="w-64 h-40 object-cover transform group-hover:scale-110 transition-transform duration-500"
                        />
                        <div className="absolute inset-0 bg-black/30 group-hover:bg-transparent transition-colors duration-300" />
                        {!metadata.is_playlist && (
                            <div className="absolute bottom-2 right-2 bg-black/80 text-white text-xs font-bold px-2 py-1 rounded backdrop-blur-md">
                                {formatDuration(metadata.duration)}
                            </div>
                        )}
                        {metadata.is_playlist && (
                            <div className="absolute top-2 right-2 bg-neo-mint text-black text-xs font-bold px-2 py-1 rounded shadow-md">
                                PLAYLIST
                            </div>
                        )}
                    </div>
                </div>

                {/* Right: Info */}
                <div className="flex-1 flex flex-col justify-between">
                    <div>
                        <h2 className="text-3xl font-black text-white leading-tight drop-shadow-sm line-clamp-2 mb-2">
                            {metadata.title}
                        </h2>
                        <div className="flex items-center gap-4 text-text-muted font-medium text-sm">
                            {!metadata.is_playlist ? (
                                <>
                                    <span className="bg-white/10 px-3 py-1 rounded-full backdrop-blur-sm border border-white/10">
                                        {formatDuration(metadata.duration)} Duration
                                    </span>
                                    <span>•</span>
                                    <span className="text-neo-mint font-bold tracking-wide">
                                        {formats.length} Formats Available
                                    </span>
                                </>
                            ) : (
                                <>
                                    <span className="bg-white/10 px-3 py-1 rounded-full backdrop-blur-sm border border-white/10 text-electric-lavender">
                                        {metadata.video_count} Videos
                                    </span>
                                    <span>•</span>
                                    <span className="text-neo-mint font-bold tracking-wide">
                                        Ready to Download
                                    </span>
                                </>
                            )}
                        </div>
                    </div>

                    {/* Playlist Action Button */}
                    {metadata.is_playlist && (
                        <div className="mt-4">
                            <button
                                onClick={() => onDownload("best")} // Downloads whole playlist with best quality
                                className="px-6 py-3 rounded-xl bg-neo-mint text-black font-black hover:shadow-[0_0_20px_rgba(0,229,204,0.6)] hover:-translate-y-1 transition-all flex items-center gap-2"
                            >
                                <span>DOWNLOAD ALL VIDEOS</span>
                                <span className="text-xl">⬇</span>
                            </button>
                        </div>
                    )}
                </div>
            </div>

            {/* Quality List - ONLY show if NOT a playlist */}
            {!metadata.is_playlist && (
                <div className="mt-8 pt-6 border-t border-white/10 relative z-10">
                    <h3 className="text-sm font-bold text-text-muted uppercase tracking-widest mb-4 flex items-center gap-2">
                        <span className="w-2 h-2 rounded-full bg-neo-mint shadow-[0_0_10px_rgba(0,229,204,0.5)]"></span>
                        Select Quality to Download
                    </h3>

                    <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-3">
                        {formats.map((fmt) => (
                            <button
                                key={fmt.format_id}
                                onClick={() => onDownload(fmt.format_id)}
                                className="group relative overflow-hidden rounded-xl border border-neo-mint/30 bg-card-darker hover:bg-neo-mint transition-all duration-300 p-3 text-left shadow-sm hover:shadow-neon-mint"
                            >
                                <div className="relative z-10 flex flex-col items-center justify-center text-center gap-1">
                                    <span className="text-lg font-black text-white group-hover:text-black transition-colors">
                                        {getQualityLabel(fmt)}
                                    </span>
                                    <span className="text-xs font-mono text-text-muted group-hover:text-black/70 transition-colors">
                                        {fmt.ext.toUpperCase()} • {formatBytes(fmt.filesize)}
                                    </span>
                                </div>
                            </button>
                        ))}
                    </div>
                </div>
            )}
        </motion.div>
    );
};

export default VideoMetadataCard;
