use poise::serenity_prelude as serenity;
use crate::modules::discord::{db, Context, Error};
use serenity::{
    CreateActionRow, CreateButton, CreateEmbed, CreateInteractionResponse, 
    CreateInteractionResponseMessage, CreateSelectMenu, CreateSelectMenuKind, 
    CreateSelectMenuOption, CreateInputText, InputTextStyle, CreateModal,
    CreateInteractionResponseFollowup,
};
use std::time::Duration;
use serenity::futures::StreamExt;
use serde_json::json;

// --- Settings Command ---
/// Open the Settings Dashboard
#[poise::command(slash_command, required_permissions = "ADMINISTRATOR")]
pub async fn settings(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap().to_string();
    let channel_id = ctx.channel_id().to_string();

    // Initial render
    let handle = send_settings_menu(&ctx, &guild_id, &channel_id).await?;

    // Component Interaction Loop using stream()
    let mut collector = handle.message().await?.await_component_interactions(ctx.serenity_context())
        .timeout(Duration::from_secs(60 * 15)) // 15 minutes timeout
        .stream();

    while let Some(mci) = collector.next().await {
        let custom_id = &mci.data.custom_id;

        // Handle Toggles
        if custom_id == "toggle_listen" {
            let mut config = db::get_channel_config(&channel_id)?;
            config.is_listening = !config.is_listening;
            config.guild_id = guild_id.clone();
            db::update_channel_config(&config)?;

            // If turning ON, refresh the cache
            if config.is_listening {
                let cache_clone = ctx.data().mention_cache.clone();
                let http = ctx.serenity_context().http.clone();
                let gid_str = guild_id.clone();
                // Parse guild_id safely
                if let Ok(gid_u64) = gid_str.parse::<u64>() {
                    let gid = serenity::GuildId::new(gid_u64);

                    tokio::spawn(async move {
                        use crate::modules::discord::{Replacement, GuildCache};
                        
                        let mut new_replacements = Vec::new();
                         use serenity::Mentionable;

                        // 1. Roles
                        if let Ok(roles) = gid.roles(&http).await {
                            for (role_id, role) in roles {
                                new_replacements.push(Replacement {
                                    pattern: format!("@{}", role.name),
                                    value: format!("<@&{}>", role_id),
                                });
                            }
                        }

                        // 2. Channels
                        if let Ok(channels) = gid.channels(&http).await {
                            for (cid, ch_obj) in channels {
                                new_replacements.push(Replacement {
                                    pattern: format!("#{}", ch_obj.name),
                                    value: format!("<#{}>", cid),
                                });
                            }
                        }

                        // 3. Members (Limit 1000)
                        if let Ok(members) = gid.members(&http, Some(1000), None).await {
                             for member in members {
                                 let user = &member.user;
                                 let mut names: Vec<String> = Vec::new();
                                 names.push(user.name.clone());
                                 if let Some(gn) = &user.global_name { names.push(gn.to_string()); }
                                 if let Some(nick) = &member.nick { names.push(nick.to_string()); }

                                 let mention_str = format!("<@{}>", user.id);
                                 for name in names {
                                     new_replacements.push(Replacement {
                                         pattern: format!("@{}", name),
                                         value: mention_str.clone(),
                                     });
                                 }
                            }
                        }

                        // Sort descending
                        new_replacements.sort_by(|a, b| b.pattern.len().cmp(&a.pattern.len()));

                        let mut lock = cache_clone.write().await;
                        lock.insert(gid_str, GuildCache { replacements: new_replacements });
                    });
                }
            }

            update_settings_menu(&ctx, &mci, &guild_id, &channel_id).await?;
        } 
        else if custom_id == "toggle_shared" {
            let mut config = db::get_channel_config(&channel_id)?;
            config.shared_chat = !config.shared_chat;
            config.guild_id = guild_id.clone();
            db::update_channel_config(&config)?;
            update_settings_menu(&ctx, &mci, &guild_id, &channel_id).await?;
        }
        else if custom_id == "toggle_udin" {
            let mut config = db::get_channel_config(&channel_id)?;
            config.listen_udin = !config.listen_udin;
            config.guild_id = guild_id.clone();
            db::update_channel_config(&config)?;

            // If turning ON, refresh the cache
            if config.listen_udin {
                let cache_clone = ctx.data().mention_cache.clone();
                let http = ctx.serenity_context().http.clone();
                let gid_str = guild_id.clone();
                if let Ok(gid_u64) = gid_str.parse::<u64>() {
                    let gid = serenity::GuildId::new(gid_u64);

                    tokio::spawn(async move {
                        use crate::modules::discord::{Replacement, GuildCache};
                        
                        let mut new_replacements = Vec::new();

                        // 1. Roles
                        if let Ok(roles) = gid.roles(&http).await {
                            for (role_id, role) in roles {
                                new_replacements.push(Replacement {
                                    pattern: format!("@{}", role.name),
                                    value: format!("<@&{}>", role_id),
                                });
                            }
                        }

                        // 2. Channels
                        if let Ok(channels) = gid.channels(&http).await {
                            for (cid, ch_obj) in channels {
                                new_replacements.push(Replacement {
                                    pattern: format!("#{}", ch_obj.name),
                                    value: format!("<#{}>", cid),
                                });
                            }
                        }

                        // 3. Members (Limit 1000)
                        if let Ok(members) = gid.members(&http, Some(1000), None).await {
                             for member in members {
                                 let user = &member.user;
                                 let mut names: Vec<String> = Vec::new();
                                 names.push(user.name.clone());
                                 if let Some(gn) = &user.global_name { names.push(gn.to_string()); }
                                 if let Some(nick) = &member.nick { names.push(nick.to_string()); }

                                 let mention_str = format!("<@{}>", user.id);
                                 for name in names {
                                     new_replacements.push(Replacement {
                                         pattern: format!("@{}", name),
                                         value: mention_str.clone(),
                                     });
                                 }
                            }
                        }

                        // Sort descending
                        new_replacements.sort_by(|a, b| b.pattern.len().cmp(&a.pattern.len()));

                        let mut lock = cache_clone.write().await;
                        lock.insert(gid_str, GuildCache { replacements: new_replacements });
                    });
                }
            }

            update_settings_menu(&ctx, &mci, &guild_id, &channel_id).await?;
        }
        // Handle Personality Modal
        else if custom_id == "btn_personality" {
            let guild_config = db::get_guild_config(&guild_id)?;
            
            let input = CreateInputText::new(InputTextStyle::Paragraph, "System Prompt", "prompt")
                .value(&guild_config.system_prompt)
                .placeholder("You are a helpful assistant...");
            
            let modal = CreateModal::new("modal_personality", "Edit Personality")
                .components(vec![CreateActionRow::InputText(input)]);

            mci.create_response(ctx, CreateInteractionResponse::Modal(modal)).await?;
            
            // Wait for modal submit
            if let Some(modal_interaction) = mci.message.await_modal_interaction(ctx.serenity_context())
                .timeout(Duration::from_secs(300))
                .await 
            {
                if modal_interaction.data.custom_id == "modal_personality" {
                    // Extract value
                    for row in &modal_interaction.data.components {
                        for component in &row.components {
                            if let serenity::ActionRowComponent::InputText(text) = component {
                                if text.custom_id == "prompt" {
                                    let mut new_config = db::get_guild_config(&guild_id)?;
                                    new_config.system_prompt = text.value.clone().unwrap_or_default();
                                    db::update_guild_config(&new_config)?;
                                }
                            }
                        }
                    }
                    
                    modal_interaction.create_response(ctx, CreateInteractionResponse::Acknowledge).await?;
                    // Refresh menu
                    let _ = handle.edit(ctx, 
                        build_settings_message(&guild_id, &channel_id)?
                    ).await;
                }
            }
        }
        // Handle Select Menus (in Models view)
        else if custom_id == "select_chat_model" || custom_id == "select_image_model" {
            // Serenity 0.12 way to get selected values from ComponentInteractionDataKind::StringSelect
            let mut selected_value = String::new();
            if let serenity::ComponentInteractionDataKind::StringSelect { values } = &mci.data.kind {
                if let Some(val) = values.first() {
                    selected_value = val.clone();
                }
            }

            if !selected_value.is_empty() {
                let mut config = db::get_guild_config(&guild_id)?;
                
                if custom_id == "select_chat_model" {
                    config.chat_model = selected_value;
                } else {
                    config.image_model = selected_value;
                }
                db::update_guild_config(&config)?;
                
                // Stay on models view after selection
                let (embed, components) = build_models_view(&guild_id)?;
                mci.create_response(ctx, CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(embed)
                        .components(components)
                )).await?;
            }
        }
        // Handle OAuth Login Flow (Copy-Paste OOB Flow)
        else if custom_id == "btn_oauth_login" {
            // 1. Generate OOB URL and show to user with a link + instructions
            let auth_url = crate::modules::oauth::get_oob_auth_url();
            
            // 2. Show the link and a button to open the code submission modal
            mci.create_response(ctx, CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .embed(CreateEmbed::new()
                        .title("🔐 Google OAuth Login")
                        .description("**Step 1:** Click the link below to authenticate with Google\n\n\
                            **Step 2:** After granting access, Google will show you an **authorization code**\n\n\
                            **Step 3:** Copy the code and click **\"Submit Code\"** below")
                        .field("Login Link", format!("[Click Here to Login]({})", auth_url), false)
                        .color(0x4285f4)
                        .footer(serenity::CreateEmbedFooter::new("Code expires in a few minutes")))
                    .components(vec![
                        CreateActionRow::Buttons(vec![
                            CreateButton::new("btn_submit_oauth_code")
                                .label("Submit Code")
                                .style(serenity::ButtonStyle::Success)
                                .emoji('📋'),
                            CreateButton::new("btn_cancel_oauth")
                                .label("Cancel")
                                .style(serenity::ButtonStyle::Secondary),
                        ])
                    ])
            )).await?;
        }
        // Handle OAuth Code Submission Modal
        else if custom_id == "btn_submit_oauth_code" {
            let input = CreateInputText::new(InputTextStyle::Short, "Authorization Code", "oauth_code")
                .placeholder("Paste the code from Google here...")
                .required(true)
                .min_length(10)
                .max_length(200);
            
            let modal = CreateModal::new("modal_oauth_code", "Enter Authorization Code")
                .components(vec![CreateActionRow::InputText(input)]);
            
            mci.create_response(ctx, CreateInteractionResponse::Modal(modal)).await?;
            
            // Wait for modal submit
            if let Some(modal_interaction) = mci.message.await_modal_interaction(ctx.serenity_context())
                .timeout(Duration::from_secs(300))
                .await 
            {
                if modal_interaction.data.custom_id == "modal_oauth_code" {
                    // Extract the code
                    let mut auth_code = String::new();
                    for row in &modal_interaction.data.components {
                        for component in &row.components {
                            if let serenity::ActionRowComponent::InputText(text) = component {
                                if text.custom_id == "oauth_code" {
                                    auth_code = text.value.clone().unwrap_or_default().trim().to_string();
                                }
                            }
                        }
                    }
                    
                    if auth_code.is_empty() {
                        modal_interaction.create_response(ctx, CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .ephemeral(true)
                                .content("❌ No authorization code provided.")
                        )).await?;
                    } else {
                        // Acknowledge and show processing
                        modal_interaction.create_response(ctx, CreateInteractionResponse::Defer(
                            CreateInteractionResponseMessage::new().ephemeral(true)
                        )).await?;
                        
                        // Exchange code for tokens
                        match crate::modules::oauth::exchange_code(&auth_code, crate::modules::oauth::OOB_REDIRECT_URI).await {
                            Ok(token_res) => {
                                match crate::modules::oauth::get_user_info(&token_res.access_token).await {
                                    Ok(user_info) => {
                                        let token_data = crate::models::TokenData::new(
                                            token_res.access_token,
                                            token_res.refresh_token.unwrap_or_default(),
                                            token_res.expires_in,
                                            Some(user_info.email.clone()),
                                            None,
                                            None
                                        );
                                        
                                        if let Err(e) = crate::modules::upsert_account(user_info.email.clone(), user_info.get_display_name(), token_data) {
                                            let _ = modal_interaction.create_followup(ctx, CreateInteractionResponseFollowup::new()
                                                .ephemeral(true)
                                                .content(format!("❌ **Save Failed**: {}", e))
                                            ).await;
                                        } else {
                                            let _ = modal_interaction.create_followup(ctx, CreateInteractionResponseFollowup::new()
                                                .ephemeral(true)
                                                .content(format!("✅ **Success!** Account `{}` added.", user_info.email))
                                            ).await;
                                        }
                                    },
                                    Err(e) => {
                                        let _ = modal_interaction.create_followup(ctx, CreateInteractionResponseFollowup::new()
                                            .ephemeral(true)
                                            .content(format!("❌ **Failed to get user info**: {}", e))
                                        ).await;
                                    }
                                }
                            },
                            Err(e) => {
                                let _ = modal_interaction.create_followup(ctx, CreateInteractionResponseFollowup::new()
                                    .ephemeral(true)
                                    .content(format!("❌ **Code Exchange Failed**: {}\n\nMake sure you copied the complete code.", e))
                                ).await;
                            }
                        }
                    }
                    
                    // Return to settings menu
                    let _ = handle.edit(ctx, build_settings_message(&guild_id, &channel_id)?).await;
                }
            }
        }
        // Handle OAuth Cancel
        else if custom_id == "btn_cancel_oauth" {
            // Just return to settings menu
            update_settings_menu(&ctx, &mci, &guild_id, &channel_id).await?;
        }
        // Handle Models Button - Show model selection view
        else if custom_id == "btn_models" {
            let (embed, components) = build_models_view(&guild_id)?;
            mci.create_response(ctx, CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .embed(embed)
                    .components(components)
            )).await?;
        }
        // Handle Back to Settings
        else if custom_id == "btn_back_settings" {
            update_settings_menu(&ctx, &mci, &guild_id, &channel_id).await?;
        }
        // Handle Clear Memory
        else if custom_id == "btn_clear_memory" {
            db::clear_chat_history(&guild_id)?;
            mci.create_response(ctx, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .ephemeral(true)
                    .content("✅ **Success!** Chat memory for this server has been cleared.")
            )).await?;
        }
    }

    Ok(())
}

