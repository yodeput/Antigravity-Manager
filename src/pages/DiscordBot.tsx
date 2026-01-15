import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useConfigStore } from '../stores/useConfigStore';
import { Bot, Play, Square, Trash2, Settings, Eye, EyeOff, Terminal, X, Server, MessageSquare, Radio, ChevronDown, ChevronRight } from 'lucide-react';

interface DiscordBotStatus {
    running: boolean;
    enabled: boolean;
}

interface DiscordLogEntry {
    timestamp: string;
    level: string;
    message: string;
}

interface ChannelStats {
    channel_id: string;
    guild_id: string;
    is_listening: boolean;
    shared_chat: boolean;
    listen_udin: boolean;
    message_count: number;
}

interface GuildStats {
    guild_id: string;
    chat_model: string;
    system_prompt_preview: string;
    channels: ChannelStats[];
    total_messages: number;
}

interface DiscordStats {
    guilds: GuildStats[];
    total_channels: number;
    total_messages: number;
}

interface MessageEntry {
    role: string;
    author_name: string | null;
    content: string;
}

export default function DiscordBot() {
    const { t } = useTranslation();
    const { config, loadConfig, saveConfig } = useConfigStore();
    const [status, setStatus] = useState<DiscordBotStatus>({ running: false, enabled: false });
    const [logs, setLogs] = useState<DiscordLogEntry[]>([]);
    const [stats, setStats] = useState<DiscordStats | null>(null);
    const [loading, setLoading] = useState(false);
    const [showConfig, setShowConfig] = useState(false);
    const [showConsole, setShowConsole] = useState(false);
    const [showMessages, setShowMessages] = useState<{channelId: string; messages: MessageEntry[]} | null>(null);
    const [token, setToken] = useState('');
    const [showToken, setShowToken] = useState(false);
    const [collapsedGuilds, setCollapsedGuilds] = useState<Set<string>>(new Set());
    const logContainerRef = useRef<HTMLDivElement>(null);

    const toggleGuild = (guildId: string) => {
        const newCollapsed = new Set(collapsedGuilds);
        if (newCollapsed.has(guildId)) {
            newCollapsed.delete(guildId);
        } else {
            newCollapsed.add(guildId);
        }
        setCollapsedGuilds(newCollapsed);
    };

    useEffect(() => {
        loadConfig();
        fetchStatus();
        fetchLogs();
        fetchStats();
        
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

    const fetchStats = async () => {
        try {
            const s = await invoke<DiscordStats>('get_discord_stats');
            setStats(s);
        } catch (e) {
            console.error('Failed to get stats:', e);
        }
    };

    const openMessageHistory = async (channelId: string) => {
        try {
            const messages = await invoke<MessageEntry[]>('get_channel_messages', { channelId, limit: 100 });
            setShowMessages({ channelId, messages });
        } catch (e) {
            console.error('Failed to get messages:', e);
        }
    };

    const clearChannelMessages = async (channelId: string) => {
        if (!confirm('Clear all messages for this channel?')) return;
        try {
            await invoke('clear_channel_messages', { channelId });
            setShowMessages(null);
            await fetchStats();
        } catch (e) {
            console.error('Failed to clear messages:', e);
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
            await fetchStats();
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

    const formatChannelId = (id: string) => {
        if (id.length > 8) {
            return `...${id.slice(-6)}`;
        }
        return id;
    };

    return (
        <div className="h-full w-full overflow-y-auto">
            <div className="p-5 space-y-4 max-w-7xl mx-auto">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                    <div className="p-3 bg-gradient-to-br from-indigo-500 to-purple-600 rounded-xl shadow-lg">
                        <Bot className="w-8 h-8 text-white" />
                    </div>
                    <div>
                    <h1 className="text-2xl font-bold text-gray-900 dark:text-base-content">
                            Discord Bot
                        </h1>
                        <p className="text-sm text-gray-500 dark:text-gray-400">
                            AI-powered chat assistant for your Discord server
                        </p>
                    </div>
                </div>
                
                <div className="flex items-center gap-2">
                    {/* Status Badge */}
                    <div className={`px-3 py-1 rounded-full text-xs font-medium flex items-center gap-2 ${
                        status.running 
                            ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400 border border-green-200 dark:border-green-800' 
                            : 'bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400 border border-gray-200 dark:border-gray-700'
                    }`}>
                        <div className={`w-2.5 h-2.5 rounded-full ${status.running ? 'bg-green-500 animate-pulse' : 'bg-gray-400'}`} />
                        {status.running ? 'Online' : 'Offline'}
                    </div>

                    {/* Console Button */}
                    <button 
                        className="px-3 py-1 rounded-lg text-xs font-medium transition-colors flex items-center gap-2 border bg-white text-gray-600 border-gray-200 hover:bg-gray-50 hover:text-blue-600 relative"
                        onClick={() => setShowConsole(true)}
                        title="Open Console"
                    >
                        <Terminal size={14} />
                        
                        {/* {logs.length > 0 && (
                            <span className="ml-1 px-1 min-w-[1.2em] h-[1.2em] bg-red-500 text-white text-[10px] rounded-full flex items-center justify-center">
                                {logs.length > 99 ? '99+' : logs.length}
                            </span>
                        )} */}
                    </button>

                    {/* Config Button */}
                    {/* Config Button */}
                    <button 
                         className="px-3 py-1 rounded-lg text-xs font-medium transition-colors flex items-center gap-2 border bg-white text-gray-600 border-gray-200 hover:bg-gray-50 hover:text-blue-600"
                        onClick={() => setShowConfig(!showConfig)}
                        title="Configure"
                    >
                        <Settings size={14} />
                        
                    </button>

                    {/* Main Action Button */}
                    <button 
                        className={`px-3 py-1 rounded-lg text-xs font-medium transition-colors flex items-center gap-2 ${status.running
                            ? 'bg-red-50 text-red-600 hover:bg-red-100 border border-red-200'
                            : 'bg-blue-600 hover:bg-blue-700 text-white shadow-sm shadow-blue-500/30'
                            } ${loading ? 'opacity-50 cursor-not-allowed' : ''}`}
                        onClick={toggleBot}
                        disabled={loading}
                    >
                        {loading ? (
                            <span className="loading loading-spinner loading-xs"></span>
                        ) : status.running ? (
                            <>
                                <Square size={14} fill="currentColor" />
                                Stop Bot
                            </>
                        ) : (
                            <>
                                <Play size={14} fill="currentColor" />
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
                    <div className="space-y-4">
                        <div className="relative">
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
                        
                        <div className="flex items-center justify-between">
                            <p className="text-xs text-gray-500">
                                Get your bot token from the <a href="https://discord.com/developers/applications" target="_blank" className="text-indigo-500 hover:underline">Discord Developer Portal</a>
                            </p>
                            <button 
                                className="px-4 py-2 rounded-lg text-sm font-medium transition-colors bg-blue-600 hover:bg-blue-700 text-white shadow-sm shadow-blue-500/30"
                                onClick={handleSaveConfig}
                            >
                                Save
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Stats Section */}
            <div>
                {/* Quick Stats */}
                <div className="grid grid-cols-3 gap-3 mb-6">
                    <div className="bg-white dark:bg-base-100 rounded-xl p-4 shadow-sm border border-gray-100 dark:border-base-200">
                        <div className="flex items-center gap-3">
                            <div className="p-2 bg-indigo-100 dark:bg-indigo-900/30 rounded-lg">
                                <Server size={20} className="text-indigo-600 dark:text-indigo-400" />
                            </div>
                            <div>
                                <p className="text-2xl font-bold text-gray-900 dark:text-white">{stats?.guilds.length || 0}</p>
                                <p className="text-sm text-gray-500">Servers</p>
                            </div>
                        </div>
                    </div>
                    <div className="bg-white dark:bg-base-100 rounded-xl p-4 shadow-sm border border-gray-100 dark:border-base-200">
                        <div className="flex items-center gap-3">
                            <div className="p-2 bg-green-100 dark:bg-green-900/30 rounded-lg">
                                <Radio size={20} className="text-green-600 dark:text-green-400" />
                            </div>
                            <div>
                                <p className="text-2xl font-bold text-gray-900 dark:text-white">{stats?.total_channels || 0}</p>
                                <p className="text-sm text-gray-500">Channels</p>
                            </div>
                        </div>
                    </div>
                    <div className="bg-white dark:bg-base-100 rounded-xl p-4 shadow-sm border border-gray-100 dark:border-base-200">
                        <div className="flex items-center gap-3">
                            <div className="p-2 bg-purple-100 dark:bg-purple-900/30 rounded-lg">
                                <MessageSquare size={20} className="text-purple-600 dark:text-purple-400" />
                            </div>
                            <div>
                                <p className="text-2xl font-bold text-gray-900 dark:text-base-content">{stats?.total_messages || 0}</p>
                                <p className="text-sm text-gray-500">Messages</p>
                            </div>
                        </div>
                    </div>
                </div>

                {/* Server List */}
                <div className="bg-white dark:bg-base-100 rounded-xl shadow-sm border border-gray-100 dark:border-base-200 overflow-hidden">
                    <div className="px-4 py-3 border-b border-gray-200 dark:border-base-300">
                        <h3 className="font-medium text-gray-900 dark:text-white flex items-center gap-2">
                            <Server size={18} />
                            Connected Servers
                        </h3>
                    </div>
                    
                    {!stats || stats.guilds.length === 0 ? (
                        <div className="p-8 text-center text-gray-500">
                            <Server size={48} className="mx-auto mb-4 opacity-30" />
                            <p>No servers configured yet.</p>
                            <p className="text-sm">Start the bot and use /settings in a Discord channel.</p>
                        </div>
                    ) : (
                        <div className="divide-y divide-gray-200 dark:divide-base-300">
                            {stats.guilds.map((guild) => (
                                <div key={guild.guild_id} className="p-4 transition-colors hover:bg-gray-100 dark:hover:bg-base-200/5">
                                    <div 
                                        className="flex items-center justify-between mb-3 cursor-pointer select-none group"
                                        onClick={() => toggleGuild(guild.guild_id)}
                                    >
                                        <div className="flex items-center gap-3">
                                            <div className="text-gray-400 group-hover:text-gray-600 dark:group-hover:text-gray-200 transition-colors">
                                                {collapsedGuilds.has(guild.guild_id) ? <ChevronRight size={18} /> : <ChevronDown size={18} />}
                                            </div>
                                            <div className="w-8 h-8 bg-gradient-to-br from-indigo-500 to-purple-600 rounded-lg flex items-center justify-center text-white font-bold text-sm shadow-sm">
                                                {guild.guild_id.slice(-2)}
                                            </div>
                                            <div>
                                                <p className="font-medium text-gray-900 dark:text-base-content group-hover:text-indigo-600 dark:group-hover:text-indigo-400 transition-colors">Server {formatChannelId(guild.guild_id)}</p>
                                                <p className="text-xs text-gray-500">{guild.chat_model}</p>
                                            </div>
                                        </div>
                                        <div className="text-right">
                                            <p className="text-sm font-medium text-gray-900 dark:text-base-content">{guild.total_messages} msgs</p>
                                            <p className="text-xs text-gray-500">{guild.channels.length} channels</p>
                                        </div>
                                    </div>
                                    
                                    {/* Channels */}
                                    {!collapsedGuilds.has(guild.guild_id) && (
                                        <div className="ml-11 space-y-1 animate-in slide-in-from-top-1 duration-200">
                                        {guild.channels.map((channel) => (
                                            <div 
                                                key={channel.channel_id}
                                                className="flex items-center justify-between py-1.5 px-2 rounded hover:bg-gray-50 dark:hover:bg-base-300"
                                            >
                                                <div className="flex items-center gap-2">
                                                    <span className={`w-2 h-2 rounded-full ${channel.is_listening ? 'bg-green-500' : 'bg-gray-400'}`} />
                                                    <span className="text-sm text-gray-700 dark:text-gray-300">
                                                        #{formatChannelId(channel.channel_id)}
                                                    </span>
                                                    {channel.shared_chat && (
                                                        <span className="px-1.5 py-0.5 text-xs bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400 rounded">
                                                            Shared
                                                        </span>
                                                    )}
                                                    {channel.listen_udin && (
                                                        <span className="px-1.5 py-0.5 text-xs bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400 rounded">
                                                            Udin
                                                        </span>
                                                    )}
                                                </div>
                                                <button 
                                                    className="text-xs text-indigo-500 hover:text-indigo-700 hover:underline cursor-pointer"
                                                    onClick={() => openMessageHistory(channel.channel_id)}
                                                >
                                                    {channel.message_count} msgs
                                                </button>
                                            </div>
                                        ))}
                                        </div>
                                    )}
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            </div>

            {/* Console Modal */}
            {showConsole && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
                    <div className="w-full max-w-4xl h-[80vh] bg-[#1e1e2e] rounded-xl overflow-hidden flex flex-col shadow-2xl border border-gray-800 m-4">
                        {/* Console Header */}
                        <div className="flex items-center justify-between px-4 py-3 bg-[#181825] border-b border-gray-800">
                            <div className="flex items-center gap-3">
                                <div className="flex gap-1.5">
                                    <button 
                                        className="w-3 h-3 rounded-full bg-red-500 hover:bg-red-600 transition-colors"
                                        onClick={() => setShowConsole(false)}
                                    />
                                    <div className="w-3 h-3 rounded-full bg-yellow-500"></div>
                                    <div className="w-3 h-3 rounded-full bg-green-500"></div>
                                </div>
                                <span className="text-sm font-medium text-gray-400">Console</span>
                            </div>
                            <div className="flex items-center gap-2">
                                <button 
                                    className="text-gray-500 hover:text-gray-300 transition-colors"
                                    onClick={clearLogs}
                                    title="Clear logs"
                                >
                                    <Trash2 size={16} />
                                </button>
                                <button 
                                    className="text-gray-500 hover:text-gray-300 transition-colors"
                                    onClick={() => setShowConsole(false)}
                                    title="Close"
                                >
                                    <X size={18} />
                                </button>
                            </div>
                        </div>

                        {/* Console Content */}
                        <div 
                            ref={logContainerRef}
                            className="flex-1 overflow-auto p-4 font-mono text-sm"
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
            )}

            {/* Message History Modal */}
            {showMessages && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
                    <div className="w-full max-w-3xl h-[80vh] bg-white dark:bg-base-200 rounded-xl overflow-hidden flex flex-col shadow-2xl border border-gray-200 dark:border-base-300 m-4">
                        {/* Header */}
                        <div className="flex items-center justify-between px-4 py-3 bg-gray-50 dark:bg-base-300 border-b border-gray-200 dark:border-base-300">
                            <div className="flex items-center gap-2">
                                <MessageSquare size={18} className="text-indigo-500" />
                                <span className="font-medium text-gray-900 dark:text-white">
                                    Channel #{formatChannelId(showMessages.channelId)}
                                </span>
                                <span className="text-sm text-gray-500">
                                    ({showMessages.messages.length} messages)
                                </span>
                            </div>
                            <div className="flex items-center gap-2">
                                <button 
                                    className="btn btn-sm btn-error gap-1"
                                    onClick={() => clearChannelMessages(showMessages.channelId)}
                                >
                                    <Trash2 size={14} />
                                    Clear All
                                </button>
                                <button 
                                    className="btn btn-sm btn-ghost btn-circle"
                                    onClick={() => setShowMessages(null)}
                                >
                                    <X size={18} />
                                </button>
                            </div>
                        </div>

                        {/* Messages Content */}
                        <div className="flex-1 overflow-auto p-4 space-y-2">
                            {showMessages.messages.length === 0 ? (
                                <div className="h-full flex flex-col items-center justify-center text-gray-500">
                                    <MessageSquare size={48} className="mb-4 opacity-30" />
                                    <p>No messages in this channel.</p>
                                </div>
                            ) : (
                                showMessages.messages.map((msg, i) => (
                                    <div 
                                        key={i}
                                        className={`p-3 rounded-lg ${
                                            msg.role === 'assistant' 
                                                ? 'bg-indigo-50 dark:bg-indigo-900/20 border-l-4 border-indigo-500' 
                                                : 'bg-gray-50 dark:bg-base-300'
                                        }`}
                                    >
                                        <div className="flex items-center gap-2 mb-1">
                                            <span className={`text-xs font-medium ${
                                                msg.role === 'assistant' 
                                                    ? 'text-indigo-600 dark:text-indigo-400' 
                                                    : 'text-gray-600 dark:text-gray-400'
                                            }`}>
                                                {msg.role === 'assistant' ? '🤖 Bot' : msg.author_name || 'User'}
                                            </span>
                                        </div>
                                        <p className="text-sm text-gray-800 dark:text-gray-200 whitespace-pre-wrap break-words">
                                            {msg.content.length > 500 ? msg.content.slice(0, 500) + '...' : msg.content}
                                        </p>
                                    </div>
                                ))
                            )}
                        </div>
                    </div>
                </div>
            )}
            </div>
        </div>
    );
}
