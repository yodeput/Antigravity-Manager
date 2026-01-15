use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::modules::account::get_data_dir;

#[derive(Debug, Serialize, Deserialize)]
pub struct GuildConfig {
    pub guild_id: String,
    pub chat_model: String,
    pub image_model: String,
    pub system_prompt: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub channel_id: String,
    pub guild_id: String,
    pub is_listening: bool,
    pub shared_chat: bool,
    #[serde(default)]
    pub listen_udin: bool,
}

pub fn get_db_path() -> Result<PathBuf, String> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join("discord_bot.db"))
}

pub fn init_db() -> Result<(), String> {
    let db_path = get_db_path()?;
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS guild_configs (
            guild_id TEXT PRIMARY KEY,
            chat_model TEXT NOT NULL DEFAULT 'gemini-2.0-flash',
            image_model TEXT,
            system_prompt TEXT
        )",
        [],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS channel_configs (
            channel_id TEXT PRIMARY KEY,
            guild_id TEXT,
            is_listening BOOLEAN DEFAULT 0,
            shared_chat BOOLEAN DEFAULT 0,
            listen_udin BOOLEAN DEFAULT 0
        )",
        [],
    ).map_err(|e| e.to_string())?;

    // Migration for existing tables
    let _ = conn.execute("ALTER TABLE channel_configs ADD COLUMN listen_udin BOOLEAN DEFAULT 0", []);

    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            guild_id TEXT,
            channel_id TEXT,
            user_id TEXT,
            author_name TEXT,
            role TEXT,
            content TEXT,
            created_at INTEGER
        )",
        [],
    ).map_err(|e| e.to_string())?;

    // Migration for existing tables
    let _ = conn.execute("ALTER TABLE messages ADD COLUMN author_name TEXT", []);;

    Ok(())
}

pub fn get_guild_config(guild_id: &str) -> Result<GuildConfig, String> {
    let db_path = get_db_path()?;
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    
    let config = conn.query_row(
        "SELECT guild_id, chat_model, image_model, system_prompt FROM guild_configs WHERE guild_id = ?",
        [guild_id],
        |row| Ok(GuildConfig {
            guild_id: row.get(0)?,
            chat_model: row.get(1)?,
            image_model: row.get(2).unwrap_or_default(),
            system_prompt: row.get(3).unwrap_or_else(|_| "You are a helpful assistant.".to_string()),
        })
    ).optional().map_err(|e| e.to_string())?;

    Ok(config.unwrap_or(GuildConfig {
        guild_id: guild_id.to_string(),
        chat_model: "gemini-2.5-flash".to_string(),
        image_model: "gemini-3-pro-image".to_string(),
        system_prompt: "You are a helpful assistant.".to_string(),
    }))
}

pub fn update_guild_config(config: &GuildConfig) -> Result<(), String> {
    let db_path = get_db_path()?;
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO guild_configs (guild_id, chat_model, image_model, system_prompt) 
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(guild_id) DO UPDATE SET 
            chat_model = excluded.chat_model,
            image_model = excluded.image_model,
            system_prompt = excluded.system_prompt",
        params![config.guild_id, config.chat_model, config.image_model, config.system_prompt],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn get_channel_config(channel_id: &str) -> Result<ChannelConfig, String> {
    let db_path = get_db_path()?;
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;

    // We select explicitly to match struct
    // Handle case where column might be missing if migration failed (unlikely but safe to use defaults)
    // Actually we should assume init_db ran.
    // Since we can't easily dynamically check columns in simple rusqlite query_row without boilerplate,
    // we'll rely on the ALTER TABLE above working.

    let config = conn.query_row(
        "SELECT channel_id, guild_id, is_listening, shared_chat, listen_udin FROM channel_configs WHERE channel_id = ?",
        [channel_id],
        |row| Ok(ChannelConfig {
            channel_id: row.get(0)?,
            guild_id: row.get(1)?,
            is_listening: row.get(2)?,
            shared_chat: row.get(3)?,
            listen_udin: row.get(4).unwrap_or(false), // fallback
        })
    ).optional().map_err(|e| e.to_string())?;

    Ok(config.unwrap_or(ChannelConfig {
        channel_id: channel_id.to_string(),
        guild_id: "".to_string(),
        is_listening: false,
        shared_chat: false,
        listen_udin: false,
    }))
}

pub fn update_channel_config(config: &ChannelConfig) -> Result<(), String> {
    let db_path = get_db_path()?;
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO channel_configs (channel_id, guild_id, is_listening, shared_chat, listen_udin) 
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(channel_id) DO UPDATE SET 
            guild_id = excluded.guild_id,
            is_listening = excluded.is_listening,
            shared_chat = excluded.shared_chat,
            listen_udin = excluded.listen_udin",
        params![config.channel_id, config.guild_id, config.is_listening, config.shared_chat, config.listen_udin],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn save_message(guild_id: &str, channel_id: &str, user_id: &str, author_name: &str, role: &str, content: &str) -> Result<(), String> {
    let db_path = get_db_path()?;
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    
    conn.execute(
        "INSERT INTO messages (guild_id, channel_id, user_id, author_name, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![guild_id, channel_id, user_id, author_name, role, content, chrono::Utc::now().timestamp()],
    ).map_err(|e| e.to_string())?;
    
    Ok(())
}

pub struct ChatMessage {
    pub role: String,
    pub author_name: Option<String>,
    pub content: String,
}

pub fn get_chat_history(channel_id: &str, user_id: Option<&str>, limit: usize) -> Result<Vec<ChatMessage>, String> {
    let db_path = get_db_path()?;
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;

    let history = if let Some(uid) = user_id {
        // User mode
        let mut stmt = conn.prepare(
            "SELECT role, author_name, content FROM messages 
             WHERE channel_id = ? AND (user_id = ? OR role = 'assistant') 
             ORDER BY created_at DESC LIMIT ?"
        ).map_err(|e| e.to_string())?;
        
        let rows = stmt.query_map(params![channel_id, uid, limit], |row| {
            Ok(ChatMessage { role: row.get(0)?, author_name: row.get(1)?, content: row.get(2)? })
        }).map_err(|e| e.to_string())?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(|e| e.to_string())?);
        }
        items
    } else {
        // Shared mode
        let mut stmt = conn.prepare(
            "SELECT role, author_name, content FROM messages 
             WHERE channel_id = ? 
             ORDER BY created_at DESC LIMIT ?"
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map(params![channel_id, limit], |row| {
            Ok(ChatMessage { role: row.get(0)?, author_name: row.get(1)?, content: row.get(2)? })
        }).map_err(|e| e.to_string())?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(|e| e.to_string())?);
        }
        items
    };

    let mut final_history = history;
    final_history.reverse(); // Return in chronological order
    Ok(final_history)
}

pub fn clear_chat_history(guild_id: &str) -> Result<(), String> {
    let db_path = get_db_path()?;
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;

    conn.execute(
        "DELETE FROM messages WHERE guild_id = ?",
        [guild_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}
