import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { Download, ExternalLink, X, AlertTriangle } from "lucide-react";

interface YouTubeBrowserProps {
    sidebarWidth: number;
    onVideoDetected: (url: string) => void;
}

export default function YouTubeBrowser({ onVideoDetected }: YouTubeBrowserProps) {
    const [isBrowserOpen, setIsBrowserOpen] = useState(false);
    const [browserUrl, setBrowserUrl] = useState("https://www.youtube.com");
    const [manualUrl, setManualUrl] = useState("");

    // Open the YouTube browser window
    const openBrowser = async () => {
        try {
            await invoke("open_youtube_window", { url: browserUrl });
            setIsBrowserOpen(true);
        } catch (error) {
            console.error("Failed to open YouTube browser:", error);
        }
    };

    // Close the YouTube browser window
    const closeBrowser = async () => {
        try {
            await invoke("close_youtube_webview");
            setIsBrowserOpen(false);
        } catch (error) {
            console.error("Failed to close YouTube browser:", error);
        }
    };

    // Close browser when leaving the tab
    useEffect(() => {
        return () => {
            closeBrowser();
        };
    }, []);

    const handleManualSubmit = () => {
        if (manualUrl.trim()) {
            onVideoDetected(manualUrl);
        }
    };

    return (
        <motion.div
            key="browse"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            className="flex-1 flex flex-col p-10 overflow-y-auto"
        >
            {/* Header */}
            <div className="mb-10 border-b border-white/10 pb-6">
                <h1 className="text-5xl text-white mb-3 font-black tracking-tight drop-shadow-sm">
                    <span className="text-red-500">{'>'}</span> BROWSE_YOUTUBE
                </h1>
                <p className="text-text-muted font-medium text-lg ml-2">
                    Browse YouTube in a separate window.
                </p>
            </div>

            {/* Status Card */}
            <div className="glass-card rounded-2xl p-8 mb-8 border border-white/10">
                <div className="flex flex-col gap-6">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-4">
                            <div className={`w-4 h-4 rounded-full ${isBrowserOpen ? 'bg-neo-mint animate-pulse' : 'bg-gray-500'}`} />
                            <span className="text-white font-bold text-lg">
                                {isBrowserOpen ? 'YouTube Browser Active' : 'Browser Closed'}
                            </span>
                        </div>

                        {!isBrowserOpen && (
                            <div className="flex items-center gap-2 flex-1 max-w-md mx-4">
                                <span className="text-gray-400 text-sm whitespace-nowrap">Start URL:</span>
                                <input
                                    type="text"
                                    value={browserUrl}
                                    onChange={(e) => setBrowserUrl(e.target.value)}
                                    className="flex-1 bg-black/20 border border-white/10 rounded px-3 py-1 text-white text-sm"
                                />
                            </div>
                        )}

                        <div className="flex gap-3">
                            {!isBrowserOpen ? (
                                <button
                                    onClick={openBrowser}
                                    className="flex items-center gap-2 px-6 py-3 rounded-xl bg-red-500 text-white font-bold hover:bg-red-600 transition-all"
                                >
                                    <ExternalLink size={18} />
                                    Open Browser
                                </button>
                            ) : (
                                <button
                                    onClick={closeBrowser}
                                    className="flex items-center gap-2 px-6 py-3 rounded-xl bg-white/10 text-text-muted font-bold hover:bg-white/20 transition-all"
                                >
                                    <X size={18} />
                                    Close Browser
                                </button>
                            )}
                        </div>
                    </div>

                    {/* Disclaimer / Instructions */}
                    <div className="bg-black/20 rounded-xl p-6 border border-white/5 relative overflow-hidden">
                        <div className="absolute top-0 left-0 w-1 h-full bg-yellow-500/50"></div>
                        <h3 className="text-yellow-500 font-bold mb-3 flex items-center gap-2">
                            <AlertTriangle size={18} />
                            IMPORTANT DISCLAIMER
                        </h3>
                        <ol className="text-text-muted space-y-3 list-decimal list-inside font-mono text-sm">
                            <li>
                                <span className="text-white">Copy the link of desired video</span> and paste it below or in the main app input.
                            </li>
                            <li>
                                If the <span className="text-cyber-pink">Ad-Block doesn't work</span>, close and start the browser again.
                            </li>
                            <li className="text-gray-500 italic">
                                YouTube by default is just for testing. Users are legally liable for their own actions.
                            </li>
                        </ol>
                    </div>
                </div>
            </div>

            {/* Manual Paste Section */}
            <div className="bg-card-dark rounded-2xl p-8 border border-white/10 shadow-lg">
                <h3 className="text-white font-bold text-xl mb-4 flex items-center gap-2">
                    <Download size={24} className="text-neo-mint" />
                    PASTE LINK TO DOWNLOAD
                </h3>
                <div className="flex gap-4">
                    <input
                        type="text"
                        value={manualUrl}
                        onChange={(e) => setManualUrl(e.target.value)}
                        placeholder="Paste YouTube Link here..."
                        className="flex-1 p-4 rounded-lg bg-black/30 border border-white/10 text-white placeholder-gray-500 focus:outline-none focus:border-neo-mint transition-all"
                    />
                    <button
                        onClick={handleManualSubmit}
                        disabled={!manualUrl.trim()}
                        className="px-8 py-4 rounded-lg bg-neo-mint text-black font-black hover:brightness-110 disabled:opacity-50 disabled:cursor-not-allowed transition-all shadow-[0_0_15px_rgba(0,255,163,0.3)]"
                    >
                        PROCESS
                    </button>
                </div>
            </div>
        </motion.div>
    );
}
