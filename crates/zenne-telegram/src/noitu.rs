use crate::minigames;
use crate::state::{
    gen_join_token, gen_room_code, html_escape, user_display_name, user_mention, MpWizardStep,
    NoituRoomState, TelePlayer, TelegramState,
};
use rand::seq::SliceRandom;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use teloxide::payloads::{AnswerCallbackQuerySetters, EditMessageTextSetters, SendMessageSetters};
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
use tokio::sync::Mutex;
use url::Url;
use zenne_core::db::PublicRoomRow;
use zenne_core::wiki;

fn build_lobby_text(game: &NoituRoomState, turn_timeout: u64) -> String {
    let players_str = game
        .players
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{}. {}", i + 1, user_mention(p.id, &p.name)))
        .collect::<Vec<_>>()
        .join("\n");
    let mode_str = if game.is_bot_mode {
        "🤖 Solo với Bot"
    } else {
        "👥 Nhiều người chơi"
    };
    let max_str = if game.max_players <= 0 {
        "Vô hạn (∞)".to_string()
    } else {
        format!("{} người", game.max_players)
    };

    format!(
        "🎮 <b>PHÒNG CHỜ NỐI TỪ WIKIPEDIA</b> 🎮\n\n\
        👑 <b>Chủ phòng:</b> {}\n\
        🏷️ <b>Mã phòng:</b> <code>[{}]</code>\n\
        🌐 <b>Ngôn ngữ:</b> <code>{}</code>\n\
        🕹️ <b>Chế độ:</b> {}\n\
        👥 <b>Giới hạn:</b> {}\n\
        ⏳ <b>Thời gian mỗi lượt:</b> {} giây\n\n\
        👥 <b>Người chơi đã tham gia ({}):</b>\n{}\n\n\
        🔗 <b>Link mời bạn bè tham gia:</b>\n<code>{}</code>\n\n\
        👉 <i>Nhấn nút <b>🔗 Lấy Link Mời (Get Link)</b> bên dưới để lấy link gửi cho bạn bè cùng vào chơi! Khi đã đủ người, chủ phòng bấm <b>▶️ Bắt Đầu</b>.</i>",
        user_mention(game.host_id, &game.host_name),
        game.room_code,
        game.lang.to_uppercase(),
        mode_str,
        max_str,
        turn_timeout,
        game.players.len(),
        players_str,
        html_escape(&game.join_url)
    )
}

fn build_lobby_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "🔗 Lấy Link Mời (Get Link)",
            "nt_getlink",
        )],
        vec![
            InlineKeyboardButton::callback("▶️ Bắt Đầu", "nt_start"),
            InlineKeyboardButton::callback("⏹️ Hủy Phòng", "nt_cancel"),
        ],
    ])
}

fn build_multiplayer_main_text() -> &'static str {
    "🌐 <b>CHẾ ĐỘ CHƠI NHIỀU NGƯỜI CÔNG KHAI (PUBLIC MULTIPLAYER)</b> 🌐\n\n\
    Chào mừng bạn đến với <b>Sảnh Chơi Công Khai</b>!\n\
    Tại đây bạn có thể tạo phòng mới để giao lưu với người chơi trên toàn hệ thống \
    (Web, Discord, Telegram) hoặc tham gia các phòng đang mở.\n\n\
    👉 <i>Hãy chọn một chức năng bên dưới:</i>"
}

fn build_multiplayer_main_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "➕ Tạo Phòng Công Khai",
            "mp_create",
        )],
        vec![InlineKeyboardButton::callback(
            "🌐 Tham Gia Phòng Công Khai",
            "mp_list",
        )],
        vec![InlineKeyboardButton::callback("🔙 Về Lại Menu", "mp_menu")],
    ])
}

pub async fn cmd_start(bot: &Bot, msg: &Message, st: &TelegramState, args: &str) {
    let chat_id = msg.chat.id;
    let (uid, uname) = user_display_name(msg.from.as_ref());

    let arg = args.split_whitespace().next().unwrap_or("");
    if let Some(token) = arg.strip_prefix("join_") {
        let token = token.trim();
        let mut target_chat_id = st.room_tokens.get(token).map(|v| *v);
        if target_chat_id.is_none() {
            if let Some((_, c_id, _, _, _)) = st.app.db.get_telegram_token(token).await {
                target_chat_id = Some(c_id);
            }
        }

        if let Some(room_chat_id) = target_chat_id {
            if let Some(room_arc) = st.noitu_rooms.get(&room_chat_id).map(|r| r.value().clone()) {
                let mut g = room_arc.lock().await;
                if !g.join_token.is_empty() && g.join_token != token {
                    let _ = bot
                        .send_message(
                            chat_id,
                            "❌ Mã tham gia phòng không hợp lệ hoặc đã hết hạn.",
                        )
                        .await;
                    return;
                }
                if g.joining {
                    if g.max_players > 0 && (g.players.len() as i64) >= g.max_players {
                        let _ = bot
                            .send_message(
                                chat_id,
                                format!(
                                    "⚠️ <b>Phòng [{}] đã đầy!</b> (Tối đa: {} người).",
                                    g.room_code, g.max_players
                                ),
                            )
                            .parse_mode(ParseMode::Html)
                            .await;
                        return;
                    }
                    if g.players.iter().any(|p| p.id == uid) {
                        let _ = bot
                            .send_message(
                                chat_id,
                                format!(
                                    "✅ <b>Bạn đã ở trong phòng [{}] rồi!</b> Hãy sẵn sàng khi chủ phòng bắt đầu nhé.",
                                    g.room_code
                                ),
                            )
                            .parse_mode(ParseMode::Html)
                            .await;
                        return;
                    }

                    g.players.push(TelePlayer {
                        id: uid,
                        name: uname.clone(),
                    });
                    let p_count = g.players.len() as i64;
                    let room_code = g.room_code.clone();
                    let host_mention = user_mention(g.host_id, &g.host_name);
                    let lang_up = g.lang.to_uppercase();
                    let max_str = if g.max_players > 0 {
                        format!("{}/{}", p_count, g.max_players)
                    } else {
                        format!("{}/∞ (Vô hạn)", p_count)
                    };
                    let group_chat = g.chat_id;
                    let lobby_msg_id = g.lobby_msg_id;
                    let lobby_text = build_lobby_text(&g, st.app.config.turn_timeout);
                    drop(g);

                    st.app
                        .db
                        .update_public_room_players(&room_code, p_count)
                        .await;

                    let _ = bot
                        .send_message(
                            chat_id,
                            format!(
                                "🎉 <b>ĐÃ CONNECT VỚI PHÒNG [{}] THÀNH CÔNG!</b> 🎉\n\n\
                                👑 <b>Chủ phòng:</b> {}\n\
                                🌐 <b>Ngôn ngữ:</b> <code>{}</code>\n\
                                👥 <b>Số người hiện tại:</b> {}\n\n\
                                👉 <i>Hãy quay lại nhóm hoặc đợi chủ phòng bấm <b>Bắt Đầu</b> nhé!</i>",
                                room_code, host_mention, lang_up, max_str
                            ),
                        )
                        .parse_mode(ParseMode::Html)
                        .await;

                    let _ = bot
                        .send_message(
                            group_chat,
                            format!(
                                "👋 {} <b>đã connect với phòng <code>[{}]</code></b> qua link mời! (Tổng: {} người)",
                                user_mention(uid, &uname),
                                room_code,
                                p_count
                            ),
                        )
                        .parse_mode(ParseMode::Html)
                        .await;

                    if let Some(mid) = lobby_msg_id {
                        let _ = bot
                            .edit_message_text(group_chat, mid, lobby_text)
                            .parse_mode(ParseMode::Html)
                            .reply_markup(build_lobby_keyboard())
                            .await;
                    }
                    return;
                }
            }
        }

        let _ = bot
            .send_message(
                chat_id,
                "⚠️ <b>Phòng này hiện không còn mở hoặc ván đấu đã kết thúc!</b>\n\
                Bạn có thể dùng lệnh <code>/noitu</code> hoặc <code>/multiplayer</code> để tạo phòng mới nhé.",
            )
            .parse_mode(ParseMode::Html)
            .await;
        return;
    }

    let welcome = format!(
        "👋 <b>Xin chào {}! Chào mừng bạn đến với Zenne Nối Từ Bot!</b>\n\n\
        🤖 Bot hỗ trợ <b>Trò chơi Nối Từ Wikipedia</b> đỉnh cao cùng hệ sinh thái <b>Multiplayer &amp; Mini-games</b> phong phú:\n\n\
        🌐 <b>Chơi Nhiều Người (Multiplayer):</b>\n\
        • <code>/multiplayer</code> - Sảnh chơi công khai liên thông Web, Discord, Telegram\n\
        • <code>/noitu</code> - Mở phòng nối từ &amp; lấy link mời bạn bè (Get Link)\n\
        • <code>/solo</code> hoặc <code>/botplay</code> - Chơi nối từ 1-1 trực tiếp với Bot\n\
        • <code>/stop</code> - Dừng trận đấu hiện tại\n\
        • <code>/diem</code> - Xem điểm số ván đấu\n\
        • <code>/tratu &lt;từ&gt;</code> - Tra từ điển / Wikipedia\n\
        • <code>/contukhong &lt;âm tiết&gt;</code> - Kiểm tra âm tiết có từ nối không\n\
        • <code>/top</code> - Bảng xếp hạng nối từ\n\n\
        🎲 <b>Kho Mini-games Trí Tuệ &amp; Dân Gian:</b>\n\
        • <code>/quiz</code> - Trắc nghiệm Toán &amp; Kiến thức tổng hợp\n\
        • <code>/vtv</code> - Vua Tiếng Việt (Xáo từ đoán chữ)\n\
        • <code>/taixiu</code> - Lắc Xúc Xắc Tài Xỉu trực quan\n\
        • <code>/baucua</code> - Bầu Cua Tôm Cá dân gian\n\
        • <code>/cadao</code> - Đoán Ca Dao Tục Ngữ Việt Nam\n\
        • <code>/chinhta</code> - Đố lỗi chính tả Tiếng Việt\n\
        • <code>/flag</code> - Đoán Quốc Kỳ các nước trên thế giới\n\
        • <code>/trieuphu</code> - Gameshow Ai Là Triệu Phú (15 câu hỏi)\n\
        • <code>/wordle</code> - Đoán từ 5 chữ cái phong cách Wordle\n\
        • <code>/ask &lt;câu hỏi&gt;</code> - Hỏi đáp Trợ lý AI Kira thông minh\n\n\
        💡 <i>Gõ <code>/help</code> bất kỳ lúc nào để xem hướng dẫn chi tiết!</i>",
        html_escape(&uname)
    );

    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🌐 Sảnh Multiplayer", "mp_menu_open"),
            InlineKeyboardButton::callback("🎮 Mở Phòng Nối Từ", "btn_quick_noitu"),
        ],
        vec![
            InlineKeyboardButton::callback("🤖 Chơi Solo với Bot", "btn_quick_solo"),
            InlineKeyboardButton::callback("📚 Đố Vui Quiz", "btn_quick_quiz"),
        ],
        vec![
            InlineKeyboardButton::callback("👑 Vua Tiếng Việt", "btn_quick_vtv"),
            InlineKeyboardButton::callback("🎲 Tài Xỉu", "btn_quick_taixiu"),
        ],
        vec![
            InlineKeyboardButton::callback("🏆 Bảng Xếp Hạng", "btn_quick_top"),
            InlineKeyboardButton::callback("📖 Hướng Dẫn", "btn_quick_help"),
        ],
    ]);

    let _ = bot
        .send_message(chat_id, welcome)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await;
}