// --- Imagine Command ---

#[derive(Debug, poise::ChoiceParameter)]
pub enum ImageSize {
    #[name = "Square (1:1)"]
    Square,
    #[name = "Portrait (9:16)"]
    Portrait,
    #[name = "Landscape (16:9)"]
    Landscape,
}

/// Generate an image using AI
#[poise::command(slash_command)]
pub async fn imagine(
    ctx: Context<'_>,
    #[description = "The prompt for the image"] prompt: String,
    #[description = "Aspect ratio of the image"] size: Option<ImageSize>,
    #[description = "Number of images to generate (default 1)"] count: Option<u8>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = ctx.guild_id().map(|g| g.to_string()).unwrap_or_default();
    
    // Ensure we have a config, or use defaults
    let guild_config = match db::get_guild_config(&guild_id) {
        Ok(c) => c,
        Err(_) => db::GuildConfig {
            guild_id: guild_id.clone(),
            chat_model: "gemini-2.5-flash".to_string(),
            image_model: "gemini-3-pro-image".to_string(),
            system_prompt: String::new(),
        }
    };

    // Determine size string
    let size_str = match size.unwrap_or(ImageSize::Square) {
        ImageSize::Square => "1024x1024",
        ImageSize::Portrait => "720x1280",
        ImageSize::Landscape => "1280x720",
    };

    let model = if guild_config.image_model.is_empty() {
        "gemini-3-pro-image".to_string()
    } else {
        guild_config.image_model.clone()
    };

    // Call Proxy
    let proxy_state = ctx.data().proxy_state.instance.read().await;
    if let Some(instance) = proxy_state.as_ref() {
        let port = instance.config.port;
        let client = reqwest::Client::new();

        let resp = client.post(format!("http://127.0.0.1:{}/v1/chat/completions", port))
            .header("Authorization", "Bearer sk-antigravity")
            .json(&json!({
                "model": model,
                "messages": [{"role": "user", "content": prompt}],
                "extra_body": { "size": size_str },
                "n": count.unwrap_or(1).max(1)
            }))
            .send()
            .await;

        match resp {
            Ok(response) => {
                if response.status().is_success() {
                    let body: serde_json::Value = response.json().await?;
                    if let Some(content) = body["choices"][0]["message"]["content"].as_str() {
                        // Clean up content (remove markdown if present)
                        let clean_content = if content.starts_with("![") {
                            content.split('(').nth(1).and_then(|s| s.split(')').next()).unwrap_or(content)
                        } else {
                            content
                        };
                        
                        // Check if it's a base64 string
                        // Usually starts with "data:image/png;base64," or just raw base64
                        // For simplicity, if it's not a http url, we assume it might be base64 if it's long enough
                        
                        let is_url = clean_content.starts_with("http://") || clean_content.starts_with("https://");
                        
                        if is_url {
                            // Standard URL handling
                            let display_prompt = if prompt.len() > 1000 {
                                format!("{}...", &prompt[..1000])
                            } else {
                                prompt.clone()
                            };

                            let embed = CreateEmbed::new()
                                .title("🎨 Image Generated")
                                .field("Prompt", display_prompt, false)
                                .field("Model", &model, true)
                                .field("Size", size_str, true)
                                .image(clean_content)
                                .color(0x9b59b6)
                                .footer(serenity::CreateEmbedFooter::new(format!("Requested by {}", ctx.author().name)));

                            ctx.send(poise::CreateReply::default().embed(embed)).await?;
                        } else {
                            // Try to decode as base64
                            // Remove data prefix if exists
                            let base64_str = if let Some(idx) = clean_content.find(',') {
                                &clean_content[idx+1..]
                            } else {
                                clean_content
                            };

                            // Remove newlines if any
                            let base64_clean = base64_str.replace(['\n', '\r'], "");

                            use base64::Engine as _;
                            match base64::engine::general_purpose::STANDARD.decode(&base64_clean) {
                                Ok(image_data) => {
                                    let filename = "generated_image.png";
                                    let attachment = serenity::CreateAttachment::bytes(image_data, filename);

                                    let display_prompt = if prompt.len() > 1000 {
                                        format!("{}...", &prompt[..1000])
                                    } else {
                                        prompt.clone()
                                    };

                                    let embed = CreateEmbed::new()
                                        .title("🎨 Image Generated")
                                        .field("Prompt", display_prompt, false)
                                        .field("Model", &model, true)
                                        .field("Size", size_str, true)
                                        .attachment(filename)
                                        .color(0x9b59b6)
                                        .footer(serenity::CreateEmbedFooter::new(format!("Requested by {}", ctx.author().name)));

                                    ctx.send(poise::CreateReply::default().embed(embed).attachment(attachment)).await?;
                                },
                                Err(e) => {
                                    ctx.say("❌ Something went wrong with the bot. Please try again later.").await?;
                                }
                            }
                        }
                    } else {
                         ctx.say("❌ Something went wrong with the bot. Please try again later.").await?;
                    }
                } else {
                    let err_text = response.text().await.unwrap_or_default();
                     if err_text.contains("Only one candidate can be specified") {
                         ctx.say("⚠️ This model only supports generating 1 image at a time. Please try again without the count parameter or set it to 1.").await?;
                    } else {
                        ctx.say("❌ Something went wrong with the bot. Please try again later.").await?;
                    }
                }
            },
            Err(_e) => {
                ctx.say("❌ Something went wrong with the bot. Please try again later.").await?;
            }
        }
    } else {
        ctx.say("❌ The AI service is currently unavailable. Please try again later.").await?;
    }

    Ok(())
}

