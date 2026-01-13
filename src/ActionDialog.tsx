import { motion, AnimatePresence } from "framer-motion";
import { Play, Download, X } from "lucide-react";
import { createPortal } from "react-dom";
import { useEffect } from "react";

interface ActionDialogProps {
    videoUrl: string;
    videoTitle: string;
    thumbnailUrl: string;
    onPlay: () => void;
    onDownload: () => void;
    onClose: () => void;
}

export default function ActionDialog({
    videoTitle,
    thumbnailUrl,
    onPlay,
    onDownload,
    onClose
}: ActionDialogProps) {
    // Handle Escape key
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === "Escape") onClose();
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
    }, [onClose]);

    return createPortal(
        <AnimatePresence>
            <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="fixed inset-0 z-[9998] bg-black/80 backdrop-blur-sm flex items-center justify-center"
                onClick={onClose}
            >
                <motion.div
                    initial={{ scale: 0.9, opacity: 0, y: 20 }}
                    animate={{ scale: 1, opacity: 1, y: 0 }}
                    exit={{ scale: 0.9, opacity: 0, y: 20 }}
                    transition={{ type: "spring", damping: 25, stiffness: 300 }}
                    className="relative bg-card-dark border border-white/10 rounded-2xl overflow-hidden shadow-[0_0_60px_rgba(0,255,163,0.15)] max-w-md w-full mx-4"
                    onClick={(e) => e.stopPropagation()}
                >
                    {/* Close Button */}
                    <button
                        onClick={onClose}
                        className="absolute top-3 right-3 z-10 p-2 rounded-full bg-black/50 hover:bg-black/70 text-white/70 hover:text-white transition-all"
                    >
                        <X size={18} />
                    </button>

                    {/* Thumbnail Preview */}
                    <div className="relative aspect-video bg-black/40">
                        {thumbnailUrl ? (
                            <img
                                src={thumbnailUrl}
                                alt={videoTitle}
                                className="w-full h-full object-cover"
                            />
                        ) : (
                            <div className="w-full h-full flex items-center justify-center bg-gray-800">
                                <span className="text-gray-500 text-6xl">🎬</span>
                            </div>
                        )}
                        {/* Gradient overlay */}
                        <div className="absolute inset-0 bg-gradient-to-t from-card-dark via-transparent to-transparent" />
                    </div>

                    {/* Content */}
                    <div className="p-5">
                        {/* Title */}
                        <h3 className="text-white font-semibold text-lg line-clamp-2 mb-4">
                            {videoTitle}
                        </h3>

                        {/* Question */}
                        <p className="text-gray-400 text-sm mb-5 font-mono">
                            What would you like to do?
                        </p>

                        {/* Action Buttons */}
                        <div className="flex gap-3">
                            {/* Play Button */}
                            <motion.button
                                whileHover={{ scale: 1.02 }}
                                whileTap={{ scale: 0.98 }}
                                onClick={onPlay}
                                className="flex-1 flex items-center justify-center gap-2 px-4 py-3 bg-neo-mint text-black font-bold rounded-xl transition-all hover:shadow-[0_0_20px_rgba(0,255,163,0.4)]"
                            >
                                <Play size={20} fill="currentColor" />
                                PLAY
                            </motion.button>

                            {/* Download Button */}
                            <motion.button
                                whileHover={{ scale: 1.02 }}
                                whileTap={{ scale: 0.98 }}
                                onClick={onDownload}
                                className="flex-1 flex items-center justify-center gap-2 px-4 py-3 bg-white/5 border border-white/20 text-white font-bold rounded-xl transition-all hover:bg-white/10 hover:border-neo-mint/50"
                            >
                                <Download size={20} />
                                DOWNLOAD
                            </motion.button>
                        </div>
                    </div>

                    {/* Decorative corners */}
                    <div className="absolute top-0 left-0 w-6 h-6 border-t-2 border-l-2 border-neo-mint/50 rounded-tl-lg pointer-events-none" />
                    <div className="absolute top-0 right-0 w-6 h-6 border-t-2 border-r-2 border-neo-mint/50 rounded-tr-lg pointer-events-none" />
                    <div className="absolute bottom-0 left-0 w-6 h-6 border-b-2 border-l-2 border-neo-mint/50 rounded-bl-lg pointer-events-none" />
                    <div className="absolute bottom-0 right-0 w-6 h-6 border-b-2 border-r-2 border-neo-mint/50 rounded-br-lg pointer-events-none" />
                </motion.div>
            </motion.div>
        </AnimatePresence>,
        document.body
    );
}
