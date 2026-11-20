# Forensic Audit Report — ID 28

**Scope:** Database Transactions, Atomicity & Concurrency parity audit across:
- Python: `C:\Users\Ai Hoi\Downloads\archive-2026-09-23T144146Z\bot.py`
- Rust: `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\db.rs` (plus direct `db.conn()` callers in `crates\zenne-api\src\auth_routes.rs`, `crates\zenne-api\src\lib.rs`, and `crates\zenne-discord\src\extra_cmds.rs`)

---

## 1. Connection Pooling & Locking

| Aspect | Python (`bot.py`) | Rust (`zenne-core/src/db.rs`) |
|---|---|---|
| **Connection Strategy** | `get_db()` (`bot.py:303-307`) opens a **brand-new** `sqlite3.connect(DB_PATH, check_same_thread=False)` on every call and executes `PRAGMA journal_mode=WAL`. **No `conn.close()`** is ever called (`with get_db() as conn:` only commits/rolls back on `__exit__`, leaking FDs until GC). **No `PRAGMA busy_timeout`** is set. | `DbManager::new` (`db.rs:70-85`) builds a `libsql::Database` via `Builder::new_remote(...)` (`db.rs:74-76`). `DbManager::conn(&self)` (`db.rs:87-89`) calls `self.db.connect()` per operation. **No connection pool** (every DB helper opens a fresh remote `Connection`). |
| **Locking** | Global `_db_lock = threading.Lock()` (`bot.py:301`) guards `db_*` helpers called via `_executor`. | **No mutex/lock** in `DbManager`; relies entirely on remote Turso/libSQL server serialization. |
| **Lock Bypass / Event-Loop Blocking (CRITICAL)** | 1. `_dbc()` (`bot.py:3214-3217`) opens raw `sqlite3.connect(DB_PATH)` **without `_db_lock`**, **without WAL pragma**, and **without `busy_timeout`**, called synchronously inside `async def _ah_*` handlers (`_ah_stats`, `_ah_pending`, `_ah_review`, `_ah_review_bulk`, `_ah_wordbank_get`, `_ah_wordbank_del` at `bot.py:3224-3351`). This races with `_db_lock` threads (`database is locked`) and blocks the asyncio event loop.<br>2. Auth/recovery handlers (`_ah_auth_register` `4251`, `_ah_register_email_verify` `4319`, `_ah_register_email_resend` `4381`, `_ah_recovery_request` `4618`, `_ah_recovery_email_verify` `4660`, `_ah_recovery_token_verify` `4709`, `_ah_recovery_status` `4746`, `_ah_recovery_reset_password` `4775`) and `slash_resetpassword_admin` (`3950`) acquire `_db_lock` **directly on the asyncio event loop** instead of `run_in_executor`. | Async `libsql` calls do not block the Tokio runtime, and `auth_routes.rs` / `extra_cmds.rs` / `lib.rs` call `db.conn()` directly without local locks. |

---

## 2. Multi-Statement Operations Lacking `BEGIN IMMEDIATE` / Transactions

- **Python (`bot.py`)**:
  - **Zero `BEGIN IMMEDIATE` statements** in the entire file. `sqlite3` uses default `isolation_level=""` (deferred `BEGIN`). Because `_dbc()` (`bot.py:3214`) bypasses `_db_lock`, concurrent `_dbc()` + `_db_lock` writers can hit SQLite's deferred-write-upgrade deadlock (`SQLITE_BUSY` immediately).
  - Multi-statement R/W sequences without explicit `BEGIN IMMEDIATE`:
    - `db_submit_pending` (`bot.py:545-579`): `SELECT` `word_bank` → `SELECT` `pending_words` → `UPDATE`/`INSERT` `pending_words`.
    - `db_review_word` (`bot.py:599-622`): `SELECT` `pending_words` → `UPDATE` `pending_words` → `INSERT OR IGNORE` `word_bank`.
    - `_ah_review` (`bot.py:3272-3284`) & `_ah_review_bulk` (`bot.py:3300-3311`): **no `_db_lock`**, `SELECT` → `UPDATE` → `INSERT OR IGNORE` → `c.commit()`.
    - `db_lb_add_game` (`bot.py:736-757`), `db_tournament_add_word` (`bot.py:1068-1088`), `db_tournament_end` (`bot.py:1104-1120`): multi-statement writes relying on `with get_db() as conn:` implicit commit (no explicit `conn.commit()`).
    - `_ah_recovery_request` (`4618-4636`), `_ah_recovery_email_verify` (`4660-4688`), `_ah_recovery_token_verify` (`4709-4731`), `_ah_recovery_reset_password` (`4775-4795`), `slash_resetpassword_admin` (`3950-3963`): multi-statement writes also omit `conn.commit()`, relying on `with get_db() as conn:` `__exit__`.

