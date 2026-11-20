# Forensic Audit Report — ID 56

**Target Repository:** `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs`
**Files Audited:** `crates/zenne-core/src/db.rs`, `crates/zenne-api/src/auth_routes.rs`.

**Executive Summary:**
All database operations use remote libSQL (`Builder::new_remote` in `db.rs:70-85`) with a new `Connection` per call (`db.rs:87-89`) and **zero** transactions (`BEGIN`/`COMMIT`/`ROLLBACK` or `conn.transaction()`), paired with pervasive silent error swallowing (`let _ =`, `.ok()`, `.unwrap_or(...)`) and non-atomic `attempts < 5` OTP checks.

---

## 1. Multi-Step Remote libSQL Calls Without Transactions
1. **`DbManager::review_word` (`db.rs:431-461`):** `SELECT` -> `let _ = UPDATE` (lacks `AND status = 'pending'`) -> `add_approved_word` on a second `conn()`. Double-review TOCTOU or partial failure leaves `pending_words` and `word_bank` out of sync.
2. **`DbManager::submit_pending` (`db.rs:359-398`):** Separate `word_exists` + `INSERT` + `SELECT id` (instead of `RETURNING id`), mapping any libSQL error to `"Từ này đã được gửi chờ duyệt trước đó!"`.
3. **`handle_recovery_reset_password` (`auth_routes.rs:1097-1220`):** `verify_totp_or_backup` (deletes backup code) -> `UPDATE password_recovery` (`used_at`) -> `let _ = UPDATE web_users` -> `let _ = UPDATE password_recovery` (invalidate rest). If `UPDATE web_users` fails, the token and backup code are burned while returning 200 OK.
4. **`DbManager::lb_add_game` (`db.rs:611-627`):** Loops over players executing independent auto-committed `let _ = conn.execute(...)` calls.
5. **`DbManager::tournament_add_word` (`db.rs:1138-1158`):** Separate `SELECT` active tournament + `let _ = INSERT`, falling back to `tournament_id = 0` on read error.

---

## 2. Silent Error Swallowing
- `db.rs`: 16+ write methods discard `Result` with `let _ = conn.execute(...)`; read methods mask DB/network errors as `None`/`false`/`0` via `.ok()` / `.unwrap_or(...)`.
- `auth_routes.rs`: `let _ = conn.execute(...)` at lines `74, 319, 325, 331, 462, 490, 541, 547, 554, 625, 696, 722, 773, 779, 786, 891, 926, 1025, 1206, 1212` returns `200 OK` even when DB mutations fail.

---

## 3. `attempts < 5` TOCTOU
- `handle_register_email_verify` (`auth_routes.rs:415-488`) and `handle_recovery_email_verify` (`auth_routes.rs:845-917`) check `attempts >= 5` in Rust after `SELECT`, then run `UPDATE ... SET attempts = attempts + 1 WHERE id = ?1` without `AND attempts < 5` and consume with `WHERE id = ?1 AND used_at IS NULL` without `AND attempts < 5`.
