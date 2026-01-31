use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Spotify OAuth token with expiry tracking
#[derive(Debug, Clone)]
pub struct SpotifyToken {
    pub access_token: String,
    pub expires_at: std::time::Instant,
}

/// Cached token storage
pub type SpotifyTokenCache = Arc<RwLock<Option<SpotifyToken>>>;

/// Create a new token cache
pub fn new_token_cache() -> SpotifyTokenCache {
    Arc::new(RwLock::new(None))
}

/// Spotify Track from Search API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyTrack {
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
    pub album_year: Option<String>,
    pub spotify_url: String,
    pub preview_url: Option<String>,
    pub album_image: Option<String>,
}

/// Spotify Playlist from Search API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyPlaylist {
    pub name: String,
    pub owner: String,
    pub track_count: u32,
    pub spotify_url: String,
    pub image: Option<String>,
}

/// Spotify Artist from Search API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyArtist {
    pub name: String,
    pub genres: Vec<String>,
    pub followers: u32,
    pub spotify_url: String,
    pub image: Option<String>,
}

/// Search type enum
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchType {
    Track,
    Playlist,
    Artist,
}

impl SearchType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SearchType::Track => "track",
            SearchType::Playlist => "playlist",
            SearchType::Artist => "artist",
        }
    }
}

/// Get or refresh Spotify access token using Client Credentials flow
pub async fn get_access_token(
    client_id: &str,
    client_secret: &str,
    cache: &SpotifyTokenCache,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Check cache first
    {
        let token_guard = cache.read().await;
        if let Some(token) = token_guard.as_ref() {
            // Check if token is still valid (with 60s buffer)
            if token.expires_at > std::time::Instant::now() + std::time::Duration::from_secs(60) {
                return Ok(token.access_token.clone());
            }
        }
    }

    // Fetch new token
    let client = reqwest::Client::new();
    let auth = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        format!("{}:{}", client_id, client_secret),
    );

    let resp = client
        .post("https://accounts.spotify.com/api/token")
        .header("Authorization", format!("Basic {}", auth))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials")
        .send()
        .await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await.unwrap_or_default();
        return Err(format!("Spotify auth failed: {}", error_text).into());
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        expires_in: u64,
    }

    let token_resp: TokenResponse = resp.json().await?;
    let new_token = SpotifyToken {
        access_token: token_resp.access_token.clone(),
        expires_at: std::time::Instant::now() + std::time::Duration::from_secs(token_resp.expires_in),
    };

    // Update cache
    {
        let mut token_guard = cache.write().await;
        *token_guard = Some(new_token);
    }

    Ok(token_resp.access_token)
}

/// Search Spotify for tracks
pub async fn search_tracks(
    query: &str,
    limit: u32,
    access_token: &str,
) -> Result<Vec<SpotifyTrack>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.spotify.com/v1/search?q={}&type=track&limit={}",
        urlencoding::encode(query),
        limit
    );

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await.unwrap_or_default();
        return Err(format!("Spotify search failed: {}", error_text).into());
    }

    let body: serde_json::Value = resp.json().await?;
    let mut tracks = Vec::new();

    if let Some(items) = body["tracks"]["items"].as_array() {
        for item in items {
            let artists: Vec<String> = item["artists"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| a["name"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let album_year = item["album"]["release_date"]
                .as_str()
                .map(|d| d.split('-').next().unwrap_or("").to_string());

            let album_image = item["album"]["images"]
                .as_array()
                .and_then(|imgs| imgs.first())
                .and_then(|img| img["url"].as_str())
                .map(|s| s.to_string());

            tracks.push(SpotifyTrack {
                name: item["name"].as_str().unwrap_or("Unknown").to_string(),
                artists,
                album: item["album"]["name"].as_str().unwrap_or("Unknown").to_string(),
                album_year,
                spotify_url: item["external_urls"]["spotify"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                preview_url: item["preview_url"].as_str().map(|s| s.to_string()),
                album_image,
            });
        }
    }

    Ok(tracks)
}

/// Search Spotify for playlists
pub async fn search_playlists(
    query: &str,
    limit: u32,
    access_token: &str,
) -> Result<Vec<SpotifyPlaylist>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.spotify.com/v1/search?q={}&type=playlist&limit={}",
        urlencoding::encode(query),
        limit
    );

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await.unwrap_or_default();
        return Err(format!("Spotify search failed: {}", error_text).into());
    }

    let body: serde_json::Value = resp.json().await?;
    let mut playlists = Vec::new();

    if let Some(items) = body["playlists"]["items"].as_array() {
        for item in items {
            let image = item["images"]
                .as_array()
                .and_then(|imgs| imgs.first())
                .and_then(|img| img["url"].as_str())
                .map(|s| s.to_string());

            playlists.push(SpotifyPlaylist {
                name: item["name"].as_str().unwrap_or("Unknown").to_string(),
                owner: item["owner"]["display_name"]
                    .as_str()
                    .unwrap_or("Unknown")
                    .to_string(),
                track_count: item["tracks"]["total"].as_u64().unwrap_or(0) as u32,
                spotify_url: item["external_urls"]["spotify"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                image,
            });
        }
    }

    Ok(playlists)
}

/// Search Spotify for artists
pub async fn search_artists(
    query: &str,
    limit: u32,
    access_token: &str,
) -> Result<Vec<SpotifyArtist>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.spotify.com/v1/search?q={}&type=artist&limit={}",
        urlencoding::encode(query),
        limit
    );

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await.unwrap_or_default();
        return Err(format!("Spotify search failed: {}", error_text).into());
    }

    let body: serde_json::Value = resp.json().await?;
    let mut artists = Vec::new();

    if let Some(items) = body["artists"]["items"].as_array() {
        for item in items {
            let genres: Vec<String> = item["genres"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|g| g.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let image = item["images"]
                .as_array()
                .and_then(|imgs| imgs.first())
                .and_then(|img| img["url"].as_str())
                .map(|s| s.to_string());

            artists.push(SpotifyArtist {
                name: item["name"].as_str().unwrap_or("Unknown").to_string(),
                genres,
                followers: item["followers"]["total"].as_u64().unwrap_or(0) as u32,
                spotify_url: item["external_urls"]["spotify"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                image,
            });
        }
    }

    Ok(artists)
}
