import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { RefreshCw, X, Sparkles } from 'lucide-react';

interface UpdateInfo {
    version: string;
    body?: string;
}

interface UpdateNotificationProps {
    isVisible: boolean;
    updateInfo: UpdateInfo | null;
    onRestart: () => void;
    onDismiss: () => void;
}

const UpdateNotification: React.FC<UpdateNotificationProps> = ({
    isVisible,
    updateInfo,
    onRestart,
    onDismiss
}) => {
    return (
        <AnimatePresence>
            {isVisible && updateInfo && (
                <motion.div
                    initial={{ opacity: 0, y: -100, scale: 0.8 }}
                    animate={{ opacity: 1, y: 0, scale: 1 }}
                    exit={{ opacity: 0, y: -50, scale: 0.8 }}
                    transition={{ type: "spring", damping: 20, stiffness: 300 }}
                    className="fixed top-6 left-1/2 transform -translate-x-1/2 z-[9999]"
                >
                    <div className="bg-gradient-to-r from-emerald-500/95 to-teal-500/95 backdrop-blur-xl text-white px-6 py-4 rounded-2xl shadow-[0_8px_32px_rgba(16,185,129,0.4)] border border-white/20 flex items-center gap-4 min-w-[380px] max-w-[500px]">
                        {/* Icon */}
                        <div className="relative">
                            <div className="w-12 h-12 bg-white/20 rounded-xl flex items-center justify-center">
                                <Sparkles size={24} className="text-white" />
                            </div>
                            <motion.div
                                animate={{ scale: [1, 1.2, 1] }}
                                transition={{ repeat: Infinity, duration: 2 }}
                                className="absolute -top-1 -right-1 w-4 h-4 bg-yellow-400 rounded-full border-2 border-white"
                            />
                        </div>

                        {/* Content */}
                        <div className="flex-1 min-w-0">
                            <h4 className="font-bold text-lg leading-tight">
                                Update Ready! 🎉
                            </h4>
                            <p className="text-white/80 text-sm mt-0.5">
                                Version {updateInfo.version} downloaded
                            </p>
                        </div>

                        {/* Actions */}
                        <div className="flex items-center gap-2">
                            <motion.button
                                whileHover={{ scale: 1.05 }}
                                whileTap={{ scale: 0.95 }}
                                onClick={onRestart}
                                className="flex items-center gap-2 bg-white text-emerald-600 font-bold px-4 py-2 rounded-xl hover:bg-white/90 transition-colors shadow-lg"
                            >
                                <RefreshCw size={16} />
                                Restart
                            </motion.button>
                            <button
                                onClick={onDismiss}
                                className="p-2 rounded-xl hover:bg-white/20 transition-colors"
                                title="Dismiss (update on next restart)"
                            >
                                <X size={18} />
                            </button>
                        </div>
                    </div>
                </motion.div>
            )}
        </AnimatePresence>
    );
};

export default UpdateNotification;