pub async fn cmd_help(bot: &Bot, chat_id: ChatId) {
    let help_text = "📖 <b>DANH SÁCH LỆNH ZENNE BOT TELEGRAM</b> 📖\n\n\
        🌐 <b>1. Chơi Nhiều Người &amp; Nối Từ (Multiplayer):</b>\n\
        • <code>/multiplayer</code> (<code>/mp</code>) - Sảnh chơi công khai (Web, Discord, Telegram)\n\
        • <code>/noitu</code> - Mở phòng chờ nối từ &amp; nhận link mời (Get Link)\n\
        • <code>/taophong &lt;Tên&gt; | &lt;Lang&gt; | &lt;Max&gt;</code> - Tạo nhanh phòng công khai (Max 0 = Vô hạn)\n\
        • <code>/solo</code> hoặc <code>/botplay</code> - Chơi nối từ 1-1 trực tiếp với Bot\n\
        • <code>/batdau</code> hoặc <code>/play</code> - Chủ phòng bắt đầu ván đấu\n\
        • <code>/stop</code> - Dừng ván đấu và tổng kết điểm\n\
        • <code>/diem</code> - Xem điểm số ván đấu hiện tại\n\
        • <code>/tratu &lt;từ&gt;</code> - Tra cứu từ trên từ điển Wikipedia\n\
        • <code>/contukhong &lt;âm tiết&gt;</code> - Tra từ nối tiếp cho âm tiết\n\
        • <code>/gopthemtu &lt;từ&gt;</code> - Đóng góp từ mới vào hệ thống\n\
        • <code>/setlang &lt;mã&gt;</code> - Đổi ngôn ngữ từ điển (vi, en, fr, ja,...)\n\
        • <code>/top</code> - Xem Bảng xếp hạng nối từ\n\n\
        🎲 <b>2. Mini-games Giải Trí &amp; Trí Tuệ:</b>\n\
        • <code>/quiz [toan|khoahoc]</code> - Đấu trí Toán / Kiến thức\n\
        • <code>/vtv</code> - Vua Tiếng Việt (sắp xếp lại các ký tự bị đảo)\n\
        • <code>/taixiu</code> - Đặt cược Tài (11-18) hoặc Xỉu (3-10)\n\
        • <code>/baucua</code> - Đặt cược 6 con vật Bầu Cua Tôm Cá\n\
        • <code>/cadao</code> - Điền khuyết câu Ca dao, tục ngữ\n\
        • <code>/chinhta</code> - Thử thách phát hiện từ viết đúng chính tả\n\
        • <code>/flag</code> - Đoán Quốc kỳ thế giới\n\
        • <code>/trieuphu</code> - Gameshow Ai Là Triệu Phú 15 câu\n\
        • <code>/wordle</code> - Đoán từ 5 chữ cái kiểu Wordle\n\n\
        🤖 <b>3. Tiện Ích Khác:</b>\n\
        • <code>/ask &lt;câu hỏi&gt;</code> - Hỏi đáp Trợ lý AI Kira\n\
        • <code>/mygame</code> - Xem kỷ lục &amp; điểm tích lũy của bạn";

    let _ = bot
        .send_message(chat_id, help_text)
        .parse_mode(ParseMode::Html)
        .await;
}

pub async fn cmd_noitu(bot: &Bot, chat_id: ChatId, uid: i64, uname: String, st: &TelegramState) {
    if let Some(room_arc) = st.noitu_rooms.get(&chat_id.0).map(|r| r.value().clone()) {
        if room_arc.lock().await.active {
            let _ = bot
                .send_message(
                    chat_id,
                    "⚠️ Trận nối từ đang diễn ra! Gõ <code>/stop</code> nếu muốn kết thúc.",
                )
                .parse_mode(ParseMode::Html)
                .await;
            return;
        }
    }

    let lang = st.app.db.get_lang(chat_id.0).await;
    let room_code = gen_room_code();
    let join_token = gen_join_token();
    let web_base = st.web_base_url();
    let join_url = format!("{}/telegram/?join={}", web_base, join_token);
    let web_url = format!("{}/pub/play/{}", web_base, room_code);

    let game = NoituRoomState {
        chat_id,
        host_id: uid,
        host_name: uname.clone(),
        lang: lang.clone(),
        is_bot_mode: false,
        active: false,
        joining: true,
        lobby_msg_id: None,
        players: vec![TelePlayer {
            id: uid,
            name: uname.clone(),
        }],
        turn_index: 0,
        current_word: String::new(),
        last_syllable: String::new(),
        used_words: HashSet::new(),
        scores: HashMap::new(),
        streaks: HashMap::new(),
        timeout_handle: None,
        room_code: room_code.clone(),
        join_token: join_token.clone(),
        join_url: join_url.clone(),
        max_players: 0,
    };
    let lobby_text = build_lobby_text(&game, st.app.config.turn_timeout);
    let room_arc = Arc::new(Mutex::new(game));
    st.noitu_rooms.insert(chat_id.0, room_arc.clone());
    st.room_tokens.insert(join_token.clone(), chat_id.0);

    st.app
        .db
        .save_telegram_token(&join_token, &room_code, chat_id.0, uid, &uname, &lang)
        .await;
    st.app
        .db
        .create_public_room(&PublicRoomRow {
            room_id: room_code,
            name: format!("Phòng của {}", uname),
            platform: "telegram".to_string(),
            host_id: uid.to_string(),
            host_name: uname,
            lang,
            max_players: 0,
            current_players: 1,
            join_token: Some(join_token),
            web_url: Some(web_url),
            tele_url: Some(join_url),
            status: "waiting".to_string(),
        })
        .await;

    if let Ok(sent) = bot
        .send_message(chat_id, lobby_text)
        .parse_mode(ParseMode::Html)
        .reply_markup(build_lobby_keyboard())
        .await
    {
        room_arc.lock().await.lobby_msg_id = Some(sent.id);
    }
}

