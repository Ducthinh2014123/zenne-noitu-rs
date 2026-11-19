use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use zenne_core::{wiki, AppContext};
use zenne_games::{baucua, taixiu, GameHub};

#[derive(Clone, Default)]
struct RevoltNoituRoom {
    active: bool,
    solo: bool,
    lang: String,
    current_word: String,
    last_syl: String,
    used: HashSet<String>,
    scores: HashMap<String, i64>,
}

fn ch_id_hash(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

async fn send_revolt_msg(http: &reqwest::Client, token: &str, channel_id: &str, content: &str) {
    let url = format!("https://api.revolt.chat/channels/{}/messages", channel_id);
    let _ = http
        .post(&url)
        .header("x-bot-token", token)
        .json(&json!({ "content": content }))
        .send()
        .await;
}

pub async fn run_revolt_bot(ctx: AppContext, games: GameHub) {
    let token = ctx.config.revolt_bot_token.trim().to_string();
    if token.is_empty() {
        info!("[Revolt] Chua cau hinh REVOLT_BOT_TOKEN (Bo qua).");
        return;
    }
    let prefix = ctx.config.revolt_command_prefix.clone();
    let rooms: DashMap<String, RevoltNoituRoom> = DashMap::new();

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
                            if ev["author"].as_str().is_none() {
                                continue;
                            }
                            let author_id = ev["author"].as_str().unwrap_or("user").to_string();
                            let ch_hash = ch_id_hash(channel_id);

                            if !content.starts_with(&prefix) {
                                if let Some((true, ans)) = games.vuatiengviet.check_answer(ch_hash, content) {
                                    send_revolt_msg(&ctx.http, &token, channel_id, &format!("🎉 Chính xác VTV: **{}**!", ans)).await;
                                    continue;
                                }
                                if let Some((true, ans)) = games.cadao.check_answer(ch_hash, content) {
                                    send_revolt_msg(&ctx.http, &token, channel_id, &format!("🌾 Chính xác Ca Dao: **{}**!", ans)).await;
                                    continue;
                                }
                                if let Some((true, ans)) = games.chinhta.check_answer(ch_hash, content) {
                                    send_revolt_msg(&ctx.http, &token, channel_id, &format!("✍️ Chuẩn chính tả: **{}**!", ans)).await;
                                    continue;
                                }
                                if let Some((true, ans)) = games.flag_quiz.check_answer(ch_hash, content) {
                                    send_revolt_msg(&ctx.http, &token, channel_id, &format!("🌍 Chính xác Quốc kỳ: **{}**!", ans)).await;
                                    continue;
                                }
                                let upper = content.trim().to_uppercase();
                                if matches!(upper.as_str(), "A" | "B" | "C" | "D") {
                                    if let Some((ok, right_ans)) = games.quiz.answer(ch_hash, &upper) {
                                        let msg_txt = if ok {
                                            format!("✅ Chính xác! Đáp án: **{}**", right_ans)
                                        } else {
                                            format!("❌ Sai rồi! Đáp án đúng là **{}**.", right_ans)
                                        };
                                        send_revolt_msg(&ctx.http, &token, channel_id, &msg_txt).await;
                                        continue;
                                    }
                                }

                                if let Some(r) = rooms.get(channel_id) {
                                    if !r.active {
                                        continue;
                                    }
                                    let word = wiki::normalize_word(content);
                                    if word.is_empty() || !wiki::valid_chain(&r.current_word, &word, &r.lang) {
                                        continue;
                                    }
                                    if r.used.contains(&word.to_lowercase()) {
                                        send_revolt_msg(&ctx.http, &token, channel_id, &format!("🔁 Từ `{}` đã dùng rồi!", word)).await;
                                        continue;
                                    }
                                    let lang = r.lang.clone();
                                    drop(r);
                                    let (ok, _) = wiki::check_word(&ctx.http, &ctx.db, &ctx.cache, 0, &lang, &word).await;
                                    if ok {
                                        if let Some(mut r) = rooms.get_mut(channel_id) {
                                            r.used.insert(word.to_lowercase());
                                            r.current_word = word.clone();
                                            r.last_syl = wiki::last_syl(&word, &r.lang);
                                            *r.scores.entry(author_id).or_insert(0) += 1;
                                            let next_syl = r.last_syl.clone();
                                            let solo = r.solo;
                                            let used_vec: Vec<String> = r.used.iter().cloned().collect();
                                            drop(r);
                                            let mut reply = format!("✅ `{}` (+1đ) | Nối tiếp: **{}**", word, next_syl);
                                            if solo {
                                                let sugs = wiki::find_continuations(&ctx.db, 0, &lang, &next_syl, &used_vec, 5).await;
                                                if let Some(bw) = sugs.first() {
                                                    if let Some(mut r) = rooms.get_mut(channel_id) {
                                                        r.used.insert(bw.to_lowercase());
                                                        r.current_word = bw.clone();
                                                        r.last_syl = wiki::last_syl(bw, &r.lang);
                                                        reply = format!("✅ `{}` (+1đ) | 🤖 Bot nối: `{}` -> Tiếp theo: **{}**", word, bw, r.last_syl);
                                                    }
                                                }
                                            }
                                            send_revolt_msg(&ctx.http, &token, channel_id, &reply).await;
                                        }
                                    }
                                }
                                continue;
                            }

                            let cmd_line = content[prefix.len()..].trim();
                            let mut parts = cmd_line.splitn(2, ' ');
                            let cmd = parts.next().unwrap_or("").to_lowercase();
                            let arg = parts.next().unwrap_or("").trim();

                            let reply = match cmd.as_str() {
                                "help" => "🤖 **Zenne Bot (Revolt)**: `!noitu [solo]`, `!stop`, `!diem`, `!tratu`, `!contukhong`, `!taixiu [tai|xiu]`, `!baucua`, `!vtv`, `!quiz`, `!cadao`, `!chinhta`, `!flag`, `!ask`".to_string(),
                                "noitu" => {
                                    let solo = arg.eq_ignore_ascii_case("solo");
                                    let lang = "vi".to_string();
                                    let first = wiki::random_word(&ctx.http, &ctx.db, &lang).await;
                                    let syl = wiki::last_syl(&first, &lang);
                                    let mut used = HashSet::new();
                                    used.insert(first.to_lowercase());
                                    rooms.insert(channel_id.to_string(), RevoltNoituRoom {
                                        active: true,
                                        solo,
                                        lang,
                                        current_word: first.clone(),
                                        last_syl: syl.clone(),
                                        used,
                                        scores: HashMap::new(),
                                    });
                                    format!("🎯 **Bắt đầu Nối Từ{}!** Từ đầu: `{}` -> Nối với **{}**", if solo { " (Solo Bot)" } else { "" }, first, syl)
                                }
                                "stop" | "dung" => {
                                    rooms.remove(channel_id);
                                    "🛑 Đã dừng ván Nối Từ!".to_string()
                                }
                                "diem" | "score" => {
                                    if let Some(r) = rooms.get(channel_id) {
                                        let s: Vec<String> = r.scores.iter().map(|(u, p)| format!("{}: **{}**", u, p)).collect();
                                        if s.is_empty() { "📊 Chưa ai ghi điểm.".to_string() } else { format!("📊 Điểm: {}", s.join(" | ")) }
                                    } else { "❌ Chưa có ván nối từ nào.".to_string() }
                                }
                                "tratu" | "check" if !arg.is_empty() => {
                                    let (ok, _) = wiki::check_word(&ctx.http, &ctx.db, &ctx.cache, 0, "vi", arg).await;
                                    if ok { format!("✅ Từ `{}` **HỢP LỆ**!", arg) } else { format!("❌ Không tìm thấy `{}`.", arg) }
                                }
                                "contukhong" | "ctk" if !arg.is_empty() => {
                                    let sugs = wiki::find_continuations(&ctx.db, 0, "vi", arg, &[], 5).await;
                                    if sugs.is_empty() { format!("⚠️ Không tìm thấy từ nối cho `{}`.", arg) } else { format!("✅ Gợi ý cho `{}`: {}", arg, sugs.join(", ")) }
                                }
                                "taixiu" | "tx" => {
                                    let r = taixiu::roll_taixiu();
                                    let kq = if r.is_triple { "BỘ BA" } else if r.is_tai { "TÀI" } else { "XỈU" };
                                    let bet = arg.to_lowercase();
                                    let bonus = if (bet == "tai" && r.is_tai) || (bet == "xiu" && !r.is_tai && !r.is_triple) { " — 🎉 **THẮNG!**" } else if !bet.is_empty() { " — 😅 Thua rồi!" } else { "" };
                                    format!("🎲 Xúc xắc: **{} - {} - {}** = **{}** (**{}**){}", r.dice[0], r.dice[1], r.dice[2], r.total, kq, bonus)
                                }
                                "baucua" | "bc" => {
                                    let r = baucua::roll_baucua();
                                    format!("🦀 Kết quả: {} {} | {} {} | {} {}", r[0].display().0, r[0].display().1, r[1].display().0, r[1].display().1, r[2].display().0, r[2].display().1)
                                }
                                "vtv" => {
                                    let s = games.vuatiengviet.start(ch_hash);
                                    format!("👑 **Vua Tiếng Việt**: Ghép chữ -> `{}` (gõ đáp án vào chat!)", s.scrambled)
                                }
                                "quiz" => {
                                    let q = games.quiz.start_general(ch_hash);
                                    format!("❓ **[{}]** {}\nA. {}\nB. {}\nC. {}\nD. {}\n*(Nhắn A/B/C/D để trả lời)*", q.category, q.question, q.options[0], q.options[1], q.options[2], q.options[3])
                                }
                                "cadao" => {
                                    let s = games.cadao.start(ch_hash);
                                    format!("🌾 **Ca dao**: *\"{}\"* (gõ từ còn thiếu)", s.prompt)
                                }
                                "chinhta" => {
                                    let s = games.chinhta.start(ch_hash);
                                    format!("✍️ Từ nào đúng chính tả? **{}** hay **{}**", s.opt_a, s.opt_b)
                                }
                                "flag" => {
                                    let (flag, _) = games.flag_quiz.start(ch_hash);
                                    format!("🌍 Quốc kỳ {} là của nước nào?", flag)
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

                            send_revolt_msg(&ctx.http, &token, channel_id, &reply).await;
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
