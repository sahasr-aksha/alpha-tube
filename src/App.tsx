import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import VideoMetadataCard, { VideoMetadataResponse, VideoFormat } from "./VideoMetadataCard";
import PlaylistMetadataCard, { PlaylistMetadataResponse, SelectedVideo } from "./PlaylistMetadataCard";
import { listen } from "@tauri-apps/api/event";
import { downloadDir } from "@tauri-apps/api/path";
import { motion, AnimatePresence } from "framer-motion";
import { sendNotification } from "@tauri-apps/plugin-notification";
import { open } from "@tauri-apps/plugin-dialog";
import Downloads from "./Downloads";
import Settings from "./Settings";
import AboutUs from "./AboutUs";

import SearchResultCard, { VideoSearchResult } from "./SearchResultCard";
import { SkeletonGrid, FormatSkeletonGrid } from "./SkeletonCard";
import { reRankSearchResults } from "./searchUtils";
import PlatformSelector, { PLATFORMS } from "./PlatformSelector";
import { Search, Library, Info, Menu, ChevronLeft, Settings as SettingsIcon, X, Clock } from "lucide-react";
import "./App.css";
import Toast from "./Toast";
import ActionDialog from "./ActionDialog";
import StreamPlayer from "./StreamPlayer";
import UpdateNotification from "./UpdateNotification";
import LegalDisclaimer from "./LegalDisclaimer";
import { check } from "@tauri-apps/plugin-updater";

// Export this interface so it can be used in Downloads.tsx
export interface DownloadProgress {
  id: string;
  percent: number;
  speed: string;
  eta: string;
  status: string;
  filename: string;
  // Rich metadata for Snaptube-like download cards
  title?: string;
  thumbnail?: string;
  duration?: number;
  // User-friendly error message when status is "error"
  error_message?: string;
}

export interface AppConfig {
  download_dir: string | null;
}

/**
 * Convert technical error messages to user-friendly text with actionable suggestions
 */
function getUserFriendlyError(error: string): string {
  const errorLower = error.toLowerCase();

  if (errorLower.includes('network') || errorLower.includes('fetch') || errorLower.includes('connection')) {
    return "Unable to connect. Please check your internet connection and try again.";
  }
  if (errorLower.includes('not found') || errorLower.includes('404')) {
    return "Video not found. The link may be broken or the video was removed.";
  }
  if (errorLower.includes('private') || errorLower.includes('unavailable')) {
    return "This video is private or unavailable in your region.";
  }
  if (errorLower.includes('age') || errorLower.includes('sign in')) {
    return "This video requires age verification or sign-in, which is not supported.";
  }
  if (errorLower.includes('rate limit') || errorLower.includes('too many')) {
    return "Too many requests. Please wait a moment and try again.";
  }
  if (errorLower.includes('format') || errorLower.includes('no video')) {
    return "No downloadable formats found for this video.";
  }
  if (errorLower.includes('timeout')) {
    return "Request timed out. The server may be slow - try again later.";
  }

  // Default fallback with original error for debugging
  return `Something went wrong: ${error.slice(0, 100)}`;
}

