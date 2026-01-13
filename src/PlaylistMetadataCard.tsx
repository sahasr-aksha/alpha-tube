import React, { useState, useMemo } from 'react';
import { motion } from 'framer-motion';
import { Check, Download } from 'lucide-react';

// Types matching Rust structs in playlist.rs
export interface PlaylistVideoFormat {
    format_id: string;
    ext: string;
    resolution: string;
    fps: number;
    filesize: number;
    vcodec: string;
    acodec: string;
    note: string;
}

export interface PlaylistVideoEntry {
    id: string;
    title: string;
    thumbnail_url: string;
    duration: number;
    url: string;
    index: number;
    formats: PlaylistVideoFormat[];
}

export interface PlaylistMetadataResponse {
    title: string;
    thumbnail_url: string;
    video_count: number;
    playlist_id: string;
    videos: PlaylistVideoEntry[];
}

// Selected video with quality choice
export interface SelectedVideo {
    url: string;
    formatId: string | null;
    quality: string;
    title: string;
    thumbnail: string;
    duration: number;
    index: number;
}

interface PlaylistMetadataCardProps {
    metadata: PlaylistMetadataResponse;
    onDownloadSelected: (videos: SelectedVideo[]) => void;
}

// Quality preset options
const QUALITY_PRESETS = [
    { value: 'best', label: 'Best Quality' },
    { value: '2160p', label: '4K (2160p)' },
    { value: '1440p', label: '2K (1440p)' },
    { value: '1080p', label: 'Full HD (1080p)' },
    { value: '720p', label: 'HD (720p)' },
    { value: '480p', label: 'SD (480p)' },
    { value: '360p', label: 'Low (360p)' },
    { value: 'mp3', label: 'Audio Only (MP3)' },
];

