# Forensic Audit Report — ID 27

**Scope:** Database Indexes, Queries & Performance parity audit across:
- **Python:** `C:\Users\Ai Hoi\Downloads\archive-2026-09-23T144146Z\bot.py`
- **Rust:** `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\db.rs`

---

## 1. Indexes (Schema Parity, Redundancy & Gaps)

### 1.1 Parity Status
Both `init_db` (`bot.py:382-386, 396, 408-410, 425-427`) and `DbManager::init_schema` (`db.rs:220-231`) create the exact same 12 secondary indexes.

### 1.2 Redundant Single-Column Indexes (Already Covered by Left-Prefix of `UNIQUE` / `PK`)
- `idx_wb_guild` on `word_bank(guild_id)` (`bot.py:382`, `db.rs:220`) — covered by `UNIQUE(guild_id, word)` (`bot.py:325`, `db.rs:107`).
- `idx_pw_guild` on `pending_words(guild_id)` (`bot.py:383`, `db.rs:221`) — covered by `UNIQUE(guild_id, word)` (`bot.py:337`, `db.rs:118`).
- `idx_wc_lang` on `wiki_cache(lang)` (`bot.py:385`, `db.rs:223`) — covered by `UNIQUE(lang, word)` (`bot.py:346`, `db.rs:126`).
- `idx_lb_guild` on `leaderboard(guild_id)` (`bot.py:386`, `db.rs:224`) — covered by `PRIMARY KEY (guild_id, user_id)` (`bot.py:358`, `db.rs:137`).
- `idx_gs_game` on `game_scores(game)` (`bot.py:396`, `db.rs:225`) — covered by `UNIQUE(game, user_id)` (`bot.py:394`, `db.rs:166`).
- `idx_wu_email` on `web_users(email)` (`bot.py:408`, `db.rs:226`) — covered by `email TEXT UNIQUE NOT NULL` (`bot.py:400`, `db.rs:171`).

### 1.3 Missing Composite / Covering Indexes
- `word_bank(added_at DESC, id DESC)`: needed for global `ORDER BY added_at DESC` in `DbManager::pub_wordbank` (`db.rs:753, 759`), `_ah_wordbank_get` (`bot.py:3330`), `_ah_pub_wordbank` (`bot.py:3401`), `cmd_danhsachtu` (`bot.py:3869`), `slash_danhsachtu` (`bot.py:3887`).
- `pending_words(status, reviewed_at DESC, id DESC)` and `pending_words(guild_id, status, id DESC)` / `(status, submitted_at)`: single-column `idx_pw_guild` and `idx_pw_status` force filesorts in `DbManager::get_pending` (`db.rs:405, 411`), `DbManager::pub_rejected` (`db.rs:807, 813`), `db_get_pending` (`bot.py:587, 593`), `_ah_pending` (`bot.py:3245-3253`), `_ah_pub_rejected` (`bot.py:3410-3414`).
- `leaderboard(guild_id, total_words DESC, max_streak DESC)` and `(guild_id, max_streak DESC, total_words DESC)`: needed for `lb_get_top` (`db.rs:592`, `bot.py:765-767`), `lb_streak_top` (`db.rs:634`, `bot.py:780-782`), `_ah_leaderboard` (`bot.py:3360-3364`). Also `leaderboard(user_id)` is missing for global `GROUP BY user_id` in `pub_leaderboard` (`db.rs:855, 857`) and `_ah_pub_leaderboard` (`bot.py:3427-3428`).
- `game_scores(game, best_score DESC)`: needed for `game_top` (`db.rs:690`, `bot.py:816-817`) and `pub_game_leaderboard` (`db.rs:883`).
- `tournaments(guild_id, status, id DESC)`: needed for `tournament_add_word` (`db.rs:1142`), `db_tournament_get_active` (`bot.py:1041`), `db_tournament_add_word` (`bot.py:1073`).
- `tournament_players(tournament_id, words_scored DESC)`: needed for `tournament_players` (`db.rs:1165`), `db_tournament_players` (`bot.py:1096-1098`), `db_tournament_end` (`bot.py:1114-1116`).
- `wiki_cache(lang, hit_count DESC)`: needed for `db_cache_stats` (`bot.py:674-675`) and `db_suggest_similar` (`bot.py:1218-1220`).
- `public_rooms(status, created_at DESC)` (Rust-only table, `db.rs:195-210`): zero secondary indexes exist. `get_public_rooms` (`db.rs:929-937`) scans the table, and `get_public_room` (`db.rs:966`) uses `WHERE UPPER(room_id) = UPPER(?1)`, bypassing `room_id TEXT PRIMARY KEY`.