function App() {
  // Legal disclaimer state - check if user has accepted terms
  const [termsAccepted, setTermsAccepted] = useState(() => {
    return localStorage.getItem("termsAccepted") === "true";
  });

  const [url, setUrl] = useState("");
  // Replaced single downloading boolean with derived state from activeDownloads keys
  const [activeDownloads, setActiveDownloads] = useState<Record<string, DownloadProgress>>({});
  const [statusMessage, setStatusMessage] = useState("");
  const [toastMessage, setToastMessage] = useState("");
  const [showToast, setShowToast] = useState(false);
  const [activeTab, setActiveTab] = useState("home"); // home, downloads, browse, about
  const [formats, setFormats] = useState<VideoFormat[]>([]);
  const [videoMetadata, setVideoMetadata] = useState<VideoMetadataResponse | null>(null);
  const [playlistMetadata, setPlaylistMetadata] = useState<PlaylistMetadataResponse | null>(null);
  const [loadingFormats, setLoadingFormats] = useState(false);

  // Search results state
  const [searchResults, setSearchResults] = useState<VideoSearchResult[]>([]);
  const [selectedPlatform, setSelectedPlatform] = useState("ytsearch");
  const [searchLoading, setSearchLoading] = useState(false);
  // Pagination state for dynamic loading
  const [searchPage, setSearchPage] = useState(1);
  const [searchQuery, setSearchQuery] = useState("");
  const [hasMoreResults, setHasMoreResults] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  // Exclude YouTube Shorts from search results (default: enabled)
  const [excludeShorts, setExcludeShorts] = useState(true);
  const MIN_DISPLAY_RESULTS = 8; // Minimum results to show before considering "enough"

  // Search history state
  const [searchHistory, setSearchHistory] = useState<string[]>(() => {
    const saved = localStorage.getItem("searchHistory");
    return saved ? JSON.parse(saved) : [];
  });
  const [showSearchHistory, setShowSearchHistory] = useState(false);

  // Save search to history
  const addToSearchHistory = (query: string) => {
    if (!query.trim() || query.startsWith('http')) return; // Don't save URLs
    const updated = [query, ...searchHistory.filter(h => h !== query)].slice(0, 10);
    setSearchHistory(updated);
    localStorage.setItem("searchHistory", JSON.stringify(updated));
  };

  // Clear search history
  const clearSearchHistory = () => {
    setSearchHistory([]);
    localStorage.removeItem("searchHistory");
  };

  // App Config
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [showSetup, setShowSetup] = useState(false);

  // Collapsible sidebar state
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => {
    const saved = localStorage.getItem("sidebarCollapsed");
    return saved ? JSON.parse(saved) : false;
  });

  // Sidebar width for YouTubeBrowser positioning
  const sidebarWidth = sidebarCollapsed ? 70 : 260;

  // Auto-Update state
  const [updateReady, setUpdateReady] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<{ version: string; body?: string } | null>(null);

  // Action Dialog & Stream Player state
  const [actionDialogVisible, setActionDialogVisible] = useState(false);
  const [streamPlayerVisible, setStreamPlayerVisible] = useState(false);
  const [pendingVideo, setPendingVideo] = useState<{ url: string; title: string; thumbnail: string } | null>(null);

  // Load Config
  useEffect(() => {
    invoke<AppConfig>("get_app_config").then(c => {
      setConfig(c);
      if (!c.download_dir) {
        setShowSetup(true);
      }
    }).catch(console.error);
  }, []);

  const handleSetDirectory = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select Download Location"
      });

      if (typeof selected === "string") {
        await invoke("set_download_dir", { path: selected });
        setConfig({ download_dir: selected });
        setShowSetup(false);
      }
    } catch (err) {
      console.error("Failed to select directory:", err);
    }
  };



  useEffect(() => {
    const unlisten = listen<DownloadProgress>("download-progress", async (event) => {
      const { id, status, filename } = event.payload;

      setActiveDownloads(prev => {
        const old = prev[id];
        let newProgress = event.payload;

        // PRESERVE STATE ON PAUSE/RESUME
        // If backend sends 0% or empty filename during pause/resume events, keep existing values
        if (old && (status === 'paused' || status.includes('resuming'))) {
          if (newProgress.percent === 0) newProgress.percent = old.percent;
          if (!newProgress.filename) newProgress.filename = old.filename;
        }

        // PRESERVE RICH METADATA (title, thumbnail, duration) from initial state
        // Backend only sends basic progress info, we keep the metadata we stored initially
        if (old) {
          newProgress.title = old.title;
          newProgress.thumbnail = old.thumbnail;
          newProgress.duration = old.duration;
        }

        const newState = { ...prev, [id]: newProgress };
        return newState;
      });

      // Notifications
      if (status === "complete") {
        setStatusMessage(`Download complete: ${filename}`);
        // Only clear formats if it was the current video? Hard to tell. 
        setTimeout(() => setStatusMessage(""), 5000);

        try {
          await sendNotification({
            title: 'Download Complete',
            body: `Finished: ${filename}`,
          });
        } catch (e) {
          console.error(e);
        }

        // Cleanup finished download from list after 30s (improved visibility)
        setTimeout(() => {
          setActiveDownloads(prev => {
            const copy = { ...prev };
            delete copy[id];
            return copy;
          });
        }, 30000);

      } else if (status === "error") {
        setStatusMessage(`Error: ${filename}`);

        try {
          await sendNotification({
            title: 'Download Failed',
            body: `Error: ${filename}`,
          });
        } catch (e) {
          console.error(e);
        }

        // Cleanup failed download event from list after 10s
        setTimeout(() => {
          setActiveDownloads(prev => {
            const copy = { ...prev };
            delete copy[id];
            return copy;
          });
        }, 10000);
      } else if (status === "cancelled") {
        setActiveDownloads(prev => {
          const copy = { ...prev };
          delete copy[id];
          return copy;
        });
      }
    });

    const unlistenVideo = listen<{ url: string }>("video-detected", (event) => {
      console.log("Video detected:", event.payload.url);
      setUrl(event.payload.url);
      setActiveTab("home");

      setLoadingFormats(true);
      setStatusMessage("Video Detected: Analyzing...");

      invoke<VideoMetadataResponse>("get_video_metadata", { url: event.payload.url })
        .then((result) => {
          setVideoMetadata(result);
          const sorted = result.formats.sort((a, b) => b.filesize - a.filesize);
          setFormats(sorted);
          setStatusMessage(`Found ${result.formats.length} formats for "${result.title}"`);
        })
        .catch((error) => {
          console.error(error);
          setStatusMessage(`Failed to fetch formats: ${error}`);
        })
        .finally(() => {
          setLoadingFormats(false);
        });
    });

    return () => {
      unlisten.then((f) => f());
      unlistenVideo.then((f) => f());
    };
  }, []);

  // Listen for update-ready event from backend
  useEffect(() => {
    const unlistenUpdate = listen<{ version: string; body?: string }>("update-ready", (event) => {
      console.log("[Update] Ready to install:", event.payload);
      setUpdateInfo(event.payload);
      setUpdateReady(true);
    });

    return () => {
      unlistenUpdate.then((f) => f());
    };
  }, []);

  // Handle restart to apply update
  const handleRestartForUpdate = async () => {
    try {
      // Check for update again and install
      const update = await check();
      if (update) {
        await update.downloadAndInstall();
        // Note: On Windows NSIS, the installer handles relaunch automatically
        // Do not call relaunch() here as it may interfere with the installer
      }
    } catch (error) {
      console.error("[Update] Failed to install:", error);
      setStatusMessage(`Update failed: ${error}`);
      setToastMessage("Update failed. Try manual update in Settings.");
      setShowToast(true);
    }
  };

  // KAWAII LOADING EFFECT
  useEffect(() => {
    if (!loadingFormats) return;

    const messages = [
      "Summoning pixels... ✨",
      "Reading the matrix... 🌸",
      "Beep boop! Working hard! 🤖",
      "Parsing video vibes... 📼",
      "Fetching the data cookies... 🍪",
      "Dusting off the cyber-shelves... 🧹",
      "Consulting the oracle... 🔮"
    ];

    let i = 0;
    // Set initial message immediately
    setStatusMessage(messages[0]);

    const interval = setInterval(() => {
      i = (i + 1) % messages.length;
      setStatusMessage(messages[i]);
    }, 800);

    return () => clearInterval(interval);
  }, [loadingFormats]);

  // Helper to detect if input is a URL
  const isUrl = (input: string): boolean => {
    const trimmed = input.trim();
    return trimmed.includes("://") ||
      trimmed.startsWith("www.") ||
      trimmed.includes("youtube.com") ||
      trimmed.includes("youtu.be");
  };

  const handleSearch = async () => {
    if (!url) {
      setStatusMessage("Please enter a URL or search query.");
      return;
    }

    // Clear previous results
    setFormats([]);
    setVideoMetadata(null);
    setPlaylistMetadata(null);
    setSearchResults([]);

    if (isUrl(url)) {
      // URL mode - fetch video metadata (existing behavior)
      setLoadingFormats(true);
      setStatusMessage("Fetching video formats...");

      try {
        const result = await invoke<VideoMetadataResponse>("get_video_metadata", { url });

        setVideoMetadata(result);

        // Sort: 4K/High res first
        if (!result.is_playlist) {
          setPlaylistMetadata(null);
          const sorted = result.formats.sort((a, b) => {
            return b.filesize - a.filesize;
          });
          setFormats(sorted);
          setStatusMessage(`Found ${result.formats.length} formats for "${result.title}"`);
        } else {
          // PLAYLIST DETECTED - fetch full playlist metadata
          setVideoMetadata(null);
          setFormats([]);
          setStatusMessage(`Playlist detected: "${result.title}" - Fetching video list...`);

          try {
            const playlistData = await invoke<PlaylistMetadataResponse>("get_playlist_metadata", { url });
            setPlaylistMetadata(playlistData);
            setStatusMessage(`Playlist "${playlistData.title}" - ${playlistData.videos.length} videos ready`);
          } catch (playlistError) {
            console.error("Failed to fetch playlist details:", playlistError);
            setStatusMessage(`Playlist found but couldn't load details: ${playlistError}`);
          }
        }
      } catch (error) {
        console.error(error);
        setStatusMessage(getUserFriendlyError(String(error)));
      } finally {
        setLoadingFormats(false);
      }
    } else {
      // Text search mode - search using selected platform
      const platformName = PLATFORMS.find(p => p.id === selectedPlatform)?.name || "YouTube";
      setSearchLoading(true);
      setStatusMessage(`Searching ${platformName}...`);
      setSearchQuery(url); // Store query for Load More
      setSearchPage(1);
      addToSearchHistory(url); // Save to search history

      try {
        // Fetch larger initial pool for better filtering
        let allResults: VideoSearchResult[] = [];
        let currentPage = 1;
        const PAGE_SIZE = 25;
        const MAX_PAGES = 4; // Max pages to auto-fetch

        // Keep fetching until we have enough filtered results or hit max
        while (currentPage <= MAX_PAGES) {
          const results = await invoke<VideoSearchResult[]>("search_videos", {
            query: url,
            platform: selectedPlatform,
            page: currentPage,
            pageSize: PAGE_SIZE,
            excludeShorts: selectedPlatform === "ytsearch" && excludeShorts,
          });

          if (results.length === 0) {
            // No more results from backend
            setHasMoreResults(false);
            break;
          }

          allResults = [...allResults, ...results];

          // Apply fuzzy re-ranking: filter by 90% similarity threshold, sort by view count
          const rankedResults = reRankSearchResults(allResults, url, 0.9);

          if (rankedResults.length >= MIN_DISPLAY_RESULTS || currentPage >= MAX_PAGES) {
            setSearchResults(rankedResults);
            setSearchPage(currentPage);
            setHasMoreResults(results.length === PAGE_SIZE); // More available if we got full page
            setStatusMessage(
              `Found ${allResults.length} results on ${platformName}, showing ${rankedResults.length} relevant matches`
            );
            break;
          }

          // Not enough filtered results, fetch next page
          setStatusMessage(`Searching for more relevant results... (page ${currentPage + 1})`);
          currentPage++;
        }

        if (allResults.length === 0) {
          setStatusMessage(`No results found for "${url}"`);
          setHasMoreResults(false);
        }
      } catch (error) {
        console.error(error);
        setStatusMessage(getUserFriendlyError(String(error)));
      } finally {
        setSearchLoading(false);
      }
    }
  };

  // Load more search results
  const handleLoadMore = async () => {
    if (!searchQuery || loadingMore) return;

    const platformName = PLATFORMS.find(p => p.id === selectedPlatform)?.name || "YouTube";
    setLoadingMore(true);
    const nextPage = searchPage + 1;

    try {
      const results = await invoke<VideoSearchResult[]>("search_videos", {
        query: searchQuery,
        platform: selectedPlatform,
        page: nextPage,
        pageSize: 25,
        excludeShorts: selectedPlatform === "ytsearch" && excludeShorts,
      });

      if (results.length === 0) {
        setHasMoreResults(false);
        setStatusMessage("No more results available");
        return;
      }

      // Filter new results and add to existing
      const newFiltered = reRankSearchResults(results, searchQuery, 0.9);

      // Merge with existing results (avoid duplicates by video_url)
      const existingUrls = new Set(searchResults.map(r => r.video_url));
      const uniqueNew = newFiltered.filter(r => !existingUrls.has(r.video_url));

      // Re-sort combined results by view count
      const combined = [...searchResults, ...uniqueNew].sort((a, b) =>
        (b.view_count ?? 0) - (a.view_count ?? 0)
      );

      setSearchResults(combined);
      setSearchPage(nextPage);
      setHasMoreResults(results.length === 25);
      setStatusMessage(
        `Showing ${combined.length} relevant matches from ${platformName}`
      );
    } catch (error) {
      console.error(error);
      setStatusMessage(`Failed to load more: ${error}`);
    } finally {
      setLoadingMore(false);
    }
  };

  // Handler when user clicks a search result - show action dialog
  const handleSearchResultClick = async (videoUrl: string, title?: string, thumbnail?: string | null) => {
    setPendingVideo({
      url: videoUrl,
      title: title || "Video",
      thumbnail: thumbnail || ""
    });
    setActionDialogVisible(true);
  };

  // Handle Play choice from action dialog
  const handlePlayChoice = () => {
    setActionDialogVisible(false);
    setStreamPlayerVisible(true);
  };

  // Handle Download choice from action dialog
  const handleDownloadChoice = async () => {
    if (!pendingVideo) return;

    setActionDialogVisible(false);
    setUrl(pendingVideo.url);
    setSearchResults([]);
    setLoadingFormats(true);
    setStatusMessage("Fetching video metadata...");

    try {
      const result = await invoke<VideoMetadataResponse>("get_video_metadata", { url: pendingVideo.url });
      setVideoMetadata(result);

      if (!result.is_playlist) {
        const sorted = result.formats.sort((a, b) => b.filesize - a.filesize);
        setFormats(sorted);
        setStatusMessage(`Found ${result.formats.length} formats for "${result.title}"`);
      } else {
        setFormats([]);
        setStatusMessage(`Playlist detected: ${result.title}`);
      }
    } catch (error) {
      console.error(error);
      setStatusMessage(`Failed to fetch formats: ${error}`);
    } finally {
      setLoadingFormats(false);
    }
  };

  // Handle switching from stream player to download
  const handleStreamToDownload = async () => {
    if (!pendingVideo) return;

    setStreamPlayerVisible(false);
    setUrl(pendingVideo.url);
    setSearchResults([]);
    setLoadingFormats(true);
    setStatusMessage("Preparing download options...");

    try {
      const result = await invoke<VideoMetadataResponse>("get_video_metadata", { url: pendingVideo.url });
      setVideoMetadata(result);

      if (!result.is_playlist) {
        const sorted = result.formats.sort((a, b) => b.filesize - a.filesize);
        setFormats(sorted);
        setStatusMessage(`Found ${result.formats.length} formats for "${result.title}"`);
      } else {
        setFormats([]);
        setStatusMessage(`Playlist detected: ${result.title}`);
      }
    } catch (error) {
      console.error(error);
      setStatusMessage(`Failed to fetch formats: ${error}`);
    } finally {
      setLoadingFormats(false);
    }
  };

  // Handler for playlist batch download
  const handlePlaylistDownload = async (selectedVideos: SelectedVideo[]) => {
    if (!playlistMetadata || selectedVideos.length === 0) return;

    setToastMessage(`Starting ${selectedVideos.length} downloads from playlist...`);
    setShowToast(true);
    setStatusMessage(`Queueing ${selectedVideos.length} videos from "${playlistMetadata.title}"...`);

    const output_path = await downloadDir();

    // Queue each selected video as individual download
    for (const video of selectedVideos) {
      const downloadId = Date.now().toString() + Math.random().toString(36).substring(2, 9);

      // Add to active downloads with playlist context
      setActiveDownloads(prev => ({
        ...prev,
        [downloadId]: {
          id: downloadId,
          percent: 0,
          speed: "0 MB/s",
          eta: "--:--",
          status: "queued",
          filename: `[${video.index}/${selectedVideos.length}] ${video.title}`,
          title: video.title,
          thumbnail: video.thumbnail,
          duration: video.duration,
          // Playlist context (will be used for grouping in Downloads.tsx later)
        }
      }));

      // Start the download
      try {
        await invoke("download_video", {
          options: {
            id: downloadId,
            url: video.url,
            quality: video.quality,
            output_path,
            format_id: video.formatId
          },
        });
        console.log("Playlist video download started:", downloadId, video.title);
      } catch (error) {
        console.error("Failed to start download for:", video.title, error);
        // Remove failed download from state
        setActiveDownloads(prev => {
          const copy = { ...prev };
          delete copy[downloadId];
          return copy;
        });
      }
    }

    setStatusMessage(`Downloads started for ${selectedVideos.length} videos`);
  };


  const handleDownload = async (formatId: string | null = null, qualityPreset: string = "best") => {
    if (!url) return;

    // Generate unique ID
    const downloadId = Date.now().toString() + Math.random().toString(36).substring(2, 9);

    setStatusMessage("Starting download...");
    setToastMessage("Download Started! 🚀");
    setShowToast(true);

    // Initial Progress State with rich metadata for Snaptube-like cards
    setActiveDownloads(prev => ({
      ...prev,
      [downloadId]: {
        id: downloadId,
        percent: 0,
        speed: "0 MB/s",
        eta: "--:--",
        status: "starting",
        filename: "Initializing...",
        // Include video metadata for rich download cards
        title: videoMetadata?.title || "Unknown Video",
        thumbnail: videoMetadata?.thumbnail_url || "",
        duration: videoMetadata?.duration || 0,
      }
    }));

    // Send Start Notification
    try {
      await sendNotification({
        title: 'Download Started',
        body: videoMetadata?.title ? `Downloading: ${videoMetadata.title}` : 'Downloading video...',
      });
    } catch (e) {
      console.error("Failed to send notification:", e);
    }


    try {
      const output_path = await downloadDir();
      await invoke("download_video", {
        options: {
          id: downloadId,
          url,
          quality: qualityPreset, // "best" is fallback/default if formatId logic fails, or if mp3
          output_path,
          format_id: formatId
        },
      });
      console.log("Download started", downloadId);
    } catch (error) {
      console.error(error);
      // Remove from map if start failed
      setActiveDownloads(prev => {
        const copy = { ...prev };
        delete copy[downloadId];
        return copy;
      });
      setStatusMessage(`Failed to start download: ${error}`);
    }
  };

  // Toggle sidebar collapsed state
  const toggleSidebar = () => {
    const newState = !sidebarCollapsed;
    setSidebarCollapsed(newState);
    localStorage.setItem("sidebarCollapsed", JSON.stringify(newState));
  };






  // Pause/Resume handlers
  const handlePause = async (id: string) => {
    // Optimistic update
    setActiveDownloads(prev => {
      const current = prev[id];
      if (!current) return prev;
      return {
        ...prev,
        [id]: { ...current, status: "pausing..." }
      };
    });

    try {
      await invoke("pause_download", { id });
      console.log(`Paused download ${id}`);
    } catch (error) {
      console.error(`Failed to pause download ${id}:`, error);
      setStatusMessage(`Failed to pause: ${error}`);
      // Revert status if needed, but the backend "downloading" events might override it anyway
    }
  };

  const handleResume = async (id: string) => {
    // Optimistic update
    setActiveDownloads(prev => {
      const current = prev[id];
      if (!current) return prev;
      return {
        ...prev,
        [id]: { ...current, status: "resuming..." }
      };
    });

    try {
      await invoke("resume_download", { id });
      console.log(`Resumed download ${id}`);
    } catch (error) {
      console.error(`Failed to resume download ${id}:`, error);
      setStatusMessage(`Failed to resume: ${error}`);

      // Revert to paused if failed
      setActiveDownloads(prev => {
        const current = prev[id];
        if (!current) return prev;
        return {
          ...prev,
          [id]: { ...current, status: "paused" }
        };
      });
    }
  };

  // Show legal disclaimer on first launch
  if (!termsAccepted) {
    return <LegalDisclaimer onAccept={() => setTermsAccepted(true)} />;
  }

  return (
    <div className="h-screen flex flex-row relative overflow-hidden font-sans text-gray-100 app-glass-container">

      {/* Sidebar - Solid/Semi-Solid Adobe Style */}
      <aside
        className={`sidebar-glass flex flex-col p-4 gap-4 relative z-20 transition-all duration-300 ease-in-out h-full`}
        style={{ width: `${sidebarWidth}px` }}
      >
        {/* Header with toggle button */}
        <div className="flex items-center gap-3 relative z-10 mb-2">
          <button
            onClick={toggleSidebar}
            className="p-2 rounded-md hover:bg-white/10 text-gray-400 hover:text-white transition-all"
            title={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          >
            {sidebarCollapsed ? <Menu size={18} /> : <ChevronLeft size={18} />}
          </button>
          {!sidebarCollapsed && (
            <div className="text-lg font-bold tracking-wide text-white flex items-center gap-2 select-none">
              <span className="text-neo-mint text-2xl drop-shadow-md">α</span> Tube
            </div>
          )}
        </div>

        <nav className="flex flex-col gap-1 relative z-10 flex-1">
          {/* Nav Items - Adobe style: thinner, cleaner, hover highlight */}
          <button
            onClick={() => setActiveTab("home")}
            className={`p-2 rounded-md transition-all duration-150 text-sm font-medium flex items-center gap-3 ${activeTab === "home"
              ? "bg-[#37373D] text-white shadow-sm border-l-2 border-neo-mint"
              : "text-gray-400 hover:bg-[#2D2D30] hover:text-white"
              }`}
            title="Search"
            aria-label="Search for videos"
            aria-current={activeTab === "home" ? "page" : undefined}
          >
            <Search size={18} />
            {!sidebarCollapsed && <span>Search</span>}
          </button>

          <button
            onClick={() => setActiveTab("downloads")}
            className={`p-2 rounded-md transition-all duration-150 text-sm font-medium flex items-center gap-3 ${activeTab === "downloads"
              ? "bg-[#37373D] text-white shadow-sm border-l-2 border-neo-mint"
              : "text-gray-400 hover:bg-[#2D2D30] hover:text-white"
              }`}
            title="Library"
            aria-label="View download library"
            aria-current={activeTab === "downloads" ? "page" : undefined}
          >
            <Library size={18} />
            {!sidebarCollapsed && <span>Library</span>}
          </button>



          <button
            onClick={() => setActiveTab("about")}
            className={`p-2 rounded-md transition-all duration-150 text-sm font-medium flex items-center gap-3 ${activeTab === "about"
              ? "bg-[#37373D] text-white shadow-sm border-l-2 border-electric-lavender"
              : "text-gray-400 hover:bg-[#2D2D30] hover:text-white"
              }`}
            title="About"
            aria-label="About Alpha Tube"
            aria-current={activeTab === "about" ? "page" : undefined}
          >
            <Info size={18} />
            {!sidebarCollapsed && <span>About</span>}
          </button>

          <button
            onClick={() => setActiveTab("settings")}
            className={`p-2 rounded-md transition-all duration-150 text-sm font-medium flex items-center gap-3 ${activeTab === "settings"
              ? "bg-[#37373D] text-white shadow-sm border-l-2 border-gray-400"
              : "text-gray-400 hover:bg-[#2D2D30] hover:text-white"
              }`}
            title="Settings"
            aria-label="Open settings"
            aria-current={activeTab === "settings" ? "page" : undefined}
          >
            <SettingsIcon size={18} />
            {!sidebarCollapsed && <span>Settings</span>}
          </button>
        </nav>


      </aside>

      {/* Main Area - Transparent to show Desktop (Acrylic) */}
      <main className="flex-1 relative flex flex-col overflow-hidden content-area z-10">

        <AnimatePresence mode="wait">
          {activeTab === "home" && (
            <motion.div
              key="home"
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -10 }}
              transition={{ duration: 0.2 }}
              className="flex-1 flex flex-col p-12 overflow-y-auto"
            >
              {/* Header Section */}
              <div className="mb-12 flex items-start justify-between">
                <div>
                  <h1 className="text-4xl text-white mb-2 font-bold tracking-tight drop-shadow-sm flex items-center gap-3">
                    <div className="w-1 h-8 bg-neo-mint rounded-full"></div>
                    SEARCH
                  </h1>
                  <p className="text-gray-400 text-base ml-4">Enter a URL or search for videos by title.</p>
                </div>
                {/* Platform Selector - top right */}
                <PlatformSelector
                  selectedPlatform={selectedPlatform}
                  onSelect={(platform) => {
                    setSelectedPlatform(platform);
                    const platformName = PLATFORMS.find(p => p.id === platform)?.name || "YouTube";
                    setToastMessage(`Source: ${platformName} (applies to text search only)`);
                    setShowToast(true);
                  }}
                  excludeShorts={excludeShorts}
                  onExcludeShortsChange={setExcludeShorts}
                />
              </div>

              {/* Input Section with Search History */}
              <div className="flex gap-4 mb-10">
                <div className="flex-1 relative">
                  <input
                    type="text"
                    value={url}
                    onChange={(e) => setUrl(e.target.value)}
                    placeholder="Enter URL or search query..."
                    className="w-full p-4 pr-10 rounded-lg input-solid text-white placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-neo-mint shadow-md text-base transition-all"
                    onKeyDown={(e) => e.key === "Enter" && handleSearch()}
                    onFocus={() => !url && searchHistory.length > 0 && setShowSearchHistory(true)}
                    onBlur={() => setTimeout(() => setShowSearchHistory(false), 200)}
                  />

                  {/* Clear Button */}
                  {url && (
                    <button
                      onClick={() => {
                        setUrl("");
                        setSearchResults([]);
                        setVideoMetadata(null);
                        setFormats([]);
                        setStatusMessage("");
                      }}
                      className="absolute right-3 top-1/2 -translate-y-1/2 p-1 rounded-full hover:bg-white/10 text-gray-400 hover:text-white transition-all"
                      aria-label="Clear search"
                    >
                      <X size={18} />
                    </button>
                  )}

                  {/* Search History Dropdown */}
                  <AnimatePresence>
                    {showSearchHistory && searchHistory.length > 0 && (
                      <motion.div
                        initial={{ opacity: 0, y: -10 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0, y: -10 }}
                        className="absolute top-full left-0 right-0 mt-2 bg-[#1E1E24] border border-white/10 rounded-lg shadow-xl z-50 overflow-hidden"
                      >
                        <div className="flex justify-between items-center px-3 py-2 border-b border-white/5">
                          <span className="text-xs text-gray-500 font-mono">Recent Searches</span>
                          <button
                            onClick={clearSearchHistory}
                            className="text-xs text-gray-500 hover:text-red-400 transition-colors"
                          >
                            Clear All
                          </button>
                        </div>
                        {searchHistory.map((query, index) => (
                          <button
                            key={`${query}-${index}`}
                            onMouseDown={() => {
                              setUrl(query);
                              setShowSearchHistory(false);
                              setTimeout(() => handleSearch(), 100);
                            }}
                            className="w-full px-3 py-2 text-left text-sm text-white/80 hover:bg-white/5 hover:text-neo-mint transition-colors flex items-center gap-2"
                          >
                            <Clock size={12} className="text-gray-500" />
                            <span className="truncate">{query}</span>
                          </button>
                        ))}
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
                <button
                  onClick={handleSearch}
                  disabled={loadingFormats || searchLoading}
                  className="px-8 py-4 rounded-lg bg-neo-mint text-black font-bold tracking-wide shadow-md hover:brightness-110 active:scale-95 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {loadingFormats || searchLoading ? "SEARCHING..." : "SEARCH"}
                </button>
              </div>

              {/* Quick Audio Download */}
              {formats.length === 0 && !loadingFormats && videoMetadata && !videoMetadata.is_playlist && (
                <div className="mb-8">
                  <button
                    onClick={() => handleDownload(null, "mp3")}
                    disabled={!url}
                    className="solid-card px-6 py-3 rounded-lg text-cyber-pink hover:text-white hover:bg-cyber-pink/20 transition-all font-bold border border-cyber-pink/30 flex items-center gap-2"
                  >
                    <span>⚡</span> Quick Audio Download (MP3)
                  </button>
                </div>
              )}

              {/* Status Message - SOLID CARD */}
              {statusMessage && (
                <div className="mb-6 font-mono text-sm text-neo-mint solid-card p-4 rounded-lg border-l-4 border-neo-mint shadow-md bg-[#1E1E20]">
                  {'>'} {statusMessage}
                </div>
              )}

              {/* Skeleton Loading State */}
              {searchLoading && (
                <div className="mb-10">
                  <h2 className="text-lg text-white font-semibold mb-4 flex items-center gap-2">
                    <span className="text-neo-mint animate-pulse">●</span> Searching...
                  </h2>
                  <SkeletonGrid count={8} />
                </div>
              )}

              {/* Search Results Grid */}
              {!searchLoading && searchResults.length > 0 && (
                <div className="mb-10">
                  <h2 className="text-lg text-white font-semibold mb-4 flex items-center gap-2">
                    <span className="text-neo-mint">▶</span> Search Results
                  </h2>
                  <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                    {searchResults.map((result, index) => (
                      <SearchResultCard
                        key={`${result.video_url}-${index}`}
                        result={result}
                        onClick={handleSearchResultClick}
                      />
                    ))}
                  </div>

                  {/* Load More Button */}
                  {hasMoreResults && (
                    <div className="flex justify-center mt-8">
                      <button
                        onClick={handleLoadMore}
                        disabled={loadingMore}
                        className="px-8 py-3 rounded-lg bg-[#2D2D30] text-white font-medium hover:bg-[#37373D] transition-all border border-white/10 flex items-center gap-2 disabled:opacity-50"
                      >
                        {loadingMore ? (
                          <>
                            <span className="animate-spin">⟳</span> Loading...
                          </>
                        ) : (
                          <>
                            <span>↓</span> Load More Results
                          </>
                        )}
                      </button>
                    </div>
                  )}
                </div>
              )}

              {/* Skeleton Loading State for Video Formats */}
              {loadingFormats && (
                <div className="mb-10">
                  <FormatSkeletonGrid count={10} />
                </div>
              )}

              {/* Video Metadata Card (single videos only) */}
              {!loadingFormats && videoMetadata && !videoMetadata.is_playlist && (
                <div className="mb-10">
                  <VideoMetadataCard
                    metadata={videoMetadata}
                    formats={formats}
                    onDownload={(id) => handleDownload(id, "best")}
                  />
                </div>
              )}

              {/* Playlist Metadata Card (playlists only) */}
              {playlistMetadata && (
                <div className="mb-10">
                  <PlaylistMetadataCard
                    metadata={playlistMetadata}
                    onDownloadSelected={handlePlaylistDownload}
                  />
                </div>
              )}

            </motion.div>
          )}

          {activeTab === "downloads" && (
            <Downloads
              onBack={() => setActiveTab("home")}
              activeDownloads={activeDownloads}
              onPause={handlePause}
              onResume={handleResume}
              config={config}
              onCheckConfig={handleSetDirectory} // Allow changing it later
            />
          )}

          {activeTab === "settings" && <Settings />}

          {activeTab === "about" && <AboutUs />}

        </AnimatePresence>
        {/* First Run Setup Overlay */}
        <AnimatePresence>
          {showSetup && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="absolute inset-0 z-50 bg-black/80 backdrop-blur-md flex items-center justify-center p-8"
            >
              <div className="bg-[#1E1E20] border-2 border-neo-mint p-8 rounded-2xl shadow-[0_0_50px_rgba(0,255,163,0.2)] max-w-md w-full text-center">
                <h2 className="text-3xl font-black text-white mb-4 tracking-tighter">
                  <span className="text-neo-mint">INITIALIZE</span> SYSTEM
                </h2>
                <p className="text-gray-400 mb-8">
                  Please select a storage location for your downloads to begin using Alpha Tube.
                </p>

                <button
                  onClick={handleSetDirectory}
                  className="w-full py-4 bg-neo-mint text-black font-bold rounded-lg hover:brightness-110 active:scale-95 transition-all text-lg shadow-lg"
                >
                  [ SELECT STORAGE ]
                </button>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </main>
      <Toast
        message={toastMessage}
        isVisible={showToast}
        onClose={() => setShowToast(false)}
      />
      <UpdateNotification
        isVisible={updateReady}
        updateInfo={updateInfo}
        onRestart={handleRestartForUpdate}
        onDismiss={() => setUpdateReady(false)}
      />

      {/* Action Dialog - Play or Download */}
      {actionDialogVisible && pendingVideo && (
        <ActionDialog
          videoUrl={pendingVideo.url}
          videoTitle={pendingVideo.title}
          thumbnailUrl={pendingVideo.thumbnail}
          onPlay={handlePlayChoice}
          onDownload={handleDownloadChoice}
          onClose={() => setActionDialogVisible(false)}
        />
      )}

      {/* Stream Player */}
      {streamPlayerVisible && pendingVideo && (
        <StreamPlayer
          videoUrl={pendingVideo.url}
          videoTitle={pendingVideo.title}
          onClose={() => setStreamPlayerVisible(false)}
          onDownload={handleStreamToDownload}
        />
      )}
    </div >
  );
}

export default App;
