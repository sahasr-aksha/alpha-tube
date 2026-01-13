import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { motion } from "framer-motion";
import {
    Settings as SettingsIcon, Download, RefreshCw, CheckCircle,
    AlertCircle, Rocket, ArrowDownCircle
} from "lucide-react";

interface AppUpdateInfo {
    version: string;
    notes: string;
    download_url: string;
    current_version: string;
    update_available: boolean;
}

interface AppUpdateProgress {
    percent: number;
    downloaded_bytes: number;
    total_bytes: number;
    status: string;
}

export default function Settings() {
    // yt-dlp update state
    const [updating, setUpdating] = useState(false);
    const [status, setStatus] = useState<{ type: "success" | "error" | "info"; message: string } | null>(null);

    // App update state
    const [appUpdateStatus, setAppUpdateStatus] = useState<
        "idle" | "checking" | "available" | "downloading" | "ready" | "current" | "error"
    >("idle");
    const [updateInfo, setUpdateInfo] = useState<AppUpdateInfo | null>(null);
    const [downloadProgress, setDownloadProgress] = useState(0);
    const [installerPath, setInstallerPath] = useState<string | null>(null);
    const [appUpdateError, setAppUpdateError] = useState<string | null>(null);

    // Listen for download progress
    useEffect(() => {
        const unlisten = listen<AppUpdateProgress>("app-update-progress", (event) => {
            setDownloadProgress(event.payload.percent);
            if (event.payload.status === "complete") {
                setAppUpdateStatus("ready");
            }
        });
        return () => { unlisten.then(f => f()); };
    }, []);

    const handleCheckAppUpdate = async () => {
        setAppUpdateStatus("checking");
        setAppUpdateError(null);
        try {
            const info = await invoke<AppUpdateInfo>("check_app_update");
            setUpdateInfo(info);
            setAppUpdateStatus(info.update_available ? "available" : "current");
        } catch (error) {
            setAppUpdateError(String(error));
            setAppUpdateStatus("error");
        }
    };

    const handleDownloadAppUpdate = async () => {
        if (!updateInfo?.download_url) return;
        setAppUpdateStatus("downloading");
        setDownloadProgress(0);
        try {
            const path = await invoke<string>("download_app_update", {
                downloadUrl: updateInfo.download_url
            });
            setInstallerPath(path);
            setAppUpdateStatus("ready");
        } catch (error) {
            setAppUpdateError(String(error));
            setAppUpdateStatus("error");
        }
    };

    const handleInstallAppUpdate = async () => {
        if (!installerPath) return;
        try {
            await invoke("install_app_update", { installerPath });
        } catch (error) {
            setAppUpdateError(String(error));
            setAppUpdateStatus("error");
        }
    };

    const handleUpdate = async () => {
        if (updating) return;

        setUpdating(true);
        setStatus({ type: "info", message: "Updating download engine..." });

        try {
            const result = await invoke<string>("update_ytdlp");
            console.log("Update result:", result);
            setStatus({ type: "success", message: "Download engine updated successfully!" });
        } catch (error) {
            console.error("Update failed:", error);
            setStatus({ type: "error", message: `Update failed: ${error}` });
        } finally {
            setUpdating(false);
        }
    };

    return (
        <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            transition={{ duration: 0.2 }}
            className="flex-1 flex flex-col p-12 overflow-y-auto"
        >
            <div className="mb-8 flex items-center gap-4">
                <div className="p-3 bg-neo-mint/20 rounded-xl">
                    <SettingsIcon size={32} className="text-neo-mint" />
                </div>
                <div>
                    <h1 className="text-3xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-white to-gray-400">
                        Settings
                    </h1>
                    <p className="text-gray-400">Manage application preferences</p>
                </div>
            </div>

            <div className="grid grid-cols-1 gap-6 max-w-2xl">

                {/* === APP UPDATE SECTION === */}
                <section className="bg-[#1E1E24]/60 p-6 rounded-xl border border-white/5 backdrop-blur-sm">
                    <h2 className="text-xl font-semibold mb-4 flex items-center gap-2">
                        <Rocket size={20} className="text-neo-mint" />
                        App Update
                    </h2>

                    <div className="space-y-4">
                        {/* Idle State */}
                        {appUpdateStatus === "idle" && (
                            <div className="flex items-center justify-between">
                                <div>
                                    <p className="font-medium text-gray-200">Alpha Tube</p>
                                    <p className="text-sm text-gray-500">Check for new versions</p>
                                </div>
                                <button
                                    onClick={handleCheckAppUpdate}
                                    className="px-4 py-2 rounded-lg font-medium text-sm bg-neo-mint hover:bg-neo-mint/80 text-black transition-all flex items-center gap-2"
                                >
                                    <RefreshCw size={16} />
                                    Check for Updates
                                </button>
                            </div>
                        )}

                        {/* Checking State */}
                        {appUpdateStatus === "checking" && (
                            <div className="flex items-center gap-3 text-blue-400">
                                <RefreshCw size={16} className="animate-spin" />
                                Checking for updates...
                            </div>
                        )}

                        {/* Up to Date State */}
                        {appUpdateStatus === "current" && updateInfo && (
                            <div className="flex items-center justify-between">
                                <div className="flex items-center gap-3 text-green-400">
                                    <CheckCircle size={16} />
                                    You're up to date! (v{updateInfo.current_version})
                                </div>
                                <button
                                    onClick={() => setAppUpdateStatus("idle")}
                                    className="text-xs text-gray-500 hover:text-gray-400"
                                >
                                    Check again
                                </button>
                            </div>
                        )}

                        {/* Update Available State */}
                        {appUpdateStatus === "available" && updateInfo && (
                            <div className="space-y-3">
                                <div className="flex items-center justify-between">
                                    <div>
                                        <p className="font-medium text-white">
                                            v{updateInfo.version} available!
                                        </p>
                                        <p className="text-sm text-gray-400">
                                            Current: v{updateInfo.current_version}
                                        </p>
                                        {updateInfo.notes && (
                                            <p className="text-sm text-gray-500 mt-1">{updateInfo.notes}</p>
                                        )}
                                    </div>
                                    <button
                                        onClick={handleDownloadAppUpdate}
                                        className="px-4 py-2 rounded-lg font-medium text-sm bg-electric-lavender hover:bg-electric-lavender/80 text-white transition-all flex items-center gap-2"
                                    >
                                        <ArrowDownCircle size={16} />
                                        Download
                                    </button>
                                </div>
                            </div>
                        )}

                        {/* Downloading State */}
                        {appUpdateStatus === "downloading" && (
                            <div className="space-y-2">
                                <div className="flex items-center justify-between text-sm">
                                    <span className="text-gray-400">Downloading update...</span>
                                    <span className="text-neo-mint font-medium">{downloadProgress.toFixed(0)}%</span>
                                </div>
                                <div className="h-2 bg-gray-700 rounded-full overflow-hidden">
                                    <motion.div
                                        className="h-full bg-gradient-to-r from-neo-mint to-electric-lavender"
                                        initial={{ width: 0 }}
                                        animate={{ width: `${downloadProgress}%` }}
                                        transition={{ duration: 0.2 }}
                                    />
                                </div>
                            </div>
                        )}

                        {/* Ready to Install State */}
                        {appUpdateStatus === "ready" && (
                            <div className="flex items-center justify-between">
                                <div>
                                    <p className="font-medium text-green-400">Update downloaded!</p>
                                    <p className="text-sm text-gray-500">Click Install to apply</p>
                                </div>
                                <button
                                    onClick={handleInstallAppUpdate}
                                    className="px-5 py-2.5 rounded-lg font-bold text-sm bg-green-500 hover:bg-green-400 text-white transition-all flex items-center gap-2 shadow-lg shadow-green-500/30"
                                >
                                    <Download size={16} />
                                    Install Now
                                </button>
                            </div>
                        )}

                        {/* Error State */}
                        {appUpdateStatus === "error" && appUpdateError && (
                            <div className="space-y-2">
                                <div className="p-3 rounded-lg bg-red-500/10 text-red-400 border border-red-500/20 text-sm flex items-center gap-2">
                                    <AlertCircle size={16} />
                                    {appUpdateError}
                                </div>
                                <button
                                    onClick={() => setAppUpdateStatus("idle")}
                                    className="text-xs text-gray-500 hover:text-gray-400"
                                >
                                    Try again
                                </button>
                            </div>
                        )}
                    </div>
                </section>

                {/* === YT-DLP ENGINE SECTION === */}
                <section className="bg-[#1E1E24]/60 p-6 rounded-xl border border-white/5 backdrop-blur-sm">
                    <h2 className="text-xl font-semibold mb-4 flex items-center gap-2">
                        <Download size={20} className="text-electric-lavender" />
                        Download Engine
                    </h2>

                    <div className="flex items-center justify-between">
                        <div>
                            <p className="font-medium text-gray-200">yt-dlp Core</p>
                            <p className="text-sm text-gray-500">The underlying engine used for video downloads</p>
                        </div>

                        <button
                            onClick={handleUpdate}
                            disabled={updating}
                            className={`px-4 py-2 rounded-lg font-medium text-sm transition-all flex items-center gap-2
                ${updating
                                    ? "bg-gray-700 text-gray-400 cursor-not-allowed"
                                    : "bg-electric-lavender hover:bg-electric-lavender/80 text-white shadow-lg shadow-electric-lavender/20"
                                }`}
                        >
                            <RefreshCw size={16} className={updating ? "animate-spin" : ""} />
                            {updating ? "Updating..." : "Update Engine"}
                        </button>
                    </div>

                    {status && (
                        <motion.div
                            initial={{ opacity: 0, height: 0 }}
                            animate={{ opacity: 1, height: "auto" }}
                            className={`mt-4 p-3 rounded-lg text-sm flex items-center gap-2
                ${status.type === "success" ? "bg-green-500/10 text-green-400 border border-green-500/20" : ""}
                ${status.type === "error" ? "bg-red-500/10 text-red-400 border border-red-500/20" : ""}
                ${status.type === "info" ? "bg-blue-500/10 text-blue-400 border border-blue-500/20" : ""}
              `}
                        >
                            {status.type === "success" && <CheckCircle size={16} />}
                            {status.type === "error" && <AlertCircle size={16} />}
                            {status.type === "info" && <RefreshCw size={16} className="animate-spin" />}
                            {status.message}
                        </motion.div>
                    )}
                </section>

                {/* Placeholder for future settings */}
                <section className="bg-[#1E1E24]/60 p-6 rounded-xl border border-white/5 backdrop-blur-sm opacity-50 cursor-not-allowed">
                    <h2 className="text-xl font-semibold mb-4 text-gray-400">General (Coming Soon)</h2>
                    <p className="text-gray-500 text-sm">More configuration options will be available here.</p>
                </section>
            </div>
        </motion.div>
    );
}
