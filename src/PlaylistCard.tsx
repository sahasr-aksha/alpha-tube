import { motion, AnimatePresence } from "framer-motion";
import { useState } from "react";

interface VideoFile {
    name: string;
    path: string;
    size: number;
    modified_date: string;
    extension: string;
}

interface PlaylistFolder {
    name: string;
    path: string;
    video_count: number;
    thumbnail_path: string | null;
    videos: VideoFile[];
}

interface PlaylistCardProps {
    playlist: PlaylistFolder;
    onVideoSelect: (video: VideoFile) => void;
    formatBytes: (bytes: number) => string;
    onOpenFolder: (path: string) => void;
}

function PlaylistCard({ playlist, onVideoSelect, formatBytes, onOpenFolder }: PlaylistCardProps) {
    const [isExpanded, setIsExpanded] = useState(false);

    return (
        <div className="bg-card-dark rounded-2xl overflow-hidden shadow-card-float border border-white/10">
            {/* Playlist Header - Clickable */}
            <div
                onClick={() => setIsExpanded(!isExpanded)}
                className="p-4 cursor-pointer hover:bg-white/5 transition-all duration-300 group relative overflow-hidden"
            >
                {/* Playlist icon decoration */}
                <div className="absolute top-0 right-0 w-20 h-20 bg-gradient-to-bl from-electric-lavender/20 to-transparent rounded-bl-full -mr-4 -mt-4 group-hover:from-electric-lavender/40 transition-all" />

                <div className="flex items-center gap-4 relative z-10">
                    {/* Playlist Icon */}
                    <div className="w-16 h-16 bg-gradient-to-br from-electric-lavender to-cyber-pink rounded-xl flex items-center justify-center shadow-md">
                        <span className="text-white text-2xl font-bold">▶</span>
                    </div>

                    <div className="flex-1">
                        <h3 className="text-white font-bold text-lg truncate" title={playlist.name}>
                            {playlist.name}
                        </h3>
                        <div className="flex items-center gap-3 text-sm text-text-muted font-mono mt-1">
                            <span className="bg-electric-lavender/20 text-electric-lavender px-2 py-0.5 rounded-full text-xs font-bold">
                                {playlist.video_count} videos
                            </span>
                            <button
                                onClick={(e) => {
                                    e.stopPropagation();
                                    onOpenFolder(playlist.path);
                                }}
                                className="text-electric-lavender hover:bg-electric-lavender/20 p-1 rounded transition-colors opacity-0 group-hover:opacity-100"
                                title="Open Playlist Folder"
                            >
                                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z"></path></svg>
                            </button>
                        </div>
                    </div>

                    {/* Expand/Collapse Arrow */}
                    <motion.span
                        animate={{ rotate: isExpanded ? 180 : 0 }}
                        transition={{ duration: 0.2 }}
                        className="text-text-muted text-xl"
                    >
                        ▼
                    </motion.span>
                </div>
            </div>

            {/* Expanded Video List */}
            <AnimatePresence>
                {isExpanded && (
                    <motion.div
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: "auto", opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        transition={{ duration: 0.3 }}
                        className="border-t border-white/10 bg-card-darker"
                    >
                        <div className="p-4 space-y-2 max-h-80 overflow-y-auto">
                            {playlist.videos.map((video, index) => (
                                <div
                                    key={video.path}
                                    onClick={() => onVideoSelect(video)}
                                    className="flex items-center gap-3 p-3 bg-card-dark rounded-xl cursor-pointer hover:shadow-md transition-all group border border-white/5 hover:border-neo-mint/30"
                                >
                                    {/* Video Index */}
                                    <span className="w-8 h-8 bg-card-darker rounded-lg flex items-center justify-center text-text-muted font-bold text-sm group-hover:bg-neo-mint group-hover:text-black transition-all">
                                        {index + 1}
                                    </span>

                                    {/* Video Info */}
                                    <div className="flex-1 min-w-0">
                                        <p className="text-white font-medium truncate text-sm" title={video.name}>
                                            {video.name}
                                        </p>
                                        <div className="flex gap-3 text-xs text-text-muted font-mono mt-0.5">
                                            <span>{formatBytes(video.size)}</span>
                                            <span className="text-cyber-pink">{video.extension.toUpperCase()}</span>
                                        </div>
                                    </div>

                                    {/* Play indicator */}
                                    <span className="text-neo-mint opacity-0 group-hover:opacity-100 transition-opacity text-lg">
                                        ▶
                                    </span>
                                </div>
                            ))}
                        </div>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
}

export default PlaylistCard;
export type { PlaylistFolder, VideoFile };