---

## 2. Query Patterns & Parity Findings

- **Connection & Locking Overhead:**
  - `bot.py::get_db` (`bot.py:303-307`) opens a new `sqlite3.connect(DB_PATH)` and runs `PRAGMA journal_mode=WAL` on every call under a global `_db_lock` (`bot.py:301`), while `bot.py::_dbc` (`bot.py:3214-3217`) opens connections **without** `_db_lock` or WAL pragma, risking lock contention.
  - `db.rs::DbManager::conn` (`db.rs:87-89`) calls `self.db.connect()` per method against remote libSQL (`Builder::new_remote`, `db.rs:74`).
- **Multi-Round-Trip Queries:**
  - `DbManager::stats_summary` (`db.rs:706-738`): executes **8 sequential remote queries** (`word_bank`, 3x `pending_words`, `wiki_cache`, 3x `leaderboard`). Similarly `bot.py::_ah_stats` (`bot.py:3224-3234`) runs 6 queries and `_ah_pub_status` (`bot.py:3379-3386`) runs 4 queries.
  - `DbManager::cache_exists` (`db.rs:463-483`) and `bot.py::db_cache_exists` (`bot.py:636-652`): execute `SELECT 1` then `UPDATE ... hit_count = hit_count + 1` (2 round-trips instead of `UPDATE ... RETURNING 1`).
  - `DbManager::submit_pending` (`db.rs:359-398`): executes 3 queries (`word_exists`, `INSERT ... ON CONFLICT`, `SELECT id` instead of `RETURNING id`).
  - `DbManager::review_word` (`db.rs:431-461`): executes `SELECT`, `UPDATE`, then opens a **second connection** via `add_approved_word` (`db.rs:457`).
- **Write-on-Read:** `DbManager::get_public_rooms` (`db.rs:927-933`) executes an `UPDATE public_rooms SET status = 'closed'...` on every read.
- **Full-Table Random Sort:** `DbManager::random_cached_word` (`db.rs:497-510`) uses `ORDER BY RANDOM() LIMIT 1`, scanning all rows for `lang`.
- **`SELECT *` Usage:** `bot.py` uses `SELECT *` in `db_get_pending` (`587, 593`), `db_review_word` (`604`), `_ah_pending` (`3245`), `_ah_review` (`3273`), `_ah_review_bulk` (`3302`), `_ah_wordbank_get` (`3322`), `_ah_leaderboard` (`3361`), `db_tournament_get_active` (`1041`), and `web_users` queries (`4416, 4438, 4445, 4461, 4498, 4522, 4542, 4565, 4587`). `db.rs` uses explicit column lists.
- **Parity / Correctness Bugs:**
  - `bot.py::_ah_guilds` (`bot.py:3373`) selects `game_channel_id` from `guild_settings`, but the schema column is `game_channel` (`bot.py:315`, `db.rs:97`).
  - `DbManager::words_starting_with` (`db.rs:520`) only uses `"{} %"` for `"vi"` and does not sort `wiki_cache` results by `hit_count DESC` (`db.rs:542`), unlike `bot.py::_syllable_prefix_pattern` (`bot.py:1206`) and `db_suggest_similar` (`bot.py:1220`).
  - `DbManager::get_pending` (`db.rs:405, 411`) sorts `ORDER BY id DESC` (newest first, hardcoded `LIMIT 100`), whereas `bot.py::db_get_pending` (`bot.py:588, 594`) sorts `ORDER BY submitted_at ASC`.

---

## 3. N+1 Loops

