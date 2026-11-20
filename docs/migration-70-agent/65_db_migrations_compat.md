# Forensic Audit Report — ID 65

## Scope & Files Audited
- `zenne-core/src/db.rs` (`DbManager::new` L70–85, `DbManager::init_schema` L91–236)
- `gdrive-bot-source/gdrive-bot/bot.py` (`init_db` L835–1048)

---

## Findings
1. **Rust `init_schema()` Performs Zero `ALTER TABLE` Migrations on Existing Tables:** Only executes `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS`. Running against a legacy Python SQLite/Turso DB leaves `web_users.backup_codes` (never present in Python), `totp_secret`, `totp_enabled`, `ui_lang`, `email_verified`, `guild_settings.ui_lang`, `leaderboard.{games_played,games_won,max_streak}`, and `tournaments.{started_at,ended_at,message_id}` missing, causing runtime SQL errors.
2. **Schema Default & Type Divergences:** `email_verified` default (`1` in Python migration vs `0` in Rust); `created_at` (`CURRENT_TIMESTAMP` UTC vs `datetime('now','localtime')`).
3. **Silent DDL Error Suppression:** `db.rs:233-235` uses `let _ = conn.execute(stmt, ()).await;`. `DbManager::new` ignores `SQLITE_PATH` local SQLite fallback.
4. **Compatible Areas:** All 12 tables & 12 indexes match on fresh DBs; `is_expired` parses both float epoch and RFC3339; backup codes accept plaintext and SHA-256 hashes.