pub async fn cmd_solo(bot: &Bot, chat_id: ChatId, uid: i64, uname: String, st: &TelegramState) {
    if let Some(room_arc) = st.noitu_rooms.get(&chat_id.0).map(|r| r.value().clone()) {
        if room_arc.lock().await.active {
            let _ = bot
                .send_message(
                    chat_id,
                    "⚠️ Trận nối từ đang diễn ra! Gõ <code>/stop</code> để dừng lại trước.",
                )
                .parse_mode(ParseMode::Html)
                .await;
            return;
        }
    }

    let lang = st.app.db.get_lang(chat_id.0).await;
    let room_code = gen_room_code();
    let join_token = gen_join_token();
    let join_url = format!("{}/telegram/?join={}", st.web_base_url(), join_token);

    let room_arc = Arc::new(Mutex::new(NoituRoomState {
        chat_id,
        host_id: uid,
        host_name: uname.clone(),
        lang,
        is_bot_mode: true,
        active: false,
        joining: false,
        lobby_msg_id: None,
        players: vec![TelePlayer {
            id: uid,
            name: uname,
        }],
        turn_index: 0,
        current_word: String::new(),
        last_syllable: String::new(),
        used_words: HashSet::new(),
        scores: HashMap::new(),
        streaks: HashMap::new(),
        timeout_handle: None,
        room_code,
        join_token,
        join_url,
        max_players: 1,
    }));
    st.noitu_rooms.insert(chat_id.0, room_arc.clone());
    start_game_match(bot, st, room_arc).await;
}

pub async fn cmd_play(bot: &Bot, msg: &Message, st: &TelegramState) {
    let chat_id = msg.chat.id;
    let (uid, _) = user_display_name(msg.from.as_ref());
    let Some(room_arc) = st.noitu_rooms.get(&chat_id.0).map(|r| r.value().clone()) else {
        let _ = bot
            .send_message(
                chat_id,
                "❌ Không có phòng chờ nào đang mở! Gõ <code>/noitu</code> để tạo phòng.",
            )
            .parse_mode(ParseMode::Html)
            .await;
        return;
    };
    {
        let g = room_arc.lock().await;
        if !g.joining {
            let _ = bot
                .send_message(
                    chat_id,
                    "❌ Không có phòng chờ nào đang mở! Gõ <code>/noitu</code> để tạo phòng.",
                )
                .parse_mode(ParseMode::Html)
                .await;
            return;
        }
        if uid != g.host_id && g.players.len() > 1 {
            let _ = bot
                .send_message(chat_id, "⛔ Chỉ chủ phòng mới có quyền bắt đầu trận đấu!")
                .await;
            return;
        }
    }
    start_game_match(bot, st, room_arc).await;
}

