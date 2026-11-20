use dashmap::DashMap;
use poise::serenity_prelude as serenity;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use zenne_core::{i18n, wiki, AppContext};

#[derive(Debug, Clone, Default)]
pub struct WordChainRoom {
    pub active: bool,
    pub players: Vec<u64>,
    pub turn_idx: usize,
    pub last_word: String,
    pub used: HashSet<String>,
    pub scores: HashMap<u64, u32>,
    pub round: u32,
    pub bot_mode: bool,
    pub timeout_handle: Option<tokio::task::AbortHandle>,
}

impl WordChainRoom {
    pub fn cancel_timeout(&mut self) {
        if let Some(h) = self.timeout_handle.take() {
            h.abort();
        }
    }
}

#[derive(Clone, Default)]
pub struct NoituManager {
    pub rooms: Arc<DashMap<u64, Arc<Mutex<WordChainRoom>>>>,
}

impl NoituManager {
    pub fn get_room(&self, channel_id: u64) -> Arc<Mutex<WordChainRoom>> {
        self.rooms
            .entry(channel_id)
            .or_insert_with(|| Arc::new(Mutex::new(WordChainRoom::default())))
            .clone()
    }
}

pub async fn handle_word_message(
    ctx_ser: &serenity::Context,
    msg: &serenity::Message,
    app: &AppContext,
    noitu: &NoituManager,
) -> bool {
    let ch_id = msg.channel_id.get();
    let Some(room_arc) = noitu.rooms.get(&ch_id).map(|r| r.clone()) else {
        return false;
    };

    let word = wiki::normalize_word(&msg.content);
    if word.is_empty() || word.starts_with('!') || word.starts_with('/') {
        return false;
    }

    let guild_id = msg.guild_id.map(|g| g.get() as i64).unwrap_or(0);
    let lang = app.db.get_lang(guild_id).await;
    let ui_lang = app.db.get_ui_lang(guild_id).await;
    let uid = msg.author.id.get();

    let room = room_arc.lock().await;
    if !room.active {
        return false;
    }

    if room.bot_mode && !room.players.is_empty() && room.players[0] != uid {
        return false;
    }
    if !room.bot_mode && room.players.len() > 1 {
        let curr_turn = room.players.get(room.turn_idx).copied();
        if curr_turn != Some(uid) {
            return false;
        }
    }

    if !room.last_word.is_empty() && !wiki::valid_chain(&room.last_word, &word, &lang) {
        let req_syl = wiki::last_syl(&room.last_word, &lang);
        drop(room);
        app.db.lb_reset_streak(guild_id, uid as i64).await;
        let reply = i18n::t(
            &ui_lang,
            "wrong_chain",
            &[
                ("name", &msg.author.name),
                ("word", &word),
                ("syl", &req_syl),
                ("hint", ""),
            ],
        );
        let _ = msg.reply(&ctx_ser.http, reply).await;
        return true;
    }

    if room.used.contains(&word) {
        let reply = i18n::t(&ui_lang, "word_used", &[("word", &word)]);
        let _ = msg.reply(&ctx_ser.http, reply).await;
        return true;
    }

    drop(room);
    let (valid, _extract) =
        wiki::check_word(&app.http, &app.db, &app.cache, guild_id, &lang, &word).await;

    let mut room = room_arc.lock().await;
    if !room.active {
        return false;
    }

    if !valid {
        drop(room);
        app.db.lb_reset_streak(guild_id, uid as i64).await;
        let langname = i18n::lang_display_name(&lang);
        let reply = i18n::t(
            &ui_lang,
            "wiki_not_found",
            &[("word", &word), ("langname", &langname), ("hint", "")],
        );
        let _ = msg.reply(&ctx_ser.http, reply).await;
        return true;
    }

    room.cancel_timeout();
    room.used.insert(word.clone());
    room.last_word = word.clone();
    room.round += 1;
    *room.scores.entry(uid).or_insert(0) += 1;
    if !room.players.contains(&uid) {
        room.players.push(uid);
    }
    if !room.players.is_empty() {
        room.turn_idx = (room.turn_idx + 1) % room.players.len();
    }

    let next_syl = wiki::last_syl(&word, &lang);
    let _ = msg.react(&ctx_ser.http, '✅').await;

    let bot_mode = room.bot_mode;
    let used_vec: Vec<String> = room.used.iter().cloned().collect();
    let timeout_secs = app.config.turn_timeout;
    let http_c = ctx_ser.http.clone();
    let ch_obj = msg.channel_id;
    let room_c = room_arc.clone();
    let app_c = app.clone();
    let expected_round = room.round;
    let handle = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)).await;
        let mut r = room_c.lock().await;
        if !r.active || r.round != expected_round {
            return;
        }
        r.active = false;
        let players_i64: Vec<i64> = r.players.iter().map(|&id| id as i64).collect();
        let winner = r
            .scores
            .iter()
            .max_by_key(|(_, s)| *s)
            .map(|(id, _)| *id as i64);
        drop(r);
        if !players_i64.is_empty() {
            app_c.db.lb_add_game(guild_id, &players_i64, winner).await;
        }
        let _ = ch_obj
            .say(
                &http_c,
                format!(
                    "⏰ Hết {} giây không ai nối tiếp! Ván Nối Từ kết thúc.",
                    timeout_secs
                ),
            )
            .await;
    })
    .abort_handle();
    room.timeout_handle = Some(handle);
    drop(room);

    app.db.lb_add_word(guild_id, uid as i64).await;
    app.db.tournament_add_word(guild_id, uid as i64).await;

    if bot_mode {
        let cands =
            wiki::find_continuations(&app.db, guild_id, &lang, &next_syl, &used_vec, 5).await;
        if let Some(bot_word) = cands.first() {
            let mut room = room_arc.lock().await;
            room.used.insert(bot_word.clone());
            room.last_word = bot_word.clone();
            room.round += 1;
            let bot_next = wiki::last_syl(bot_word, &lang);
            drop(room);
            let _ = msg
                .channel_id
                .say(
                    &ctx_ser.http,
                    format!(
                        "🤖 Bot nối: **{}** → Tiếp theo bắt đầu bằng **`{}`**",
                        bot_word, bot_next
                    ),
                )
                .await;
        } else {
            let mut room = room_arc.lock().await;
            room.active = false;
            let _ = msg
                .channel_id
                .say(
                    &ctx_ser.http,
                    format!(
                        "🏆 Bạn đã thắng Bot! Không còn từ nào bắt đầu bằng **`{}`**.",
                        next_syl
                    ),
                )
                .await;
        }
    }

    true
}
