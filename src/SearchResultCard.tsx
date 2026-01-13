import { motion } from "framer-motion";
import { Clock, Eye, User } from "lucide-react";

export interface VideoSearchResult {
    title: string;
    thumbnail_url: string | null;
    video_url: string;
    duration: string | null;
    uploader: string | null;
    view_count: number | null;
}

interface SearchResultCardProps {
    result: VideoSearchResult;
    onClick: (url: string, title?: string, thumbnail?: string | null) => void;
}

function formatViewCount(count: number): string {
    if (count >= 1_000_000) {
        return `${(count / 1_000_000).toFixed(1)}M`;
    } else if (count >= 1_000) {
        return `${(count / 1_000).toFixed(1)}K`;
    }
    return count.toString();
}

export default function SearchResultCard({ result, onClick }: SearchResultCardProps) {
    return (
        <motion.div
            whileHover={{ scale: 1.02, y: -2 }}
            whileTap={{ scale: 0.98 }}
            className="search-result-card relative rounded-xl overflow-hidden cursor-pointer group"
            onClick={() => onClick(result.video_url, result.title, result.thumbnail_url)}
        >
            {/* Thumbnail Container */}
            <div className="relative aspect-video bg-black/40">
                {result.thumbnail_url ? (
                    <img
                        src={result.thumbnail_url}
                        alt={result.title}
                        className="w-full h-full object-cover"
                        loading="lazy"
                    />
                ) : (
                    <div className="w-full h-full flex items-center justify-center bg-gray-800">
                        <span className="text-gray-500 text-4xl">🎬</span>
                    </div>
                )}

                {/* Hover Overlay with Play Button */}
                <div className="absolute inset-0 bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                    <div className="w-14 h-14 rounded-full bg-neo-mint flex items-center justify-center shadow-[0_0_20px_rgba(0,255,163,0.5)]">
                        <span className="text-black text-2xl ml-1">▶</span>
                    </div>
                </div>

                {/* Duration Badge */}
                {result.duration && (
                    <div className="absolute bottom-2 right-2 px-2 py-1 bg-black/80 rounded text-xs text-white font-mono flex items-center gap-1">
                        <Clock size={10} />
                        {result.duration}
                    </div>
                )}
            </div>

            {/* Info Section */}
            <div className="p-3 bg-card-dark border-t border-white/5">
                {/* Title */}
                <h3 className="text-white text-sm font-semibold line-clamp-2 mb-2 group-hover:text-neo-mint transition-colors">
                    {result.title}
                </h3>

                {/* Meta Info */}
                <div className="flex items-center gap-3 text-xs text-gray-400">
                    {result.uploader && (
                        <span className="flex items-center gap-1 truncate">
                            <User size={10} />
                            {result.uploader}
                        </span>
                    )}
                    {result.view_count && (
                        <span className="flex items-center gap-1">
                            <Eye size={10} />
                            {formatViewCount(result.view_count)}
                        </span>
                    )}
                </div>
            </div>

            {/* Glow Border on Hover */}
            <div className="absolute inset-0 rounded-xl border border-white/10 group-hover:border-neo-mint/50 transition-colors pointer-events-none" />
        </motion.div>
    );
}
