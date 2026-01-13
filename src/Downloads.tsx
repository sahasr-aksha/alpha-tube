import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { downloadDir } from "@tauri-apps/api/path";
import CyberPlayer from "./CyberPlayer";
import PlaylistCard, { PlaylistFolder, VideoFile } from "./PlaylistCard";
import { DownloadProgress } from "./App";
import { Search, ArrowUpDown, X } from "lucide-react";
import "./Downloads.css";

interface LibraryContent {
    playlists: PlaylistFolder[];
    singles: VideoFile[];
}

import { AppConfig } from "./App";
import { open } from "@tauri-apps/plugin-shell"; // For opening folder
import { ask } from "@tauri-apps/plugin-dialog"; // Native confirm

interface DownloadsProps {
    onBack?: () => void;
    activeDownloads?: Record<string, DownloadProgress>;
    onPause?: (id: string) => void;
    onResume?: (id: string) => void;
    config: AppConfig | null;
    onCheckConfig: () => void; // Trigger directory change dialog
}

function Downloads({ onBack: _onBack, activeDownloads = {}, onPause, onResume, config, onCheckConfig }: DownloadsProps) {
    const [library, setLibrary] = useState<LibraryContent>({ playlists: [], singles: [] });
    const [loading, setLoading] = useState(true);
    const [selectedVideo, setSelectedVideo] = useState<VideoFile | null>(null);

    // Search and sort state
    const [searchFilter, setSearchFilter] = useState("");
    const [sortBy, setSortBy] = useState<"date" | "name" | "size">("date");
    const [sortMenuOpen, setSortMenuOpen] = useState(false);

    const activeDownloadsList = Object.values(activeDownloads);
    const hasDownloads = activeDownloadsList.length > 0;

    const loadLibrary = async () => {
        setLoading(true);
        try {
            const directory = await downloadDir();
            const content = await invoke<LibraryContent>("scan_library", {
                directory,
            });
            setLibrary(content);
        } catch (error) {
            console.error("Failed to load library:", error);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        loadLibrary();
    }, [hasDownloads]); // Reload library when downloading status changes (e.g. some finish, list changes)

    const formatBytes = (bytes: number) => {
        if (bytes === 0) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    };

    const totalVideos = library.playlists.reduce((sum, p) => sum + p.video_count, 0) + library.singles.length;

    // Filter and sort videos
    const filteredSingles = library.singles
        .filter(video =>
            !searchFilter || video.name.toLowerCase().includes(searchFilter.toLowerCase())
        )
        .sort((a, b) => {
            switch (sortBy) {
                case "name":
                    return a.name.localeCompare(b.name);
                case "size":
                    return b.size - a.size;
                case "date":
                default:
                    return b.modified_date.localeCompare(a.modified_date);
            }
        });

    const filteredPlaylists = library.playlists
        .filter(playlist =>
            !searchFilter || playlist.name.toLowerCase().includes(searchFilter.toLowerCase())
        );

    const handleCancel = async (id: string) => {
        try {
            await invoke("cancel_download", { id });
        } catch (e) {
            console.error("Failed to cancel", e);
        }
    };

    const handleDelete = async (path: string) => {
        const yes = await ask("Are you sure you want to delete this file permanently?", {
            title: 'Delete Video',
            kind: 'warning'
        });
        if (!yes) return;

        try {
            await invoke("delete_file", { path });
            loadLibrary(); // Refresh
        } catch (e) {
            console.error("Failed to delete", e);
            alert("Failed to delete file: " + e);
        }
    };

    const handleRevealFile = async (path: string) => {
        try {
            await invoke("reveal_file_in_explorer", { path });
        } catch (e) {
            console.error("Failed to reveal", e);
        }
    }

    const handleOpenFolder = async () => {
        if (config?.download_dir) {
            try {
                await open(config.download_dir);
            } catch (e) {
                console.error("Failed to open folder", e);
            }
        }
    };

    return (
        <div className="downloads-page p-10 h-full flex flex-col overflow-y-auto">
            <div className="flex justify-between items-center mb-8 border-b border-white/10 pb-4">
                <h1 className="text-4xl font-black text-white tracking-tight">
                    <span className="text-neo-mint">{'>'}</span> LIBRARY_INDEX
                </h1>
                <div className="flex items-center gap-4">
                    <span className="text-text-muted font-mono text-sm mr-4 border-r border-white/10 pr-4">
                        {config?.download_dir || "Unknown Location"}
                    </span>
                    <button
                        onClick={handleOpenFolder}
                        className="text-white hover:text-neo-mint transition-colors p-2"
                        title="Open Folder"
                    >
                        <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z"></path></svg>
                    </button>
                    <button
                        onClick={onCheckConfig}
                        className="text-white hover:text-neo-mint transition-colors p-2"
                        title="Change Directory"
                    >
                        <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M20 14.66V20a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h5.34"></path><polygon points="18 2 22 6 12 16 8 16 8 12 18 2"></polygon></svg>
                    </button>
                    <div className="w-px h-6 bg-white/10 mx-2"></div>
                    <span className="text-text-muted font-mono text-sm">
                        {library.playlists.length} playlists • {library.singles.length} singles
                    </span>
                    <button
                        onClick={loadLibrary}
                        className="border border-neo-mint/30 bg-neo-mint/10 text-neo-mint px-6 py-2 rounded-lg hover:bg-neo-mint hover:text-black uppercase text-sm font-bold transition-all shadow-sm hover:shadow-neon-mint"
                    >
                        [ REFRESH ]
                    </button>
                </div>
            </div>

            {/* Search and Sort Bar */}
            <div className="flex gap-4 mb-6">
                <div className="flex-1 relative">
                    <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-500" />
                    <input
                        type="text"
                        value={searchFilter}
                        onChange={(e) => setSearchFilter(e.target.value)}
                        placeholder="Search library..."
                        className="w-full pl-10 pr-10 py-2 rounded-lg bg-[#2D2D30] text-white placeholder-gray-500 border border-white/10 focus:outline-none focus:border-neo-mint/50 transition-all"
                    />
                    {searchFilter && (
                        <button
                            onClick={() => setSearchFilter("")}
                            className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-white"
                        >
                            <X size={16} />
                        </button>
                    )}
                </div>
                <div className="relative">
                    <button
                        onClick={() => setSortMenuOpen(!sortMenuOpen)}
                        className="flex items-center gap-2 px-4 py-2 rounded-lg bg-[#2D2D30] text-white border border-white/10 hover:border-neo-mint/50 transition-all"
                    >
                        <ArrowUpDown size={16} />
                        <span className="text-sm">Sort: {sortBy.charAt(0).toUpperCase() + sortBy.slice(1)}</span>
                    </button>
                    {sortMenuOpen && (
                        <div className="absolute top-full right-0 mt-2 bg-[#1E1E24] border border-white/10 rounded-lg shadow-xl z-50 overflow-hidden">
                            {(["date", "name", "size"] as const).map((option) => (
                                <button
                                    key={option}
                                    onClick={() => {
                                        setSortBy(option);
                                        setSortMenuOpen(false);
                                    }}
                                    className={`w-full px-4 py-2 text-left text-sm transition-colors ${sortBy === option
                                        ? "bg-neo-mint/20 text-neo-mint"
                                        : "text-white/80 hover:bg-white/5"
                                        }`}
                                >
                                    {option.charAt(0).toUpperCase() + option.slice(1)}
                                </button>
                            ))}
                        </div>
                    )}
                </div>
            </div>

            {/* Active Download Queue */}
            {hasDownloads && (
                <div className="mb-12">
                    <h2 className="text-xl font-bold text-white mb-4 flex items-center gap-2">
                        <span className="text-cyber-pink animate-pulse">●</span> ACTIVE_QUEUE ({activeDownloadsList.length})
                        <span className="text-2xl animate-bounce ml-2">👾</span>
                    </h2>
                    <div className="space-y-4">
                        {activeDownloadsList.map((progress) => {
                            // Format duration helper
                            const formatDuration = (seconds: number) => {
                                if (!seconds) return "--:--";
                                const min = Math.floor(seconds / 60);
                                const sec = Math.floor(seconds % 60);
                                return `${min}:${sec < 10 ? '0' : ''}${sec}`;
                            };

                            return (
                                <div key={progress.id} className="solid-card rounded-xl shadow-lg border border-neo-mint/20 bg-[#1E1E20] relative overflow-hidden group">
                                    {/* Background Pulse Effect (only if downloading) */}
                                    {progress.status !== 'paused' && progress.status !== 'complete' && (
                                        <div className="absolute top-0 left-0 w-full h-1 bg-gradient-to-r from-neo-mint to-cyber-pink animate-pulse" />
                                    )}

                                    <div className="flex gap-4 p-4">
                                        {/* Thumbnail Section */}
                                        <div className="relative flex-shrink-0 w-40 h-24 rounded-lg overflow-hidden bg-card-darker border border-white/10">
                                            {progress.thumbnail ? (
                                                <img
                                                    src={progress.thumbnail}
                                                    alt={progress.title || "Video thumbnail"}
                                                    className="w-full h-full object-cover"
                                                />
                                            ) : (
                                                <div className="w-full h-full flex items-center justify-center text-gray-600">
                                                    <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                                        <polygon points="23 7 16 12 23 17 23 7"></polygon>
                                                        <rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect>
                                                    </svg>
                                                </div>
                                            )}

                                            {/* Duration Badge */}
                                            {progress.duration && progress.duration > 0 && (
                                                <div className="absolute bottom-1 right-1 bg-black/80 text-white text-xs font-bold px-1.5 py-0.5 rounded">
                                                    {formatDuration(progress.duration)}
                                                </div>
                                            )}

                                            {/* Progress Overlay on Thumbnail */}
                                            <div
                                                className="absolute bottom-0 left-0 h-1 bg-neo-mint shadow-[0_0_8px_rgba(0,255,163,0.6)]"
                                                style={{ width: `${progress.percent}%` }}
                                            />
                                        </div>

                                        {/* Content Section */}
                                        <div className="flex-1 min-w-0 flex flex-col justify-between">
                                            {/* Title and Controls Row */}
                                            <div className="flex items-start justify-between gap-3">
                                                <div className="min-w-0 flex-1">
                                                    <h3 className={`text-white font-bold text-base truncate ${progress.status !== 'complete' && progress.status !== 'paused' && progress.status !== 'error' ? 'kawaii-text-change' : ''}`} title={progress.title}>
                                                        {progress.title || progress.filename || "Downloading... 🚀"}
                                                    </h3>
                                                    <p className="text-text-muted font-mono text-xs truncate mt-1" title={progress.filename}>
                                                        {progress.filename || "Initializing..."}
                                                    </p>
                                                </div>

                                                {/* Controls */}
                                                <div className="flex items-center gap-2 flex-shrink-0">
                                                    <span className={`text-xs font-bold px-2 py-1 rounded ${progress.status === 'paused' ? 'bg-yellow-500/20 text-yellow-400' :
                                                        progress.status === 'complete' ? 'bg-green-500/20 text-green-400' :
                                                            progress.status === 'error' ? 'bg-red-500/20 text-red-400' :
                                                                progress.status === 'muxing' ? 'bg-purple-500/20 text-purple-400' :
                                                                    'bg-neo-mint/20 text-neo-mint'
                                                        }`}>
                                                        {progress.status === 'muxing' ? 'MUXING' : progress.status.toUpperCase()}
                                                    </span>

                                                    {/* Pause/Resume Controls */}
                                                    {progress.status === 'paused' ? (
                                                        <button
                                                            onClick={() => onResume && onResume(progress.id)}
                                                            className="p-1.5 rounded-md bg-green-500/20 text-green-400 hover:bg-green-500 hover:text-black transition-all"
                                                            title="Resume Download"
                                                        >
                                                            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                                                                <path d="M5 3l14 9-14 9V3z" />
                                                            </svg>
                                                        </button>
                                                    ) : progress.status !== 'complete' && progress.status !== 'error' && (
                                                        <button
                                                            onClick={() => onPause && onPause(progress.id)}
                                                            className="p-1.5 rounded-md bg-yellow-500/20 text-yellow-400 hover:bg-yellow-500 hover:text-black transition-all"
                                                            title="Pause Download"
                                                        >
                                                            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                                                                <rect x="6" y="4" width="4" height="16" />
                                                                <rect x="14" y="4" width="4" height="16" />
                                                            </svg>
                                                        </button>
                                                    )}

                                                    {/* Cancel Button */}
                                                    {progress.status !== 'complete' && (
                                                        <button
                                                            onClick={() => handleCancel(progress.id)}
                                                            className="p-1.5 rounded-md bg-red-500/20 text-red-400 hover:bg-red-500 hover:text-white transition-all"
                                                            title="Cancel Download"
                                                        >
                                                            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                                                                <line x1="18" y1="6" x2="6" y2="18"></line>
                                                                <line x1="6" y1="6" x2="18" y2="18"></line>
                                                            </svg>
                                                        </button>
                                                    )}
                                                </div>
                                            </div>

                                            {/* Progress Bar */}
                                            <div className="mt-3">
                                                <div className="w-full bg-[#2D2D30] h-2 rounded-full overflow-hidden">
                                                    <div
                                                        className={`h-full transition-all duration-300 ease-out ${progress.status === 'paused' ? 'bg-yellow-500' :
                                                            progress.status === 'complete' ? 'bg-green-500' :
                                                                progress.status === 'error' ? 'bg-red-500' :
                                                                    progress.status === 'muxing' ? 'bg-purple-500 shadow-[0_0_10px_rgba(168,85,247,0.5)]' :
                                                                        'bg-neo-mint shadow-[0_0_10px_rgba(0,255,163,0.5)]'
                                                            }`}
                                                        style={{ width: `${progress.status === 'error' ? 100 : progress.percent}%` }}
                                                    />
                                                </div>

                                                {/* Stats Row - Show error message OR progress stats */}
                                                {progress.status === 'error' ? (
                                                    <div className="flex items-center gap-2 mt-2 text-xs text-red-400">
                                                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                                            <circle cx="12" cy="12" r="10"></circle>
                                                            <line x1="12" y1="8" x2="12" y2="12"></line>
                                                            <line x1="12" y1="16" x2="12.01" y2="16"></line>
                                                        </svg>
                                                        <span className="flex-1 truncate" title={progress.error_message || "Unknown error"}>
                                                            {progress.error_message || "Download failed. Please try again."}
                                                        </span>
                                                    </div>
                                                ) : (
                                                    <div className="flex justify-between items-center mt-2 text-xs font-mono">
                                                        <div className="flex items-center gap-4 text-gray-500">
                                                            <span><span className="text-neo-mint">{progress.percent.toFixed(1)}%</span></span>
                                                            <span>↓ {progress.speed || "0 B/s"}</span>
                                                        </div>
                                                        <span className="text-gray-500">ETA: {progress.eta || "--:--"}</span>
                                                    </div>
                                                )}
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                </div>
            )}

            {loading ? (
                <div className="text-cyber-pink font-mono animate-pulse text-xl">
                    {'>'} SCANNING_STORAGE_MEDIA...
                </div>
            ) : totalVideos === 0 && !hasDownloads ? (
                <div className="text-text-muted font-mono bg-card-dark p-6 rounded-xl border border-white/10">
                    {'>'} NO_FILES_FOUND
                </div>
            ) : (
                <div className="space-y-10 pb-6">
                    {/* Playlists Section */}
                    {filteredPlaylists.length > 0 && (
                        <section>
                            <h2 className="text-xl font-bold text-white mb-4 flex items-center gap-2">
                                <span className="text-electric-lavender">▶</span> PLAYLISTS
                                {searchFilter && <span className="text-xs text-gray-500 font-normal">({filteredPlaylists.length} matching)</span>}
                            </h2>
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                                {filteredPlaylists.map((playlist) => (
                                    <PlaylistCard
                                        key={playlist.path}
                                        playlist={playlist}
                                        onVideoSelect={setSelectedVideo}
                                        formatBytes={formatBytes}
                                        onOpenFolder={handleRevealFile}
                                    />
                                ))}
                            </div>
                        </section>
                    )}

                    {/* Singles Section */}
                    {filteredSingles.length > 0 && (
                        <section>
                            <h2 className="text-xl font-bold text-white mb-4 flex items-center gap-2">
                                <span className="text-neo-mint">◆</span> VIDEOS
                                {searchFilter && <span className="text-xs text-gray-500 font-normal">({filteredSingles.length} matching)</span>}
                            </h2>
                            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6">
                                {filteredSingles.map((video) => (
                                    <div
                                        key={video.path}
                                        onClick={() => setSelectedVideo(video)}
                                        className="bg-card-dark rounded-2xl p-4 cursor-pointer hover:scale-105 transition-all duration-300 shadow-card-float group border border-white/10 relative overflow-hidden hover:border-neo-mint/30"
                                    >
                                        {/* Decoration */}
                                        <div className="absolute top-0 right-0 w-16 h-16 bg-gradient-to-bl from-neo-mint/10 to-transparent rounded-bl-full -mr-4 -mt-4 group-hover:from-neo-mint/30 transition-all" />

                                        <div className="aspect-video bg-card-darker mb-4 rounded-xl flex items-center justify-center border border-white/5 group-hover:border-neo-mint/30 transition-colors shadow-inner overflow-hidden relative">
                                            <span className="text-5xl text-gray-600 group-hover:text-neo-mint transition-colors relative z-10">
                                                ▶
                                            </span>
                                            {/* Subtle pattern overlay */}
                                            <div className="absolute inset-0 opacity-5 bg-[radial-gradient(#fff_1px,transparent_1px)] [background-size:8px_8px] pointer-events-none"></div>
                                        </div>

                                        <div className="mb-3">
                                            <h3 className="text-white font-bold text-base truncate" title={video.name}>
                                                {video.name}
                                            </h3>
                                        </div>

                                        <div className="flex justify-between text-xs text-text-muted font-mono bg-card-darker px-3 py-2 rounded-lg border border-white/5">
                                            <span className="font-semibold">{formatBytes(video.size)}</span>
                                            <span className="text-cyber-pink font-bold">{video.extension.toUpperCase()}</span>
                                        </div>

                                        <div className="text-xs text-gray-500 font-mono mt-2 flex justify-between items-center">
                                            <span>{video.modified_date}</span>

                                            <button
                                                onClick={(e) => {
                                                    e.stopPropagation();
                                                    handleRevealFile(video.path);
                                                }}
                                                className="opacity-0 group-hover:opacity-100 p-1.5 hover:bg-neo-mint/20 text-neo-mint rounded transition-all mr-1"
                                                title="Open in Folder"
                                            >
                                                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z"></path></svg>
                                            </button>
                                            <button
                                                onClick={(e) => {
                                                    e.stopPropagation();
                                                    handleDelete(video.path);
                                                }}
                                                className="opacity-0 group-hover:opacity-100 p-1.5 hover:bg-red-500/20 text-red-500 rounded transition-all"
                                                title="Delete File"
                                            >
                                                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>
                                            </button>
                                        </div>
                                    </div>
                                ))}
                            </div>
                        </section>
                    )}
                </div>
            )}

            {selectedVideo && (
                <CyberPlayer
                    videoPath={selectedVideo.path}
                    videoName={selectedVideo.name}
                    onClose={() => setSelectedVideo(null)}
                />
            )}
        </div>
    );
}

export default Downloads;
