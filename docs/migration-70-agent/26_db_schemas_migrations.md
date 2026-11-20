# Forensic Audit Report — ID 26

## Scope & Files Audited
- **Python**: `C:\Users\Ai Hoi\Downloads\archive-2026-09-23T144146Z\bot.py` — `get_db()` (lines 303–307), `init_db()` (lines 309–465). Local SQLite (`noitu_bot.db`, line 38) with `PRAGMA journal_mode=WAL` (line 306).
- **Rust**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\db.rs` — `DbManager::new()` (lines 70–85), `DbManager::init_schema()` (lines 91–236). Remote libSQL/Turso (`Builder::new_remote`, lines 74–76).
- **Web**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\lib\db.js` — `queryDb(sql, args)` (lines 10–61). Turso HTTP `/v2/pipeline` client with **no schema definitions, no indexes, and no migrations**. Only two API routes issue SQL via `queryDb`:
  - `pages/api/pub/multiplayer/create.js` (lines 31–37): `INSERT INTO public_rooms (...)`
  - `pages/api/pub/multiplayer/rooms.js` (lines 16–22): `SELECT ... FROM public_rooms`

---

## Executive Summary of Divergences

1. **Missing Tables in Python (`bot.py`)**:
   - `public_rooms` (`db.rs:195–210`, queried by Web `create.js:31–37` & `rooms.js:16–22`) does not exist in `bot.py`.
   - `telegram_room_tokens` (`db.rs:211–219`) does not exist in `bot.py`.
2. **Missing Column in Python (`bot.py`)**:
   - `web_users.backup_codes TEXT DEFAULT NULL` (`db.rs:179`, used by Rust `crates/zenne-api/src/auth_routes.rs:63,76,698,724`) is absent from `bot.py`.
3. **Incompatible Column Type & Defaults in `web_users`**:
   - `web_users.email_verified`: Python migration (`bot.py:461`) uses `INTEGER DEFAULT 1` (and backfills `UPDATE web_users SET email_verified=1` at line 462, while new credentials signups explicitly insert `0` at line 4276). Rust (`db.rs:180`) uses `INTEGER DEFAULT 0`.
   - `web_users.created_at`: Python (`bot.py:406`) uses `DATETIME DEFAULT CURRENT_TIMESTAMP` (UTC). Rust (`db.rs:182`) uses `TEXT DEFAULT (datetime('now','localtime'))` (server local time).
4. **Missing `ALTER TABLE` Migrations in Rust (`db.rs`)**:
   - Python `init_db()` runs 10 `_migrations` `ALTER TABLE` statements (`bot.py:432–451`) plus a `PRAGMA table_info(web_users)` check for `email_verified` (`bot.py:457–463`).
   - Rust `DbManager::init_schema()` (`db.rs:91–236`) has **zero `ALTER TABLE` migrations**—it only runs `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS`. Running Rust against a legacy Python-created DB will not add `ui_lang`, `started_at`, `ended_at`, `message_id`, `games_played`, `games_won`, `max_streak`, `totp_secret`, `totp_enabled`, `email_verified`, or `backup_codes` (causing runtime failures on `backup_codes` in `auth_routes.rs`).
5. **In-Definition vs. Migration-Only Column in Python (`tournaments.started_at`)**:
   - `tournaments.started_at TEXT DEFAULT NULL` is omitted from Python's `CREATE TABLE tournaments` (`bot.py:361–372`) and only added via `_migrations` (`bot.py:433`). Rust includes it directly in `CREATE TABLE tournaments` (`db.rs:148`).
6. **Web Schema-less Dependency & Type Coercion**:
   - `lib/db.js` defines no tables; Web multiplayer routes fail if Rust `DbManager::init_schema()` has not already created `public_rooms` on Turso.
   - `lib/db.js:14` maps all JS `typeof a === 'number'` args to `{ type: 'integer', value: String(a) }` (unsafe for floats if ever passed, though `create.js` only passes integers/strings).

---

## Table-by-Table Parity Matrix

*(Note: **No `CHECK` constraints** exist in any table across Python, Rust, or Web.)*

### 1. `guild_settings`
| Column | Python (`bot.py:312–316, 441`) | Rust (`db.rs:94–99`) | Web | Status |
|---|---|---|---|---|
| `guild_id` | `INTEGER PRIMARY KEY` | `INTEGER PRIMARY KEY` | — | Match |
| `lang` | `TEXT DEFAULT 'vi'` | `TEXT DEFAULT 'vi'` | — | Match |
| `game_channel` | `INTEGER DEFAULT NULL` | `INTEGER DEFAULT NULL` | — | Match |
| `ui_lang` | `TEXT DEFAULT 'vi'` (via `ALTER TABLE` at `:441`) | `TEXT DEFAULT 'vi'` | — | Match (fresh DB); Rust lacks `ALTER` |