- **Rust `db.rs`:**
  1. `DbManager::pub_leaderboard` (`db.rs:852-876`): fetches top `limit` rows from `leaderboard`, then calls `self.resolve_display_user(&conn, uid).await` (`db.rs:865`, `905-923`) inside the loop, issuing 1 `SELECT username, avatar FROM web_users WHERE id = ?1` per negative `uid`.
  2. `DbManager::pub_game_leaderboard` (`db.rs:878-903`): same 1+N pattern calling `resolve_display_user` (`db.rs:892`) per row.
  3. `DbManager::lb_add_game` (`db.rs:611-627`): loops `for &uid in players` (`db.rs:613-625`), executing a separate remote `INSERT ... ON CONFLICT` per player instead of batching.
- **Python `bot.py`:**
  1. `_ah_pub_leaderboard` (`bot.py:3420-3449`): loops `for r in rows:` (`3431-3448`); opens a **new connection** (`with _dbc() as c2:`) to query `web_users` for each `uid < 0` (`3436-3437`), or awaits `bot.fetch_user(uid)` for each `uid >= 0` (`3443`).
  2. `_ah_pub_game_leaderboard` (`bot.py:3551-3578`): identical N+1 pattern (`3565-3566` for `web_users`, `3572` for `bot.fetch_user`).
  3. `_ah_review_bulk` (`bot.py:3288-3312`): loops `for pid in ids:` (`3301-3310`) running 2–3 SQL statements per ID.
  4. `db_lb_add_game` (`bot.py:736-757`): loops `for uid in players:` (`740-749`) executing `INSERT ... ON CONFLICT` per player.

---

## 4. Pagination (`LIMIT`/`OFFSET`)

- **Offset Scans + Duplicate `COUNT(*)` Queries:**
  - `DbManager::pub_wordbank` (`db.rs:740-792`): `LIMIT ?1 OFFSET ?2` (`753, 759`) + second `SELECT COUNT(*)` (`773, 776`).
  - `DbManager::pub_rejected` (`db.rs:794-850`): `LIMIT ?1 OFFSET ?2` (`807, 813`) + second `SELECT COUNT(*)` (`828, 834`).
  - `bot.py::_ah_pending` (`bot.py:3237-3257`): `LIMIT ? OFFSET ?` (`3253`) + `COUNT(*)` (`3256`). `offset` not clamped `>= 0`.
  - `bot.py::_ah_wordbank_get` (`bot.py:3315-3334`): `LIMIT ? OFFSET ?` (`3330`) + `COUNT(*)` (`3333`). `offset` not clamped `>= 0`.
  - `bot.py::_ah_pub_wordbank` (`bot.py:3395-3405`): `LIMIT ? OFFSET ?` (`3401`) + `COUNT(*)` (`3404`). `offset` not clamped `>= 0`.
  - `bot.py::_ah_pub_rejected` (`bot.py:3407-3418`): `LIMIT ? OFFSET ?` (`3414`) + `COUNT(*)` (`3417`). `offset` not clamped `>= 0`.
  - `bot.py::cmd_danhsachtu` (`bot.py:3869-3871`) & `slash_danhsachtu` (`bot.py:3887-3889`): `LIMIT ? OFFSET ?` + `COUNT(*)`.
- **Missing/Unbounded Pagination:**
  - `DbManager::get_pending` (`db.rs:400-429`): hardcoded `LIMIT 100`, no offset/cursor.
  - `DbManager::get_public_rooms` (`db.rs:937`): hardcoded `LIMIT 50`, no pagination.
  - `bot.py::_ah_guilds` (`bot.py:3373`): unbounded `SELECT` with no `LIMIT`.

---

## 5. `LIKE` Scans

- **Leading-Wildcard `%...%` Full Scans (Non-Indexable, Unescaped Wildcards):**
  - `DbManager::pub_wordbank` (`db.rs:749, 759, 776`): `WHERE word LIKE ?1` with `format!("%{}%", q_str.trim())`.
  - `DbManager::pub_rejected` (`db.rs:803, 813, 834`): `WHERE status='rejected' AND word LIKE ?1` with `format!("%{}%", q_str.trim())`.
  - `bot.py::_ah_pending` (`bot.py:3248-3249`): `AND word LIKE ?` with `f"%{search}%"`.
  - `bot.py::_ah_wordbank_get` (`bot.py:3328-3329`): `AND word LIKE ?` with `f"%{search}%"`.
  - `bot.py::_ah_pub_wordbank` (`bot.py:3399`): `AND word LIKE ?` with `f"%{q_str}%"`.
  - `bot.py::_ah_pub_rejected` (`bot.py:3411, 3413`): `AND word LIKE ?` with `f"%{q_str}%"`.
  - None escape `%` or `_` in user input.
