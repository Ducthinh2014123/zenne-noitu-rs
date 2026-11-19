use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use zenne_core::AppContext;
use zenne_games::{baucua, taixiu, GameHub};

pub async fn run_revolt_bot(ctx: AppContext, games: GameHub) {
    let token = ctx.config.revolt_bot_token.trim().to_string();
    if token.is_empty() {
        info!("[Revolt] Chua cau hinh REVOLT_BOT_TOKEN (Bo qua).");
        return;
    }
    let prefix = ctx.config.revolt_command_prefix.clone();

    loop {
        info!("[Revolt] Dang ket noi WebSocket toi Revolt (Stoat)...");
        match connect_async("wss://ws.revolt.chat").await {
            Ok((ws, _)) => {
                let (mut write, mut read) = ws.split();
                let auth_msg = json!({ "type": "Authenticate", "token": token });
                let _ = write.send(Message::Text(auth_msg.to_string())).await;
                let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));

                loop {
                    let msg = tokio::select! {
                        _ = ping_interval.tick() => {
                            let ping = json!({ "type": "Ping", "data": 0 });
                            if write.send(Message::Text(ping.to_string())).await.is_err() {
                                break;
                            }
                            continue;
                        }
                        m = read.next() => match m {
                            Some(x) => x,
                            None => break,
                        }
                    };
                    let Ok(Message::Text(txt)) = msg else {
                        continue;
                    };
                    let Ok(ev) = serde_json::from_str::<Value>(&txt) else {
                        continue;
                    };
                    match ev["type"].as_str() {
                        Some("Ready") => info!("[Revolt] Bot da san sang!"),
                        Some("Message") => {
                            let Some(content) = ev["content"].as_str() else {
                                continue;
                            };
                            let Some(channel_id) = ev["channel"].as_str() else {
                                continue;
                            };
                            if !content.starts_with(&prefix) {
                                continue;
                            }
                            let cmd_line = content[prefix.len()..].trim();
                            let mut parts = cmd_line.splitn(2, ' ');
                            let cmd = parts.next().unwrap_or("").to_lowercase();
                            let arg = parts.next().unwrap_or("").trim();

                            let reply = match cmd.as_str() {
                                "help" => "🤖 **Zenne Rust Bot (Revolt)**: `!taixiu`, `!baucua`, `!vtv`, `!quiz`, `!ask <câu hỏi>`".to_string(),
                                "taixiu" | "tx" => {
                                    let r = taixiu::roll_taixiu();
                                    let kq = if r.is_triple { "BỘ BA" } else if r.is_tai { "TÀI" } else { "XỈU" };
                                    format!("🎲 Xúc xắc: **{} - {} - {}** = **{}** (**{}**)", r.dice[0], r.dice[1], r.dice[2], r.total, kq)
                                }
                                "baucua" | "bc" => {
                                    let r = baucua::roll_baucua();
                                    format!("🦀 Kết quả: {} {} | {} {} | {} {}", r[0].display().0, r[0].display().1, r[1].display().0, r[1].display().1, r[2].display().0, r[2].display().1)
                                }
                                "vtv" => {
                                    let s = games.vuatiengviet.start(1);
                                    format!("👑 **Vua Tiếng Việt**: Ghép chữ -> `{}`", s.scrambled)
                                }
                                "quiz" => {
                                    let q = games.quiz.start_general(1);
                                    format!("❓ **[{}]** {}\nA. {}\nB. {}\nC. {}\nD. {}", q.category, q.question, q.options[0], q.options[1], q.options[2], q.options[3])
                                }
                                "ask" if !arg.is_empty() => {
                                    zenne_core::kira::ask_ai_kira(
                                        &ctx.http,
                                        &ctx.config.kira_api_endpoint,
                                        &ctx.config.kira_api_key,
                                        &ctx.config.kira_default_model,
                                        arg,
                                        None,
                                    )
                                    .await
                                    .unwrap_or_else(|e| format!("Lỗi AI: {}", e))
                                }
                                _ => continue,
                            };

                            let url =
                                format!("https://api.revolt.chat/channels/{}/messages", channel_id);
                            let _ = ctx
                                .http
                                .post(&url)
                                .header("x-bot-token", &token)
                                .json(&json!({ "content": reply }))
                                .send()
                                .await;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => error!("[Revolt] Loi ket noi: {}", e),
        }
        warn!("[Revolt] Mat ket noi, thu lai sau 5 giay...");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}
