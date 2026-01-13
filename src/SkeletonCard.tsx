import { motion } from "framer-motion";

/**
 * Skeleton loading placeholder for search result cards
 * Shows animated shimmer effect while content loads
 */
export default function SkeletonCard() {
    return (
        <div className="relative rounded-xl overflow-hidden bg-[#1E1E24] border border-white/5">
            {/* Thumbnail Skeleton */}
            <div className="relative aspect-video bg-[#2A2A30]">
                <div className="absolute inset-0 shimmer-animation" />
            </div>

            {/* Info Section Skeleton */}
            <div className="p-3 space-y-2">
                {/* Title lines */}
                <div className="h-4 bg-[#2A2A30] rounded w-full shimmer-animation" />
                <div className="h-4 bg-[#2A2A30] rounded w-3/4 shimmer-animation" />

                {/* Meta info */}
                <div className="flex items-center gap-3 pt-1">
                    <div className="h-3 bg-[#2A2A30] rounded w-20 shimmer-animation" />
                    <div className="h-3 bg-[#2A2A30] rounded w-14 shimmer-animation" />
                </div>
            </div>
        </div>
    );
}

/**
 * Skeleton grid - shows multiple skeleton cards
 */
export function SkeletonGrid({ count = 8 }: { count?: number }) {
    return (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
            {Array.from({ length: count }).map((_, i) => (
                <motion.div
                    key={i}
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{ delay: i * 0.05 }}
                >
                    <SkeletonCard />
                </motion.div>
            ))}
        </div>
    );
}

/**
 * Skeleton for video format/quality selection cards
 */
export function FormatCardSkeleton() {
    return (
        <div className="rounded-xl border border-white/10 bg-[#1A1A1F] p-3">
            <div className="flex flex-col items-center justify-center text-center gap-2">
                <div className="h-5 w-16 bg-[#2A2A30] rounded shimmer-animation" />
                <div className="h-4 w-12 bg-[#2A2A30] rounded shimmer-animation" />
                <div className="h-3 w-20 bg-[#2A2A30] rounded shimmer-animation" />
            </div>
        </div>
    );
}

/**
 * Skeleton grid for format cards during URL metadata loading
 */
export function FormatSkeletonGrid({ count = 10 }: { count?: number }) {
    return (
        <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className="w-full max-w-4xl mx-auto bg-[#1E1E24] rounded-3xl p-8 border border-white/10 shadow-lg"
        >
            {/* Header skeleton */}
            <div className="flex flex-col md:flex-row gap-8 mb-8">
                {/* Thumbnail skeleton */}
                <div className="flex-shrink-0">
                    <div className="w-64 h-40 bg-[#2A2A30] rounded-2xl shimmer-animation" />
                </div>

                {/* Info skeleton */}
                <div className="flex-1 space-y-3">
                    <div className="h-8 bg-[#2A2A30] rounded w-3/4 shimmer-animation" />
                    <div className="h-6 bg-[#2A2A30] rounded w-1/2 shimmer-animation" />
                    <div className="flex gap-3 mt-4">
                        <div className="h-8 w-24 bg-[#2A2A30] rounded-full shimmer-animation" />
                        <div className="h-8 w-32 bg-[#2A2A30] rounded-full shimmer-animation" />
                    </div>
                </div>
            </div>

            {/* Quality header skeleton */}
            <div className="pt-6 border-t border-white/10">
                <div className="flex items-center gap-2 mb-4">
                    <div className="w-2 h-2 rounded-full bg-neo-mint/50" />
                    <div className="h-4 w-40 bg-[#2A2A30] rounded shimmer-animation" />
                </div>

                {/* Format cards skeleton grid */}
                <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-3">
                    {Array.from({ length: count }).map((_, i) => (
                        <motion.div
                            key={i}
                            initial={{ opacity: 0 }}
                            animate={{ opacity: 1 }}
                            transition={{ delay: i * 0.03 }}
                        >
                            <FormatCardSkeleton />
                        </motion.div>
                    ))}
                </div>
            </div>
        </motion.div>
    );
}