pub async fn cmd_stop(bot: &Bot, msg: &Message, st: &TelegramState) {
    let chat_id = msg.chat.id;
    let Some((_, room_arc)) = st.noitu_rooms.remove(&chat_id.0) else {
        let _ = bot
            .send_message(chat_id, "❌ Không có trận nối từ nào đang diễn ra.")
            .await;
        return;
    };

    let (room_code, scores_str) = {
        let mut g = room_arc.lock().await;
        g.cancel_timeout();
        g.active = false;
        g.joining = false;
        if !g.join_token.is_empty() {
            st.room_tokens.remove(&g.join_token);
        }
        let rc = g.room_code.clone();

        let s_str = if g.scores.is_empty() {
            "<i>Chưa ai ghi được điểm nào.</i>".to_string()
        } else {
            let mut sorted: Vec<(i64, i64)> = g.scores.iter().map(|(k, v)| (*k, *v)).collect();
            sorted.sort_by_key(|a| std::cmp::Reverse(a.1));
            let medals = ["🥇", "🥈", "🥉"];
            sorted
                .iter()
                .enumerate()
                .map(|(i, (uid, pts))| {
                    let p_name = g
                        .players
                        .iter()
                        .find(|p| p.id == *uid)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| format!("User {}", uid));
                    let medal = if i < 3 {
                        medals[i].to_string()
                    } else {
                        format!("{}.", i + 1)
                    };
                    format!(
                        "{} {}: <b>{}</b> điểm",
                        medal,
                        user_mention(*uid, &p_name),
                        pts
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        (rc, s_str)
    };

    st.app.db.close_public_room(&room_code).await;
    let _ = bot
        .send_message(
            chat_id,
            format!(
                "⏹️ <b>Trận đấu nối từ đã kết thúc theo yêu cầu!</b>\n\n📊 <b>Bảng Điểm Chung Cuộc:</b>\n{}",
                scores_str
            ),
        )
        .parse_mode(ParseMode::Html)
        .await;
}

pub async fn cmd_diem(bot: &Bot, msg: &Message, st: &TelegramState) {
    let chat_id = msg.chat.id;
    let Some(room_arc) = st.noitu_rooms.get(&chat_id.0).map(|r| r.value().clone()) else {
        let _ = bot
            .send_message(chat_id, "❌ Chưa có ván đấu nào đang diễn ra.")
            .await;
        return;
    };
    let g = room_arc.lock().await;
    if !g.active {
        let _ = bot
            .send_message(chat_id, "❌ Chưa có ván đấu nào đang diễn ra.")
            .await;
        return;
    }
    if g.scores.is_empty() {
        let _ = bot
            .send_message(chat_id, "📊 Chưa ai ghi được điểm nào trong ván này.")
            .await;
        return;
    }

    let mut sorted: Vec<(i64, i64)> = g.scores.iter().map(|(k, v)| (*k, *v)).collect();
    sorted.sort_by_key(|a| std::cmp::Reverse(a.1));
    let lines = sorted
        .iter()
        .enumerate()
        .map(|(i, (uid, pts))| {
            let p_name = g
                .players
                .iter()
                .find(|p| p.id == *uid)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| format!("User {}", uid));
            let streak = g.streaks.get(uid).copied().unwrap_or(0);
            format!(
                "{}. {}: <b>{}</b> điểm (Chuỗi: 🔥x{})",
                i + 1,
                user_mention(*uid, &p_name),
                pts,
                streak
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let _ = bot
        .send_message(chat_id, format!("📊 <b>BẢNG ĐIỂM HIỆN TẠI:</b>\n{}", lines))
        .parse_mode(ParseMode::Html)
        .await;
}

async fn start_game_match(bot: &Bot, st: &TelegramState, room_arc: Arc<Mutex<NoituRoomState>>) {
    let lang = {
        let mut g = room_arc.lock().await;
        g.joining = false;
        g.active = true;
        g.lang.clone()
    };

    let start_word = wiki::random_word(&st.app.http, &st.app.db, &lang).await;
    let (chat_id, room_code, next_syl, first_player) = {
        let mut g = room_arc.lock().await;
        let syl = wiki::last_syl(&start_word, &g.lang);
        g.current_word = start_word.clone();
        g.last_syllable = syl.clone();
        g.used_words.insert(start_word.to_lowercase());
        g.turn_index = 0;
        let fp = g.players[0].clone();
        (g.chat_id, g.room_code.clone(), syl, fp)
    };

    let timeout_secs = st.app.config.turn_timeout;
    let text = format!(
        "🚀 <b>TRẬN ĐẤU NỐI TỪ BẮT ĐẦU! (Phòng [{}])</b> 🚀\n\n\
        🎯 <b>Từ mở đầu:</b> <code>{}</code>\n\
        🔤 <b>Âm tiết cần nối:</b> <b><code>{}</code></b>\n\
        👤 <b>Lượt đầu tiên:</b> {}\n\
        ⏳ <b>Thời gian:</b> {} giây\n\n\
        👉 <i>Hãy nhập từ có âm tiết đầu khớp với <b>{}</b> vào chat!</i>",
        room_code,
        html_escape(&start_word),
        html_escape(&next_syl),
        user_mention(first_player.id, &first_player.name),
        timeout_secs,
        html_escape(&next_syl)
    );
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .await;

    schedule_turn_timeout(bot, st, room_arc).await;
}

async fn schedule_turn_timeout(
    bot: &Bot,
    st: &TelegramState,
    room_arc: Arc<Mutex<NoituRoomState>>,
) {
    let bot_c = bot.clone();
    let st_c = st.clone();
    let room_c = room_arc.clone();
    let timeout_secs = st.app.config.turn_timeout;

    let handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)).await;

            let mut g = room_c.lock().await;
            if !g.active || g.players.is_empty() {
                return;
            }
            let curr_player = g.players[g.turn_index].clone();
            let chat_id = g.chat_id;

            if g.is_bot_mode {
                let score = g.scores.get(&curr_player.id).copied().unwrap_or(0);
                g.active = false;
                drop(g);
                st_c.noitu_rooms.remove(&chat_id.0);
                let _ = bot_c
                    .send_message(
                        chat_id,
                        format!(
                            "⏰ <b>Hết giờ!</b> Bạn đã không đưa ra từ trong {} giây.\n\n\
                            🏁 <b>Kết thúc ván đấu Solo!</b>\n\
                            🎉 Tổng điểm đạt được: <b>{}</b> điểm!\n\
                            Gõ <code>/solo</code> để bắt đầu ván mới nhé!",
                            timeout_secs, score
                        ),
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
                return;
            }

            let idx = g.turn_index;
            g.players.remove(idx);
            if !g.players.is_empty() && g.turn_index >= g.players.len() {
                g.turn_index = 0;
            }

            if g.players.is_empty() {
                let rc = g.room_code.clone();
                g.active = false;
                drop(g);
                st_c.app.db.close_public_room(&rc).await;
                st_c.noitu_rooms.remove(&chat_id.0);
                let _ = bot_c
                    .send_message(
                        chat_id,
                        "⏰ <b>Hết giờ!</b> Tất cả người chơi đã bị loại. Trận đấu kết thúc!",
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
                return;
            }

            if g.players.len() == 1 {
                let winner = g.players[0].clone();
                let rc = g.room_code.clone();
                let jt = g.join_token.clone();
                g.active = false;
                drop(g);
                if !jt.is_empty() {
                    st_c.room_tokens.remove(&jt);
                }
                st_c.app.db.game_submit_score("noitu", winner.id, 5).await;
                st_c.app.db.close_public_room(&rc).await;
                st_c.noitu_rooms.remove(&chat_id.0);
                let _ = bot_c
                    .send_message(
                        chat_id,
                        format!(
                            "⏰ {} đã hết thời gian và bị loại!\n\n\
                            👑 <b>CHÚC MỪNG QUÁN QUÂN:</b> {} 🏆\n\
                            🎉 Thưởng quán quân: <b>+5 điểm tích lũy</b>!",
                            user_mention(curr_player.id, &curr_player.name),
                            user_mention(winner.id, &winner.name)
                        ),
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
                return;
            }

            let nxt_player = g.players[g.turn_index].clone();
            let syl = g.last_syllable.clone();
            drop(g);

            let _ = bot_c
                .send_message(
                    chat_id,
                    format!(
                        "⏰ {} hết thời gian và bị loại!\n\n\
                        ➡️ <b>Lượt tiếp theo:</b> {}\n\
                        🔤 Âm tiết bắt đầu: <b><code>{}</code></b> ⏳ ({}s)",
                        user_mention(curr_player.id, &curr_player.name),
                        user_mention(nxt_player.id, &nxt_player.name),
                        html_escape(&syl),
                        timeout_secs
                    ),
                )
                .parse_mode(ParseMode::Html)
                .await;
        }
    });

    room_arc.lock().await.timeout_handle = Some(handle.abort_handle());
}

pub async fn cmd_multiplayer(bot: &Bot, chat_id: ChatId) {
    let _ = bot
        .send_message(chat_id, build_multiplayer_main_text())
        .parse_mode(ParseMode::Html)
        .reply_markup(build_multiplayer_main_keyboard())
        .await;
}

pub async fn cmd_taophong(bot: &Bot, msg: &Message, st: &TelegramState, args: &str) {
    let chat_id = msg.chat.id;
    let (uid, uname) = user_display_name(msg.from.as_ref());
    st.mp_wizard.remove(&(chat_id.0, uid));

    let raw = args.trim();
    if raw.is_empty() {
        let _ = bot
            .send_message(
                chat_id,
                "💡 <b>CÚ PHÁP TẠO PHÒNG CÔNG KHAI:</b>\n\
                <code>/taophong &lt;Tên phòng&gt; | &lt;Ngôn ngữ&gt; | &lt;Số người tối đa&gt;</code>\n\n\
                • Ngôn ngữ: vi, en, fr, ja, ko, zh,... (hơn 300 ngôn ngữ)\n\
                • Số người tối đa: Nhập <b>0</b> nếu muốn <b>Vô hạn</b> người chơi!\n\n\
                Ví dụ:\n\
                • <code>/taophong Đại Chiến Nối Từ | vi | 0</code>\n\
                • <code>/taophong Solo Nhanh | en | 2</code>",
            )
            .parse_mode(ParseMode::Html)
            .await;
        return;
    }

    let parts: Vec<&str> = raw.split('|').map(|s| s.trim()).collect();
    let name = parts
        .first()
        .filter(|s| !s.is_empty())
        .copied()
        .unwrap_or("Phòng Nối Từ")
        .to_string();
    let lang = parts
        .get(1)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "vi".to_string());
    let max_p: i64 = parts
        .get(2)
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);

    execute_create_public_room(bot, chat_id, uid, uname, st, name, lang, max_p).await;
}

#[allow(clippy::too_many_arguments)]
async fn execute_create_public_room(
    bot: &Bot,
    chat_id: ChatId,
    uid: i64,
    uname: String,
    st: &TelegramState,
    name: String,
    lang: String,
    max_p: i64,
) {
    let room_id = gen_room_code();
    let join_token = gen_join_token();
    let web_base = st.web_base_url();
    let web_url = format!("{}/pub/play/{}", web_base, room_id);
    let tele_url = format!("{}/telegram/?join={}", web_base, join_token);

    let room_arc = Arc::new(Mutex::new(NoituRoomState {
        chat_id,
        host_id: uid,
        host_name: uname.clone(),
        lang: lang.clone(),
        is_bot_mode: false,
        active: false,
        joining: true,
        lobby_msg_id: None,
        players: vec![TelePlayer {
            id: uid,
            name: uname.clone(),
        }],
        turn_index: 0,
        current_word: String::new(),
        last_syllable: String::new(),
        used_words: HashSet::new(),
        scores: HashMap::new(),
        streaks: HashMap::new(),
        timeout_handle: None,
        room_code: room_id.clone(),
        join_token: join_token.clone(),
        join_url: tele_url.clone(),
        max_players: max_p,
    }));

    st.noitu_rooms.insert(chat_id.0, room_arc.clone());
    st.room_tokens.insert(join_token.clone(), chat_id.0);

    st.app
        .db
        .save_telegram_token(&join_token, &room_id, chat_id.0, uid, &uname, &lang)
        .await;
    st.app
        .db
        .create_public_room(&PublicRoomRow {
            room_id: room_id.clone(),
            name: name.clone(),
            platform: "telegram".to_string(),
            host_id: uid.to_string(),
            host_name: uname.clone(),
            lang: lang.clone(),
            max_players: max_p,
            current_players: 1,
            join_token: Some(join_token),
            web_url: Some(web_url.clone()),
            tele_url: Some(tele_url.clone()),
            status: "waiting".to_string(),
        })
        .await;

    let max_str = if max_p <= 0 {
        "Vô hạn (∞)".to_string()
    } else {
        format!("{} người", max_p)
    };
    let text = format!(
        "🎉 <b>PHÒNG CÔNG KHAI ĐÃ TẠO THÀNH CÔNG!</b> 🎉\n\n\
        🏷️ <b>Tên phòng:</b> <b>{}</b>\n\
        🔑 <b>Mã phòng (6 ký tự):</b> <code>[{}]</code>\n\
        👑 <b>Chủ phòng:</b> {} (Telegram)\n\
        🌐 <b>Ngôn ngữ:</b> <code>{}</code>\n\
        👥 <b>Giới hạn người chơi:</b> <b>{}</b>\n\n\
        🔗 <b>Link mời Web:</b>\n<code>{}</code>\n\
        🔗 <b>Link mời Telegram (Get Link):</b>\n<code>{}</code>\n\n\
        📢 <i>Phòng đã được niêm yết công khai trên toàn hệ thống (Web, Discord, Telegram)! Mọi người có thể vào trực tiếp để đấu cùng bạn.</i>",
        html_escape(&name),
        room_id,
        user_mention(uid, &uname),
        lang.to_uppercase(),
        max_str,
        html_escape(&web_url),
        html_escape(&tele_url)
    );

    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🔗 Lấy Link Mời (Get Link)", "nt_getlink"),
            InlineKeyboardButton::callback("▶️ Bắt Đầu", "nt_start"),
        ],
        vec![InlineKeyboardButton::callback(
            "🌐 Xem Danh Sách Phòng",
            "mp_list",
        )],
    ]);

    if let Ok(sent) = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await
    {
        room_arc.lock().await.lobby_msg_id = Some(sent.id);
    }
}

