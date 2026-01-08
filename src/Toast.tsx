import React, { useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X } from 'lucide-react';

interface ToastProps {
    message: string;
    isVisible: boolean;
    onClose: () => void;
    duration?: number;
}

const Toast: React.FC<ToastProps> = ({ message, isVisible, onClose, duration = 3000 }) => {
    useEffect(() => {
        if (isVisible) {
            const timer = setTimeout(() => {
                onClose();
            }, duration);
            return () => clearTimeout(timer);
        }
    }, [isVisible, duration, onClose]);

    return (
        <AnimatePresence>
            {isVisible && (
                <motion.div
                    initial={{ opacity: 0, y: 50, scale: 0.8 }}
                    animate={{ opacity: 1, y: 0, scale: 1 }}
                    exit={{ opacity: 0, y: 20, scale: 0.8 }}
                    className="fixed bottom-8 left-1/2 transform -translate-x-1/2 z-50 pointer-events-none"
                >
                    <div className="bg-neo-mint/90 backdrop-blur-md text-black pl-6 pr-4 py-4 rounded-2xl shadow-[0_0_20px_rgba(0,229,204,0.4)] border-2 border-white flex items-center gap-4 min-w-[320px] relative overflow-hidden pointer-events-auto">
                        <span className="text-2xl animate-bounce">✨</span>
                        <div className="flex flex-col flex-1">
                            <span className="font-black text-xs tracking-wider uppercase text-black/60 mb-0.5">Notification</span>
                            <span className="font-bold text-lg leading-tight">{message}</span>
                        </div>
                        <button
                            onClick={onClose}
                            className="p-1 rounded-full hover:bg-black/10 transition-colors"
                        >
                            <X size={20} />
                        </button>

                        {/* Progress Bar */}
                        <motion.div
                            initial={{ width: "100%" }}
                            animate={{ width: "0%" }}
                            transition={{ duration: duration / 1000, ease: "linear" }}
                            className="absolute bottom-0 left-0 h-1 bg-black/20"
                        />
                    </div>
                </motion.div>
            )}
        </AnimatePresence>
    );
};

export default Toast;