### 2. `word_bank`
| Column / Constraint | Python (`bot.py:318–326, 382`) | Rust (`db.rs:100–108, 220`) | Web | Status |
|---|---|---|---|---|
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` | `INTEGER PRIMARY KEY AUTOINCREMENT` | — | Match |
| `guild_id` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `word` | `TEXT NOT NULL COLLATE NOCASE` | `TEXT NOT NULL COLLATE NOCASE` | — | Match |
| `added_by` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `approved_by` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `added_at` | `TEXT DEFAULT (datetime('now','localtime'))` | `TEXT DEFAULT (datetime('now','localtime'))` | — | Match |
| `UNIQUE` / Index | `UNIQUE(guild_id, word)`, `idx_wb_guild` | `UNIQUE(guild_id, word)`, `idx_wb_guild` | — | Match |

### 3. `pending_words`
| Column / Constraint | Python (`bot.py:328–338, 383–384`) | Rust (`db.rs:109–119, 221–222`) | Web | Status |
|---|---|---|---|---|
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` | `INTEGER PRIMARY KEY AUTOINCREMENT` | — | Match |
| `guild_id` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `word` | `TEXT NOT NULL COLLATE NOCASE` | `TEXT NOT NULL COLLATE NOCASE` | — | Match |
| `submitted_by` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `submitted_at` | `TEXT DEFAULT (datetime('now','localtime'))` | `TEXT DEFAULT (datetime('now','localtime'))` | — | Match |
| `status` | `TEXT DEFAULT 'pending'` | `TEXT DEFAULT 'pending'` | — | Match |
| `reviewed_by` | `INTEGER DEFAULT NULL` | `INTEGER DEFAULT NULL` | — | Match |
| `reviewed_at` | `TEXT DEFAULT NULL` | `TEXT DEFAULT NULL` | — | Match |
| `UNIQUE` / Indexes | `UNIQUE(guild_id, word)`, `idx_pw_guild`, `idx_pw_status` | `UNIQUE(guild_id, word)`, `idx_pw_guild`, `idx_pw_status` | — | Match |

### 4. `wiki_cache`
| Column / Constraint | Python (`bot.py:340–347, 385`) | Rust (`db.rs:120–127, 223`) | Web | Status |
|---|---|---|---|---|
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` | `INTEGER PRIMARY KEY AUTOINCREMENT` | — | Match |
| `lang` | `TEXT NOT NULL` | `TEXT NOT NULL` | — | Match |
| `word` | `TEXT NOT NULL COLLATE NOCASE` | `TEXT NOT NULL COLLATE NOCASE` | — | Match |
| `cached_at` | `TEXT DEFAULT (datetime('now','localtime'))` | `TEXT DEFAULT (datetime('now','localtime'))` | — | Match |
| `hit_count` | `INTEGER DEFAULT 1` | `INTEGER DEFAULT 1` | — | Match |
| `UNIQUE` / Index | `UNIQUE(lang, word)`, `idx_wc_lang` | `UNIQUE(lang, word)`, `idx_wc_lang` | — | Match |

### 5. `leaderboard`
| Column / Constraint | Python (`bot.py:349–359, 386, 436–438`) | Rust (`db.rs:128–138, 224`) | Web | Status |
|---|---|---|---|---|
| `guild_id` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `user_id` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `total_words` | `INTEGER DEFAULT 0` | `INTEGER DEFAULT 0` | — | Match |
| `current_streak` | `INTEGER DEFAULT 0` | `INTEGER DEFAULT 0` | — | Match |
| `max_streak` | `INTEGER DEFAULT 0` (also `_migrations:438`) | `INTEGER DEFAULT 0` | — | Match |
| `games_played` | `INTEGER DEFAULT 0` (also `_migrations:436`) | `INTEGER DEFAULT 0` | — | Match |
| `games_won` | `INTEGER DEFAULT 0` (also `_migrations:437`) | `INTEGER DEFAULT 0` | — | Match |
| `last_played` | `TEXT DEFAULT NULL` | `TEXT DEFAULT NULL` | — | Match |
| `PRIMARY KEY` / Index | `PRIMARY KEY (guild_id, user_id)`, `idx_lb_guild` | `PRIMARY KEY (guild_id, user_id)`, `idx_lb_guild` | — | Match |

### 6. `tournaments`
| Column / Constraint | Python (`bot.py:361–372, 410, 433–435`) | Rust (`db.rs:139–151, 228`) | Web | Status |
|---|---|---|---|---|
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` | `INTEGER PRIMARY KEY AUTOINCREMENT` | — | Match |
| `guild_id` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `channel_id` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `host_id` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `name` | `TEXT NOT NULL` | `TEXT NOT NULL` | — | Match |
| `prize` | `TEXT DEFAULT NULL` | `TEXT DEFAULT NULL` | — | Match |
| `status` | `TEXT DEFAULT 'open'` | `TEXT DEFAULT 'open'` | — | Match |
| `created_at` | `TEXT DEFAULT (datetime('now','localtime'))` | `TEXT DEFAULT (datetime('now','localtime'))` | — | Match |
| `started_at` | `TEXT DEFAULT NULL` (**only in `_migrations:433`**) | `TEXT DEFAULT NULL` (in `CREATE TABLE:148`) | — | Match after migration; Rust lacks `ALTER` |
| `ended_at` | `TEXT DEFAULT NULL` (also `_migrations:434`) | `TEXT DEFAULT NULL` | — | Match |
| `message_id` | `INTEGER DEFAULT NULL` (also `_migrations:435`) | `INTEGER DEFAULT NULL` | — | Match |
| Index | `idx_tn_guild` | `idx_tn_guild` | — | Match |