pub async fn cmd_tratu(bot: &Bot, msg: &Message, st: &TelegramState, args: &str) {
    let chat_id = msg.chat.id;
    let query = args.trim();
    if query.is_empty() {
        let _ = bot
            .send_message(
                chat_id,
                "💡 Cách dùng: <code>/tratu &lt;từ cần tra&gt;</code> (Ví dụ: <code>/tratu mặt trời</code>)",
            )
            .parse_mode(ParseMode::Html)
            .await;
        return;
    }

    let lang = st.app.db.get_lang(chat_id.0).await;
    let in_bank = st.app.db.word_exists(chat_id.0, query).await;
    let in_cache = st.app.db.cache_exists(&lang, query).await;
    let mut in_wiki = false;
    if !in_bank && !in_cache {
        let (ok, _) = wiki::check_word(
            &st.app.http,
            &st.app.db,
            &st.app.cache,
            chat_id.0,
            &lang,
            query,
        )
        .await;
        in_wiki = ok;
    }

    let is_valid = in_bank || in_cache || in_wiki;
    let status_icon = if is_valid {
        "✅ HỢP LỆ"
    } else {
        "❌ KHÔNG TÌM THẤY"
    };
    let source = if in_bank {
        "Ngân hàng server"
    } else if in_cache {
        "Cache cục bộ"
    } else if in_wiki {
        "Wikipedia"
    } else {
        "Không tìm thấy"
    };
    let wiki_url = format!(
        "https://{}.wikipedia.org/wiki/{}",
        lang,
        wiki::urlencoding_simple(&query.replace(' ', "_"))
    );

    let text = format!(
        "🔍 <b>KẾT QUẢ TRA CỨU:</b> <code>{}</code>\n\n\
        • <b>Trạng thái:</b> {}\n\
        • <b>Nguồn xác nhận:</b> {}\n\
        • <b>Ngôn ngữ:</b> {}\n\
        • <b>Liên kết:</b> <a href=\"{}\">Xem trên Wikipedia</a>",
        html_escape(query),
        status_icon,
        source,
        lang.to_uppercase(),
        wiki_url
    );
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .await;
}

pub async fn cmd_contukhong(bot: &Bot, msg: &Message, st: &TelegramState, args: &str) {
    let chat_id = msg.chat.id;
    let syl = args.split_whitespace().next().unwrap_or("").trim();
    if syl.is_empty() {
        let _ = bot
            .send_message(
                chat_id,
                "💡 Cách dùng: <code>/contukhong &lt;âm tiết&gt;</code> (Ví dụ: <code>/contukhong hoa</code>)",
            )
            .parse_mode(ParseMode::Html)
            .await;
        return;
    }

    let lang = st.app.db.get_lang(chat_id.0).await;
    let sugs = wiki::find_continuations(&st.app.db, chat_id.0, &lang, syl, &[], 8).await;
    if !sugs.is_empty() {
        let formatted = sugs
            .iter()
            .map(|w| format!("• <code>{}</code>", html_escape(w)))
            .collect::<Vec<_>>()
            .join("\n");
        let _ = bot
            .send_message(
                chat_id,
                format!(
                    "✅ Âm tiết <b><code>{}</code></b> hoàn toàn <b>CÒN TỪ NỐI ĐƯỢC</b>!\n\n🔍 <b>Các từ gợi ý:</b>\n{}",
                    html_escape(syl),
                    formatted
                ),
            )
            .parse_mode(ParseMode::Html)
            .await;
    } else {
        let _ = bot
            .send_message(
                chat_id,
                format!(
                    "⚠️ Rất tiếc, âm tiết <b><code>{}</code></b> hiện tại <b>KHÔNG TÌM THẤY TỪ NỐI TIẾP</b> trong từ điển Wikipedia!",
                    html_escape(syl)
                ),
            )
            .parse_mode(ParseMode::Html)
            .await;
    }
}

pub async fn cmd_gopthemtu(bot: &Bot, msg: &Message, st: &TelegramState, args: &str) {
    let chat_id = msg.chat.id;
    let (uid, _) = user_display_name(msg.from.as_ref());
    let word = args.trim();
    if word.is_empty() {
        let _ = bot
            .send_message(
                chat_id,
                "💡 Cách dùng: <code>/gopthemtu &lt;từ muốn góp&gt;</code>",
            )
            .parse_mode(ParseMode::Html)
            .await;
        return;
    }

    let res = st.app.db.submit_pending(chat_id.0, word, uid).await;
    let text = match res {
        Ok(_) => format!(
            "✅ <b>Góp từ thành công!</b> Từ <code>{}</code> đã được gửi vào danh sách chờ admin duyệt.",
            html_escape(word)
        ),
        Err(err) => format!(
            "⚠️ {} (<code>{}</code>)",
            html_escape(&err),
            html_escape(word)
        ),
    };
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .await;
}

