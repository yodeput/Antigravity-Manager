import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useConfigStore } from '../stores/useConfigStore';
import { Bot, Play, Square, Trash2, Settings, Eye, EyeOff } from 'lucide-react';

interface DiscordBotStatus {
    running: boolean;
    enabled: boolean;
}

interface DiscordLogEntry {
    timestamp: string;
    level: string;
    message: string;
}

export default function DiscordBot() {
    const { t } = useTranslation();
    const { config, loadConfig, saveConfig } = useConfigStore();
    const [status, setStatus] = useState<DiscordBotStatus>({ running: false, enabled: false });
    const [logs, setLogs] = useState<DiscordLogEntry[]>([]);
    const [loading, setLoading] = useState(false);
    const [showConfig, setShowConfig] = useState(false);
    const [token, setToken] = useState('');
    const [showToken, setShowToken] = useState(false);
    const logContainerRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        loadConfig();
        fetchStatus();
        fetchLogs();
        
        // Listen for real-time log updates
        const unlisten = listen<DiscordLogEntry>('discord-log', (event) => {
            setLogs(prev => [...prev, event.payload]);
        });

        return () => {
            unlisten.then(fn => fn());
        };
    }, []);

    useEffect(() => {
        if (config?.discord_bot) {
            setToken(config.discord_bot.bot_token);
        }
    }, [config]);

    // Auto-scroll to bottom when new logs arrive
    useEffect(() => {
        if (logContainerRef.current) {
            logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
        }
    }, [logs]);

    const fetchStatus = async () => {
        try {
            const s = await invoke<DiscordBotStatus>('get_discord_bot_status');
            setStatus(s);
        } catch (e) {
            console.error('Failed to get status:', e);
        }
    };

    const fetchLogs = async () => {
        try {
            const entries = await invoke<DiscordLogEntry[]>('get_discord_logs');
            setLogs(entries);
        } catch (e) {
            console.error('Failed to get logs:', e);
        }
    };

    const handleSaveConfig = async () => {
        if (!config) return;
        try {
            const newConfig = {
                ...config,
                discord_bot: {
                    enabled: true,
                    bot_token: token
                }
            };
            await saveConfig(newConfig);
            setShowConfig(false);
        } catch (e) {
            console.error(e);
            alert('Error: ' + e);
        }
    };

    const toggleBot = async () => {
        setLoading(true);
        try {
            if (status.running) {
                await invoke('stop_discord_bot');
            } else {
                if (!token) {
                    setShowConfig(true);
                    setLoading(false);
                    return;
                }
                await invoke('start_discord_bot', { config: { enabled: true, bot_token: token } });
            }
            await fetchStatus();
        } catch (e) {
            alert('Error: ' + e);
        } finally {
            setLoading(false);
        }
    };

    const clearLogs = async () => {
        try {
            await invoke('clear_discord_logs');
            setLogs([]);
        } catch (e) {
            console.error(e);
        }
    };

    const getLogColor = (level: string) => {
        switch (level) {
            case 'error': return 'text-red-400';
            case 'warn': return 'text-yellow-400';
            case 'success': return 'text-green-400';
            default: return 'text-gray-300';
        }
    };

    return (
        <div className="h-full flex flex-col p-6 max-w-6xl mx-auto">
            {/* Header */}
            <div className="flex items-center justify-between mb-6">
                <div className="flex items-center gap-4">
                    <div className="p-3 bg-gradient-to-br from-indigo-500 to-purple-600 rounded-xl shadow-lg">
                        <Bot className="w-8 h-8 text-white" />
                    </div>
                    <div>
                        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
                            Discord Bot
                        </h1>
                        <p className="text-sm text-gray-500 dark:text-gray-400">
                            AI-powered chat assistant for your Discord server
                        </p>
                    </div>
                </div>
                
                <div className="flex items-center gap-3">
                    {/* Status Badge */}
                    <div className={`px-4 py-2 rounded-full text-sm font-medium flex items-center gap-2 ${
                        status.running 
                            ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400 border border-green-200 dark:border-green-800' 
                            : 'bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400 border border-gray-200 dark:border-gray-700'
                    }`}>
                        <div className={`w-2.5 h-2.5 rounded-full ${status.running ? 'bg-green-500 animate-pulse' : 'bg-gray-400'}`} />
                        {status.running ? 'Online' : 'Offline'}
                    </div>

                    {/* Config Button */}
                    <button 
                        className="btn btn-ghost btn-circle"
                        onClick={() => setShowConfig(!showConfig)}
                        title="Configure"
                    >
                        <Settings size={20} />
                    </button>

                    {/* Main Action Button */}
                    <button 
                        className={`btn gap-2 min-w-[140px] ${
                            status.running 
                                ? 'btn-error hover:bg-red-600' 
                                : 'bg-gradient-to-r from-indigo-500 to-purple-600 text-white border-0 hover:from-indigo-600 hover:to-purple-700'
                        }`}
                        onClick={toggleBot}
                        disabled={loading}
                    >
                        {loading ? (
                            <span className="loading loading-spinner loading-sm"></span>
                        ) : status.running ? (
                            <>
                                <Square size={18} fill="currentColor" />
                                Stop Bot
                            </>
                        ) : (
                            <>
                                <Play size={18} fill="currentColor" />
                                Start Bot
                            </>
                        )}
                    </button>
                </div>
            </div>

            {/* Config Panel (Collapsible) */}
            {showConfig && (
                <div className="mb-6 bg-white dark:bg-base-200 rounded-xl border border-gray-200 dark:border-base-300 p-4 shadow-sm animate-in slide-in-from-top-2">
                    <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Bot Configuration</h3>
                    <div className="flex gap-3">
                        <div className="flex-1 relative">
                            <input
                                type={showToken ? "text" : "password"}
                                value={token}
                                onChange={(e) => setToken(e.target.value)}
                                placeholder="Enter Discord Bot Token..."
                                className="input input-bordered w-full pr-10"
                            />
                            <button 
                                className="absolute right-3 top-3 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
                                onClick={() => setShowToken(!showToken)}
                            >
                                {showToken ? <EyeOff size={18} /> : <Eye size={18} />}
                            </button>
                        </div>
                        <button 
                            className="btn btn-primary"
                            onClick={handleSaveConfig}
                        >
                            Save
                        </button>
                    </div>
                    <p className="text-xs text-gray-500 mt-2">
                        Get your bot token from the <a href="https://discord.com/developers/applications" target="_blank" className="text-indigo-500 hover:underline">Discord Developer Portal</a>
                    </p>
                </div>
            )}

            {/* Console */}
            <div className="flex-1 bg-[#1e1e2e] rounded-xl overflow-hidden flex flex-col shadow-xl border border-gray-800">
                {/* Console Header */}
                <div className="flex items-center justify-between px-4 py-3 bg-[#181825] border-b border-gray-800">
                    <div className="flex items-center gap-3">
                        <div className="flex gap-1.5">
                            <div className="w-3 h-3 rounded-full bg-red-500"></div>
                            <div className="w-3 h-3 rounded-full bg-yellow-500"></div>
                            <div className="w-3 h-3 rounded-full bg-green-500"></div>
                        </div>
                        <span className="text-sm font-medium text-gray-400">Console</span>
                    </div>
                    <button 
                        className="text-gray-500 hover:text-gray-300 transition-colors"
                        onClick={clearLogs}
                        title="Clear logs"
                    >
                        <Trash2 size={16} />
                    </button>
                </div>

                {/* Console Content */}
                <div 
                    ref={logContainerRef}
                    className="flex-1 overflow-auto p-4 font-mono text-sm"
                    style={{ minHeight: '400px' }}
                >
                    {logs.length === 0 ? (
                        <div className="h-full flex flex-col items-center justify-center text-gray-500">
                            <Bot size={48} className="mb-4 opacity-30" />
                            <p className="text-center">
                                No logs yet.<br />
                                <span className="text-xs">Start the bot to see activity here.</span>
                            </p>
                        </div>
                    ) : (
                        logs.map((entry, i) => (
                            <div 
                                key={i} 
                                className={`py-0.5 ${getLogColor(entry.level)} hover:bg-white/5 px-2 -mx-2 rounded`}
                            >
                                <span className="text-gray-500 mr-3">[{entry.timestamp}]</span>
                                <span>{entry.message}</span>
                            </div>
                        ))
                    )}
                </div>
            </div>
        </div>
    );
}