- **Rust (`db.rs` & `zenne-api`/`zenne-discord`)**:
  - **Zero `conn.transaction()` / `BEGIN IMMEDIATE` / `COMMIT` / `ROLLBACK` anywhere** in `db.rs` or `auth_routes.rs`. Every `execute()` / `query()` is auto-committed independently over remote libSQL:
    - `submit_pending` (`db.rs:359-398`): 3 separate round-trips (`word_exists` → `INSERT ... ON CONFLICT DO UPDATE` → `SELECT id`).
    - `review_word` (`db.rs:431-461`): `SELECT` → `UPDATE pending_words` → `self.add_approved_word(...)` (which opens a **second** `self.conn()` at `db.rs:349`). Partial failure leaves `pending_words.status='approved'` without inserting into `word_bank`.
    - `cache_exists` (`db.rs:463-483`): `SELECT 1` followed by separate `UPDATE wiki_cache SET hit_count = hit_count + 1`.
    - `lb_add_game` (`db.rs:611-627`): loops over `players` executing N independent auto-committed `INSERT ... ON CONFLICT DO UPDATE` statements; partial failure leaves inconsistent game counts.
    - `tournament_add_word` (`db.rs:1138-1158`): `SELECT id FROM tournaments` followed by separate `INSERT ... ON CONFLICT` into `tournament_players`.
    - `stats_summary` (`db.rs:706-738`), `pub_wordbank` (`db.rs:740-792`), `pub_rejected` (`db.rs:794-850`), `pub_leaderboard` (`db.rs:852-876`): multiple queries without a read transaction (non-atomic snapshots; `pub_leaderboard` also does N+1 `resolve_display_user` queries at `db.rs:865`).
    - `handle_recovery_reset_password` (`auth_routes.rs:1191-1217`): 3 independent writes (`UPDATE password_recovery` consume → `UPDATE web_users` password → `UPDATE password_recovery` invalidate all). If `UPDATE web_users` fails, the token is burned without updating the password.

---

## 3. TOCTOU Race Conditions

1. **`review_word`**:
   - **Rust (`db.rs:431-461`) — CRITICAL TOCTOU:** `SELECT guild_id, word, submitted_by FROM pending_words WHERE id = ?1 AND status = 'pending'` (`438-445`) followed by `UPDATE pending_words SET status = ?1 ... WHERE id = ?3` (`450-455`) **without `AND status = 'pending'` and without a transaction**. Two concurrent reviewers both pass `SELECT`, both overwrite `status` (last-writer-wins), both return `Some(...)`, and if one approves while the other rejects, `word_bank` still receives the word (`456-459`) while `pending_words` can end up `'rejected'`.
   - **Python (`bot.py:3260-3312`) — CRITICAL TOCTOU:** `db_review_word` (`bot.py:599-622`) is serialized by `_db_lock`, **but** `_ah_review` (`bot.py:3272-3285`) and `_ah_review_bulk` (`bot.py:3300-3312`) use `_dbc()` **without `_db_lock`**, check `row["status"] != "pending"` in Python (`3276`), and execute `UPDATE ... WHERE id=?` without `AND status='pending'` (`3279`).

2. **`submit_pending`**:
   - **Rust (`db.rs:359-398`) — TOCTOU:** `self.word_exists(guild_id, word).await` (`366`) runs on a separate connection before `INSERT INTO pending_words` (`369-380`). A concurrent approval into `word_bank` between `366` and `369` allows a duplicate word into `pending_words`.
   - **Python (`bot.py:545-579`):** Protected by `_db_lock` against `db_*` helpers, but races with `_ah_review` / `_ah_wordbank_del` (`_dbc()` without `_db_lock`).

3. **`recovery_tokens` (`password_recovery`)**:
   - **Python (`bot.py:4660-4796`):** `_ah_recovery_token_verify` (`4718-4723`) and `_ah_recovery_reset_password` (`4783-4788`) use atomic `WHERE id=? AND used_at IS NULL` + `cur.rowcount != 1`. However, `_ah_recovery_email_verify` (`4677-4681`) and `_ah_register_email_verify` (`4342-4355`) compute `attempts` / `used_at` in Python and `UPDATE ... WHERE id=?` without `AND used_at IS NULL` (mitigated only by `_db_lock` in single-process Python).
   - **Rust (`auth_routes.rs:837-1220`):** `handle_recovery_email_verify` (`904-917`), `handle_recovery_token_verify` (`1003-1016`), and `handle_recovery_reset_password` (`1191-1204`) **do** use atomic `WHERE id = ?1 AND used_at IS NULL` + `if consumed != 1`. **However**, `attempts` enforcement in `handle_recovery_email_verify` (`881-896`) and `handle_register_verify` (`451-467`) has a **TOCTOU race**: `SELECT attempts` → check `attempts >= 5` in Rust → `UPDATE ... SET attempts = attempts + 1 WHERE id = ?1` without `AND attempts < 5`, allowing concurrent brute-force requests to bypass the 5-attempt cap.