pub async fn cmd_top(bot: &Bot, chat_id: ChatId, st: &TelegramState) {
    let rows = st.app.db.lb_get_top(chat_id.0, 10).await;
    if rows.is_empty() {
        let _ = bot
            .send_message(
                chat_id,
                "🏆 Chưa có dữ liệu bảng xếp hạng cho nhóm/kênh này.",
            )
            .await;
        return;
    }
    let medals = ["🥇", "🥈", "🥉"];
    let lines = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let medal = if i < 3 {
                medals[i].to_string()
            } else {
                format!("{}.", i + 1)
            };
            format!(
                "{} <b>ID {}</b>: <b>{}</b> từ | Kỷ lục chuỗi: 🔥{} | Thắng: 🏆{}",
                medal, r.user_id, r.total_words, r.max_streak, r.games_won
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let _ = bot
        .send_message(
            chat_id,
            format!("🏆 <b>BẢNG XẾP HẠNG NỐI TỪ:</b>\n\n{}", lines),
        )
        .parse_mode(ParseMode::Html)
        .await;
}

pub async fn cmd_setlang(bot: &Bot, msg: &Message, st: &TelegramState, args: &str) {
    let chat_id = msg.chat.id;
    let new_lang = args
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if new_lang.is_empty() {
        let curr = st.app.db.get_lang(chat_id.0).await;
        let _ = bot
            .send_message(
                chat_id,
                format!(
                    "🌐 Ngôn ngữ từ điển hiện tại: <code>{}</code>\n💡 Cách đổi: <code>/setlang &lt;mã ngôn ngữ&gt;</code> (vd: <code>/setlang vi</code>, <code>/setlang en</code>, <code>/setlang fr</code>, <code>/setlang ja</code>)",
                    curr.to_uppercase()
                ),
            )
            .parse_mode(ParseMode::Html)
            .await;
        return;
    }

    st.app.db.set_lang(chat_id.0, &new_lang).await;
    let _ = bot
        .send_message(
            chat_id,
            format!(
                "✅ Đã đổi ngôn ngữ nối từ sang: <b>{}</b>",
                html_escape(&new_lang.to_uppercase())
            ),
        )
        .parse_mode(ParseMode::Html)
        .await;
}

pub async fn cmd_ask(bot: &Bot, msg: &Message, st: &TelegramState, args: &str) {
    let chat_id = msg.chat.id;
    let question = args.trim();
    if question.is_empty() {
        let _ = bot
            .send_message(
                chat_id,
                "💡 Cách dùng: <code>/ask &lt;câu hỏi của bạn&gt;</code>\nVí dụ: <code>/ask Trọng lực là gì?</code>",
            )
            .parse_mode(ParseMode::Html)
            .await;
        return;
    }

    let thinking = bot
        .send_message(
            chat_id,
            "🤔 <i>Trợ lý AI Kira đang suy nghĩ câu trả lời...</i>",
        )
        .parse_mode(ParseMode::Html)
        .await;

    let res = zenne_core::kira::ask_ai_kira(
        &st.app.http,
        &st.app.config.kira_api_endpoint,
        &st.app.config.kira_api_key,
        &st.app.config.kira_default_model,
        question,
        None,
    )
    .await;

    let reply_text = match res {
        Ok(ans) => format!("🤖 <b>TRẢ LỜI TỪ AI KIRA:</b>\n\n{}", html_escape(&ans)),
        Err(e) => format!(
            "❌ Lỗi khi kết nối với AI Kira: {}",
            html_escape(&e.to_string())
        ),
    };

    if let Ok(t_msg) = thinking {
        let _ = bot
            .edit_message_text(chat_id, t_msg.id, reply_text)
            .parse_mode(ParseMode::Html)
            .await;
    } else {
        let _ = bot
            .send_message(chat_id, reply_text)
            .parse_mode(ParseMode::Html)
            .await;
    }
}

pub async fn handle_callback_query(
    bot: Bot,
    q: CallbackQuery,
    st: TelegramState,
) -> ResponseResult<()> {
    let Some(ref data) = q.data else {
        let _ = bot.answer_callback_query(q.id.clone()).await;
        return Ok(());
    };
    let Some(ref msg) = q.message else {
        let _ = bot.answer_callback_query(q.id.clone()).await;
        return Ok(());
    };
    let chat_id = msg.chat().id;
    let msg_id = msg.id();
    let uid = q.from.id.0 as i64;
    let uname = q.from.first_name.clone();

    // 1. Quick Menu buttons from /start
    match data.as_str() {
        "btn_quick_noitu" => {
            let _ = bot.answer_callback_query(q.id.clone()).await;
            cmd_noitu(&bot, chat_id, uid, uname, &st).await;
            return Ok(());
        }
        "btn_quick_solo" => {
            let _ = bot.answer_callback_query(q.id.clone()).await;
            cmd_solo(&bot, chat_id, uid, uname, &st).await;
            return Ok(());
        }
        "btn_quick_quiz" => {
            let _ = bot.answer_callback_query(q.id.clone()).await;
            let fake_msg = msg.regular_message();
            if let Some(m) = fake_msg {
                minigames::cmd_quiz(&bot, m, &st, "").await;
            }
            return Ok(());
        }
        "btn_quick_vtv" => {
            let _ = bot.answer_callback_query(q.id.clone()).await;
            if let Some(m) = msg.regular_message() {
                minigames::cmd_vtv(&bot, m, &st).await;
            }
            return Ok(());
        }
        "btn_quick_taixiu" => {
            let _ = bot.answer_callback_query(q.id.clone()).await;
            if let Some(m) = msg.regular_message() {
                minigames::cmd_taixiu(&bot, m, &st).await;
            }
            return Ok(());
        }
        "btn_quick_top" => {
            let _ = bot.answer_callback_query(q.id.clone()).await;
            cmd_top(&bot, chat_id, &st).await;
            return Ok(());
        }
        "btn_quick_help" => {
            let _ = bot.answer_callback_query(q.id.clone()).await;
            cmd_help(&bot, chat_id).await;
            return Ok(());
        }
        _ => {}
    }

    // 2. Nối Từ Lobby callbacks
    if data == "nt_getlink" {
        if let Some(room_arc) = st.noitu_rooms.get(&chat_id.0).map(|r| r.value().clone()) {
            let g = room_arc.lock().await;
            let alert = format!("🔗 Link mời của phòng [{}]:\n{}", g.room_code, g.join_url);
            let _ = bot
                .answer_callback_query(q.id.clone())
                .text(alert)
                .show_alert(true)
                .await;
            let _ = bot
                .send_message(
                    chat_id,
                    format!(
                        "🔗 <b>LINK MỜI THAM GIA PHÒNG [{}]:</b>\n<code>{}</code>\n\n👉 <i>Hãy copy link trên gửi cho bạn bè để cùng vào phòng thi đấu nhé!</i>",
                        g.room_code,
                        html_escape(&g.join_url)
                    ),
                )
                .parse_mode(ParseMode::Html)
                .await;
            return Ok(());
        }
    } else if data == "nt_start" {
        if let Some(room_arc) = st.noitu_rooms.get(&chat_id.0).map(|r| r.value().clone()) {
            let can_start = {
                let g = room_arc.lock().await;
                if !g.joining {
                    false
                } else if uid != g.host_id && g.players.len() > 1 {
                    let _ = bot
                        .answer_callback_query(q.id.clone())
                        .text("⛔ Chỉ chủ phòng mới có thể bấm bắt đầu!")
                        .show_alert(true)
                        .await;
                    return Ok(());
                } else {
                    true
                }
            };
            let _ = bot.answer_callback_query(q.id.clone()).await;
            if can_start {
                start_game_match(&bot, &st, room_arc).await;
            }
            return Ok(());
        }
    } else if data == "nt_cancel" {
        if let Some(room_arc) = st.noitu_rooms.get(&chat_id.0).map(|r| r.value().clone()) {
            let (is_host, room_code) = {
                let g = room_arc.lock().await;
                (uid == g.host_id, g.room_code.clone())
            };
            if !is_host {
                let _ = bot
                    .answer_callback_query(q.id.clone())
                    .text("⛔ Chỉ chủ phòng mới có quyền hủy phòng!")
                    .show_alert(true)
                    .await;
                return Ok(());
            }
            let _ = bot.answer_callback_query(q.id.clone()).await;
            st.noitu_rooms.remove(&chat_id.0);
            st.app.db.close_public_room(&room_code).await;
            let _ = bot
                .edit_message_text(chat_id, msg_id, "⏹️ <b>Phòng chờ nối từ đã bị hủy.</b>")
                .parse_mode(ParseMode::Html)
                .await;
            return Ok(());
        }
    }

    // 3. Multiplayer menu callbacks
    if data == "mp_menu_open" || data == "mp_menu" {
        let _ = bot.answer_callback_query(q.id.clone()).await;
        let res = bot
            .edit_message_text(chat_id, msg_id, build_multiplayer_main_text())
            .parse_mode(ParseMode::Html)
            .reply_markup(build_multiplayer_main_keyboard())
            .await;
        if res.is_err() {
            cmd_multiplayer(&bot, chat_id).await;
        }
        return Ok(());
    } else if data == "mp_create" {
        let _ = bot.answer_callback_query(q.id.clone()).await;
        st.mp_wizard.insert((chat_id.0, uid), MpWizardStep::Name);
        let _ = bot
            .send_message(
                chat_id,
                "➕ <b>TẠO PHÒNG CÔNG KHAI MỚI (BƯỚC 1/3)</b>\n\n\
                Vui lòng nhập <b>Tên phòng</b> của bạn vào chat:\n\
                <i>(Ví dụ: Đại Chiến Nối Từ, English Practice, Vui Là Chính...)</i>\n\n\
                💡 <i>Bạn cũng có thể tạo nhanh bằng 1 lệnh:</i>\n\
                <code>/taophong &lt;Tên&gt; | &lt;Lang&gt; | &lt;Số người (0=vô hạn)&gt;</code>",
            )
            .parse_mode(ParseMode::Html)
            .await;
        return Ok(());
    } else if data == "mp_list" {
        let _ = bot.answer_callback_query(q.id.clone()).await;
        let rooms = st.app.db.get_public_rooms().await;
        if rooms.is_empty() {
            let no_rooms_text = "⚠️ <b>Hiện chưa có phòng công khai nào đang mở!</b>\n👉 Hãy bấm <b>➕ Tạo phòng công khai</b> để mở phòng đầu tiên nhé!";
            let kb = InlineKeyboardMarkup::new(vec![
                vec![InlineKeyboardButton::callback(
                    "➕ Tạo Phòng Mới",
                    "mp_create",
                )],
                vec![InlineKeyboardButton::callback("🔙 Về Lại Menu", "mp_menu")],
            ]);
            if bot
                .edit_message_text(chat_id, msg_id, no_rooms_text)
                .parse_mode(ParseMode::Html)
                .reply_markup(kb.clone())
                .await
                .is_err()
            {
                let _ = bot
                    .send_message(chat_id, no_rooms_text)
                    .parse_mode(ParseMode::Html)
                    .reply_markup(kb)
                    .await;
            }
            return Ok(());
        }

        let mut lines = Vec::new();
        let mut kb_rows = Vec::new();
        let web_base = st.web_base_url();
        for (i, r) in rooms.iter().take(10).enumerate() {
            let max_str = if r.max_players <= 0 {
                "∞".to_string()
            } else {
                r.max_players.to_string()
            };
            let p_icon = match r.platform.as_str() {
                "telegram" => "📱 Tele",
                "discord" => "💬 Discord",
                _ => "🌐 Web",
            };
            lines.push(format!(
                "{}. 🏷️ <b>{}</b> [<code>{}</code>]\n   👑 Host: <b>{}</b> ({}) | 🌐 <code>{}</code>\n   👥 Người chơi: <code>{}/{}</code>",
                i + 1,
                html_escape(&r.name),
                html_escape(&r.room_id),
                html_escape(&r.host_name),
                p_icon,
                r.lang.to_uppercase(),
                r.current_players,
                max_str
            ));
            let join_u = r
                .tele_url
                .as_deref()
                .or(r.web_url.as_deref())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}/pub/play/{}", web_base, r.room_id));
            if let Ok(parsed) = Url::parse(&join_u) {
                let short_name: String = r.name.chars().take(18).collect();
                kb_rows.push(vec![InlineKeyboardButton::url(
                    format!("🎮 Vào phòng [{}] {}", r.room_id, short_name),
                    parsed,
                )]);
            }
        }
        kb_rows.push(vec![
            InlineKeyboardButton::callback("🔄 Làm Mới Danh Sách", "mp_list"),
            InlineKeyboardButton::callback("➕ Tạo Phòng Mới", "mp_create"),
        ]);
        kb_rows.push(vec![InlineKeyboardButton::callback(
            "🔙 Về Lại Menu",
            "mp_menu",
        )]);

        let rooms_text = format!(
            "🌐 <b>DANH SÁCH PHÒNG CÔNG KHAI ĐANG MỞ ({} phòng)</b>\n\n{}\n\n👉 <i>Nhấn vào nút phòng bên dưới để tham gia trực tiếp đấu cùng người chơi!</i>",
            rooms.len(),
            lines.join("\n\n")
        );
        let kb = InlineKeyboardMarkup::new(kb_rows);
        if bot
            .edit_message_text(chat_id, msg_id, &rooms_text)
            .parse_mode(ParseMode::Html)
            .reply_markup(kb.clone())
            .await
            .is_err()
        {
            let _ = bot
                .send_message(chat_id, rooms_text)
                .parse_mode(ParseMode::Html)
                .reply_markup(kb)
                .await;
        }
        return Ok(());
    }

    // 4. Minigame callbacks
    if minigames::handle_minigame_callback(&bot, &q, &st, data).await {
        return Ok(());
    }

    let _ = bot.answer_callback_query(q.id).await;
    Ok(())
}

