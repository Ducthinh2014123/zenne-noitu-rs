use axum::extract::ws::{Message, WebSocket};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;
use zenne_core::{wiki, AppContext};

#[derive(Clone)]
pub struct WsPlayer {
    pub conn_id: String,
    pub user_id: i64,
    pub name: String,
    pub lives: i64,
    pub score: i64,
    pub hints_left: i64,
    pub tx: mpsc::UnboundedSender<String>,
}

pub struct WsRoom {
    pub room_id: String,
    pub lang: String,
    pub state: String, // "waiting" | "playing" | "ended"
    pub host_conn_id: String,
    pub players: Vec<WsPlayer>,
    pub turn_idx: usize,
    pub current_word: String,
    pub last_char: String,
    pub used_words: HashSet<String>,
    pub time_limit: u64,
}

impl WsRoom {
    fn new(room_id: String, lang: String) -> Self {
        Self {
            room_id,
            lang,
            state: "waiting".to_string(),
            host_conn_id: String::new(),
            players: Vec::new(),
            turn_idx: 0,
            current_word: String::new(),
            last_char: String::new(),
            used_words: HashSet::new(),
            time_limit: 30,
        }
    }

    fn current_turn_name(&self) -> String {
        if self.state != "playing" || self.players.is_empty() {
            return String::new();
        }
        self.players
            .get(self.turn_idx)
            .map(|p| p.name.clone())
            .unwrap_or_default()
    }

    fn players_json(&self) -> Vec<Value> {
        self.players
            .iter()
            .map(|p| {
                json!({
                    "name": p.name,
                    "lives": p.lives,
                    "score": p.score,
                })
            })
            .collect()
    }

    fn state_payload(&self) -> String {
        json!({
            "type": "state",
            "state": self.state,
            "players": self.players_json(),
            "current_turn": self.current_turn_name(),
            "current_word": self.current_word,
            "last_char": self.last_char,
            "time_limit": self.time_limit,
        })
        .to_string()
    }

    fn broadcast(&self, payload: &str) {
        for p in &self.players {
            let _ = p.tx.send(payload.to_string());
        }
    }

    fn broadcast_except(&self, skip_conn_id: &str, payload: &str) {
        for p in &self.players {
            if p.conn_id != skip_conn_id {
                let _ = p.tx.send(payload.to_string());
            }
        }
    }

    fn advance_turn(&mut self) {
        if self.players.is_empty() {
            return;
        }
        let len = self.players.len();
        for offset in 1..=len {
            let idx = (self.turn_idx + offset) % len;
            if self.players[idx].lives > 0 {
                self.turn_idx = idx;
                return;
            }
        }
    }

    fn alive_count(&self) -> usize {
        self.players.iter().filter(|p| p.lives > 0).count()
    }

    fn check_game_end(&mut self) -> Option<String> {
        if self.state != "playing" {
            return None;
        }
        let alive: Vec<&WsPlayer> = self.players.iter().filter(|p| p.lives > 0).collect();
        let should_end = if self.players.len() <= 1 {
            alive.is_empty()
        } else {
            alive.len() <= 1
        };
        if !should_end {
            return None;
        }
        self.state = "ended".to_string();
        let mut sorted: Vec<&WsPlayer> = self.players.iter().collect();
        sorted.sort_by_key(|a| std::cmp::Reverse(a.score));
        let winner = alive
            .first()
            .map(|p| p.name.clone())
            .or_else(|| sorted.first().map(|p| p.name.clone()));
        let scores: Vec<Value> = sorted
            .iter()
            .map(|p| json!({ "name": p.name, "score": p.score }))
            .collect();
        Some(
            json!({
                "type": "game_end",
                "winner": winner,
                "scores": scores,
            })
            .to_string(),
        )
    }
}

pub type WsRoomsMap = Arc<DashMap<String, Arc<Mutex<WsRoom>>>>;

