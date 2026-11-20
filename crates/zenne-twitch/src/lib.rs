use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use zenne_core::{wiki, AppContext};
use zenne_games::{baucua, taixiu, GameHub};

#[derive(Clone, Default)]
struct TwitchNoituRoom {
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
    let rooms: DashMap<String, TwitchNoituRoom> = DashMap::new();

    loop {
        info!("[Twitch] Dang ket noi IRC WebSocket toi Twitch...");
        match connect_async("wss://irc-ws.chat.twitch.tv:443").await {
            Ok((ws, _)) => {
                let (mut write, mut read) = ws.split();
                let _ = write
                    .send(Message::Text(
                        "CAP REQ :twitch.tv/tags twitch.tv/commands".to_string(),
                    ))
                    .await;
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
                        let Some((channel, user, content)) = parse_privmsg(raw) else {
                            continue;
                        };
                        if user.eq_ignore_ascii_case(&nick) {
                            continue;
                        }
                        let ch_hash = ch_id_hash(&channel);

                        if !content.starts_with(&prefix) {
                            // Check active VTV / Cadao / Chinhta / Flag / Quiz / Word-chain
                            if let Some((true, ans)) =
                                games.vuatiengviet.check_answer(ch_hash, &content)
                            {
                                let reply = format!("@{} 🎉 Đúng rồi! Đáp án VTV: {}", user, ans);
                                let _ = write
                                    .send(Message::Text(format!("PRIVMSG #{} :{}", channel, reply)))
                                    .await;
                                continue;
                            }
                            if let Some((true, ans)) = games.cadao.check_answer(ch_hash, &content) {
                                let reply = format!("@{} 🌾 Chính xác Ca Dao: {}", user, ans);
                                let _ = write
                                    .send(Message::Text(format!("PRIVMSG #{} :{}", channel, reply)))
                                    .await;
                                continue;
                            }
                            if let Some((true, ans)) = games.chinhta.check_answer(ch_hash, &content)
                            {
                                let reply = format!("@{} ✍️ Chuẩn chính tả: {}", user, ans);
                                let _ = write
                                    .send(Message::Text(format!("PRIVMSG #{} :{}", channel, reply)))
                                    .await;
                                continue;
                            }
                            if let Some((true, ans)) =
                                games.flag_quiz.check_answer(ch_hash, &content)
                            {
                                let reply = format!("@{} 🌍 Chính xác Quốc kỳ: {}", user, ans);
                                let _ = write
                                    .send(Message::Text(format!("PRIVMSG #{} :{}", channel, reply)))
                                    .await;
                                continue;
                            }
                            let upper = content.trim().to_uppercase();
                            if matches!(upper.as_str(), "A" | "B" | "C" | "D") {
                                if let Some((ok, right_ans)) = games.quiz.answer(ch_hash, &upper) {
                                    let reply = if ok {
                                        format!("@{} ✅ Chính xác! Đáp án: {}", user, right_ans)
                                    } else {
                                        format!(
                                            "@{} ❌ Sai rồi! Đáp án đúng là: {}",
                                            user, right_ans
                                        )
                                    };
                                    let _ = write
                                        .send(Message::Text(format!(
                                            "PRIVMSG #{} :{}",
                                            channel, reply
                                        )))
                                        .await;
                                    continue;
                                }
                            }

                            // Word-chain gameplay
                            if let Some(r) = rooms.get_mut(&channel) {
                                if !r.active {
                                    continue;
                                }
                                let word = wiki::normalize_word(&content);
                                if word.is_empty()
                                    || !wiki::valid_chain(&r.current_word, &word, &r.lang)
                                {
                                    continue;
                                }
                                if r.used.contains(&word.to_lowercase()) {
                                    let _ = write
                                        .send(Message::Text(format!(
                                            "PRIVMSG #{} :@{} 🔁 Từ '{}' đã dùng rồi!",
                                            channel, user, word
                                        )))
                                        .await;
                                    continue;
                                }
                                let lang = r.lang.clone();
                                drop(r);
                                let (ok, _) = wiki::check_word(
                                    &ctx.http, &ctx.db, &ctx.cache, 0, &lang, &word,
                                )
                                .await;
                                if ok {
                                    if let Some(mut r) = rooms.get_mut(&channel) {
                                        r.used.insert(word.to_lowercase());
                                        r.current_word = word.clone();
                                        r.last_syl = wiki::last_syl(&word, &r.lang);
                                        *r.scores.entry(user.clone()).or_insert(0) += 1;
                                        let next_syl = r.last_syl.clone();
                                        let solo = r.solo;
                                        let used_vec: Vec<String> =
                                            r.used.iter().cloned().collect();
                                        drop(r);
                                        let mut reply = format!(
                                            "@{} ✅ '{}' (+1đ) | Nối tiếp: '{}'",
                                            user, word, next_syl
                                        );
                                        if solo {
                                            let sugs = wiki::find_continuations(
                                                &ctx.db, 0, &lang, &next_syl, &used_vec, 5,
                                            )
                                            .await;
                                            if let Some(bw) = sugs.first() {
                                                if let Some(mut r) = rooms.get_mut(&channel) {
                                                    r.used.insert(bw.to_lowercase());
                                                    r.current_word = bw.clone();
                                                    r.last_syl = wiki::last_syl(bw, &r.lang);
                                                    reply = format!("@{} ✅ '{}' (+1đ) | 🤖 Bot nối: '{}' -> Tiếp theo: '{}'", user, word, bw, r.last_syl);
                                                }
                                            }
                                        }
                                        let _ = write
                                            .send(Message::Text(format!(
                                                "PRIVMSG #{} :{}",
                                                channel, reply
                                            )))
                                            .await;
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
                            "help" => {
                                "Zenne Bot: !noitu [solo], !stop, !diem, !tratu, !contukhong, !taixiu [tai|xiu], !baucua [con], !vtv, !quiz, !cadao, !chinhta, !flag, !ask".to_string()
                            }
                            "noitu" => {
                                let solo = arg.eq_ignore_ascii_case("solo");
                                let lang = "vi".to_string();
                                let first = wiki::random_word(&ctx.http, &ctx.db, &lang).await;
                                let syl = wiki::last_syl(&first, &lang);
                                let mut used = HashSet::new();
                                used.insert(first.to_lowercase());
                                rooms.insert(channel.clone(), TwitchNoituRoom {
                                    active: true,
                                    solo,
                                    lang,
                                    current_word: first.clone(),
                                    last_syl: syl.clone(),
                                    used,
                                    scores: HashMap::new(),
                                });
                                format!("🎯 Bắt đầu Nối Từ{}! Từ đầu: '{}' -> Nối với '{}'", if solo { " (Solo Bot)" } else { "" }, first, syl)
                            }
                            "stop" | "dung" => {
                                rooms.remove(&channel);
                                "🛑 Đã dừng ván Nối Từ!".to_string()
                            }
                            "diem" | "score" => {
                                if let Some(r) = rooms.get(&channel) {
                                    let s: Vec<String> = r.scores.iter().map(|(u, p)| format!("{}: {}", u, p)).collect();
                                    if s.is_empty() { "📊 Chưa ai ghi điểm.".to_string() } else { format!("📊 Điểm: {}", s.join(" | ")) }
                                } else { "❌ Chưa có ván nối từ nào.".to_string() }
                            }
                            "top" | "bxh" | "leaderboard" => {
                                let rows = ctx.db.lb_get_top(0, 5).await;
                                if rows.is_empty() {
                                    "🏆 Chưa có bảng xếp hạng.".to_string()
                                } else {
                                    let s: Vec<String> = rows.iter().enumerate().map(|(i, r)| format!("{}. ID {}: {} từ", i + 1, r.user_id, r.total_words)).collect();
                                    format!("🏆 Top: {}", s.join(" | "))
                                }
                            }
                            "taophong" | "create" => {
                                let rid = format!("{:06X}", ch_hash & 0xFFFFFF);
                                let name = if arg.is_empty() { format!("Phòng Twitch {}", user) } else { arg.to_string() };
                                let url = format!("{}/pub/play/{}", ctx.config.web_base_url.trim_end_matches('/'), rid);
                                ctx.db.create_public_room(&zenne_core::db::PublicRoomRow {
                                    room_id: rid.clone(),
                                    name: name.clone(),
                                    platform: "twitch".to_string(),
                                    host_id: user.clone(),
                                    host_name: user.clone(),
                                    lang: "vi".to_string(),
                                    max_players: 0,
                                    current_players: 1,
                                    join_token: None,
                                    web_url: Some(url.clone()),
                                    tele_url: None,
                                    status: "waiting".to_string(),
                                }).await;
                                format!("🌐 Tạo phòng '{}' [{}]: {}", name, rid, url)
                            }
                            "phong" | "rooms" => {
                                let list = ctx.db.get_public_rooms().await;
                                if list.is_empty() {
                                    "🌐 Hiện chưa có phòng công khai nào.".to_string()
                                } else {
                                    let s: Vec<String> = list.iter().take(5).map(|r| format!("[{}] {}", r.room_id, r.name)).collect();
                                    format!("🌐 Phòng đang mở: {}", s.join(" | "))
                                }
                            }
                            "tratu" | "check" if !arg.is_empty() => {
                                let (ok, _) = wiki::check_word(&ctx.http, &ctx.db, &ctx.cache, 0, "vi", arg).await;
                                if ok { format!("@{} ✅ Từ '{}' HỢP LỆ!", user, arg) } else { format!("@{} ❌ Không tìm thấy '{}'.", user, arg) }
                            }
                            "contukhong" | "ctk" if !arg.is_empty() => {
                                let sugs = wiki::find_continuations(&ctx.db, 0, "vi", arg, &[], 5).await;
                                if sugs.is_empty() { format!("@{} ⚠️ Không tìm thấy từ nối cho '{}'.", user, arg) } else { format!("@{} ✅ Gợi ý cho '{}': {}", user, arg, sugs.join(", ")) }
                            }
                            "taixiu" | "tx" => {
                                let r = taixiu::roll_taixiu();
                                let kq = if r.is_triple { "BỘ BA" } else if r.is_tai { "TÀI" } else { "XỈU" };
                                let bet = arg.to_lowercase();
                                let bonus = if (bet == "tai" && r.is_tai) || (bet == "xiu" && !r.is_tai && !r.is_triple) { " — 🎉 BẠN THẮNG!" } else if !bet.is_empty() { " — 😅 Thua rồi!" } else { "" };
                                format!("@{} 🎲 {}-{}-{} = {} ({}){}", user, r.dice[0], r.dice[1], r.dice[2], r.total, kq, bonus)
                            }
                            "baucua" | "bc" => {
                                let r = baucua::roll_baucua();
                                format!("@{} 🦀 {} {} | {} {} | {} {}", user, r[0].display().0, r[0].display().1, r[1].display().0, r[1].display().1, r[2].display().0, r[2].display().1)
                            }
                            "vtv" => {
                                let s = games.vuatiengviet.start(ch_hash);
                                format!("@{} 👑 Vua Tiếng Việt: Ghép chữ -> [ {} ] (gõ đáp án vào chat!)", user, s.scrambled)
                            }
                            "quiz" => {
                                let q = games.quiz.start_general(ch_hash);
                                format!("@{} ❓ [{}] {} | A: {} | B: {} | C: {} | D: {} (nhắn A/B/C/D)", user, q.category, q.question, q.options[0], q.options[1], q.options[2], q.options[3])
                            }
                            "cadao" => {
                                let s = games.cadao.start(ch_hash);
                                format!("@{} 🌾 Ca dao: \"{}\" (gõ từ còn thiếu)", user, s.prompt)
                            }
                            "chinhta" => {
                                let s = games.chinhta.start(ch_hash);
                                format!("@{} ✍️ Từ nào đúng chính tả? '{}' hay '{}'", user, s.opt_a, s.opt_b)
                            }
                            "flag" => {
                                let (flag, _) = games.flag_quiz.start(ch_hash);
                                format!("@{} 🌍 Quốc kỳ {} là của nước nào?", user, flag)
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
