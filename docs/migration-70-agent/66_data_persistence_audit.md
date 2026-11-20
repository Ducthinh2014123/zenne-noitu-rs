# Forensic Audit Report — ID 66

## 1. Database Architecture & Connection (`zenne-core`)
- **Backend**: Remote libSQL only (`crates/zenne-core/src/db.rs:64-85`, `DbManager::new`). Requires `DATABASE_URL` and `DATABASE_AUTH_TOKEN`.
- **Schema (`init_schema`, `db.rs:91-236`)**: 12 tables persisted in libSQL.

---

## 2. Persisted in DB vs Ephemeral In-Memory (`DashMap`) & Data Loss on Restart
- **Users & Auth Sessions:** `web_users` & `password_recovery` persisted in libSQL; `CacheManager::mem` (`cache.rs:15`) rate-limit counters lost on restart if Redis unset.
- **Game Sessions & Nonces (`zenne-api`):** `ws_rooms`, `ttt_sessions`, `ms_sessions`, `used_nonces` are ephemeral `DashMap`s. Wiping `used_nonces` on restart allows `afk` `session_token` replay within 3600s TTL. `ws_game.rs:456-458` calls `lb_add_word(0, p_uid)` but never `lb_add_game` or `game_submit_score`.
- **`zenne-discord`:** Persists `guild_settings`, `lb_add_word`, `pending_words`, `tournaments`; active `NoituManager::rooms` lost on restart; `lb_add_game`, `lb_reset_streak`, `game_submit_score` never called.
- **`zenne-telegram`:** Persists `telegram_room_tokens`, `public_rooms`, `leaderboard`, `game_scores`, `pending_words`, `guild_settings`; 12 `DashMap`s in `AppState` lost on restart.
- **`zenne-twitch` & `zenne-revolt`:** Ephemeral `DashMap`s only; neither calls `lb_add_word` or `game_submit_score`.

## 3. `game_scores` Cumulative vs High-Score Semantics
- `db.rs:653-656`: `game_submit_score` hardcodes `cumulative = (game == "afk")`. Telegram minigames pass fixed constants (`50`, `10`, `15`, `20`) and get capped at `MAX(best_score, ...)`.

## 4. `word_bank`, `pending_words`, and `wiki_cache` Durability
- `word_bank` & `pending_words` & `wiki_cache` are durable in libSQL. Negative hits (`"0"`, 3600s TTL) live only in `CacheManager`.