- **Index-Bypassing `LOWER(word) LIKE` in Python:**
  - `bot.py::db_suggest_similar` (`bot.py:1219`): `WHERE lang=? AND LOWER(word) LIKE ?` — wrapping `word` in `LOWER()` prevents using `UNIQUE(lang, word)` (`COLLATE NOCASE`).
  - `bot.py::db_words_starting_with` (`bot.py:1235`): `WHERE guild_id=? AND LOWER(word) LIKE ?` — wrapping `word` in `LOWER()` prevents using `UNIQUE(guild_id, word)` (`COLLATE NOCASE`).
  - `DbManager::words_starting_with` (`db.rs:528, 542`) avoids `LOWER()` (`word LIKE ?2`), though it still does not escape `%`/`_` in `prefix`.

---

## 6. Missing Tiebreakers (`ORDER BY`)

- **Rust `db.rs`:**
  - `lb_get_top` (`db.rs:592`): `ORDER BY total_words DESC` — missing `, max_streak DESC, user_id ASC` (also parity mismatch with `bot.py:767`).
  - `lb_streak_top` (`db.rs:634`): `ORDER BY max_streak DESC` — missing `, total_words DESC, user_id ASC` (parity mismatch with `bot.py:782`).
  - `pub_leaderboard` (`db.rs:855, 857`): `ORDER BY ms DESC, tw DESC` / `tw DESC, ms DESC` — missing `, user_id ASC`.
  - `game_top` (`db.rs:690`) & `pub_game_leaderboard` (`db.rs:883`): `ORDER BY best_score DESC` — missing `, plays ASC, user_id ASC`.
  - `pub_wordbank` (`db.rs:753, 759`): `ORDER BY added_at DESC` — 1s timestamp resolution; missing `, id DESC`.
  - `pub_rejected` (`db.rs:807, 813`): `ORDER BY reviewed_at DESC` — 1s timestamp (and nullable); missing `, id DESC`.
  - `get_public_rooms` (`db.rs:937`): `ORDER BY created_at DESC` — missing `, room_id ASC`.
  - `tournament_players` (`db.rs:1165`): `ORDER BY words_scored DESC` — missing `, joined_at ASC, user_id ASC`.
- **Python `bot.py`:**
  - `db_get_pending` (`bot.py:588, 594`): `ORDER BY submitted_at ASC` — missing `, id ASC`.
  - `_ah_pending` (`bot.py:3253`): `ORDER BY submitted_at DESC` — missing `, id DESC`.
  - `db_cache_stats` (`bot.py:675, 684, 687`): `ORDER BY hit_count DESC` / `ORDER BY n DESC` — missing `, word ASC` / `, lang ASC`.
  - `db_lb_get_top` (`bot.py:767`), `db_lb_streak_top` (`bot.py:782`), `_ah_leaderboard` (`bot.py:3360`), `_ah_pub_leaderboard` (`bot.py:3424`): missing `, user_id ASC`.
  - `db_game_top` (`bot.py:817`): `ORDER BY best_score DESC` — missing `, plays ASC, user_id ASC`.
  - `db_tournament_players` (`bot.py:1098`), `db_tournament_end` (`bot.py:1116`): `ORDER BY words_scored DESC` — missing `, user_id ASC`.
  - `db_suggest_similar` (`bot.py:1220`): `ORDER BY hit_count DESC` — missing `, word ASC`.
  - `_ah_wordbank_get` (`bot.py:3330`), `_ah_pub_wordbank` (`bot.py:3401`), `cmd_danhsachtu` (`bot.py:3869`), `slash_danhsachtu` (`bot.py:3887`): `ORDER BY added_at DESC` — missing `, id DESC`.
  - `_ah_pub_rejected` (`bot.py:3414`): `ORDER BY reviewed_at DESC` — missing `, id DESC`.
