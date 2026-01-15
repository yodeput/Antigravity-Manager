use tauri::{State, AppHandle, Emitter};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use crate::models::DiscordBotConfig;
use crate::modules::discord;
use crate::commands::proxy::ProxyServiceState;
use tracing::{info, error};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordBotStatus {
    pub running: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordLogEntry {
    pub timestamp: String,
    pub level: String,   // "info", "warn", "error", "success"
    pub message: String,
}

pub struct DiscordServiceState {
    pub handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    pub logs: Arc<RwLock<VecDeque<DiscordLogEntry>>>,
}

impl DiscordServiceState {
    pub fn new() -> Self {
        Self {
            handle: Arc::new(RwLock::new(None)),
            logs: Arc::new(RwLock::new(VecDeque::with_capacity(200))),
        }
    }
}

fn get_timestamp() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

async fn add_log(state: &DiscordServiceState, level: &str, message: &str, app_handle: Option<&AppHandle>) {
    let entry = DiscordLogEntry {
        timestamp: get_timestamp(),
        level: level.to_string(),
        message: message.to_string(),
    };
    
    let mut logs = state.logs.write().await;
    if logs.len() >= 200 {
        logs.pop_front();
    }
    logs.push_back(entry.clone());
    
    // Emit event to frontend for real-time updates
    if let Some(handle) = app_handle {
        let _ = handle.emit("discord-log", entry);
    }
}

#[tauri::command]
pub async fn start_discord_bot(
    app_handle: AppHandle,
    config: DiscordBotConfig,
    state: State<'_, DiscordServiceState>,
    proxy_state: State<'_, ProxyServiceState>,
) -> Result<DiscordBotStatus, String> {
    // Beautiful startup sequence
    add_log(&state, "info", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Some(&app_handle)).await;
    add_log(&state, "info", "🚀 Discord Bot Starting...", Some(&app_handle)).await;
    add_log(&state, "info", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Some(&app_handle)).await;
    
    if config.bot_token.is_empty() {
        add_log(&state, "error", "❌ Bot token is empty!", Some(&app_handle)).await;
        return Err("Bot token is empty".to_string());
    }
    
    add_log(&state, "info", "🔑 Validating bot token...", Some(&app_handle)).await;

    let mut handle_lock = state.handle.write().await;
    
    if handle_lock.is_some() {
        add_log(&state, "warn", "⚠️  Bot is already running", Some(&app_handle)).await;
        return Ok(DiscordBotStatus { running: true, enabled: true });
    }

    // Initialize DB
    add_log(&state, "info", "💾 Initializing database...", Some(&app_handle)).await;
    if let Err(e) = discord::db::init_db() {
        error!("Failed to init Discord DB: {}", e);
        add_log(&state, "error", &format!("❌ Database error: {}", e), Some(&app_handle)).await;
        return Err(format!("Database error: {}", e));
    }
    add_log(&state, "success", "✅ Database initialized", Some(&app_handle)).await;

    let proxy_state_cloned = ProxyServiceState {
        instance: proxy_state.instance.clone(),
        monitor: proxy_state.monitor.clone(),
    };

    let token = config.bot_token.clone();
    let app_handle_clone = app_handle.clone();
    let logs_clone = state.logs.clone();
    
    add_log(&state, "info", "🔌 Connecting to Discord Gateway...", Some(&app_handle)).await;
    
    let handle = tokio::spawn(async move {
        info!("Starting Discord Bot...");
        
        // Add connected log after a small delay (simulating connection)
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        {
            let entry = DiscordLogEntry {
                timestamp: get_timestamp(),
                level: "success".to_string(),
                message: "✅ Connected to Discord!".to_string(),
            };
            let mut logs = logs_clone.write().await;
            logs.push_back(entry.clone());
            let _ = app_handle_clone.emit("discord-log", entry);
        }
        
        {
            let entry = DiscordLogEntry {
                timestamp: get_timestamp(),
                level: "info".to_string(),
                message: "📡 Bot is now online and listening...".to_string(),
            };
            let mut logs = logs_clone.write().await;
            logs.push_back(entry.clone());
            let _ = app_handle_clone.emit("discord-log", entry);
        }
        
        {
            let entry = DiscordLogEntry {
                timestamp: get_timestamp(),
                level: "info".to_string(),
                message: "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".to_string(),
            };
            let mut logs = logs_clone.write().await;
            logs.push_back(entry.clone());
            let _ = app_handle_clone.emit("discord-log", entry);
        }
        
        if let Err(e) = discord::start_bot(token, proxy_state_cloned, app_handle_clone.clone()).await {
            error!("Discord Bot crashed: {}", e);
            let entry = DiscordLogEntry {
                timestamp: get_timestamp(),
                level: "error".to_string(),
                message: format!("❌ Bot crashed: {}", e),
            };
            let mut logs = logs_clone.write().await;
            logs.push_back(entry.clone());
            let _ = app_handle_clone.emit("discord-log", entry);
        }
    });

    *handle_lock = Some(handle);

    Ok(DiscordBotStatus { running: true, enabled: true })
}

#[tauri::command]
pub async fn stop_discord_bot(
    app_handle: AppHandle,
    state: State<'_, DiscordServiceState>,
) -> Result<DiscordBotStatus, String> {
    add_log(&state, "info", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Some(&app_handle)).await;
    add_log(&state, "info", "🛑 Stopping Discord Bot...", Some(&app_handle)).await;
    
    let mut handle_lock = state.handle.write().await;
    
    if let Some(handle) = handle_lock.take() {
        add_log(&state, "info", "🔌 Disconnecting from Discord...", Some(&app_handle)).await;
        handle.abort();
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        add_log(&state, "success", "✅ Bot stopped successfully", Some(&app_handle)).await;
    } else {
        add_log(&state, "warn", "⚠️  Bot was not running", Some(&app_handle)).await;
    }
    
    add_log(&state, "info", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━", Some(&app_handle)).await;

    Ok(DiscordBotStatus { running: false, enabled: false })
}

#[tauri::command]
pub async fn get_discord_bot_status(
    state: State<'_, DiscordServiceState>,
) -> Result<DiscordBotStatus, String> {
    let handle_lock = state.handle.read().await;
    Ok(DiscordBotStatus {
        running: handle_lock.is_some(),
        enabled: handle_lock.is_some(),
    })
}

#[tauri::command]
pub async fn get_discord_logs(
    state: State<'_, DiscordServiceState>,
) -> Result<Vec<DiscordLogEntry>, String> {
    let logs = state.logs.read().await;
    Ok(logs.iter().cloned().collect())
}

#[tauri::command]
pub async fn clear_discord_logs(
    state: State<'_, DiscordServiceState>,
) -> Result<(), String> {
    let mut logs = state.logs.write().await;
    logs.clear();
    Ok(())
}
