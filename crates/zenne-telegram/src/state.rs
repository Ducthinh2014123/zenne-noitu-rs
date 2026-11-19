use dashmap::DashMap;
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use teloxide::types::{ChatId, MessageId, User};
use tokio::sync::Mutex;
use tokio::task::AbortHandle;
use zenne_core::AppContext;
use zenne_games::quiz::QuizQuestion;
use zenne_games::GameHub;

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn user_mention(user_id: i64, name: &str) -> String {
    format!(
        "<a href=\"tg://user?id={}\">{}</a>",
        user_id,
        html_escape(name)
    )
}

pub fn user_display_name(user: Option<&User>) -> (i64, String) {
    match user {
        Some(u) => (u.id.0 as i64, u.first_name.clone()),
        None => (0, "Bạn".to_string()),
    }
}

pub fn gen_room_code() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 3] = rng.gen();
    format!("{:02X}{:02X}{:02X}", bytes[0], bytes[1], bytes[2])
}

pub fn gen_join_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 8] = rng.gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[derive(Debug, Clone)]
pub struct TelePlayer {
    pub id: i64,
    pub name: String,
}

pub struct NoituRoomState {
    pub chat_id: ChatId,
    pub host_id: i64,
    pub host_name: String,
    pub lang: String,
    pub is_bot_mode: bool,
    pub active: bool,
    pub joining: bool,
    pub lobby_msg_id: Option<MessageId>,
    pub players: Vec<TelePlayer>,
    pub turn_index: usize,
    pub current_word: String,
    pub last_syllable: String,
    pub used_words: HashSet<String>,
    pub scores: HashMap<i64, i64>,
    pub streaks: HashMap<i64, i64>,
    pub timeout_handle: Option<AbortHandle>,
    pub room_code: String,
    pub join_token: String,
    pub join_url: String,
    pub max_players: i64,
}

impl NoituRoomState {
    pub fn cancel_timeout(&mut self) {
        if let Some(h) = self.timeout_handle.take() {
            h.abort();
        }
    }
}

#[derive(Debug, Clone)]
pub struct VtvSession {
    pub target: String,
    pub scrambled: String,
    pub started_at: std::time::Instant,
}

#[derive(Debug, Clone)]
pub struct TaiXiuTable {
    pub accepting: bool,
    pub bets: HashMap<i64, String>,
    pub names: HashMap<i64, String>,
}

#[derive(Debug, Clone)]
pub struct BauCuaTable {
    pub bets: HashMap<i64, HashMap<String, i64>>,
    pub names: HashMap<i64, String>,
}

#[derive(Debug, Clone)]
pub struct CaDaoSession {
    pub prompt: String,
    pub answer: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ChinhTaSession {
    pub opt_a: String,
    pub opt_b: String,
    pub correct: String,
}

#[derive(Debug, Clone)]
pub struct FlagSession {
    pub flag: String,
    pub correct: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TrieuPhuQuestion {
    pub q: &'static str,
    pub options: [&'static str; 4],
    pub correct: usize,
}

#[derive(Debug, Clone)]
pub struct TrieuPhuSession {
    pub player_id: i64,
    pub level: usize,
    pub lifeline_5050: bool,
    pub lifeline_audience: bool,
    pub current_q: TrieuPhuQuestion,
}

#[derive(Debug, Clone)]
pub struct WordleSession {
    pub target: String,
    pub attempts: usize,
    pub max_attempts: usize,
    pub history: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum MpWizardStep {
    Name,
    Lang { name: String },
    MaxPlayers { name: String, lang: String },
}

#[derive(Clone)]
pub struct TelegramState {
    pub app: AppContext,
    pub games: GameHub,
    pub noitu_rooms: Arc<DashMap<i64, Arc<Mutex<NoituRoomState>>>>,
    pub room_tokens: Arc<DashMap<String, i64>>,
    pub active_quiz: Arc<DashMap<i64, QuizQuestion>>,
    pub active_vtv: Arc<DashMap<i64, VtvSession>>,
    pub active_taixiu: Arc<DashMap<i64, Arc<Mutex<TaiXiuTable>>>>,
    pub active_baucua: Arc<DashMap<i64, Arc<Mutex<BauCuaTable>>>>,
    pub active_cadao: Arc<DashMap<i64, CaDaoSession>>,
    pub active_chinhta: Arc<DashMap<i64, ChinhTaSession>>,
    pub active_flag: Arc<DashMap<i64, FlagSession>>,
    pub active_trieuphu: Arc<DashMap<i64, TrieuPhuSession>>,
    pub active_wordle: Arc<DashMap<i64, WordleSession>>,
    pub mp_wizard: Arc<DashMap<(i64, i64), MpWizardStep>>,
}

impl TelegramState {
    pub fn new(app: AppContext, games: GameHub) -> Self {
        Self {
            app,
            games,
            noitu_rooms: Arc::new(DashMap::new()),
            room_tokens: Arc::new(DashMap::new()),
            active_quiz: Arc::new(DashMap::new()),
            active_vtv: Arc::new(DashMap::new()),
            active_taixiu: Arc::new(DashMap::new()),
            active_baucua: Arc::new(DashMap::new()),
            active_cadao: Arc::new(DashMap::new()),
            active_chinhta: Arc::new(DashMap::new()),
            active_flag: Arc::new(DashMap::new()),
            active_trieuphu: Arc::new(DashMap::new()),
            active_wordle: Arc::new(DashMap::new()),
            mp_wizard: Arc::new(DashMap::new()),
        }
    }

    pub fn web_base_url(&self) -> String {
        let base = self.app.config.web_base_url.trim().trim_end_matches('/');
        if base.is_empty() {
            "https://zenne-noitu.vercel.app".to_string()
        } else {
            base.to_string()
        }
    }
}
