use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use zenne_core::AppContext;
use zenne_games::{baucua, taixiu, GameHub};

pub async fn run_twitch_bot(ctx: AppContext, games: GameHub) {
    let token = ctx.config.twitch_oauth_token.trim();
    if token.is_empty() {
        info!("[Twitch] Chua cau hinh TWITCH_OAUTH_TOKEN (Bo qua).");
        return;
    }

    let nick = ctx.config.twitch_bot_nick.to_lowercase();
    let pass = if token.starts_with("oauth:") {
        token.to_string()
    } else {
        format!("oauth:{}", token)
    };
    let prefix = ctx.config.twitch_command_prefix.clone();
    let channels = ctx.config.twitch_channels.clone();

    loop {
        info!("[Twitch] Dang ket noi IRC WebSocket toi Twitch...");
        match connect_async("wss://irc-ws.chat.twitch.tv:443").await {
            Ok((ws, _)) => {
                let (mut write, mut read) = ws.split();
                let _ = write.send(Message::Text(format!("PASS {}", pass))).await;
                let _ = write.send(Message::Text(format!("NICK {}", nick))).await;
                for ch in &channels {
                    let _ = write.send(Message::Text(format!("JOIN #{}", ch))).await;
                }
                info!(
                    "[Twitch] Da ket noi thanh cong vao cac kenh: {:?}",
                    channels
                );

                while let Some(msg) = read.next().await {
                    let Ok(Message::Text(line)) = msg else {
                        continue;
                    };
                    for raw in line.lines() {
                        if raw.starts_with("PING") {
                            let _ = write
                                .send(Message::Text("PONG :tmi.twitch.tv".to_string()))
                                .await;
                            continue;
                        }
                        if let Some((channel, user, content)) = parse_privmsg(raw) {
                            if !content.starts_with(&prefix) {
                                continue;
                            }
                            let cmd_line = content[prefix.len()..].trim();
                            let mut parts = cmd_line.splitn(2, ' ');
                            let cmd = parts.next().unwrap_or("").to_lowercase();
                            let arg = parts.next().unwrap_or("").trim();

                            let reply = match cmd.as_str() {
                                "help" => {
                                    "Zenne Rust Bot: !taixiu, !baucua, !vtv, !quiz, !ask <câu hỏi>"
                                        .to_string()
                                }
                                "taixiu" | "tx" => {
                                    let r = taixiu::roll_taixiu();
                                    let kq = if r.is_triple {
                                        "BỘ BA"
                                    } else if r.is_tai {
                                        "TÀI"
                                    } else {
                                        "XỈU"
                                    };
                                    format!(
                                        "@{} 🎲 Xúc xắc: {}-{}-{} = {} ({})",
                                        user, r.dice[0], r.dice[1], r.dice[2], r.total, kq
                                    )
                                }
                                "baucua" | "bc" => {
                                    let r = baucua::roll_baucua();
                                    format!(
                                        "@{} 🦀 Kết quả: {} {} | {} {} | {} {}",
                                        user,
                                        r[0].display().0,
                                        r[0].display().1,
                                        r[1].display().0,
                                        r[1].display().1,
                                        r[2].display().0,
                                        r[2].display().1
                                    )
                                }
                                "vtv" => {
                                    let s = games.vuatiengviet.start(0);
                                    format!(
                                        "@{} 👑 Vua Tiếng Việt: Ghép chữ -> [ {} ]",
                                        user, s.scrambled
                                    )
                                }
                                "quiz" => {
                                    let q = games.quiz.start_general(0);
                                    format!(
                                        "@{} ❓ [{}] {} | A: {} | B: {} | C: {} | D: {}",
                                        user,
                                        q.category,
                                        q.question,
                                        q.options[0],
                                        q.options[1],
                                        q.options[2],
                                        q.options[3]
                                    )
                                }
                                "ask" if !arg.is_empty() => zenne_core::kira::ask_ai_kira(
                                    &ctx.http,
                                    &ctx.config.kira_api_endpoint,
                                    &ctx.config.kira_api_key,
                                    &ctx.config.kira_default_model,
                                    arg,
                                    None,
                                )
                                .await
                                .unwrap_or_else(|e| format!("Lỗi AI: {}", e)),
                                _ => continue,
                            };

                            let _ = write
                                .send(Message::Text(format!("PRIVMSG #{} :{}", channel, reply)))
                                .await;
                        }
                    }
                }
            }
            Err(e) => error!("[Twitch] Loi ket noi: {}", e),
        }
        warn!("[Twitch] Mat ket noi, thu lai sau 5 giay...");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

fn parse_privmsg(raw: &str) -> Option<(String, String, String)> {
    let idx = raw.find(" PRIVMSG #")?;
    let user = raw.strip_prefix(':')?.split('!').next()?.to_string();
    let rest = &raw[idx + " PRIVMSG #".len()..];
    let (ch, msg) = rest.split_once(" :")?;
    Some((ch.to_string(), user, msg.trim().to_string()))
}
