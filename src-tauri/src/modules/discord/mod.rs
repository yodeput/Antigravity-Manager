pub mod db;
pub mod commands;
pub mod events;
pub mod spotify;

use poise::serenity_prelude as serenity;
use crate::commands::proxy::ProxyServiceState;
use tauri::AppHandle;

// User data, which is stored and accessible in all command invocations
pub struct Data {
    pub proxy_state: ProxyServiceState,
    pub app_handle: AppHandle,
    pub mention_cache: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, GuildCache>>>,
    // Spotify integration
    pub spotify_client_id: String,
    pub spotify_client_secret: String,
    pub spotify_token_cache: spotify::SpotifyTokenCache,
}

#[derive(Debug, Clone, Default)]
pub struct GuildCache {
    pub replacements: Vec<Replacement>,
}

#[derive(Debug, Clone)]
pub struct Replacement {
    pub pattern: String,
    pub value: String,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

pub async fn start_bot(
    token: String,
    proxy_state: ProxyServiceState,
    app_handle: AppHandle,
    spotify_client_id: String,
    spotify_client_secret: String,
) -> Result<(), Error> {
    let intents = serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let spotify_token_cache = spotify::new_token_cache();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::settings(),
                commands::imagine(),
            ],
            event_handler: |ctx, event, framework, data| {
                Box::pin(events::event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            let spotify_client_id = spotify_client_id.clone();
            let spotify_client_secret = spotify_client_secret.clone();
            let spotify_token_cache = spotify_token_cache.clone();
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    proxy_state,
                    app_handle,
                    mention_cache: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
                    spotify_client_id,
                    spotify_client_secret,
                    spotify_token_cache,
                })
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client?.start().await?;
    Ok(())
}