pub async fn handle_ws_connection(
    socket: WebSocket,
    room_id: String,
    ctx: AppContext,
    rooms: WsRoomsMap,
) {
    let rid = if room_id.trim().is_empty() {
        "LOBBY".to_string()
    } else {
        room_id.trim().to_uppercase()
    };

    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let writer_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    let room_arc = {
        if let Some(existing) = rooms.get(&rid) {
            existing.value().clone()
        } else {
            let mut lang = "vi".to_string();
            for r in ctx.db.get_public_rooms().await {
                if r.room_id.eq_ignore_ascii_case(&rid) {
                    lang = r.lang;
                    break;
                }
            }
            let r = Arc::new(Mutex::new(WsRoom::new(rid.clone(), lang)));
            rooms.insert(rid.clone(), r.clone());
            r
        }
    };

    let conn_id = Uuid::new_v4().to_string();
    let mut joined = false;

    while let Some(Ok(msg)) = ws_rx.next().await {
        let Message::Text(raw) = msg else {
            continue;
        };
        let Ok(val) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match msg_type {
            "join" => {
                let token = val.get("token").and_then(|v| v.as_str()).unwrap_or("");
                let Some(claims) =
                    zenne_core::auth::verify_web_token(&ctx.config.web_token_secret, token)
                else {
                    let _ = tx.send(
                        json!({
                            "type": "error",
                            "msg": "Phiên đăng nhập không hợp lệ hoặc đã hết hạn."
                        })
                        .to_string(),
                    );
                    break;
                };
                let token_uid = claims.get("uid").and_then(|v| v.as_i64()).unwrap_or(0);
                if token_uid == 0 {
                    let _ = tx.send(
                        json!({
                            "type": "error",
                            "msg": "Token không hợp lệ (thiếu user_id)."
                        })
                        .to_string(),
                    );
                    break;
                }
                let token_name = claims
                    .get("username")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty());

                let name = token_name
                    .or_else(|| {
                        val.get("name")
                            .and_then(|v| v.as_str())
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty())
                    })
                    .unwrap_or("Người chơi")
                    .to_string();

                let mut room = room_arc.lock().await;
                if room
                    .players
                    .iter()
                    .any(|p| p.conn_id == conn_id || (token_uid != 0 && p.user_id == token_uid))
                {
                    let _ = tx.send(
                        json!({
                            "type": "error",
                            "msg": "Bạn đã ở trong phòng này rồi."
                        })
                        .to_string(),
                    );
                    continue;
                }
                if room.players.is_empty() {
                    room.host_conn_id = conn_id.clone();
                }
                let is_host = room.host_conn_id == conn_id;
                room.players.push(WsPlayer {
                    conn_id: conn_id.clone(),
                    user_id: token_uid,
                    name: name.clone(),
                    lives: 3,
                    score: 0,
                    hints_left: 3,
                    tx: tx.clone(),
                });
                joined = true;

                let joined_payload = json!({
                    "type": "joined",
                    "is_host": is_host,
                    "time_limit": room.time_limit,
                    "state": room.state,
                    "hints_left": 3,
                    "players": room.players_json(),
                    "current_turn": room.current_turn_name(),
                    "current_word": room.current_word,
                    "last_char": room.last_char,
                })
                .to_string();
                let _ = tx.send(joined_payload);

                let pj = json!({ "type": "player_joined", "name": name }).to_string();
                room.broadcast_except(&conn_id, &pj);
                room.broadcast(&room.state_payload());
                let p_count = room.players.len() as i64;
                drop(room);

                ctx.db.update_public_room_players(&rid, p_count).await;
            }
            "start" => {
                let (can_start, lang) = {
                    let room = room_arc.lock().await;
                    (
                        room.host_conn_id == conn_id && room.state == "waiting",
                        room.lang.clone(),
                    )
                };
                if !can_start {
                    continue;
                }
                let start_word = wiki::random_word(&ctx.http, &ctx.db, &lang).await;
                let mut room = room_arc.lock().await;
                if room.state != "waiting" {
                    continue;
                }
                let last_syl = wiki::last_syl(&start_word, &room.lang);
                room.state = "playing".to_string();
                room.current_word = start_word.clone();
                room.last_char = last_syl.clone();
                room.used_words.clear();
                room.used_words.insert(start_word.to_lowercase());
                room.turn_idx = 0;
                for p in &mut room.players {
                    p.lives = 3;
                    p.score = 0;
                    p.hints_left = 3;
                }
                let payload = json!({
                    "type": "game_start",
                    "time_limit": room.time_limit,
                    "current_word": room.current_word,
                    "last_char": room.last_char,
                    "current_turn": room.current_turn_name(),
                    "players": room.players_json(),
                })
                .to_string();
                room.broadcast(&payload);
                room.broadcast(&room.state_payload());
            }
            "word" => {
                let raw_word = val.get("word").and_then(|v| v.as_str()).unwrap_or("");
                let word = wiki::normalize_word(raw_word);
                if word.is_empty() {
                    continue;
                }

                let (is_turn, player_name, curr_word, lang, already_used) = {
                    let room = room_arc.lock().await;
                    if room.state != "playing" || room.players.is_empty() {
                        (false, String::new(), String::new(), String::new(), false)
                    } else {
                        let curr = &room.players[room.turn_idx];
                        (
                            curr.conn_id == conn_id && curr.lives > 0,
                            curr.name.clone(),
                            room.current_word.clone(),
                            room.lang.clone(),
                            room.used_words.contains(&word.to_lowercase()),
                        )
                    }
                };

                if !is_turn {
                    continue;
                }

                {
                    let room = room_arc.lock().await;
                    room.broadcast(&json!({ "type": "checking" }).to_string());
                }

                if already_used {
                    let mut room = room_arc.lock().await;
                    if let Some(p) = room.players.iter_mut().find(|p| p.conn_id == conn_id) {
                        p.lives = (p.lives - 1).max(0);
                        let lives_left = p.lives;
                        let used_msg = json!({
                            "type": "word_used",
                            "player": player_name,
                            "word": word,
                            "lives": lives_left,
                        })
                        .to_string();
                        room.broadcast(&used_msg);
                        if lives_left == 0 {
                            let elim =
                                json!({ "type": "eliminated", "player": player_name }).to_string();
                            room.broadcast(&elim);
                        }
                    }
                    if let Some(end_msg) = room.check_game_end() {
                        room.broadcast(&end_msg);
                    } else {
                        room.advance_turn();
                        room.broadcast(&room.state_payload());
                    }
                    continue;
                }

                let chain_ok = wiki::valid_chain(&curr_word, &word, &lang);
                let mut word_ok = false;
                if chain_ok && wiki::is_valid_for_lang(&word, &lang) {
                    if ctx.db.word_exists(0, &word).await || ctx.db.cache_exists(&lang, &word).await
                    {
                        word_ok = true;
                    } else {
                        let (ok, _) =
                            wiki::check_word(&ctx.http, &ctx.db, &ctx.cache, 0, &lang, &word).await;
                        word_ok = ok;
                    }
                }

                let mut room = room_arc.lock().await;
                if room.state != "playing" {
                    continue;
                }

                if !chain_ok || !word_ok {
                    if let Some(p) = room.players.iter_mut().find(|p| p.conn_id == conn_id) {
                        p.lives = (p.lives - 1).max(0);
                        let lives_left = p.lives;
                        let inv_msg = json!({
                            "type": "word_invalid",
                            "player": player_name,
                            "word": word,
                            "lives": lives_left,
                        })
                        .to_string();
                        room.broadcast(&inv_msg);
                        if lives_left == 0 {
                            let elim =
                                json!({ "type": "eliminated", "player": player_name }).to_string();
                            room.broadcast(&elim);
                        }
                    }
                    if let Some(end_msg) = room.check_game_end() {
                        room.broadcast(&end_msg);
                    } else {
                        room.advance_turn();
                        room.broadcast(&room.state_payload());
                    }
                } else {
                    room.used_words.insert(word.to_lowercase());
                    room.current_word = word.clone();
                    room.last_char = wiki::last_syl(&word, &room.lang);
                    let mut p_uid = 0i64;
                    if let Some(p) = room.players.iter_mut().find(|p| p.conn_id == conn_id) {
                        p.score += 1;
                        p_uid = p.user_id;
                    }
                    let acc_msg = json!({
                        "type": "word_accepted",
                        "player": player_name,
                        "word": word,
                        "score": 1,
                    })
                    .to_string();
                    room.broadcast(&acc_msg);
                    if room.alive_count() > 1 {
                        room.advance_turn();
                    }
                    room.broadcast(&room.state_payload());
                    drop(room);
                    if p_uid != 0 {
                        ctx.db.lb_add_word(0, p_uid).await;
                    }
                }
            }
            "hint" => {
                let (allowed, hints_left, syl, lang, used) = {
                    let mut room = room_arc.lock().await;
                    if let Some(p) = room.players.iter_mut().find(|p| p.conn_id == conn_id) {
                        if p.hints_left <= 0 {
                            (false, 0, String::new(), String::new(), Vec::new())
                        } else {
                            p.hints_left -= 1;
                            (
                                true,
                                p.hints_left,
                                room.last_char.clone(),
                                room.lang.clone(),
                                room.used_words.iter().cloned().collect(),
                            )
                        }
                    } else {
                        (false, 0, String::new(), String::new(), Vec::new())
                    }
                };
                if !allowed {
                    let _ = tx.send(json!({ "type": "hint_limit_reached" }).to_string());
                    continue;
                }
                let words = wiki::find_continuations(&ctx.db, 0, &lang, &syl, &used, 5).await;
                let can_continue = !words.is_empty();
                let _ = tx.send(
                    json!({
                        "type": "hint_result",
                        "syllable": syl,
                        "can_continue": can_continue,
                        "words": words,
                        "hints_left": hints_left,
                    })
                    .to_string(),
                );
            }
            "restart" => {
                let mut room = room_arc.lock().await;
                if room.host_conn_id != conn_id {
                    continue;
                }
                room.state = "waiting".to_string();
                room.current_word.clear();
                room.last_char.clear();
                room.used_words.clear();
                room.turn_idx = 0;
                for p in &mut room.players {
                    p.lives = 3;
                    p.score = 0;
                    p.hints_left = 3;
                }
                room.broadcast(&json!({ "type": "restarted" }).to_string());
                room.broadcast(&room.state_payload());
            }
            _ => {}
        }
    }

    if joined {
        let mut room = room_arc.lock().await;
        let leaving_name = room
            .players
            .iter()
            .find(|p| p.conn_id == conn_id)
            .map(|p| p.name.clone());
        room.players.retain(|p| p.conn_id != conn_id);

        if let Some(name) = leaving_name {
            room.broadcast(&json!({ "type": "player_left", "name": name }).to_string());
        }

        if room.players.is_empty() {
            drop(room);
            rooms.remove(&rid);
            ctx.db.close_public_room(&rid).await;
        } else {
            if room.host_conn_id == conn_id {
                let new_host_id = room.players[0].conn_id.clone();
                let new_host_name = room.players[0].name.clone();
                room.host_conn_id = new_host_id;
                room.broadcast(&json!({ "type": "new_host", "name": new_host_name }).to_string());
            }
            if room.turn_idx >= room.players.len()
                || room
                    .players
                    .get(room.turn_idx)
                    .map(|p| p.lives <= 0)
                    .unwrap_or(false)
            {
                if let Some(pos) = room.players.iter().position(|p| p.lives > 0) {
                    room.turn_idx = pos;
                } else {
                    room.turn_idx = 0;
                }
            }
            if let Some(end_msg) = room.check_game_end() {
                room.broadcast(&end_msg);
            } else {
                room.broadcast(&room.state_payload());
            }
            let p_count = room.players.len() as i64;
            drop(room);
            ctx.db.update_public_room_players(&rid, p_count).await;
        }
    } else if room_arc.lock().await.players.is_empty() {
        rooms.remove(&rid);
    }

    writer_task.abort();
}