const PlaylistMetadataCard: React.FC<PlaylistMetadataCardProps> = ({ metadata, onDownloadSelected }) => {
    // Selection state: videoIndex -> { selected, quality }
    const [selections, setSelections] = useState<Record<number, { selected: boolean; quality: string }>>(() => {
        // Initialize all videos as selected with 'best' quality
        const initial: Record<number, { selected: boolean; quality: string }> = {};
        metadata.videos.forEach(v => {
            initial[v.index] = { selected: true, quality: 'best' };
        });
        return initial;
    });

    // Global quality for "Apply to All"
    const [globalQuality, setGlobalQuality] = useState('best');

    // Format duration helper
    const formatDuration = (seconds: number) => {
        if (!seconds) return '--:--';
        const hrs = Math.floor(seconds / 3600);
        const min = Math.floor((seconds % 3600) / 60);
        const sec = Math.floor(seconds % 60);
        if (hrs > 0) {
            return `${hrs}:${min.toString().padStart(2, '0')}:${sec.toString().padStart(2, '0')}`;
        }
        return `${min}:${sec.toString().padStart(2, '0')}`;
    };

    // Count selected videos
    const selectedCount = useMemo(() => {
        return Object.values(selections).filter(s => s.selected).length;
    }, [selections]);

    // Toggle single video selection
    const toggleSelection = (index: number) => {
        setSelections(prev => ({
            ...prev,
            [index]: { ...prev[index], selected: !prev[index].selected }
        }));
    };

    // Select/Deselect All
    const selectAll = () => {
        setSelections(prev => {
            const updated = { ...prev };
            Object.keys(updated).forEach(k => {
                updated[parseInt(k)].selected = true;
            });
            return updated;
        });
    };

    const deselectAll = () => {
        setSelections(prev => {
            const updated = { ...prev };
            Object.keys(updated).forEach(k => {
                updated[parseInt(k)].selected = false;
            });
            return updated;
        });
    };

    // Apply global quality to all videos
    const applyQualityToAll = () => {
        setSelections(prev => {
            const updated = { ...prev };
            Object.keys(updated).forEach(k => {
                updated[parseInt(k)].quality = globalQuality;
            });
            return updated;
        });
    };

    // Set quality for single video
    const setVideoQuality = (index: number, quality: string) => {
        setSelections(prev => ({
            ...prev,
            [index]: { ...prev[index], quality }
        }));
    };

    // Handle download button click
    const handleDownload = () => {
        const selectedVideos: SelectedVideo[] = metadata.videos
            .filter(v => selections[v.index]?.selected)
            .map(v => ({
                url: v.url,
                formatId: null, // Using quality preset
                quality: selections[v.index]?.quality || 'best',
                title: v.title,
                thumbnail: v.thumbnail_url,
                duration: v.duration,
                index: v.index,
            }));

        if (selectedVideos.length > 0) {
            onDownloadSelected(selectedVideos);
        }
    };

    return (
        <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className="w-full max-w-4xl mx-auto bg-card-dark rounded-3xl overflow-hidden border border-white/10 shadow-card-float"
        >
            {/* Header Section */}
            <div className="p-6 border-b border-white/10 relative overflow-hidden">
                {/* Gradient overlay */}
                <div className="absolute top-0 right-0 w-full h-full bg-gradient-to-bl from-electric-lavender/10 to-transparent pointer-events-none" />

                <div className="flex gap-6 relative z-10">
                    {/* Playlist Thumbnail */}
                    <div className="relative flex-shrink-0">
                        <div className="w-48 h-28 rounded-xl overflow-hidden border border-white/10 shadow-lg relative">
                            {metadata.thumbnail_url ? (
                                <img
                                    src={metadata.thumbnail_url}
                                    alt={metadata.title}
                                    className="w-full h-full object-cover"
                                />
                            ) : (
                                <div className="w-full h-full bg-card-darker flex items-center justify-center">
                                    <span className="text-3xl text-gray-600">📂</span>
                                </div>
                            )}
                            <div className="absolute top-2 right-2 bg-electric-lavender text-black text-xs font-bold px-2 py-1 rounded shadow-md">
                                PLAYLIST
                            </div>
                        </div>
                    </div>

                    {/* Playlist Info */}
                    <div className="flex-1">
                        <h2 className="text-2xl font-black text-white leading-tight mb-2 line-clamp-2">
                            {metadata.title}
                        </h2>
                        <div className="flex items-center gap-4 text-sm">
                            <span className="bg-electric-lavender/20 text-electric-lavender px-3 py-1 rounded-full font-bold">
                                {metadata.video_count} Videos
                            </span>
                            <span className="text-text-muted">
                                {selectedCount} selected for download
                            </span>
                        </div>
                    </div>
                </div>
            </div>

            {/* Selection Controls */}
            <div className="px-6 py-4 bg-card-darker border-b border-white/10 flex flex-wrap items-center gap-4">
                {/* Select All / Deselect All */}
                <div className="flex gap-2">
                    <button
                        onClick={selectAll}
                        className="px-3 py-1.5 text-xs font-bold bg-neo-mint/20 text-neo-mint rounded-lg hover:bg-neo-mint hover:text-black transition-all"
                    >
                        SELECT ALL
                    </button>
                    <button
                        onClick={deselectAll}
                        className="px-3 py-1.5 text-xs font-bold bg-red-500/20 text-red-400 rounded-lg hover:bg-red-500 hover:text-white transition-all"
                    >
                        DESELECT ALL
                    </button>
                </div>

                <div className="w-px h-6 bg-white/10" />

                {/* Global Quality Selector */}
                <div className="flex items-center gap-2">
                    <label className="text-xs text-text-muted font-mono">QUALITY:</label>
                    <select
                        value={globalQuality}
                        onChange={(e) => setGlobalQuality(e.target.value)}
                        className="bg-card-dark border border-white/10 text-white px-3 py-1.5 rounded-lg text-sm font-medium focus:outline-none focus:ring-1 focus:ring-neo-mint"
                    >
                        {QUALITY_PRESETS.map(q => (
                            <option key={q.value} value={q.value}>{q.label}</option>
                        ))}
                    </select>
                    <button
                        onClick={applyQualityToAll}
                        className="px-3 py-1.5 text-xs font-bold bg-electric-lavender/20 text-electric-lavender rounded-lg hover:bg-electric-lavender hover:text-black transition-all"
                    >
                        APPLY TO ALL
                    </button>
                </div>

                <div className="flex-1" />

                {/* Download Button */}
                <button
                    onClick={handleDownload}
                    disabled={selectedCount === 0}
                    className="px-6 py-2 rounded-xl bg-neo-mint text-black font-black hover:shadow-[0_0_20px_rgba(0,229,204,0.6)] hover:-translate-y-0.5 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                >
                    <Download size={16} />
                    DOWNLOAD {selectedCount > 0 ? `(${selectedCount})` : ''}
                </button>
            </div>

            {/* Video List */}
            <div className="max-h-[400px] overflow-y-auto">
                {metadata.videos.map((video) => {
                    const isSelected = selections[video.index]?.selected || false;
                    const quality = selections[video.index]?.quality || 'best';

                    return (
                        <div
                            key={video.id || video.index}
                            className={`border-b border-white/5 transition-colors ${isSelected ? 'bg-white/5' : 'bg-transparent hover:bg-white/[0.02]'}`}
                        >
                            <div className="flex items-center gap-3 p-3">
                                {/* Checkbox */}
                                <button
                                    onClick={() => toggleSelection(video.index)}
                                    className={`w-6 h-6 rounded-md border-2 flex items-center justify-center flex-shrink-0 transition-all ${isSelected
                                        ? 'bg-neo-mint border-neo-mint text-black'
                                        : 'border-white/30 hover:border-white/50'
                                        }`}
                                >
                                    {isSelected && <Check size={14} strokeWidth={3} />}
                                </button>

                                {/* Index */}
                                <span className="w-8 text-center text-sm font-mono text-text-muted">
                                    {video.index}
                                </span>

                                {/* Thumbnail */}
                                <div className="w-20 h-12 rounded-lg overflow-hidden flex-shrink-0 bg-card-darker border border-white/5">
                                    {video.thumbnail_url ? (
                                        <img
                                            src={video.thumbnail_url}
                                            alt=""
                                            className="w-full h-full object-cover"
                                        />
                                    ) : (
                                        <div className="w-full h-full flex items-center justify-center text-gray-600">▶</div>
                                    )}
                                </div>

                                {/* Title & Duration */}
                                <div className="flex-1 min-w-0">
                                    <p className="text-white text-sm font-medium truncate" title={video.title}>
                                        {video.title}
                                    </p>
                                    <span className="text-xs text-text-muted font-mono">
                                        {formatDuration(video.duration)}
                                    </span>
                                </div>

                                {/* Per-Video Quality Dropdown */}
                                <div className="flex items-center gap-2">
                                    <select
                                        value={quality}
                                        onChange={(e) => setVideoQuality(video.index, e.target.value)}
                                        onClick={(e) => e.stopPropagation()}
                                        className="bg-card-darker border border-white/10 text-white text-xs px-2 py-1 rounded-md focus:outline-none focus:ring-1 focus:ring-neo-mint min-w-[100px]"
                                    >
                                        {QUALITY_PRESETS.map(q => (
                                            <option key={q.value} value={q.value}>{q.label}</option>
                                        ))}
                                    </select>
                                </div>
                            </div>
                        </div>
                    );
                })}
            </div>

            {/* Footer note */}
            {metadata.video_count > metadata.videos.length && (
                <div className="p-4 text-center text-xs text-text-muted bg-card-darker border-t border-white/10">
                    Showing first {metadata.videos.length} of {metadata.video_count} videos
                </div>
            )}
        </motion.div>
    );
};

export default PlaylistMetadataCard;