// --- Helpers ---

async fn send_settings_menu<'a>(ctx: &Context<'a>, guild_id: &str, channel_id: &str) -> Result<poise::ReplyHandle<'a>, Error> {
    let builder = build_settings_message(guild_id, channel_id)?;
    Ok(ctx.send(builder).await?)
}

async fn update_settings_menu(ctx: &Context<'_>, mci: &serenity::ComponentInteraction, guild_id: &str, channel_id: &str) -> Result<(), Error> {
    let (embed, components) = build_settings_components(guild_id, channel_id)?;
    
    mci.create_response(ctx, CreateInteractionResponse::UpdateMessage(
        CreateInteractionResponseMessage::new()
            .embed(embed)
            .components(components)
    )).await?;
    
    Ok(())
}

fn build_settings_message(guild_id: &str, channel_id: &str) -> Result<poise::CreateReply, Error> {
    let (embed, components) = build_settings_components(guild_id, channel_id)?;
    Ok(poise::CreateReply::default().embed(embed).components(components))
}

fn build_settings_components(guild_id: &str, channel_id: &str) -> Result<(CreateEmbed, Vec<CreateActionRow>), Error> {
    let guild_config = db::get_guild_config(guild_id)?;
    let channel_config = db::get_channel_config(channel_id)?;

    let embed = CreateEmbed::new()
        .title("🤖 Antigravity Bot Settings")
        .field("Channel Status", 
            format!("Listening: **{}**\nShared Chat: **{}**\nListen Udin: **{}**", 
                if channel_config.is_listening { "ON" } else { "OFF" },
                if channel_config.shared_chat { "ON" } else { "OFF" },
                if channel_config.listen_udin { "ON" } else { "OFF" }
            ), true)
        .field("Server Config",
            format!("Chat Model: `{}`\nImage Model: `{}`", 
                guild_config.chat_model,
                if guild_config.image_model.is_empty() { "Not Set" } else { &guild_config.image_model }
            ), true)
        .field("Personality", 
            if guild_config.system_prompt.len() > 100 { 
                format!("{}...", &guild_config.system_prompt[..100]) 
            } else { 
                guild_config.system_prompt.clone() 
            }, false)
        .color(0x7289da);

    let mut components = Vec::new();

    // Row 1: Toggles
    components.push(CreateActionRow::Buttons(vec![
        CreateButton::new("toggle_listen")
            .label(if channel_config.is_listening { "Stop Listening" } else { "Start Listening" })
            .style(if channel_config.is_listening { serenity::ButtonStyle::Danger } else { serenity::ButtonStyle::Success })
            .emoji(if channel_config.is_listening { '🟨' } else { '👂'}),
        CreateButton::new("toggle_shared")
            .label(if channel_config.shared_chat { "Disable Shared Chat" } else { "Enable Shared Chat" })
            .style(if channel_config.shared_chat { serenity::ButtonStyle::Danger } else { serenity::ButtonStyle::Success })
             .emoji(if channel_config.shared_chat { '🟨' } else { '🚀' }),
        CreateButton::new("toggle_udin")
            .label(if channel_config.listen_udin { "Stop Udin Listener" } else { "Listen Udin" })
            .style(if channel_config.listen_udin { serenity::ButtonStyle::Danger } else { serenity::ButtonStyle::Success })
             .emoji(if channel_config.listen_udin { '🔕' } else { '🔔' }),
    ]));

    // Row 2: Personality, Models & OAuth
    components.push(CreateActionRow::Buttons(vec![
        CreateButton::new("btn_personality")
            .label("Personality")
            .style(serenity::ButtonStyle::Primary)
            .emoji('🧠'),
        CreateButton::new("btn_models")
            .label("Models")
            .style(serenity::ButtonStyle::Primary)
            .emoji('🤖'),
        CreateButton::new("btn_oauth_login")
            .label("Add Account")
            .style(serenity::ButtonStyle::Secondary)
            .emoji('🔑'),
        CreateButton::new("btn_clear_memory")
            .label("Clear Memory")
            .style(serenity::ButtonStyle::Danger)
            .emoji('🧹'),
    ]));

    Ok((embed, components))
}