pub async fn handle_text_message(bot: &Bot, msg: &Message, st: &TelegramState, raw_text: &str) {
    let chat_id = msg.chat.id;
    let (uid, uname) = user_display_name(msg.from.as_ref());

    // 1. Active Nối Từ gameplay
    if let Some(room_arc) = st.noitu_rooms.get(&chat_id.0).map(|r| r.value().clone()) {
        let is_active = room_arc.lock().await.active;
        if is_active {
            st.mp_wizard.remove(&(chat_id.0, uid));
            let (is_bot_mode, curr_id, lang, curr_word, last_syl_str, already_used) = {
                let g = room_arc.lock().await;
                let curr = g.players[g.turn_index].id;
                (
                    g.is_bot_mode,
                    curr,
                    g.lang.clone(),
                    g.current_word.clone(),
                    g.last_syllable.clone(),
                    false,
                )
            };
            let _ = already_used;

            if !is_bot_mode && uid != curr_id {
                return;
            }

            let word = wiki::normalize_word(raw_text);
            if word.is_empty() || word.chars().count() > 80 || word.contains('\n') {
                return;
            }

            room_arc.lock().await.cancel_timeout();

            // A. Valid chain check
            if !wiki::valid_chain(&curr_word, &word, &lang) {
                let sugs =
                    wiki::find_continuations(&st.app.db, chat_id.0, &lang, &last_syl_str, &[], 4)
                        .await;
                let sug_txt = if sugs.is_empty() {
                    "<i>Không có gợi ý</i>".to_string()
                } else {
                    sugs.iter()
                        .map(|s| format!("<code>{}</code>", html_escape(s)))
                        .collect::<Vec<_>>()
                        .join("  •  ")
                };
                room_arc.lock().await.streaks.insert(uid, 0);
                st.app.db.lb_reset_streak(chat_id.0, uid).await;

                let _ = bot
                    .send_message(
                        chat_id,
                        format!(
                            "❌ <b>Từ sai liên kết!</b> Từ <code>{}</code> không bắt đầu bằng âm tiết <b><code>{}</code></b>!\n\n\
                            💡 <b>Gợi ý từ hợp lệ:</b> {}\n\
                            👉 {} hãy thử lại từ khác nhé!",
                            html_escape(&word),
                            html_escape(&last_syl_str),
                            sug_txt,
                            user_mention(uid, &uname)
                        ),
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
                schedule_turn_timeout(bot, st, room_arc).await;
                return;
            }

            // B. Used word check
            let used = room_arc
                .lock()
                .await
                .used_words
                .contains(&word.to_lowercase());
            if used {
                let _ = bot
                    .send_message(
                        chat_id,
                        format!(
                            "🔁 <b>Từ đã được sử dụng!</b> Từ <code>{}</code> đã xuất hiện trong ván này rồi.\n\
                            👉 {} hãy chọn từ khác bắt đầu bằng <b><code>{}</code></b>!",
                            html_escape(&word),
                            user_mention(uid, &uname),
                            html_escape(&last_syl_str)
                        ),
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
                schedule_turn_timeout(bot, st, room_arc).await;
                return;
            }

            // C. 3-layer validation
            let in_bank = st.app.db.word_exists(chat_id.0, &word).await;
            let mut valid = in_bank;
            if !valid {
                if !wiki::is_valid_for_lang(&word, &lang) {
                    room_arc.lock().await.streaks.insert(uid, 0);
                    st.app.db.lb_reset_streak(chat_id.0, uid).await;
                    let _ = bot
                        .send_message(chat_id, "⚠️ Từ không đúng định dạng ngôn ngữ hiện tại!")
                        .await;
                    schedule_turn_timeout(bot, st, room_arc).await;
                    return;
                }
                if st.app.db.cache_exists(&lang, &word).await {
                    valid = true;
                } else {
                    let (ok, _) = wiki::check_word(
                        &st.app.http,
                        &st.app.db,
                        &st.app.cache,
                        chat_id.0,
                        &lang,
                        &word,
                    )
                    .await;
                    valid = ok;
                }
            }

            if !valid {
                room_arc.lock().await.streaks.insert(uid, 0);
                st.app.db.lb_reset_streak(chat_id.0, uid).await;
                let sugs =
                    wiki::find_continuations(&st.app.db, chat_id.0, &lang, &last_syl_str, &[], 4)
                        .await;
                let sug_txt = if sugs.is_empty() {
                    "<i>Không có gợi ý</i>".to_string()
                } else {
                    sugs.iter()
                        .map(|s| format!("<code>{}</code>", html_escape(s)))
                        .collect::<Vec<_>>()
                        .join("  •  ")
                };
                let _ = bot
                    .send_message(
                        chat_id,
                        format!(
                            "❓ <b>Không tìm thấy từ!</b> Từ <code>{}</code> không tồn tại trong từ điển Wikipedia ({})!\n\n\
                            💡 <b>Gợi ý:</b> {}\n\
                            📝 <i>Bạn có thể dùng <code>/gopthemtu {}</code> để đề xuất thêm từ này vào ngân hàng từ!</i>",
                            html_escape(&word),
                            lang.to_uppercase(),
                            sug_txt,
                            html_escape(&word)
                        ),
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
                schedule_turn_timeout(bot, st, room_arc).await;
                return;
            }

            // Valid word!
            let (new_syl, streak_now) = {
                let mut g = room_arc.lock().await;
                g.used_words.insert(word.to_lowercase());
                g.current_word = word.clone();
                let syl = wiki::last_syl(&word, &g.lang);
                g.last_syllable = syl.clone();
                *g.scores.entry(uid).or_insert(0) += 1;
                let st_val = g.streaks.entry(uid).or_insert(0);
                *st_val += 1;
                (syl, *st_val)
            };

            st.app.db.lb_add_word(chat_id.0, uid).await;
            st.app.db.game_submit_score("noitu", uid, 1).await;

            let streak_emoji = if streak_now >= 3 {
                format!(" 🔥x{}", streak_now)
            } else {
                String::new()
            };

            if is_bot_mode {
                let _ = bot
                    .send_message(
                        chat_id,
                        format!(
                            "✅ <b>Hợp lệ!</b> {}: <code>{}</code> (+1đ){}\n🤖 <i>Bot đang suy nghĩ từ nối tiếp...</i>",
                            user_mention(uid, &uname),
                            html_escape(&word),
                            streak_emoji
                        ),
                    )
                    .parse_mode(ParseMode::Html)
                    .await;

                let used_list: Vec<String> = {
                    let g = room_arc.lock().await;
                    g.used_words.iter().cloned().collect()
                };
                let bot_sugs = wiki::find_continuations(
                    &st.app.db, chat_id.0, &lang, &new_syl, &used_list, 10,
                )
                .await;
                let picked = {
                    let g = room_arc.lock().await;
                    let avail: Vec<String> = bot_sugs
                        .into_iter()
                        .filter(|w| !g.used_words.contains(&w.to_lowercase()))
                        .collect();
                    avail.choose(&mut rand::thread_rng()).cloned()
                };

                if let Some(bot_word) = picked {
                    let bot_syl = {
                        let mut g = room_arc.lock().await;
                        g.used_words.insert(bot_word.to_lowercase());
                        g.current_word = bot_word.clone();
                        let s = wiki::last_syl(&bot_word, &g.lang);
                        g.last_syllable = s.clone();
                        s
                    };
                    let _ = bot
                        .send_message(
                            chat_id,
                            format!(
                                "🤖 <b>Bot nối:</b> <code>{}</code> ⚡\n\n\
                                👉 <b>Lượt của bạn:</b> Nối từ bắt đầu bằng <b><code>{}</code></b>\n\
                                ⏳ Thời gian: {} giây",
                                html_escape(&bot_word),
                                html_escape(&bot_syl),
                                st.app.config.turn_timeout
                            ),
                        )
                        .parse_mode(ParseMode::Html)
                        .await;
                    schedule_turn_timeout(bot, st, room_arc).await;
                } else {
                    let jt = {
                        let mut g = room_arc.lock().await;
                        g.cancel_timeout();
                        g.join_token.clone()
                    };
                    if !jt.is_empty() {
                        st.room_tokens.remove(&jt);
                    }
                    st.app.db.game_submit_score("noitu", uid, 10).await;
                    st.noitu_rooms.remove(&chat_id.0);
                    let _ = bot
                        .send_message(
                            chat_id,
                            format!(
                                "👑 <b>BOT XIN CHỊU!</b> Không còn từ nào trong kho để nối tiếp âm tiết <b><code>{}</code></b>!\n\n\
                                🎉 {} đã chiến thắng vẻ vang! Thưởng <b>+10 điểm</b>!\n\
                                Gõ <code>/solo</code> để tiếp tục chinh phục ván mới!",
                                html_escape(&new_syl),
                                user_mention(uid, &uname)
                            ),
                        )
                        .parse_mode(ParseMode::Html)
                        .await;
                }
                return;
            }

            let nxt_player = {
                let mut g = room_arc.lock().await;
                g.turn_index = (g.turn_index + 1) % g.players.len();
                g.players[g.turn_index].clone()
            };

            let _ = bot
                .send_message(
                    chat_id,
                    format!(
                        "✅ <b>Chính xác!</b> {}: <code>{}</code> (+1đ){}\n\n\
                        ➡️ <b>Lượt tiếp theo:</b> {}\n\
                        🔤 Âm tiết bắt đầu: <b><code>{}</code></b> ⏳ ({}s)",
                        user_mention(uid, &uname),
                        html_escape(&word),
                        streak_emoji,
                        user_mention(nxt_player.id, &nxt_player.name),
                        html_escape(&new_syl),
                        st.app.config.turn_timeout
                    ),
                )
                .parse_mode(ParseMode::Html)
                .await;

            schedule_turn_timeout(bot, st, room_arc).await;
            return;
        }
    }

    // 2. Active Vua Tiếng Việt
    if let Some(vtv) = st.active_vtv.get(&chat_id.0).map(|v| v.clone()) {
        let clean_ans = raw_text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if clean_ans == vtv.target.to_lowercase() {
            let elapsed = vtv.started_at.elapsed().as_secs_f64();
            st.active_vtv.remove(&chat_id.0);
            st.app.db.game_submit_score("vuatiengviet", uid, 20).await;
            let _ = bot
                .send_message(
                    chat_id,
                    format!(
                        "🎉 <b>XUẤT SẮC!</b> {} đã giải được câu đố!\n\n\
                        ✅ <b>Đáp án:</b> <code>{}</code>\n\
                        ⏱️ <b>Thời gian phản xạ:</b> {:.2} giây\n\
                        💎 <b>Phần thưởng:</b> +20 điểm tích lũy!",
                        user_mention(uid, &uname),
                        html_escape(&vtv.target),
                        elapsed
                    ),
                )
                .parse_mode(ParseMode::Html)
                .await;
            return;
        }
    }

    // 3. Active Wordle
    if let Some(mut wd) = st.active_wordle.get_mut(&chat_id.0) {
        let guess = raw_text.trim().to_uppercase();
        if guess.len() == wd.target.len() && guess.chars().all(|c| c.is_ascii_alphabetic()) {
            let target_chars: Vec<char> = wd.target.chars().collect();
            let guess_chars: Vec<char> = guess.chars().collect();
            let mut tiles = ['⬛'; 5];
            let mut used = [false; 5];
            for i in 0..5.min(target_chars.len()) {
                if guess_chars[i] == target_chars[i] {
                    tiles[i] = '🟩';
                    used[i] = true;
                }
            }
            for i in 0..5.min(target_chars.len()) {
                if tiles[i] == '🟩' {
                    continue;
                }
                for j in 0..5.min(target_chars.len()) {
                    if !used[j] && guess_chars[i] == target_chars[j] {
                        tiles[i] = '🟨';
                        used[j] = true;
                        break;
                    }
                }
            }
            let tile_str: String = tiles
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            wd.attempts += 1;
            wd.history
                .push(format!("{}  <b>{}</b>", tile_str, html_escape(&guess)));
            let history_text = wd.history.join("\n");
            let attempts = wd.attempts;
            let max_attempts = wd.max_attempts;
            let target = wd.target.clone();

            if guess == target.to_uppercase() {
                drop(wd);
                st.active_wordle.remove(&chat_id.0);
                let pts = ((7usize.saturating_sub(attempts)) * 10).max(10) as i64;
                st.app.db.game_submit_score("wordle", uid, pts).await;
                let _ = bot
                    .send_message(
                        chat_id,
                        format!(
                            "🎉 <b>XIN CHÚC MỪNG!</b> {} đã đoán chính xác từ Wordle!\n\n{}\n\n🏆 <b>Lượt đoán:</b> {}/{}\n💎 <b>Phần thưởng:</b> +{} điểm tích lũy!",
                            user_mention(uid, &uname),
                            history_text,
                            attempts,
                            max_attempts,
                            pts
                        ),
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
                return;
            } else if attempts >= max_attempts {
                drop(wd);
                st.active_wordle.remove(&chat_id.0);
                let _ = bot
                    .send_message(
                        chat_id,
                        format!(
                            "💀 <b>HẾT LƯỢT ĐOÁN!</b> Rất tiếc bạn chưa tìm ra từ bí mật.\n\n{}\n\n🔑 <b>Từ đúng là:</b> <b><code>{}</code></b>\nGõ <code>/wordle</code> để thử lại ván mới!",
                            history_text,
                            html_escape(&target)
                        ),
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
                return;
            } else {
                let rem = max_attempts - attempts;
                let _ = bot
                    .send_message(
                        chat_id,
                        format!(
                            "📊 <b>Kết quả lượt đoán ({}/{}):</b>\n\n{}\n\n⏳ Còn lại: <b>{} lượt</b>. Hãy nhập tiếp từ 5 chữ cái!",
                            attempts, max_attempts, history_text, rem
                        ),
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
                return;
            }
        }
    }

    // 4. Multiplayer room creation wizard
    if let Some((_, step)) = st.mp_wizard.remove(&(chat_id.0, uid)) {
        let lower = raw_text.trim().to_lowercase();
        if matches!(lower.as_str(), "huy" | "hủy" | "cancel" | "stop" | "exit") {
            let _ = bot
                .send_message(chat_id, "⏹️ <b>Đã hủy tiến trình tạo phòng công khai.</b>")
                .parse_mode(ParseMode::Html)
                .await;
            return;
        }

        match step {
            MpWizardStep::Name => {
                let name: String = raw_text.trim().chars().take(50).collect();
                st.mp_wizard
                    .insert((chat_id.0, uid), MpWizardStep::Lang { name: name.clone() });
                let _ = bot
                    .send_message(
                        chat_id,
                        format!(
                            "🏷️ <b>Tên phòng:</b> <b>{}</b>\n\n\
                            Bước 2/3: Vui lòng nhập <b>Mã ngôn ngữ</b> (Hơn 300 ngôn ngữ trên Wikipedia):\n\
                            <i>Ví dụ: vi (Tiếng Việt), en (English), fr (Pháp), ja (Nhật), ko (Hàn), zh (Trung)...</i>\n\n\
                            💡 <i>Gõ <code>hủy</code> để thoát tạo phòng.</i>",
                            html_escape(&name)
                        ),
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
            }
            MpWizardStep::Lang { name } => {
                let lang: String = lower.chars().take(10).collect();
                st.mp_wizard.insert(
                    (chat_id.0, uid),
                    MpWizardStep::MaxPlayers {
                        name,
                        lang: lang.clone(),
                    },
                );
                let _ = bot
                    .send_message(
                        chat_id,
                        format!(
                            "🌐 <b>Ngôn ngữ:</b> <code>{}</code>\n\n\
                            Bước 3/3: Vui lòng nhập <b>Số người tối đa</b> có thể vào phòng:\n\
                            👉 <i>Nhập <b>0</b> nếu muốn <b>VÔ HẠN</b> số người vào chơi! Hoặc nhập số cụ thể (2, 4, 8...).</i>\n\n\
                            💡 <i>Gõ <code>hủy</code> để thoát tạo phòng.</i>",
                            lang.to_uppercase()
                        ),
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
            }
            MpWizardStep::MaxPlayers { name, lang } => {
                let max_p = raw_text.trim().parse::<i64>().unwrap_or(0).max(0);
                execute_create_public_room(bot, chat_id, uid, uname, st, name, lang, max_p).await;
            }
        }
    }
}