### 7. `tournament_players`
| Column / Constraint | Python (`bot.py:374–380`) | Rust (`db.rs:152–158`) | Web | Status |
|---|---|---|---|---|
| `tournament_id` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `user_id` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `words_scored` | `INTEGER DEFAULT 0` | `INTEGER DEFAULT 0` | — | Match |
| `joined_at` | `TEXT DEFAULT (datetime('now','localtime'))` | `TEXT DEFAULT (datetime('now','localtime'))` | — | Match |
| `PRIMARY KEY` | `PRIMARY KEY (tournament_id, user_id)` | `PRIMARY KEY (tournament_id, user_id)` | — | Match |

### 8. `game_scores`
| Column / Constraint | Python (`bot.py:387–396`) | Rust (`db.rs:159–167, 225`) | Web | Status |
|---|---|---|---|---|
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` | `INTEGER PRIMARY KEY AUTOINCREMENT` | — | Match |
| `game` | `TEXT NOT NULL` | `TEXT NOT NULL` | — | Match |
| `user_id` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `best_score` | `INTEGER NOT NULL DEFAULT 0` | `INTEGER NOT NULL DEFAULT 0` | — | Match |
| `plays` | `INTEGER NOT NULL DEFAULT 0` | `INTEGER NOT NULL DEFAULT 0` | — | Match |
| `updated_at` | `TEXT DEFAULT (datetime('now','localtime'))` | `TEXT DEFAULT (datetime('now','localtime'))` | — | Match |
| `UNIQUE` / Index | `UNIQUE(game, user_id)`, `idx_gs_game` | `UNIQUE(game, user_id)`, `idx_gs_game` | — | Match |

### 9. `web_users`
| Column / Constraint | Python (`bot.py:397–409, 439–440, 442, 457–463`) | Rust (`db.rs:168–183, 226–227`) | Web | Status |
|---|---|---|---|---|
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` | `INTEGER PRIMARY KEY AUTOINCREMENT` | — | Match |
| `username` | `TEXT UNIQUE NOT NULL` | `TEXT UNIQUE NOT NULL` | — | Match |
| `email` | `TEXT UNIQUE NOT NULL` | `TEXT UNIQUE NOT NULL` | — | Match |
| `password_hash` | `TEXT` | `TEXT` | — | Match |
| `provider` | `TEXT DEFAULT 'credentials'` | `TEXT DEFAULT 'credentials'` | — | Match |
| `provider_id` | `TEXT` | `TEXT` | — | Match |
| `avatar` | `TEXT` | `TEXT` | — | Match |
| `is_admin` | `INTEGER DEFAULT 0` | `INTEGER DEFAULT 0` | — | Match |
| `totp_secret` | `TEXT DEFAULT NULL` (`_migrations:439`) | `TEXT DEFAULT NULL` | — | Match (fresh); Rust lacks `ALTER` |
| `totp_enabled` | `INTEGER DEFAULT 0` (`_migrations:440`) | `INTEGER DEFAULT 0` | — | Match (fresh); Rust lacks `ALTER` |
| `backup_codes` | **MISSING** | `TEXT DEFAULT NULL` (`:179`) | — | **DIVERGENCE: Missing in Python** |
| `email_verified` | `INTEGER DEFAULT 1` (`ALTER` at `:461`) | `INTEGER DEFAULT 0` (`:180`) | — | **DIVERGENCE: Incompatible DEFAULT (1 vs 0)** |
| `ui_lang` | `TEXT DEFAULT NULL` (`_migrations:442`) | `TEXT DEFAULT NULL` | — | Match (fresh); Rust lacks `ALTER` |
| `created_at` | `DATETIME DEFAULT CURRENT_TIMESTAMP` (`:406`) | `TEXT DEFAULT (datetime('now','localtime'))` (`:182`) | — | **DIVERGENCE: Type & UTC vs localtime** |
| Indexes | `idx_wu_email`, `idx_wu_prov` | `idx_wu_email`, `idx_wu_prov` | — | Match |

