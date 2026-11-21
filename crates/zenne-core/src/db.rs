use crate::config::AppConfig;
use anyhow::Result;
use libsql::{Builder, Connection, Database};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingWord {
    pub id: i64,
    pub guild_id: i64,
    pub word: String,
    pub submitted_by: i64,
    pub submitted_at: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardRow {
    pub user_id: i64,
    pub total_words: i64,
    pub current_streak: i64,
    pub max_streak: i64,
    pub games_played: i64,
    pub games_won: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLeaderboardRow {
    pub user_id: i64,
    pub best_score: i64,
    pub plays: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomRow {
    pub room_id: String,
    pub name: String,
    pub platform: String,
    pub host_id: String,
    pub host_name: String,
    pub lang: String,
    pub max_players: i64,
    pub current_players: i64,
    pub join_token: Option<String>,
    pub web_url: Option<String>,
    pub tele_url: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUser {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: Option<String>,
    pub provider: String,
    pub is_admin: i64,
    pub totp_secret: Option<String>,
    pub totp_enabled: i64,
    pub email_verified: i64,
    pub ui_lang: Option<String>,
}

pub struct DbManager {
    db: Database,
    pub is_libsql: bool,
}

impl DbManager {
    pub async fn new(cfg: &AppConfig) -> Result<Self> {
        let (db, is_libsql) = if !cfg.database_url.is_empty() && !cfg.database_auth_token.is_empty()
        {
            info!("[DB] Dang ket noi toi libSQL remote: {}", cfg.database_url);
            let db = Builder::new_remote(cfg.database_url.clone(), cfg.database_auth_token.clone())
                .build()
                .await?;
            (db, true)
        } else {
            anyhow::bail!("Can DATABASE_URL va DATABASE_AUTH_TOKEN cho remote libSQL");
        };

        let mgr = Self { db, is_libsql };
        mgr.init_schema().await?;
        Ok(mgr)
    }

    pub fn conn(&self) -> Result<Connection> {
        Ok(self.db.connect()?)
    }

    async fn init_schema(&self) -> Result<()> {
        let conn = self.conn()?;
        let schema = r#"
        CREATE TABLE IF NOT EXISTS guild_settings (
            guild_id     INTEGER PRIMARY KEY,
            lang         TEXT    DEFAULT 'vi',
            game_channel INTEGER DEFAULT NULL,
            ui_lang      TEXT    DEFAULT 'vi'
        );
        CREATE TABLE IF NOT EXISTS word_bank (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            guild_id    INTEGER NOT NULL,
            word        TEXT    NOT NULL COLLATE NOCASE,
            added_by    INTEGER NOT NULL,
            approved_by INTEGER NOT NULL,
            added_at    TEXT    DEFAULT (datetime('now','localtime')),
            UNIQUE(guild_id, word)
        );
        CREATE TABLE IF NOT EXISTS pending_words (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            guild_id      INTEGER NOT NULL,
            word          TEXT    NOT NULL COLLATE NOCASE,
            submitted_by  INTEGER NOT NULL,
            submitted_at  TEXT    DEFAULT (datetime('now','localtime')),
            status        TEXT    DEFAULT 'pending',
            reviewed_by   INTEGER DEFAULT NULL,
            reviewed_at   TEXT    DEFAULT NULL,
            UNIQUE(guild_id, word)
        );
        CREATE TABLE IF NOT EXISTS wiki_cache (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            lang       TEXT    NOT NULL,
            word       TEXT    NOT NULL COLLATE NOCASE,
            cached_at  TEXT    DEFAULT (datetime('now','localtime')),
            hit_count  INTEGER DEFAULT 1,
            UNIQUE(lang, word)
        );
        CREATE TABLE IF NOT EXISTS leaderboard (
            guild_id        INTEGER NOT NULL,
            user_id         INTEGER NOT NULL,
            total_words     INTEGER DEFAULT 0,
            current_streak  INTEGER DEFAULT 0,
            max_streak      INTEGER DEFAULT 0,
            games_played    INTEGER DEFAULT 0,
            games_won       INTEGER DEFAULT 0,
            last_played     TEXT    DEFAULT NULL,
            PRIMARY KEY (guild_id, user_id)
        );
        CREATE TABLE IF NOT EXISTS tournaments (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            guild_id    INTEGER NOT NULL,
            channel_id  INTEGER NOT NULL,
            host_id     INTEGER NOT NULL,
            name        TEXT    NOT NULL,
            prize       TEXT    DEFAULT NULL,
            status      TEXT    DEFAULT 'open',
            created_at  TEXT    DEFAULT (datetime('now','localtime')),
            started_at  TEXT    DEFAULT NULL,
            ended_at    TEXT    DEFAULT NULL,
            message_id  INTEGER DEFAULT NULL
        );
        CREATE TABLE IF NOT EXISTS tournament_players (
            tournament_id  INTEGER NOT NULL,
            user_id        INTEGER NOT NULL,
            words_scored   INTEGER DEFAULT 0,
            joined_at      TEXT    DEFAULT (datetime('now','localtime')),
            PRIMARY KEY (tournament_id, user_id)
        );
        CREATE TABLE IF NOT EXISTS game_scores (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            game        TEXT    NOT NULL,
            user_id     INTEGER NOT NULL,
            best_score  INTEGER NOT NULL DEFAULT 0,
            plays       INTEGER NOT NULL DEFAULT 0,
            updated_at  TEXT    DEFAULT (datetime('now','localtime')),
            UNIQUE(game, user_id)
        );
        CREATE TABLE IF NOT EXISTS web_users (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            username      TEXT UNIQUE NOT NULL,
            email         TEXT UNIQUE NOT NULL,
            password_hash TEXT,
            provider      TEXT DEFAULT 'credentials',
            provider_id   TEXT,
            avatar        TEXT,
            is_admin      INTEGER DEFAULT 0,
            totp_secret   TEXT    DEFAULT NULL,
            totp_enabled  INTEGER DEFAULT 0,
            backup_codes  TEXT    DEFAULT NULL,
            email_verified INTEGER DEFAULT 0,
            ui_lang       TEXT    DEFAULT NULL,
            created_at    TEXT    DEFAULT (datetime('now','localtime'))
        );
        CREATE TABLE IF NOT EXISTS web_passkeys (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id       INTEGER NOT NULL,
            credential_id TEXT UNIQUE NOT NULL,
            public_key    TEXT NOT NULL,
            counter       INTEGER NOT NULL DEFAULT 0,
            transports    TEXT DEFAULT NULL,
            rp_id         TEXT NOT NULL,
            device_name   TEXT DEFAULT 'Passkey',
            created_at    TEXT DEFAULT (datetime('now','localtime'))
        );
        CREATE TABLE IF NOT EXISTS password_recovery (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id     INTEGER NOT NULL,
            token_hash  TEXT    NOT NULL,
            type        TEXT    NOT NULL,
            source      TEXT    DEFAULT NULL,
            expires_at  TEXT    NOT NULL,
            used_at     TEXT    DEFAULT NULL,
            attempts    INTEGER DEFAULT 0,
            created_at  TEXT    DEFAULT (datetime('now','localtime'))
        );
        CREATE TABLE IF NOT EXISTS public_rooms (
            room_id         TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            platform        TEXT NOT NULL,
            host_id         TEXT NOT NULL,
            host_name       TEXT NOT NULL,
            lang            TEXT DEFAULT 'vi',
            max_players     INTEGER DEFAULT 0,
            current_players INTEGER DEFAULT 1,
            join_token      TEXT UNIQUE,
            web_url         TEXT,
            tele_url        TEXT,
            status          TEXT DEFAULT 'waiting',
            created_at      TEXT DEFAULT (datetime('now','localtime')),
            updated_at      TEXT DEFAULT (datetime('now','localtime'))
        );
        CREATE TABLE IF NOT EXISTS telegram_room_tokens (
            token           TEXT PRIMARY KEY,
            room_code       TEXT NOT NULL,
            chat_id         INTEGER NOT NULL,
            host_id         INTEGER NOT NULL,
            host_name       TEXT NOT NULL,
            lang            TEXT DEFAULT 'vi',
            created_at      TEXT DEFAULT (datetime('now','localtime'))
        );
        CREATE INDEX IF NOT EXISTS idx_wb_guild ON word_bank(guild_id);
        CREATE INDEX IF NOT EXISTS idx_pw_guild ON pending_words(guild_id);
        CREATE INDEX IF NOT EXISTS idx_pw_status ON pending_words(status);
        CREATE INDEX IF NOT EXISTS idx_wc_lang ON wiki_cache(lang);
        CREATE INDEX IF NOT EXISTS idx_lb_guild ON leaderboard(guild_id);
        CREATE INDEX IF NOT EXISTS idx_gs_game ON game_scores(game);
        CREATE INDEX IF NOT EXISTS idx_wu_email ON web_users(email);
        CREATE INDEX IF NOT EXISTS idx_wu_prov ON web_users(provider, provider_id);
        CREATE INDEX IF NOT EXISTS idx_wp_user ON web_passkeys(user_id);
        CREATE INDEX IF NOT EXISTS idx_wp_cred ON web_passkeys(credential_id);
        CREATE INDEX IF NOT EXISTS idx_tn_guild ON tournaments(guild_id);
        CREATE INDEX IF NOT EXISTS idx_pr_user ON password_recovery(user_id);
        CREATE INDEX IF NOT EXISTS idx_pr_hash ON password_recovery(token_hash);
        CREATE INDEX IF NOT EXISTS idx_pr_type ON password_recovery(type);
        "#;
        conn.execute_batch(schema).await?;
        let migrations = [
            "ALTER TABLE guild_settings ADD COLUMN ui_lang TEXT DEFAULT 'vi'",
            "ALTER TABLE tournaments ADD COLUMN message_id INTEGER DEFAULT NULL",
            "ALTER TABLE web_users ADD COLUMN totp_secret TEXT DEFAULT NULL",
            "ALTER TABLE web_users ADD COLUMN totp_enabled INTEGER DEFAULT 0",
            "ALTER TABLE web_users ADD COLUMN backup_codes TEXT DEFAULT NULL",
            "ALTER TABLE web_users ADD COLUMN email_verified INTEGER DEFAULT 0",
            "ALTER TABLE web_users ADD COLUMN ui_lang TEXT DEFAULT NULL",
            "ALTER TABLE password_recovery ADD COLUMN source TEXT DEFAULT NULL",
            "ALTER TABLE password_recovery ADD COLUMN attempts INTEGER DEFAULT 0",
            "ALTER TABLE web_users ADD COLUMN password_changed_at INTEGER DEFAULT 0",
        ];
        for sql in migrations {
            let _ = conn.execute(sql, ()).await;
        }
        info!("[DB] Khoi tao bang du lieu hoan tat.");
        Ok(())
    }

    pub async fn get_lang(&self, guild_id: i64) -> String {
        let Ok(conn) = self.conn() else {
            return "vi".to_string();
        };
        if let Ok(mut rows) = conn
            .query(
                "SELECT lang FROM guild_settings WHERE guild_id = ?1",
                [guild_id],
            )
            .await
        {
            if let Ok(Some(row)) = rows.next().await {
                if let Ok(val) = row.get::<String>(0) {
                    return val;
                }
            }
        }
        "vi".to_string()
    }

    pub async fn set_lang(&self, guild_id: i64, lang: &str) {
        if let Ok(conn) = self.conn() {
            let _ = conn
                .execute(
                    "INSERT INTO guild_settings (guild_id, lang) VALUES (?1, ?2) ON CONFLICT(guild_id) DO UPDATE SET lang = excluded.lang",
                    (guild_id, lang),
                )
                .await;
        }
    }

    pub async fn get_ui_lang(&self, guild_id: i64) -> String {
        let Ok(conn) = self.conn() else {
            return "vi".to_string();
        };
        if let Ok(mut rows) = conn
            .query(
                "SELECT ui_lang FROM guild_settings WHERE guild_id = ?1",
                [guild_id],
            )
            .await
        {
            if let Ok(Some(row)) = rows.next().await {
                if let Ok(val) = row.get::<String>(0) {
                    if !val.is_empty() {
                        return val;
                    }
                }
            }
        }
        "vi".to_string()
    }

    pub async fn set_ui_lang(&self, guild_id: i64, ui_lang: &str) {
        if let Ok(conn) = self.conn() {
            let _ = conn
                .execute(
                    "INSERT INTO guild_settings (guild_id, ui_lang) VALUES (?1, ?2) ON CONFLICT(guild_id) DO UPDATE SET ui_lang = excluded.ui_lang",
                    (guild_id, ui_lang),
                )
                .await;
        }
    }

    pub async fn get_game_channel(&self, guild_id: i64) -> Option<i64> {
        let conn = self.conn().ok()?;
        let mut rows = conn
            .query(
                "SELECT game_channel FROM guild_settings WHERE guild_id = ?1",
                [guild_id],
            )
            .await
            .ok()?;
        if let Ok(Some(row)) = rows.next().await {
            return row.get::<Option<i64>>(0).ok().flatten();
        }
        None
    }

    pub async fn set_game_channel(&self, guild_id: i64, channel_id: i64) {
        if let Ok(conn) = self.conn() {
            let _ = conn
                .execute(
                    "INSERT INTO guild_settings (guild_id, game_channel) VALUES (?1, ?2) ON CONFLICT(guild_id) DO UPDATE SET game_channel = excluded.game_channel",
                    (guild_id, channel_id),
                )
                .await;
        }
    }

    pub async fn word_exists(&self, guild_id: i64, word: &str) -> bool {
        let Ok(conn) = self.conn() else { return false };
        if let Ok(mut rows) = conn
            .query(
                "SELECT 1 FROM word_bank WHERE guild_id = ?1 AND word = ?2 COLLATE NOCASE LIMIT 1",
                (guild_id, word),
            )
            .await
        {
            return matches!(rows.next().await, Ok(Some(_)));
        }
        false
    }

    pub async fn add_approved_word(
        &self,
        guild_id: i64,
        word: &str,
        added_by: i64,
        approved_by: i64,
    ) -> bool {
        let Ok(conn) = self.conn() else { return false };
        conn.execute(
            "INSERT OR IGNORE INTO word_bank (guild_id, word, added_by, approved_by) VALUES (?1, ?2, ?3, ?4)",
            (guild_id, word, added_by, approved_by),
        )
        .await
        .map(|n| n > 0)
        .unwrap_or(false)
    }

    pub async fn submit_pending(
        &self,
        guild_id: i64,
        word: &str,
        submitted_by: i64,
    ) -> Result<i64, String> {
        let conn = self.conn().map_err(|e| e.to_string())?;
        if self.word_exists(guild_id, word).await {
            return Err("Từ này đã có trong Ngân hàng từ của server rồi!".to_string());
        }
        let res = conn
            .execute(
                "INSERT INTO pending_words (guild_id, word, submitted_by) VALUES (?1, ?2, ?3)
                 ON CONFLICT(guild_id, word) DO UPDATE SET
                    status = 'pending',
                    submitted_by = excluded.submitted_by,
                    submitted_at = datetime('now','localtime'),
                    reviewed_by = NULL,
                    reviewed_at = NULL
                 WHERE pending_words.status = 'rejected'",
                (guild_id, word, submitted_by),
            )
            .await
            .map_err(|_| "Từ này đã được gửi chờ duyệt trước đó!".to_string())?;
        if res == 0 {
            return Err("Từ này đã được gửi chờ duyệt trước đó!".to_string());
        }
        let mut rows = conn
            .query(
                "SELECT id FROM pending_words WHERE guild_id = ?1 AND word = ?2",
                (guild_id, word),
            )
            .await
            .map_err(|e| e.to_string())?;
        if let Ok(Some(r)) = rows.next().await {
            Ok(r.get::<i64>(0).unwrap_or(0))
        } else {
            Ok(0)
        }
    }

    pub async fn get_pending(&self, guild_id: Option<i64>, status: &str) -> Vec<PendingWord> {
        let Ok(conn) = self.conn() else { return vec![] };
        let mut out = Vec::new();
        let rows_res = if let Some(gid) = guild_id {
            conn.query(
                "SELECT id, guild_id, word, submitted_by, COALESCE(submitted_at,''), status FROM pending_words WHERE guild_id = ?1 AND status = ?2 ORDER BY id DESC LIMIT 100",
                (gid, status),
            )
            .await
        } else {
            conn.query(
                "SELECT id, guild_id, word, submitted_by, COALESCE(submitted_at,''), status FROM pending_words WHERE status = ?1 ORDER BY id DESC LIMIT 100",
                [status],
            )
            .await
        };
        if let Ok(mut rows) = rows_res {
            while let Ok(Some(r)) = rows.next().await {
                out.push(PendingWord {
                    id: r.get(0).unwrap_or(0),
                    guild_id: r.get(1).unwrap_or(0),
                    word: r.get(2).unwrap_or_default(),
                    submitted_by: r.get(3).unwrap_or(0),
                    submitted_at: r.get(4).unwrap_or_default(),
                    status: r.get(5).unwrap_or_default(),
                });
            }
        }
        out
    }

    pub async fn review_word(
        &self,
        pending_id: i64,
        approve: bool,
        reviewer_id: i64,
    ) -> Option<(i64, String, i64)> {
        let conn = self.conn().ok()?;
        let mut rows = conn
            .query(
                "SELECT guild_id, word, submitted_by FROM pending_words WHERE id = ?1 AND status = 'pending'",
                [pending_id],
            )
            .await
            .ok()?;
        let row = rows.next().await.ok()??;
        let gid: i64 = row.get(0).ok()?;
        let word: String = row.get(1).ok()?;
        let sub_by: i64 = row.get(2).ok()?;
        let new_status = if approve { "approved" } else { "rejected" };
        let affected = conn
            .execute(
                "UPDATE pending_words SET status = ?1, reviewed_by = ?2, reviewed_at = datetime('now','localtime') WHERE id = ?3 AND status = 'pending'",
                (new_status, reviewer_id, pending_id),
            )
            .await
            .unwrap_or(0);
        if affected == 0 {
            return None;
        }
        if approve {
            self.add_approved_word(gid, &word, sub_by, reviewer_id)
                .await;
        }
        Some((gid, word, sub_by))
    }

    pub async fn cache_exists(&self, lang: &str, word: &str) -> bool {
        let Ok(conn) = self.conn() else { return false };
        if let Ok(mut rows) = conn
            .query(
                "SELECT 1 FROM wiki_cache WHERE lang = ?1 AND word = ?2 LIMIT 1",
                (lang, word),
            )
            .await
        {
            if matches!(rows.next().await, Ok(Some(_))) {
                let _ = conn
                    .execute(
                        "UPDATE wiki_cache SET hit_count = hit_count + 1 WHERE lang = ?1 AND word = ?2",
                        (lang, word),
                    )
                    .await;
                return true;
            }
        }
        false
    }

    pub async fn cache_add(&self, lang: &str, word: &str) {
        if let Ok(conn) = self.conn() {
            let _ = conn
                .execute(
                    "INSERT INTO wiki_cache (lang, word) VALUES (?1, ?2)
                     ON CONFLICT(lang, word) DO UPDATE SET hit_count = hit_count + 1",
                    (lang, word),
                )
                .await;
        }
    }

    pub async fn random_cached_word(&self, lang: &str) -> Option<String> {
        let conn = self.conn().ok()?;
        let mut rows = conn
            .query(
                "SELECT word FROM wiki_cache WHERE lang = ?1 ORDER BY RANDOM() LIMIT 1",
                [lang],
            )
            .await
            .ok()?;
        if let Ok(Some(r)) = rows.next().await {
            return r.get::<String>(0).ok();
        }
        None
    }

    pub async fn words_starting_with(
        &self,
        guild_id: i64,
        lang: &str,
        prefix: &str,
        limit: usize,
    ) -> Vec<String> {
        let Ok(conn) = self.conn() else { return vec![] };
        let pattern = if lang.eq_ignore_ascii_case("vi") {
            format!("{} %", prefix)
        } else {
            format!("{}%", prefix)
        };
        let mut out = Vec::new();
        if let Ok(mut rows) = conn
            .query(
                "SELECT word FROM word_bank WHERE guild_id = ?1 AND word LIKE ?2 LIMIT ?3",
                (guild_id, pattern.clone(), limit as i64),
            )
            .await
        {
            while let Ok(Some(r)) = rows.next().await {
                if let Ok(w) = r.get::<String>(0) {
                    out.push(w);
                }
            }
        }
        if out.len() < limit {
            if let Ok(mut rows) = conn
                .query(
                    "SELECT word FROM wiki_cache WHERE lang = ?1 AND word LIKE ?2 LIMIT ?3",
                    (lang, pattern, (limit - out.len()) as i64),
                )
                .await
            {
                while let Ok(Some(r)) = rows.next().await {
                    if let Ok(w) = r.get::<String>(0) {
                        if !out.contains(&w) {
                            out.push(w);
                        }
                    }
                }
            }
        }
        out
    }

    pub async fn lb_add_word(&self, guild_id: i64, user_id: i64) {
        if let Ok(conn) = self.conn() {
            let _ = conn
                .execute(
                    "INSERT INTO leaderboard (guild_id, user_id, total_words, current_streak, max_streak, last_played)
                     VALUES (?1, ?2, 1, 1, 1, datetime('now','localtime'))
                     ON CONFLICT(guild_id, user_id) DO UPDATE SET
                        total_words = total_words + 1,
                        current_streak = current_streak + 1,
                        max_streak = MAX(max_streak, current_streak + 1),
                        last_played = datetime('now','localtime')",
                    (guild_id, user_id),
                )
                .await;
        }
    }

    pub async fn lb_reset_streak(&self, guild_id: i64, user_id: i64) {
        if let Ok(conn) = self.conn() {
            let _ = conn
                .execute(
                    "UPDATE leaderboard SET current_streak = 0 WHERE guild_id = ?1 AND user_id = ?2",
                    (guild_id, user_id),
                )
                .await;
        }
    }

    pub async fn lb_get_top(&self, guild_id: i64, limit: i64) -> Vec<LeaderboardRow> {
        let Ok(conn) = self.conn() else { return vec![] };
        let mut out = Vec::new();
        if let Ok(mut rows) = conn
            .query(
                "SELECT user_id, total_words, current_streak, max_streak, games_played, games_won FROM leaderboard WHERE guild_id = ?1 ORDER BY total_words DESC, max_streak DESC, games_won DESC, user_id ASC LIMIT ?2",
                (guild_id, limit),
            )
            .await
        {
            while let Ok(Some(r)) = rows.next().await {
                out.push(LeaderboardRow {
                    user_id: r.get(0).unwrap_or(0),
                    total_words: r.get(1).unwrap_or(0),
                    current_streak: r.get(2).unwrap_or(0),
                    max_streak: r.get(3).unwrap_or(0),
                    games_played: r.get(4).unwrap_or(0),
                    games_won: r.get(5).unwrap_or(0),
                });
            }
        }
        out
    }

    pub async fn lb_add_game(&self, guild_id: i64, players: &[i64], winner_id: Option<i64>) {
        if let Ok(conn) = self.conn() {
            for &uid in players {
                let won: i64 = if winner_id == Some(uid) { 1 } else { 0 };
                let _ = conn
                    .execute(
                        "INSERT INTO leaderboard (guild_id, user_id, games_played, games_won)
                         VALUES (?1, ?2, 1, ?3)
                         ON CONFLICT(guild_id, user_id) DO UPDATE SET
                            games_played = games_played + 1,
                            games_won = games_won + excluded.games_won",
                        (guild_id, uid, won),
                    )
                    .await;
            }
        }
    }

    pub async fn lb_streak_top(&self, guild_id: i64, limit: i64) -> Vec<LeaderboardRow> {
        let Ok(conn) = self.conn() else { return vec![] };
        let mut out = Vec::new();
        if let Ok(mut rows) = conn
            .query(
                "SELECT user_id, total_words, current_streak, max_streak, games_played, games_won FROM leaderboard WHERE guild_id = ?1 AND max_streak > 0 ORDER BY max_streak DESC, total_words DESC, user_id ASC LIMIT ?2",
                (guild_id, limit),
            )
            .await
        {
            while let Ok(Some(r)) = rows.next().await {
                out.push(LeaderboardRow {
                    user_id: r.get(0).unwrap_or(0),
                    total_words: r.get(1).unwrap_or(0),
                    current_streak: r.get(2).unwrap_or(0),
                    max_streak: r.get(3).unwrap_or(0),
                    games_played: r.get(4).unwrap_or(0),
                    games_won: r.get(5).unwrap_or(0),
                });
            }
        }
        out
    }

    pub async fn game_submit_score(&self, game: &str, user_id: i64, score: i64) {
        let cumulative = matches!(
            game,
            "afk" | "taixiu" | "baucua" | "trivia" | "math" | "trieuphu" | "wordle" | "scramble"
        );
        self.game_submit_score_mode(game, user_id, score, cumulative)
            .await;
    }

    pub async fn game_submit_score_mode(
        &self,
        game: &str,
        user_id: i64,
        score: i64,
        cumulative: bool,
    ) {
        if let Ok(conn) = self.conn() {
            let sql = if cumulative {
                "INSERT INTO game_scores (game, user_id, best_score, plays, updated_at)
                 VALUES (?1, ?2, ?3, 1, datetime('now','localtime'))
                 ON CONFLICT(game, user_id) DO UPDATE SET
                    best_score = best_score + excluded.best_score,
                    plays = plays + 1,
                    updated_at = datetime('now','localtime')"
            } else {
                "INSERT INTO game_scores (game, user_id, best_score, plays, updated_at)
                 VALUES (?1, ?2, ?3, 1, datetime('now','localtime'))
                 ON CONFLICT(game, user_id) DO UPDATE SET
                    best_score = MAX(best_score, excluded.best_score),
                    plays = plays + 1,
                    updated_at = datetime('now','localtime')"
            };
            let _ = conn.execute(sql, (game, user_id, score)).await;
        }
    }

    pub async fn game_top(&self, game: &str, limit: i64) -> Vec<GameLeaderboardRow> {
        let Ok(conn) = self.conn() else { return vec![] };
        let mut out = Vec::new();
        if let Ok(mut rows) = conn
            .query(
                "SELECT user_id, best_score, plays FROM game_scores WHERE game = ?1 ORDER BY best_score DESC, updated_at ASC, user_id ASC LIMIT ?2",
                (game, limit),
            )
            .await
        {
            while let Ok(Some(r)) = rows.next().await {
                out.push(GameLeaderboardRow {
                    user_id: r.get(0).unwrap_or(0),
                    best_score: r.get(1).unwrap_or(0),
                    plays: r.get(2).unwrap_or(0),
                });
            }
        }
        out
    }

    pub async fn stats_summary(&self) -> serde_json::Value {
        let Ok(conn) = self.conn() else {
            return serde_json::json!({});
        };
        let count_of = |sql: &'static str| async {
            if let Ok(mut rows) = conn.query(sql, ()).await {
                if let Ok(Some(r)) = rows.next().await {
                    return r.get::<i64>(0).unwrap_or(0);
                }
            }
            0
        };
        let word_bank = count_of("SELECT COUNT(*) FROM word_bank").await;
        let pending = count_of("SELECT COUNT(*) FROM pending_words WHERE status='pending'").await;
        let approved = count_of("SELECT COUNT(*) FROM pending_words WHERE status='approved'").await;
        let rejected = count_of("SELECT COUNT(*) FROM pending_words WHERE status='rejected'").await;
        let cache = count_of("SELECT COUNT(*) FROM wiki_cache").await;
        let players = count_of("SELECT COUNT(DISTINCT user_id) FROM leaderboard").await;
        let games_played = count_of("SELECT COALESCE(SUM(games_played), 0) FROM leaderboard").await;
        let total_games_words =
            count_of("SELECT COALESCE(SUM(total_words), 0) FROM leaderboard").await;
        serde_json::json!({
            "word_bank": word_bank,
            "wordbank": word_bank,
            "approved": approved,
            "rejected": rejected,
            "pending_words": pending,
            "wiki_cache": cache,
            "total_players": players,
            "games_played": games_played,
            "total_games_words": total_games_words,
        })
    }

    pub async fn pub_wordbank(
        &self,
        q_str: &str,
        limit: i64,
        offset: i64,
    ) -> (Vec<serde_json::Value>, i64) {
        let Ok(conn) = self.conn() else {
            return (vec![], 0);
        };
        let like = format!("%{}%", q_str.trim());
        let mut items = Vec::new();
        let rows_res = if q_str.trim().is_empty() {
            conn.query(
                "SELECT word, COALESCE(added_at,'') FROM word_bank ORDER BY added_at DESC LIMIT ?1 OFFSET ?2",
                (limit, offset),
            )
            .await
        } else {
            conn.query(
                "SELECT word, COALESCE(added_at,'') FROM word_bank WHERE word LIKE ?1 ORDER BY added_at DESC LIMIT ?2 OFFSET ?3",
                (like.as_str(), limit, offset),
            )
            .await
        };
        if let Ok(mut rows) = rows_res {
            while let Ok(Some(r)) = rows.next().await {
                items.push(serde_json::json!({
                    "word": r.get::<String>(0).unwrap_or_default(),
                    "added_at": r.get::<String>(1).unwrap_or_default(),
                }));
            }
        }
        let total = if q_str.trim().is_empty() {
            conn.query("SELECT COUNT(*) FROM word_bank", ()).await
        } else {
            conn.query(
                "SELECT COUNT(*) FROM word_bank WHERE word LIKE ?1",
                [like.as_str()],
            )
            .await
        };
        let total_cnt = match total {
            Ok(mut rs) => rs
                .next()
                .await
                .ok()
                .flatten()
                .and_then(|r| r.get::<i64>(0).ok())
                .unwrap_or(0),
            Err(_) => 0,
        };
        (items, total_cnt)
    }

    pub async fn pub_rejected(
        &self,
        q_str: &str,
        limit: i64,
        offset: i64,
    ) -> (Vec<serde_json::Value>, i64) {
        let Ok(conn) = self.conn() else {
            return (vec![], 0);
        };
        let like = format!("%{}%", q_str.trim());
        let mut items = Vec::new();
        let rows_res = if q_str.trim().is_empty() {
            conn.query(
                "SELECT word, COALESCE(submitted_at,'') FROM pending_words WHERE status='rejected' ORDER BY reviewed_at DESC LIMIT ?1 OFFSET ?2",
                (limit, offset),
            )
            .await
        } else {
            conn.query(
                "SELECT word, COALESCE(submitted_at,'') FROM pending_words WHERE status='rejected' AND word LIKE ?1 ORDER BY reviewed_at DESC LIMIT ?2 OFFSET ?3",
                (like.as_str(), limit, offset),
            )
            .await
        };
        if let Ok(mut rows) = rows_res {
            while let Ok(Some(r)) = rows.next().await {
                items.push(serde_json::json!({
                    "word": r.get::<String>(0).unwrap_or_default(),
                    "submitted_at": r.get::<String>(1).unwrap_or_default(),
                }));
            }
        }
        let total = if q_str.trim().is_empty() {
            conn.query(
                "SELECT COUNT(*) FROM pending_words WHERE status='rejected'",
                (),
            )
            .await
        } else {
            conn.query(
                "SELECT COUNT(*) FROM pending_words WHERE status='rejected' AND word LIKE ?1",
                [like.as_str()],
            )
            .await
        };
        let total_cnt = match total {
            Ok(mut rs) => rs
                .next()
                .await
                .ok()
                .flatten()
                .and_then(|r| r.get::<i64>(0).ok())
                .unwrap_or(0),
            Err(_) => 0,
        };
        (items, total_cnt)
    }

    pub async fn pub_leaderboard(&self, mode: &str, limit: i64) -> Vec<serde_json::Value> {
        let Ok(conn) = self.conn() else { return vec![] };
        let sql = if mode == "streak" {
            "SELECT user_id, SUM(total_words) tw, MAX(max_streak) ms FROM leaderboard GROUP BY user_id ORDER BY ms DESC, tw DESC LIMIT ?1"
        } else {
            "SELECT user_id, SUM(total_words) tw, MAX(max_streak) ms FROM leaderboard GROUP BY user_id ORDER BY tw DESC, ms DESC LIMIT ?1"
        };
        let mut out = Vec::new();
        if let Ok(mut rows) = conn.query(sql, [limit]).await {
            while let Ok(Some(r)) = rows.next().await {
                let uid: i64 = r.get(0).unwrap_or(0);
                let tw: i64 = r.get(1).unwrap_or(0);
                let ms: i64 = r.get(2).unwrap_or(0);
                let (name, avatar) = self.resolve_display_user(&conn, uid).await;
                out.push(serde_json::json!({
                    "user_id": uid.to_string(),
                    "name": name,
                    "avatar": avatar,
                    "total_words": tw,
                    "max_streak": ms,
                }));
            }
        }
        out
    }

    pub async fn pub_game_leaderboard(&self, game: &str, limit: i64) -> Vec<serde_json::Value> {
        let Ok(conn) = self.conn() else { return vec![] };
        let mut out = Vec::new();
        if let Ok(mut rows) = conn
            .query(
                "SELECT user_id, best_score, plays FROM game_scores WHERE game = ?1 ORDER BY best_score DESC, updated_at ASC, user_id ASC LIMIT ?2",
                (game, limit),
            )
            .await
        {
            while let Ok(Some(r)) = rows.next().await {
                let uid: i64 = r.get(0).unwrap_or(0);
                let best: i64 = r.get(1).unwrap_or(0);
                let plays: i64 = r.get(2).unwrap_or(0);
                let (name, avatar) = self.resolve_display_user(&conn, uid).await;
                out.push(serde_json::json!({
                    "user_id": uid.to_string(),
                    "name": name,
                    "avatar": avatar,
                    "best_score": best,
                    "plays": plays,
                }));
            }
        }
        out
    }

    async fn resolve_display_user(&self, conn: &Connection, uid: i64) -> (String, Option<String>) {
        if uid < 0 {
            if let Ok(mut rows) = conn
                .query(
                    "SELECT username, avatar FROM web_users WHERE id = ?1",
                    [-uid],
                )
                .await
            {
                if let Ok(Some(r)) = rows.next().await {
                    let name: String = r.get(0).unwrap_or_else(|_| format!("Web User {}", -uid));
                    let av: Option<String> = r.get(1).ok();
                    return (name, av);
                }
            }
            return (format!("Web User {}", -uid), None);
        }
        (format!("User {}", uid), None)
    }

    pub async fn get_public_rooms(&self) -> Vec<PublicRoomRow> {
        let Ok(conn) = self.conn() else { return vec![] };
        let _ = conn
            .execute(
                "UPDATE public_rooms SET status = 'closed', updated_at = datetime('now','localtime')
                 WHERE status != 'closed' AND datetime(created_at, '+3 hours') < datetime('now','localtime')",
                (),
            )
            .await;
        let mut out = Vec::new();
        if let Ok(mut rows) = conn
            .query(
                "SELECT room_id, name, platform, host_id, host_name, lang, max_players, current_players, join_token, web_url, tele_url, status FROM public_rooms WHERE status != 'closed' ORDER BY created_at DESC LIMIT 50",
                (),
            )
            .await
        {
            while let Ok(Some(r)) = rows.next().await {
                out.push(PublicRoomRow {
                    room_id: r.get(0).unwrap_or_default(),
                    name: r.get(1).unwrap_or_default(),
                    platform: r.get(2).unwrap_or_default(),
                    host_id: r.get(3).unwrap_or_default(),
                    host_name: r.get(4).unwrap_or_default(),
                    lang: r.get(5).unwrap_or_else(|_| "vi".to_string()),
                    max_players: r.get(6).unwrap_or(0),
                    current_players: r.get(7).unwrap_or(1),
                    join_token: r.get(8).ok(),
                    web_url: r.get(9).ok(),
                    tele_url: r.get(10).ok(),
                    status: r.get(11).unwrap_or_else(|_| "waiting".to_string()),
                });
            }
        }
        out
    }

    pub async fn get_public_room(&self, room_id: &str) -> Option<PublicRoomRow> {
        let conn = self.conn().ok()?;
        let mut rows = conn
            .query(
                "SELECT room_id, name, platform, host_id, host_name, lang, max_players, current_players, join_token, web_url, tele_url, status FROM public_rooms WHERE UPPER(room_id) = UPPER(?1) LIMIT 1",
                [room_id],
            )
            .await
            .ok()?;
        let r = rows.next().await.ok()??;
        Some(PublicRoomRow {
            room_id: r.get(0).unwrap_or_default(),
            name: r.get(1).unwrap_or_default(),
            platform: r.get(2).unwrap_or_default(),
            host_id: r.get(3).unwrap_or_default(),
            host_name: r.get(4).unwrap_or_default(),
            lang: r.get(5).unwrap_or_else(|_| "vi".to_string()),
            max_players: r.get(6).unwrap_or(0),
            current_players: r.get(7).unwrap_or(1),
            join_token: r.get(8).ok(),
            web_url: r.get(9).ok(),
            tele_url: r.get(10).ok(),
            status: r.get(11).unwrap_or_else(|_| "waiting".to_string()),
        })
    }

    pub async fn create_public_room(&self, room: &PublicRoomRow) {
        if let Ok(conn) = self.conn() {
            let _ = conn
                .execute(
                    "INSERT INTO public_rooms (room_id, name, platform, host_id, host_name, lang, max_players, current_players, join_token, web_url, tele_url, status)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                     ON CONFLICT(room_id) DO UPDATE SET
                        name = excluded.name,
                        lang = excluded.lang,
                        max_players = excluded.max_players,
                        current_players = excluded.current_players,
                        web_url = excluded.web_url,
                        tele_url = excluded.tele_url,
                        status = excluded.status,
                        updated_at = datetime('now','localtime')",
                    (
                        room.room_id.as_str(),
                        room.name.as_str(),
                        room.platform.as_str(),
                        room.host_id.as_str(),
                        room.host_name.as_str(),
                        room.lang.as_str(),
                        room.max_players,
                        room.current_players,
                        room.join_token.as_deref(),
                        room.web_url.as_deref(),
                        room.tele_url.as_deref(),
                        room.status.as_str(),
                    ),
                )
                .await;
        }
    }

    pub async fn update_public_room_players(&self, room_id: &str, current_players: i64) {
        if let Ok(conn) = self.conn() {
            let _ = conn
                .execute(
                    "UPDATE public_rooms SET current_players = ?1, updated_at = datetime('now','localtime') WHERE room_id = ?2",
                    (current_players, room_id),
                )
                .await;
        }
    }

    pub async fn close_public_room(&self, room_id: &str) {
        if let Ok(conn) = self.conn() {
            let _ = conn
                .execute(
                    "UPDATE public_rooms SET status = 'closed', updated_at = datetime('now','localtime') WHERE room_id = ?1",
                    [room_id],
                )
                .await;
        }
    }

    pub async fn save_telegram_token(
        &self,
        token: &str,
        room_code: &str,
        chat_id: i64,
        host_id: i64,
        host_name: &str,
        lang: &str,
    ) {
        if let Ok(conn) = self.conn() {
            let _ = conn
                .execute(
                    "INSERT OR REPLACE INTO telegram_room_tokens (token, room_code, chat_id, host_id, host_name, lang) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    (token, room_code, chat_id, host_id, host_name, lang),
                )
                .await;
        }
    }

    pub async fn get_telegram_token(
        &self,
        token: &str,
    ) -> Option<(String, i64, i64, String, String)> {
        let conn = self.conn().ok()?;
        let mut rows = conn
            .query(
                "SELECT room_code, chat_id, host_id, host_name, lang FROM telegram_room_tokens WHERE token = ?1 LIMIT 1",
                [token],
            )
            .await
            .ok()?;
        if let Ok(Some(r)) = rows.next().await {
            return Some((
                r.get(0).unwrap_or_default(),
                r.get(1).unwrap_or(0),
                r.get(2).unwrap_or(0),
                r.get(3).unwrap_or_default(),
                r.get(4).unwrap_or_else(|_| "vi".to_string()),
            ));
        }
        None
    }

    pub async fn game_user_score(&self, game: &str, user_id: i64) -> (i64, i64) {
        let Ok(conn) = self.conn() else { return (0, 0) };
        if let Ok(mut rows) = conn
            .query(
                "SELECT best_score, plays FROM game_scores WHERE game = ?1 AND user_id = ?2 LIMIT 1",
                (game, user_id),
            )
            .await
        {
            if let Ok(Some(r)) = rows.next().await {
                return (r.get(0).unwrap_or(0), r.get(1).unwrap_or(0));
            }
        }
        (0, 0)
    }

    pub async fn find_user_by_username_or_email(&self, identifier: &str) -> Option<WebUser> {
        let conn = self.conn().ok()?;
        let mut rows = conn
            .query(
                "SELECT id, username, email, password_hash, provider, is_admin, totp_secret, totp_enabled, email_verified, ui_lang FROM web_users WHERE username = ?1 OR email = ?1 LIMIT 1",
                [identifier],
            )
            .await
            .ok()?;
        let r = rows.next().await.ok()??;
        Some(WebUser {
            id: r.get(0).ok()?,
            username: r.get(1).ok()?,
            email: r.get(2).ok()?,
            password_hash: r.get(3).ok(),
            provider: r.get(4).unwrap_or_else(|_| "credentials".to_string()),
            is_admin: r.get(5).unwrap_or(0),
            totp_secret: r.get(6).ok(),
            totp_enabled: r.get(7).unwrap_or(0),
            email_verified: r.get(8).unwrap_or(0),
            ui_lang: r.get(9).ok(),
        })
    }

    pub async fn tournament_join(&self, tournament_id: i64, user_id: i64) -> bool {
        let Ok(conn) = self.conn() else { return false };
        conn.execute(
            "INSERT OR IGNORE INTO tournament_players (tournament_id, user_id) VALUES (?1, ?2)",
            (tournament_id, user_id),
        )
        .await
        .map(|n| n > 0)
        .unwrap_or(false)
    }

    pub async fn tournament_add_word(&self, guild_id: i64, user_id: i64) {
        let Ok(conn) = self.conn() else { return };
        if let Ok(mut rows) = conn
            .query(
                "SELECT id FROM tournaments WHERE guild_id = ?1 AND status = 'active' ORDER BY id DESC LIMIT 1",
                [guild_id],
            )
            .await
        {
            if let Ok(Some(r)) = rows.next().await {
                let tid: i64 = r.get(0).unwrap_or(0);
                let _ = conn
                    .execute(
                        "INSERT INTO tournament_players (tournament_id, user_id, words_scored) VALUES (?1, ?2, 1)
                         ON CONFLICT(tournament_id, user_id) DO UPDATE SET words_scored = words_scored + 1",
                        (tid, user_id),
                    )
                    .await;
            }
        }
    }

    pub async fn tournament_players(&self, tournament_id: i64) -> Vec<(i64, i64)> {
        let Ok(conn) = self.conn() else { return vec![] };
        let mut out = Vec::new();
        if let Ok(mut rows) = conn
            .query(
                "SELECT user_id, words_scored FROM tournament_players WHERE tournament_id = ?1 ORDER BY words_scored DESC, joined_at ASC, user_id ASC",
                [tournament_id],
            )
            .await
        {
            while let Ok(Some(r)) = rows.next().await {
                out.push((r.get(0).unwrap_or(0), r.get(1).unwrap_or(0)));
            }
        }
        out
    }
}
