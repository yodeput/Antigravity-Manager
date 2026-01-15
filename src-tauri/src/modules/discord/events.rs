use poise::serenity_prelude as serenity;
use crate::modules::discord::{db, Data, Error};
use serde_json::json;

pub async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    if let serenity::FullEvent::Message { new_message } = event {
        // 1. Ignore own messages
        if new_message.author.id == ctx.cache.current_user().id {
            return Ok(());
        }

        // 2. Check if channel is listening OR if we are mentioned OR if listen_udin matches
        let channel_id = new_message.channel_id.to_string();
        let guild_id = new_message.guild_id.map(|g| g.to_string()).unwrap_or_default();
        
        let config = db::get_channel_config(&channel_id)?;
        
        let should_process = if config.is_listening {
            true
        } else {
            // Check overrides
            let mentions_me = new_message.mentions_me(&ctx.http).await.unwrap_or(false);
            let udin_triggered = config.listen_udin && new_message.content.to_lowercase().contains("din");
            
            mentions_me || udin_triggered
        };

        if !should_process {
            return Ok(());
        }

        // 2b. Player Lookup: Detect "player id 12345" patterns
        let player_re = regex::Regex::new(r"(?i)(?:player\s*id|siapa\s*player|cek\s*player|player|cek\s*akun|cek\s*id)\s*(\d+)").unwrap();
        if let Some(cap) = player_re.captures(&new_message.content) {
            if let Some(fid_match) = cap.get(1) {
                if let Ok(fid) = fid_match.as_str().parse::<u64>() {
                    // Call Player API
                    let _ = new_message.channel_id.broadcast_typing(&ctx.http).await;
                    
                    match fetch_player_data(fid).await {
                        Ok(player) => {
                            // Build Embed matching user's desired format
                            let stove_display = get_stove_level_display(player.stove_lv);
                            
                            let description = format!(
                                "👤 **{}**\n\
                                ────────────────────────\n\
                                🆔 **FID:** {}\n\
                                🔥 **Furnace Level:** {}\n\
                                🌍 **State:** {}\n\
                                ────────────────────────",
                                player.nickname,
                                player.fid,
                                stove_display,
                                player.kid
                            );
                            
                            let embed = serenity::CreateEmbed::new()
                                .description(&description)
                                .thumbnail(&player.stove_lv_content)
                                .image(&player.avatar_image)
                                .color(0x2b2d31); // Discord dark theme color
                            
                            new_message.channel_id.send_message(&ctx.http, serenity::CreateMessage::new().embed(embed)).await?;
                        },
                        Err(e) => {
                            new_message.reply(&ctx.http, format!("❌ Failed to fetch player data: {}", e)).await?;
                        }
                    }
                    return Ok(()); // Don't proceed to AI if we handled player lookup
                }
            }
        }

        // 2a. Handle Attachments (Pre-processing)
        // We need to modify new_message.content IF there are text attachments we want to read.
        // For Images, we'll handle them when constructing the AI payload.
        
        let mut final_user_content = new_message.content.clone();
        let mut image_attachments = Vec::new();
        let client = reqwest::Client::new(); // Re-use this? Or make a new one.

        for attachment in &new_message.attachments {
            // Check for Text
            if let Some(ctype) = &attachment.content_type {
                if ctype.starts_with("text/") || 
                   attachment.filename.ends_with(".rs") || 
                   attachment.filename.ends_with(".js") || 
                   attachment.filename.ends_with(".ts") || 
                   attachment.filename.ends_with(".json") || 
                   attachment.filename.ends_with(".md") || 
                   attachment.filename.ends_with(".txt") {
                       
                    // Limit text download size (e.g., 200KB)
                    if attachment.size < 200 * 1024 {
                        match client.get(&attachment.url).send().await {
                            Ok(resp) => {
                                if let Ok(text_content) = resp.text().await {
                                     final_user_content.push_str(&format!("\n\n[Attached File '{}']:\n```\n{}\n```", attachment.filename, text_content));
                                }
                            },
                            Err(e) => {
                                let _ = new_message.reply(&ctx.http, format!("⚠️ Failed to download attachment '{}': {}", attachment.filename, e)).await;
                            }
                        }
                    }
                } else if ctype.starts_with("image/") {
                    // It's an image, save for later
                     if attachment.size < 5 * 1024 * 1024 { // 5MB limit
                         image_attachments.push(attachment.url.clone());
                     }
                }
            }
        }


        // 3. Get author display name for message attribution
        let author_display_name = if let Some(gid) = new_message.guild_id {
            new_message.author.nick_in(&ctx.http, gid).await
                .unwrap_or(new_message.author.global_name.clone().unwrap_or(new_message.author.name.clone()))
        } else {
            new_message.author.global_name.clone().unwrap_or(new_message.author.name.clone())
        };

        // 4. Save User Message with author attribution (so AI knows who sent it)
        let attributed_content = format!("[{}]: {}", author_display_name, final_user_content);
        db::save_message(
            &guild_id,
            &channel_id,
            &new_message.author.id.to_string(),
            "user",
            &attributed_content,
        )?;

        // 4. Get Guild Config (Model, System Prompt)
        let guild_config = db::get_guild_config(&guild_id)?;

        // 5. Get History
        let user_id_str = new_message.author.id.to_string();
        let history = db::get_chat_history(
            &channel_id,
            if config.shared_chat { None } else { Some(&user_id_str) },
            20 // Context limit
        )?;

        // 6. Build Context & Messages for AI
        // Collect Mentions
        let mut context_info = String::from("\n[SYSTEM: ENTITY CONTEXT]\n");
        let mut has_context = false;

        // CURRENT MESSAGE AUTHOR (Always inject)
        {
            let author = &new_message.author;
            let author_display_name = if let Some(gid) = new_message.guild_id {
                author.nick_in(&ctx.http, gid).await
                    .unwrap_or(author.global_name.clone().unwrap_or(author.name.clone()))
            } else {
                author.global_name.clone().unwrap_or(author.name.clone())
            };
            context_info.push_str(&format!("\n[SYSTEM: CURRENT AUTHOR]\nThe user speaking to you now is: {} (ID: {})\nAddress them by their name: {}\n\n", 
                author_display_name, author.id, author_display_name));
        }

        // User Mentions
        if !new_message.mentions.is_empty() {
             has_context = true;
             context_info.push_str("Users:\n");
             for user in &new_message.mentions {
                 // Try to get nickname, fallback to global name, then username
                 let name = if let Some(gid) = new_message.guild_id {
                     user.nick_in(&ctx.http, gid).await.unwrap_or(user.global_name.clone().unwrap_or(user.name.clone()))
                 } else {
                     user.global_name.clone().unwrap_or(user.name.clone())
                 };
                 context_info.push_str(&format!("- @{}: <@{}>\n", name, user.id));
             }
        }

        // Role Mentions
        if !new_message.mention_roles.is_empty() {
             has_context = true;
             context_info.push_str("Roles:\n");
             // Resolve roles
             if let Some(gid) = new_message.guild_id {
                 if let Ok(roles) = gid.roles(&ctx.http).await {
                     for role_id in &new_message.mention_roles {
                         if let Some(role) = roles.get(role_id) {
                              context_info.push_str(&format!("- @{}: <@&{}>\n", role.name, role.id));
                         }
                     }
                 }
            }
        }

        // Channel Mentions (Parse from regex)
        // Regex to find <#12345>
        let channel_re = regex::Regex::new(r"<#(\d+)>").unwrap();
        let mut mentioned_channels = std::collections::HashSet::new();
        for cap in channel_re.captures_iter(&new_message.content) {
            if let Some(id_match) = cap.get(1) {
                mentioned_channels.insert(id_match.as_str().to_string());
            }
        }
        
        if !mentioned_channels.is_empty() {
            has_context = true;
            context_info.push_str("Channels:\n");
            for ch_id_str in mentioned_channels {
                 if let Ok(ch_id) = ch_id_str.parse::<u64>() {
                     // Try to get name from cache or simple generic name if fail
                     let name = if let Ok(ch) = serenity::ChannelId::new(ch_id).to_channel(&ctx.http).await {
                         ch.guild().map(|gc| gc.name).unwrap_or_else(|| "unknown-channel".to_string())
                     } else {
                         "unknown-channel".to_string()
                     };
                     context_info.push_str(&format!("- #{}: <#{}>\n", name, ch_id));
                 }
            }
        }

        // Referenced Message (Replies)
        if let Some(referenced) = &new_message.referenced_message {
            has_context = true;
            context_info.push_str("\n[SYSTEM: USER REPLYING TO]\n");
            context_info.push_str(&format!("User is replying to message by @{}:\n\"{}\"\n", 
                referenced.author.name, 
                referenced.content.replace("\n", " ")
            ));
        }

        // Add Command Instructions
        context_info.push_str("\n[SYSTEM: COMMANDS]\n");
        context_info.push_str("To send a message to a specific channel, output:\n");
        context_info.push_str("[[SEND:<#ChannelID>:Your Message Content]]\n");
        context_info.push_str("Example: [[SEND:<#12345>:Hello World]]\n");
        
        // Add Friendly Nickname Instructions
        context_info.push_str("\n[SYSTEM: FRIENDLY NICKNAMES]\n");
        context_info.push_str("When addressing users, use their friendly nicknames for a casual tone:\n");
        context_info.push_str("- Names containing 'chyaaa' or 'cyaaa' -> call them 'Cyaaa'\n");
        context_info.push_str("- Names containing 'kunnn' or 'kun' -> call them 'Kun'\n");
        context_info.push_str("- Names containing 'baemon' or 'mon' -> call them 'Mon'\n");
        context_info.push_str("- Names containing 'pecel' or 'lele' or 'cel' -> call them 'Cel'\n");
        context_info.push_str("- Names containing 'cylaa' or 'cyl' -> call them 'Cyl'\n");
        context_info.push_str("- Names containing 'dostzy' -> call them 'Dos'\n");
        context_info.push_str("- For other names, use a shortened friendly version (first part or nickname).\n");
        
        // Add Attachment Note
        if !image_attachments.is_empty() {
            context_info.push_str("\n[SYSTEM: IMAGE ATTACHED]\nUser has attached images to this message. Use your vision capabilities to analyze them.\n");
        }

        let mut messages = vec![
            json!({ "role": "system", "content": format!("{}{}", guild_config.system_prompt, context_info) })
        ];
        
        // Reconstruct history
        // If we have images, the LAST message (which corresponds to 'final_user_content' saved in DB)
        // needs to be replaced with a multimodal content block.
        
        let history_len = history.len();
        for (i, msg) in history.into_iter().enumerate() {
            // Check if this is the last message (the one we just added) AND we have images
            if i == history_len - 1 && !image_attachments.is_empty() && msg.role == "user" {
                // Construct Multimodal Message
                let mut content_parts = vec![
                    json!({ "type": "text", "text": msg.content })
                ];
                
                // Process images
                use base64::Engine as _;
                for img_url in &image_attachments {
                    let dl_result = client.get(img_url).send().await;
                    if let Ok(resp) = dl_result {
                        if let Ok(bytes) = resp.bytes().await {
                             let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                             // Determine mime type roughly or just use png/jpeg
                             let mime = if img_url.ends_with(".png") { "image/png" } else { "image/jpeg" };
                             
                             content_parts.push(json!({
                                 "type": "image_url",
                                 "image_url": {
                                     "url": format!("data:{};base64,{}", mime, b64)
                                 }
                             }));
                        }
                    }
                }
                
                messages.push(json!({ "role": msg.role, "content": content_parts }));
                
            } else {
                messages.push(json!({ "role": msg.role, "content": msg.content }));
            }
        }

        // 7. Call AI Proxy
        let proxy_state = data.proxy_state.instance.read().await;
        if let Some(instance) = proxy_state.as_ref() {
            let port = instance.config.port;
            // client already exists
            
            // Show typing indicator
            let _ = new_message.channel_id.broadcast_typing(&ctx.http).await;

            let resp = client.post(format!("http://127.0.0.1:{}/v1/chat/completions", port))
                .header("Authorization", "Bearer sk-antigravity") // Use a dummy or internal key
                .header("X-Max-Tier", "FREE") // Discord bot only uses FREE tier accounts
                .json(&json!({
                    "model": guild_config.chat_model,
                    "messages": messages
                }))
                .send()
                .await;

            match resp {
                Ok(response) => {
                    if response.status().is_success() {
                        let body: serde_json::Value = response.json().await?;
                        if let Some(mut content) = body["choices"][0]["message"]["content"].as_str().map(|s| s.to_string()) {
                            
                            // 8. Process Commands ([[SEND:<#ID>:Content]])
                            // Updated Regex to be permissive with spaces, allow channel names, and allow MULTI-LINE content ((?s))
                            let cmd_re = regex::Regex::new(r"(?s)\[\[SEND:\s*(.+?)\s*:\s*(.*?)\]\]").unwrap();
                            let mut actions_taken = Vec::new();

                            // Collect matches first to avoid borrowing issues
                            let mut commands = Vec::new();
                            for cap in cmd_re.captures_iter(&content) {
                                if let (Some(target_match), Some(msg_match)) = (cap.get(1), cap.get(2)) {
                                     commands.push((target_match.as_str().trim().to_string(), msg_match.as_str().to_string(), cap.get(0).unwrap().range()));
                                }
                            }

                            // Execute actions
                            for (target_ref, target_msg, range) in commands.iter().rev() {
                                let mut final_channel_id = None;

                                // 1. Try to parse as specific ID <#123>
                                // Regex to extract ID from <#123> or directly 123
                                let id_re = regex::Regex::new(r"^<#(\d+)>$|^(\d+)$").unwrap();
                                if let Some(cap) = id_re.captures(target_ref) {
                                     if let Some(id_m) = cap.get(1).or(cap.get(2)) {
                                         if let Ok(tid) = id_m.as_str().parse::<u64>() {
                                             final_channel_id = Some(serenity::ChannelId::new(tid));
                                         }
                                     }
                                }

                                // 2. If no ID, try to resolve by Name (if valid guild)
                                if final_channel_id.is_none() {
                                    // Clean the name (remove # if present)
                                    let clean_name = target_ref.trim_start_matches('#');
                                    
                                    // We need to fetch guild channels. This is expensive but necessary if AI fails to use ID.
                                    if let Some(gid) = new_message.guild_id {
                                        if let Ok(channels) = gid.channels(&ctx.http).await {
                                            // Case-insensitive match
                                            for (cid, ch_obj) in channels {
                                                if ch_obj.name.eq_ignore_ascii_case(clean_name) {
                                                    final_channel_id = Some(cid);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }

                                // 3. Resolve Mentions in Content (@Role -> <@&ID>, @User -> <@ID>)
                                // We mutable target_msg to replace text with IDs
                                let mut start_msg = target_msg.to_string(); // Assuming target_msg was cloned or is owned
                                
                                if start_msg.contains('@') {
                                    if let Some(gid) = new_message.guild_id {
                                        use serenity::Mentionable;

                                        // Unified Resolution: Use Cache if available, otherwise fallback to local context
                                        // FORMATS GENERATED:
                                        // User: <@USER_ID> (Standard) or <@!USER_ID> (Nickname - Legacy but supported)
                                        // Role: <@&ROLE_ID>
                                        // Channel: <#CHANNEL_ID>
                                        
                                        struct Replacement {
                                            pattern: String,
                                            value: String,
                                        }
                                        
                                        // 1. Try to get from Cache
                                        let guild_id_str = new_message.guild_id.map(|g| g.to_string()).unwrap_or_default();
                                        let cache_read = data.mention_cache.read().await;
                                        
                                        let cached_replacements = cache_read.get(&guild_id_str).map(|gc| gc.replacements.clone());
                                        drop(cache_read); // Release lock
                                        
                                        let final_replacements = if let Some(cached) = cached_replacements {
                                            // Use Cached Data (Already sorted)
                                            // We need to map our mod::Replacement to local struct if we didn't import it, 
                                            // but actually we can just iterate the cached one directly if we import it or map it.
                                            // Let's just Map it to be safe and simple.
                                            cached.into_iter().map(|r| Replacement { pattern: r.pattern, value: r.value }).collect()
                                        } else {
                                            // Fallback: Local Context (Mentions + Author + Roles if cheap)
                                            let mut local_generated = Vec::new();
                                            
                                            // A. Roles (Fetch fresh if no cache? Or skip for perf?) 
                                            // Let's fetch roles as it's usually not too heavy compared to 1000 members
                                            if let Ok(roles) = gid.roles(&ctx.http).await {
                                                for (role_id, role) in roles {
                                                    local_generated.push(Replacement {
                                                        pattern: format!("@{}", role.name),
                                                        value: role_id.mention().to_string(),
                                                    });
                                                }
                                            }

                                            // B. Users (Mentions + Author Only - Save Resources)
                                            let mut users_to_check = new_message.mentions.clone();
                                            users_to_check.push(new_message.author.clone());

                                            for user in users_to_check {
                                                let mut names = Vec::new();
                                                names.push(user.name.clone());
                                                if let Some(gn) = &user.global_name { names.push(gn.clone()); }
                                                if let Some(gid) = new_message.guild_id {
                                                    if let Some(nick) = user.nick_in(&ctx.http, gid).await {
                                                        names.push(nick);
                                                    }
                                                }
                                                let mention_str = user.mention().to_string();
                                                for name in names {
                                                    local_generated.push(Replacement {
                                                        pattern: format!("@{}", name),
                                                        value: mention_str.clone(),
                                                    });
                                                }
                                            }

                                            // C. Channels
                                             if let Ok(channels) = gid.channels(&ctx.http).await {
                                                 for (cid, ch_obj) in channels {
                                                     local_generated.push(Replacement {
                                                         pattern: format!("#{}", ch_obj.name),
                                                         value: cid.mention().to_string(),
                                                     });
                                                 }
                                             }
                                             
                                            // Sort
                                            local_generated.sort_by(|a, b| b.pattern.len().cmp(&a.pattern.len()));
                                            local_generated
                                        };

                                        // Execute Replacements
                                        for r in final_replacements {
                                             // Case-insensitive Regex with Word Boundary
                                             // Remove prefix (@ or #) from pattern for cleaner regex construction if needed, 
                                             // but pattern already has it.
                                             // Escape the pattern first
                                             
                                             // We want to match the literal pattern (e.g. "@Admin") case-insensitively.
                                             // Standard regex escape escapes the @ too which is fine.
                                             
                                             let escaped_pattern = regex::escape(&r.pattern);
                                             // We add boundary check \b at the end.
                                             // But for the start, since @/# are non-word chars, \b might not work as expected if preceded by space.
                                             // However, typically mentions are space-delimited.
                                             
                                             let regex_str = format!(r"(?i){}\b", escaped_pattern);
                                             
                                             if let Ok(re) = regex::Regex::new(&regex_str) {
                                                  start_msg = re.replace_all(&start_msg, r.value.as_str()).to_string();
                                             }
                                        }
                                    }
                                }
                                // Update target_msg with resolved content
                                let resolved_msg = start_msg;

                                if let Some(target_channel) = final_channel_id {
                                    match target_channel.say(&ctx.http, resolved_msg).await {
                                        Ok(_) => {
                                            actions_taken.push(format!("Message sent to <#{}>", target_channel));
                                        },
                                        Err(e) => {
                                            actions_taken.push(format!("Failed to send to <#{}>: {}", target_channel, e));
                                        }
                                    }
                                } else {
                                     actions_taken.push(format!("⚠️ Could not find channel '{}'", target_ref));
                                }
                                
                                // Remove command from content
                                content.replace_range(range.clone(), "");
                            }

                            // 9a. Apply Mention Resolution to Main Content
                            if content.contains('@') || content.contains('#') {
                                if let Some(gid) = new_message.guild_id {
                                    let guild_id_str = gid.to_string();
                                    let cache_read = data.mention_cache.read().await;
                                    
                                    if let Some(gc) = cache_read.get(&guild_id_str) {
                                        // Apply cached replacements
                                        for r in &gc.replacements {
                                            let escaped_pattern = regex::escape(&r.pattern);
                                            let regex_str = format!(r"(?i){}\b", escaped_pattern);
                                            if let Ok(re) = regex::Regex::new(&regex_str) {
                                                content = re.replace_all(&content, r.value.as_str()).to_string();
                                            }
                                        }
                                    }
                                    drop(cache_read);
                                }
                            }

                            let final_reply = content.trim();
                            
                            // 9. Reply to Discord
                            // If content is huge, use Embeds
                            
                            if !final_reply.is_empty() {
                                if final_reply.len() > 2000 {
                                    // Use Embeds
                                    let mut remaining = final_reply;
                                    while !remaining.is_empty() {
                                        // Embed description limit is 4096. Secure limit 4000.
                                        let split_idx = if remaining.len() > 4000 {
                                            let limit = 4000;
                                            remaining[..limit].rfind(['\n', ' ']).unwrap_or(limit)
                                        } else {
                                            remaining.len()
                                        };
                                        
                                        let (chunk, rest) = remaining.split_at(split_idx);
                                        
                                        // Create Embed
                                        let embed = serenity::CreateEmbed::new()
                                            .description(chunk)
                                            .color(0x3498db); // Nice blue
                                        
                                        new_message.channel_id.send_message(&ctx.http, serenity::CreateMessage::new().embed(embed)).await?;
                                        
                                        remaining = rest;
                                    }
                                } else {
                                    // Normal message
                                    new_message.reply(&ctx.http, final_reply).await?;
                                }
                            } else if actions_taken.is_empty() {
                                // If content is empty and no actions, maybe just send "Done" or nothing?
                                // Usually shouldn't happen unless AI only outputted command
                                new_message.reply(&ctx.http, "✅ Action processed.").await?;
                            }

                            if !actions_taken.is_empty() {
                                // Simplify response: If user asked to send, just say "Message sent"
                                // Unless there are errors
                                let has_errors = actions_taken.iter().any(|s| s.contains("Failed") || s.contains("Could not find"));
                                
                                if has_errors {
                                     let report = actions_taken.join("\n");
                                     new_message.reply(&ctx.http, format!("🤖 **System Report:**\n{}", report)).await?;
                                } else {
                                     // Success case - brief confirmation
                                     // We merge multiple successes if any
                                     new_message.reply(&ctx.http, "✅ Message sent.").await?;
                                }
                            }
                            
                            // 10. Save Assistant Message (Original content or Cleaned?)
                            // Saving cleaned content + actions report seems appropriate
                            let saved_content = if !actions_taken.is_empty() {
                                format!("{}\n[System Report: {}]", final_reply, actions_taken.join(", "))
                            } else {
                                final_reply.to_string()
                            };

                            db::save_message(
                                &guild_id,
                                &channel_id,
                                &ctx.cache.current_user().id.to_string(),
                                "assistant",
                                &saved_content,
                            )?;
                        }
                    } else {
                        new_message.reply(&ctx.http, "❌ Something went wrong with the bot. Please try again later.").await?;
                    }
                }
                Err(_e) => {
                    new_message.reply(&ctx.http, "❌ Something went wrong with the bot. Please try again later.").await?;
                }
            }
        } else {
            new_message.reply(&ctx.http, "❌ The AI service is currently unavailable. Please try again later.").await?;
        }
    }

    Ok(())
}

// Player Lookup Helpers
#[derive(Debug, serde::Deserialize)]
struct WosApiResponse {
    #[allow(dead_code)]
    code: i32,
    data: Option<PlayerData>,
    #[allow(dead_code)]
    msg: String,
    err_code: String,
}

#[derive(Debug, serde::Deserialize)]
struct PlayerData {
    fid: u64,
    nickname: String,
    kid: u32,
    stove_lv: u32,
    stove_lv_content: String,
    avatar_image: String,
    #[allow(dead_code)]
    total_recharge_amount: u32,
}

async fn fetch_player_data(fid: u64) -> Result<PlayerData, Box<dyn std::error::Error + Send + Sync>> {
    const SECRET: &str = "tB87#kPtkxqOS2";
    
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    
    let form_string = format!("fid={}&time={}", fid, current_time);
    let sign = format!("{:x}", md5::compute(format!("{}{}", form_string, SECRET)));
    
    let client = reqwest::Client::new();
    let response = client
        .post("https://wos-giftcode-api.centurygame.com/api/player")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Origin", "https://wos-giftcode.centurygame.com")
        .header("Referer", "https://wos-giftcode.centurygame.com/")
        .body(format!("sign={}&fid={}&time={}", sign, fid, current_time))
        .send()
        .await?;
    
    let api_response: WosApiResponse = response.json().await?;
    
    if api_response.err_code != "" {
        return Err("Player not found".into());
    }
    
    api_response.data.ok_or_else(|| "No player data returned".into())
}

fn get_stove_level_display(level: u32) -> String {
    match level {
        31 => "30-1".to_string(),
        32 => "30-2".to_string(),
        33 => "30-3".to_string(),
        34 => "30-4".to_string(),
        35 => "FC 1".to_string(),
        36 => "FC 1-1".to_string(),
        37 => "FC 1-2".to_string(),
        38 => "FC 1-3".to_string(),
        39 => "FC 1-4".to_string(),
        40 => "FC 2".to_string(),
        41 => "FC 2-1".to_string(),
        42 => "FC 2-2".to_string(),
        43 => "FC 2-3".to_string(),
        44 => "FC 2-4".to_string(),
        45 => "FC 3".to_string(),
        46 => "FC 3-1".to_string(),
        47 => "FC 3-2".to_string(),
        48 => "FC 3-3".to_string(),
        49 => "FC 3-4".to_string(),
        50 => "FC 4".to_string(),
        51 => "FC 4-1".to_string(),
        52 => "FC 4-2".to_string(),
        53 => "FC 4-3".to_string(),
        54 => "FC 4-4".to_string(),
        55 => "FC 5".to_string(),
        56 => "FC 5-1".to_string(),
        57 => "FC 5-2".to_string(),
        58 => "FC 5-3".to_string(),
        59 => "FC 5-4".to_string(),
        60 => "FC 6".to_string(),
        61 => "FC 6-1".to_string(),
        62 => "FC 6-2".to_string(),
        63 => "FC 6-3".to_string(),
        64 => "FC 6-4".to_string(),
        65 => "FC 7".to_string(),
        66 => "FC 7-1".to_string(),
        67 => "FC 7-2".to_string(),
        68 => "FC 7-3".to_string(),
        69 => "FC 7-4".to_string(),
        70 => "FC 8".to_string(),
        71 => "FC 8-1".to_string(),
        72 => "FC 8-2".to_string(),
        73 => "FC 8-3".to_string(),
        74 => "FC 8-4".to_string(),
        75 => "FC 9".to_string(),
        76 => "FC 9-1".to_string(),
        77 => "FC 9-2".to_string(),
        78 => "FC 9-3".to_string(),
        79 => "FC 9-4".to_string(),
        80 => "FC 10".to_string(),
        81 => "FC 10-1".to_string(),
        82 => "FC 10-2".to_string(),
        83 => "FC 10-3".to_string(),
        84 => "FC 10-4".to_string(),
        _ => format!("Level {}", level),
    }
}