### 10. `password_recovery`
| Column / Constraint | Python (`bot.py:414–427`) | Rust (`db.rs:184–194, 229–231`) | Web | Status |
|---|---|---|---|---|
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` | `INTEGER PRIMARY KEY AUTOINCREMENT` | — | Match |
| `user_id` | `INTEGER NOT NULL` | `INTEGER NOT NULL` | — | Match |
| `token_hash` | `TEXT NOT NULL` | `TEXT NOT NULL` | — | Match |
| `type` | `TEXT NOT NULL` | `TEXT NOT NULL` | — | Match |
| `source` | `TEXT DEFAULT NULL` | `TEXT DEFAULT NULL` | — | Match |
| `expires_at` | `TEXT NOT NULL` | `TEXT NOT NULL` | — | Match |
| `used_at` | `TEXT DEFAULT NULL` | `TEXT DEFAULT NULL` | — | Match |
| `attempts` | `INTEGER DEFAULT 0` | `INTEGER DEFAULT 0` | — | Match |
| `created_at` | `TEXT DEFAULT (datetime('now','localtime'))` | `TEXT DEFAULT (datetime('now','localtime'))` | — | Match |
| Indexes | `idx_pr_user`, `idx_pr_hash`, `idx_pr_type` | `idx_pr_user`, `idx_pr_hash`, `idx_pr_type` | — | Match |

### 11. `public_rooms`
| Column / Constraint | Python (`bot.py`) | Rust (`db.rs:195–210`) | Web (`create.js:31–37`, `rooms.js:16–22`) | Status |
|---|---|---|---|---|
| `room_id` | **MISSING TABLE** | `TEXT PRIMARY KEY` | Inserted & Selected | **Missing in Python** |
| `name` | — | `TEXT NOT NULL` | Inserted & Selected | **Missing in Python** |
| `platform` | — | `TEXT NOT NULL` | Inserted (`'web'`) & Selected | **Missing in Python** |
| `host_id` | — | `TEXT NOT NULL` | Inserted (`'web_user'`) & Selected | **Missing in Python** |
| `host_name` | — | `TEXT NOT NULL` | Inserted & Selected | **Missing in Python** |
| `lang` | — | `TEXT DEFAULT 'vi'` | Inserted & Selected | **Missing in Python** |
| `max_players` | — | `INTEGER DEFAULT 0` | Inserted & Selected | **Missing in Python** |
| `current_players` | — | `INTEGER DEFAULT 1` | Inserted (`1`) & Selected | **Missing in Python** |
| `join_token` | — | `TEXT UNIQUE` | Inserted & Selected | **Missing in Python** |
| `web_url` | — | `TEXT` | Inserted & Selected | **Missing in Python** |
| `tele_url` | — | `TEXT` | Inserted & Selected | **Missing in Python** |
| `status` | — | `TEXT DEFAULT 'waiting'` | Inserted (`'waiting'`), Filtered (`'waiting','playing'`) | **Missing in Python** |
| `created_at` | — | `TEXT DEFAULT (datetime('now','localtime'))` | Inserted (`datetime('now','localtime')`), Selected, Ordered | **Missing in Python** |
| `updated_at` | — | `TEXT DEFAULT (datetime('now','localtime'))` | Inserted (`datetime('now','localtime')`) | **Missing in Python** |

### 12. `telegram_room_tokens`
| Column / Constraint | Python (`bot.py`) | Rust (`db.rs:211–219`) | Web | Status |
|---|---|---|---|---|
| `token` | **MISSING TABLE** | `TEXT PRIMARY KEY` | — | **Missing in Python** |
| `room_code` | — | `TEXT NOT NULL` | — | **Missing in Python** |
| `chat_id` | — | `INTEGER NOT NULL` | — | **Missing in Python** |
| `host_id` | — | `INTEGER NOT NULL` | — | **Missing in Python** |
| `host_name` | — | `TEXT NOT NULL` | — | **Missing in Python** |
| `lang` | — | `TEXT DEFAULT 'vi'` | — | **Missing in Python** |
| `created_at` | — | `TEXT DEFAULT (datetime('now','localtime'))` | — | **Missing in Python** |
