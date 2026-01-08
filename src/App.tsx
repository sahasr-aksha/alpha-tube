import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import VideoMetadataCard, { VideoMetadataResponse, VideoFormat } from "./VideoMetadataCard";
import { listen } from "@tauri-apps/api/event";
import { downloadDir } from "@tauri-apps/api/path";
import { motion, AnimatePresence } from "framer-motion";
import { sendNotification } from "@tauri-apps/plugin-notification";
import { open } from "@tauri-apps/plugin-dialog";
import Downloads from "./Downloads";
import AboutUs from "./AboutUs";
import YouTubeBrowser from "./YouTubeBrowser";
import { Terminal, Library, Globe, Info, Menu, ChevronLeft } from "lucide-react";
import "./App.css";
import Toast from "./Toast";
import UpdateNotification from "./UpdateNotification";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

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
}

export interface AppConfig {
  download_dir: string | null;
}

function App() {
  const [url, setUrl] = useState("");
  // Replaced single downloading boolean with derived state from activeDownloads keys
  const [activeDownloads, setActiveDownloads] = useState<Record<string, DownloadProgress>>({});
  const [statusMessage, setStatusMessage] = useState("");
  const [toastMessage, setToastMessage] = useState("");
  const [showToast, setShowToast] = useState(false);
  const [activeTab, setActiveTab] = useState("home"); // home, downloads, browse, about
  const [formats, setFormats] = useState<VideoFormat[]>([]);
  const [videoMetadata, setVideoMetadata] = useState<VideoMetadataResponse | null>(null);
  const [loadingFormats, setLoadingFormats] = useState(false);

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

        // Cleanup finished download from list after 5s
        setTimeout(() => {
          setActiveDownloads(prev => {
            const copy = { ...prev };
            delete copy[id];
            return copy;
          });
        }, 5000);

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
        await relaunch();
      }
    } catch (error) {
      console.error("[Update] Failed to install:", error);
      setStatusMessage(`Update failed: ${error}`);
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

  const handleFetchFormats = async () => {
    if (!url) {
      setStatusMessage("Please enter a URL first.");
      return;
    }

    setLoadingFormats(true);
    setStatusMessage("Fetching video formats...");
    setFormats([]);

    try {
      const result = await invoke<VideoMetadataResponse>("get_video_metadata", { url });

      setVideoMetadata(result);

      // Sort: 4K/High res first
      if (!result.is_playlist) {
        const sorted = result.formats.sort((a, b) => {
          // Simple heuristic: higher filesize usually means better quality
          return b.filesize - a.filesize;
        });
        setFormats(sorted);
        setStatusMessage(`Found ${result.formats.length} formats for "${result.title}"`);
      } else {
        setFormats([]);
        setStatusMessage(`Playlist found: "${result.title}" (${result.video_count} videos)`);
      }
    } catch (error) {
      console.error(error);
      setStatusMessage(`Failed to fetch formats: ${error}`);
    } finally {
      setLoadingFormats(false);
    }
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

  // Handle video detected from YouTubeBrowser - switch to home tab with URL
  const handleVideoDetected = async (detectedUrl: string) => {
    setUrl(detectedUrl);
    setActiveTab("home");

    // Auto-fetch metadata
    setLoadingFormats(true);
    setStatusMessage("Video Detected: Fetching metadata...");

    try {
      const result = await invoke<VideoMetadataResponse>("get_video_metadata", { url: detectedUrl });
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
            title="Terminal"
          >
            <Terminal size={18} />
            {!sidebarCollapsed && <span>Terminal</span>}
          </button>

          <button
            onClick={() => setActiveTab("downloads")}
            className={`p-2 rounded-md transition-all duration-150 text-sm font-medium flex items-center gap-3 ${activeTab === "downloads"
              ? "bg-[#37373D] text-white shadow-sm border-l-2 border-neo-mint"
              : "text-gray-400 hover:bg-[#2D2D30] hover:text-white"
              }`}
            title="Library"
          >
            <Library size={18} />
            {!sidebarCollapsed && <span>Library</span>}
          </button>

          <button
            onClick={() => setActiveTab("browse")}
            className={`p-2 rounded-md transition-all duration-150 text-sm font-medium flex items-center gap-3 ${activeTab === "browse"
              ? "bg-[#37373D] text-white shadow-sm border-l-2 border-red-500"
              : "text-gray-400 hover:bg-[#2D2D30] hover:text-white"
              }`}
            title="Browse YouTube"
          >
            <Globe size={18} />
            {!sidebarCollapsed && <span>Browse</span>}
          </button>

          <button
            onClick={() => setActiveTab("about")}
            className={`p-2 rounded-md transition-all duration-150 text-sm font-medium flex items-center gap-3 ${activeTab === "about"
              ? "bg-[#37373D] text-white shadow-sm border-l-2 border-electric-lavender"
              : "text-gray-400 hover:bg-[#2D2D30] hover:text-white"
              }`}
            title="About"
          >
            <Info size={18} />
            {!sidebarCollapsed && <span>About</span>}
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
              <div className="mb-12">
                <h1 className="text-4xl text-white mb-2 font-bold tracking-tight drop-shadow-sm flex items-center gap-3">
                  <div className="w-1 h-8 bg-neo-mint rounded-full"></div>
                  INPUT TARGET
                </h1>
                <p className="text-gray-400 text-base ml-4">Enter a YouTube URL to extract metadata and formats.</p>
              </div>

              {/* Input Section - OPAQUE/SOLID */}
              <div className="flex gap-4 mb-10">
                <input
                  type="text"
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                  placeholder="Paste URL here..."
                  className="flex-1 p-4 rounded-lg input-solid text-white placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-neo-mint shadow-md text-base transition-all"
                />
                <button
                  onClick={handleFetchFormats}
                  disabled={loadingFormats}
                  className="px-8 py-4 rounded-lg bg-neo-mint text-black font-bold tracking-wide shadow-md hover:brightness-110 active:scale-95 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {loadingFormats ? "SCANNING..." : "SCAN"}
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

              {/* Video Metadata Card */}
              {videoMetadata && (
                <div className="mb-10">
                  <VideoMetadataCard
                    metadata={videoMetadata}
                    formats={formats}
                    onDownload={(id) => handleDownload(id, "best")}
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

          {activeTab === "browse" && (
            <YouTubeBrowser
              sidebarWidth={sidebarWidth}
              onVideoDetected={handleVideoDetected}
            />
          )}

          {activeTab === "about" && (
            <AboutUs />
          )}

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
    </div >
  );
}

export default App;