4. **`lb_add_word`**:
   - **Rust (`db.rs:559-574`):** Single atomic `INSERT ... ON CONFLICT(guild_id, user_id) DO UPDATE SET total_words = total_words + 1, current_streak = current_streak + 1, max_streak = MAX(max_streak, current_streak + 1)` — **atomic**, no TOCTOU. (Note: concurrent `lb_reset_streak` at `576-585` can interleave).
   - **Python (`bot.py:708-721`):** Single `INSERT ... ON CONFLICT DO UPDATE` under `_db_lock`, but `max_streak` uses caller-supplied `current_streak` (`excluded.max_streak`) rather than SQL-side `current_streak + 1`.

5. **`tournament_*`**:
   - **Rust (`db.rs:1127-1158` & `extra_cmds.rs:245-315`):**
     - `tochucgiaidau` (`extra_cmds.rs:253-260`) inserts a new `'active'` tournament **without checking** if one is already active for `guild_id` (allowing multiple concurrent active tournaments).
     - `tournament_add_word` (`db.rs:1138-1158`) has a **TOCTOU** between `SELECT id FROM tournaments WHERE guild_id = ?1 AND status = 'active'` (`1141-1145`) and `INSERT INTO tournament_players` (`1150-1155`); `ketthucgiaidau` (`extra_cmds.rs:302-315`) can end the tournament in between, yet the word still scores.
     - `tournament_join` (`db.rs:1127-1136`) does not verify `tournaments.status = 'active'`.
   - **Python (`bot.py:1023-1120`, `2568-2595`, `3024-3038`):** `db_tournament_*` helpers are serialized by `_db_lock`, **but** the Discord command handlers (`2568` & `3024`) call `db_tournament_get_active` and `db_tournament_create` in two separate `run_in_executor` calls — a **TOCTOU** allowing two concurrent `!tochucgiaidau` commands to both create active tournaments.

---

## 4. Turso / libSQL Error Handling (`db.rs` & `auth_routes.rs`)

- **Pervasive silent error swallowing (`let _ =`, `.ok()`, `.unwrap_or(...)`):**
  - Write methods in `db.rs` return `()` and discard all `libsql::Error`s: `set_lang` (`260`), `set_ui_lang` (`293`), `set_game_channel` (`319`), `cache_add` (`487`), `lb_add_word` (`561`), `lb_reset_streak` (`578`), `lb_add_game` (`615`), `game_submit_score_mode` (`681`), `create_public_room` (`990`), `update_public_room_players` (`1024`), `close_public_room` (`1035`), `save_telegram_token` (`1054`), `tournament_add_word` (`1149`).
  - `review_word` (`db.rs:450-455`) does `let _ = conn.execute("UPDATE pending_words ...", ...).await;` — if the `UPDATE` fails, it still calls `add_approved_word` and returns `Some(...)`!
- **Misleading error masking:**
  - `submit_pending` (`db.rs:382`) uses `.map_err(|_| "Từ này đã được gửi chờ duyệt trước đó!".to_string())?`, masking network timeouts, auth failures, HTTP 5xx, and schema errors as a duplicate-word user error.
- **Dangerous fallback on read errors:**
  - `word_exists` (`db.rs:328-340`) returns `false` on connection/query error; `submit_pending` (`366`) relies on it, so a transient libSQL error is treated as "word does not exist in `word_bank`".
  - `get_lang` (`238-256`), `stats_summary` (`706-738`), `pub_wordbank` (`740-792`), `pub_rejected` (`794-850`), `add_approved_word` (`342-357`) all silently return defaults (`"vi"`, `0`, `vec![]`, `false`) on DB failure.
- **No logging & no retry/backoff:** `db.rs` has zero `tracing::error!` / `warn!` calls and no retry logic for transient remote Turso/Hrana errors.
- **`auth_routes.rs`:** `.unwrap_or(0)` at `910`, `1009`, `1197` conflates DB/network errors with `0 rows affected`, and `let _ = conn.execute(...)` at `891`, `926`, `1025`, `1206`, `1212` silently drops failures on password/recovery mutations.
