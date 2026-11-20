# Forensic Audit Report — Assignment 05

- **ID:** `05`
- **Scope:** Discord Utility, AI & Admin Commands (`bot.py`: `c_ask`, `sl_ask`, `c_setnoitu`, `c_setngonngu`, `c_setuilang`, `c_listlang`, `c_cachestats`, `slash_resetpassword_admin`, `cmd_revoke_2fa`, `on_command_error`, `on_ready`) vs `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\{lib,extra_cmds}.rs` (with Python baseline at `C:\Users\Ai Hoi\Downloads\archive-2026-09-23T144146Z\bot.py`).
- **Status:** **PARTIAL / GAPS IDENTIFIED** (Critical missing admin command, missing permission checks & validation, hardcoded telemetry, and missing error handler).

---

## Findings

### 1. Missing `/resetpassword_admin` Slash Command (High)
- **Python:** `bot.py:3942-3969` (`slash_resetpassword_admin`) verifies `_is_bot_admin(interaction)`, looks up `web_users` by email, inserts a one-time `recovery_token` (`source="admin"`) into `password_recovery`, and replies with an **ephemeral** Embed.
- **Rust:** Absent from `crates/zenne-discord/src/lib.rs:616-679` (`all_commands()`) and `crates/zenne-discord/src/extra_cmds.rs` (even though `crates/zenne-api/src/auth_routes.rs:928,947` supports `recovery_token` verification).

### 2. Missing Admin Permission Checks on `setnoitu` and `setlang` (`setngonngu`) (High — Security / AuthZ)
- **Python:**
  - `c_setnoitu` (`bot.py:1973-1977`) enforces `if not is_admin(ctx):` before calling `db_set_game_channel`.
  - `c_setngonngu` (`bot.py:1987-1991`) enforces `if not is_admin(ctx):` before updating guild dictionary language.
- **Rust:**
  - `extra_cmds::setnoitu` (`extra_cmds.rs:6-17`) has **no** admin/permission check; any guild member can overwrite the official Nối Từ channel.
  - `setlang` (`lib.rs:107-119`, alias `setngonngu`) has **no** admin/permission check; any guild member can change the guild's dictionary language.

### 3. Missing Language Validation & Optional Listing in `setlang` / `setuilang` (Medium)
- **`setlang` (`lib.rs:107-119`) vs `c_setngonngu` (`bot.py:1988-2031`):**
  - Python makes `lang_code` optional (shows `POPULAR_LANGS` 3-column embed when omitted), validates format (`len <= 12`, alphanumeric/`-`/`_`), and verifies `{lang_code}.wikipedia.org` via `validate_lang()`.
  - Rust requires `code: String`, performs **no validation**, and persists arbitrary strings directly to DB (`lib.rs:110-111`). Also missing Python aliases `lang`, `ngonngu`, `language`.
- **`setuilang` (`lib.rs:122-134`) vs `c_set_ui_lang` (`bot.py:2044-2061`):**
  - Rust requires `code: String` (cannot omit to view current UI lang), lacks aliases (`uilang`, `doingondngiaodien`, `ngugiaodien`), and hardcodes `"vi"` on invalid input (`lib.rs:127`) instead of using the guild's current `ui_lang` (`bot.py:2057`).

### 4. `on_ready` Hardcoded `latency_ms` & Missing Presence (Medium)
- **Python:** `on_ready` (`bot.py:1772-1784`) logs `bot.user`, syncs slash commands, sets Discord activity presence (`Playing "Noi Tu Wiki | !help_noitu"`), and starts the web API.
- **Rust:** `FullEvent::Ready` in `event_handler` (`lib.rs:480-489`) hardcodes `live.latency_ms = Some(25.0);` (`lib.rs:488`) instead of reading actual shard/gateway latency, and never sets bot presence/activity.

### 5. Missing `on_command_error` Typo Suggestions & Argument Hints (Medium)
- **Python:** `on_command_error` (`bot.py:1787-1801`) handles `CommandNotFound` with a `suggest` dictionary (`thangia`/`thamgiam` -> `thamgia`, `batdau` -> `batdautro`, `bdl` -> `batdaulai`, `score` -> `diem`, `help` -> `help_noitu`, `setlanguage`/`language` -> `setngonngu`) and handles `MissingRequiredArgument`.
- **Rust:** `poise::FrameworkOptions` (`lib.rs:571-581`) uses `..Default::default()` with **no** `on_error` callback, so typos and missing arguments receive no custom guidance.

### 6. `listlang` and `cachestats` Functional Downgrades (Low–Medium)
- **`listlang` (`extra_cmds.rs:131-138`) vs `c_list_lang` (`bot.py:2064-2085`):**
  - Python accepts an optional search keyword (`tukhoa`), searches 300+ languages in `LANGUAGE_CATALOG` (`limit=25`), and formats an Embed.
  - Rust takes no search parameter and returns a static hardcoded 10-language message.
- **`cachestats` (`extra_cmds.rs:141-150`) vs `c_cachestats` (`bot.py:2690-2730`):**
  - Python renders an Embed with per-language cache count, global total, top 10 cached words by `hit_count`, and per-language distribution.
  - Rust dumps raw JSON from `db.stats_summary().await` in a code block.

### 7. `revoke_2fa` & `ask` (`c_ask` / `sl_ask`) Observations (Low–Medium)
- **Note on Python baseline:** `c_ask`, `sl_ask`, and `cmd_revoke_2fa` are **not present** in `archive-2026-09-23T144146Z\bot.py` (only web 2FA routes exist at `bot.py:4530-4585`).
- **Rust `extra_cmds::revoke_2fa` (`extra_cmds.rs:318-340`):**
  - Checks `super_admin_ids.contains(&uid)`, which is good, **but** ignores SQL `execute` result and `rows_affected` (`extra_cmds.rs:327-338`) — always reporting `"✅ Đã gỡ 2FA..."` even when no matching `username`/`email` exists or the query fails. Also not ephemeral.
- **Rust `ask` (`lib.rs:223-246`):**
  - Implemented as hybrid slash/prefix command calling `zenne_core::kira::ask_ai_kira` with `ctx.defer().await?` and 1900-char truncation.

---

## Recommended Tests
1. **AuthZ / Permission Tests:** Verify non-admin users are rejected on `setnoitu` and `setlang`, while guild admins / `super_admin_ids` succeed.
2. **`/resetpassword_admin` Integration Test:** Verify admin-only access, unknown email error, valid email `password_recovery` row insertion (`type='recovery_token', source='admin'`), and ephemeral response.
3. **`setlang` & `setuilang` Validation Tests:** Reject malformed/non-existent Wikipedia codes in `setlang`; verify `setuilang` preserves guild `ui_lang` on invalid code error.
4. **`revoke_2fa` DB Outcome Test:** Assert distinct error/not-found response when `rows_affected == 0` vs success when `rows_affected == 1`.
5. **Error Handler (`on_error`) Unit Test:** Feed typo commands (`!thangia`, `!bdl`, `!setlanguage`) and missing-argument invocations into Poise `on_error` and assert expected suggestion strings.
6. **Telemetry Test:** Verify `live.latency_ms` reflects shard runner latency (or `None` before first heartbeat) rather than a hardcoded `25.0`.