/// Build the Models selection view
fn build_models_view(guild_id: &str) -> Result<(CreateEmbed, Vec<CreateActionRow>), Error> {
    let guild_config = db::get_guild_config(guild_id)?;

    let embed = CreateEmbed::new()
        .title("🤖 Model Selection")
        .description("Select the AI models to use for this server")
        .field("Current Chat Model", format!("`{}`", guild_config.chat_model), true)
        .field("Current Image Model", format!("`{}`", if guild_config.image_model.is_empty() { "Not Set" } else { &guild_config.image_model }), true)
        .color(0x5865f2);

    let mut components = Vec::new();

    // Row 1: Chat Model Select
    let chat_models = vec![
        "gemini-2.5-flash",
        "gemini-2.5-flash-lite", 
        "gemini-2.5-pro",
        "gemini-2.5-flash-thinking",
        "gemini-3-flash",
        "gemini-3-pro-high",
        "gemini-3-pro-low",
    ];
    let mut chat_options = Vec::new();
    for m in chat_models {
        chat_options.push(CreateSelectMenuOption::new(m, m).default_selection(m == guild_config.chat_model));
    }
    components.push(CreateActionRow::SelectMenu(
        CreateSelectMenu::new("select_chat_model", CreateSelectMenuKind::String { options: chat_options })
            .placeholder("Select Chat Model")
    ));
    
    // Row 2: Image Model Select
    let img_models = vec!["gemini-3-pro-image"];
    let mut img_options = Vec::new();
    for m in img_models {
        img_options.push(CreateSelectMenuOption::new(m, m).default_selection(m == guild_config.image_model));
    }
    components.push(CreateActionRow::SelectMenu(
        CreateSelectMenu::new("select_image_model", CreateSelectMenuKind::String { options: img_options })
            .placeholder("Select Image Model")
    ));

    // Row 3: Back Button
    components.push(CreateActionRow::Buttons(vec![
        CreateButton::new("btn_back_settings")
            .label("Back to Settings")
            .style(serenity::ButtonStyle::Secondary)
            .emoji('◀'),
    ]));

    Ok((embed, components))
}
