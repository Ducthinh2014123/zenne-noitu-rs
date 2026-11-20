# Forensic Audit Report — ID `03`

**Scope:** Word Bank, Pending/Rejected Review & Reactions  
- **Python (`C:\Users\Ai Hoi\Downloads\gdrive-bot-source\bot.py`):** `ApproveView`, `send_pending_to_admin`, `c_gopthemtu`, `sl_kiemduyettu`, `c_tutruong`, `cmd_danhsachtu`, `db_submit_pending`, `db_review_word`, `on_raw_reaction_add`
- **Rust (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\`):** `zenne-discord/src/lib.rs`, `zenne-discord/src/extra_cmds.rs`, `zenne-core/src/db.rs`

**Status:** **Partial / Divergent** — Core SQLite schemas (`word_bank`, `pending_words`) and `submit_pending`/`review_word` state transitions match, but interactive review UI (`ApproveView`), admin DM notifications, guild-admin permissions, guild-scoped word bank listing, pagination, and lobby reaction handling are missing or altered in Rust.

---

## Findings

1. **Missing `ApproveView` Interactive UI & Admin/Submitter DMs**
   - **Python:** `ApproveView` (`bot.py:2449-2537`) provides persistent `approve_btn`/`reject_btn` buttons, edits the embed with color/footer status, and DMs `submitter_id` on decision. `send_pending_to_admin` (`bot.py:2539-2556`) DMs `SUPER_ADMIN` immediately on `!gopthemtu` (`bot.py:3038`). `sl_kiemduyettu` (`bot.py:3639-3720`) lists up to 20 pending words with an `ApproveView` per word.
   - **Rust:** `gopthemtu` (`lib.rs:198-220`) only replies in-channel with the pending ID (no admin DM, no buttons). `duyettu` / alias `kiemduyettu` (`extra_cmds.rs:721-748`) requires manually passing `id: i64` and `approve: Option<bool>` (no pending queue listing, no submitter DM). `event_handler` (`lib.rs:473-547`) has no `InteractionCreate` / component handler.

2. **Review Permission Model Narrowed to Super Admins Only**
   - **Python:** `ApproveView._check_perm` (`bot.py:2461-2470`) and `sl_kiemduyettu` (`bot.py:3641-3647`) permit `is_super_admin(uid)` **or** guild `administrator` / `manage_channels`.
   - **Rust:** `duyettu` (`extra_cmds.rs:728-731`) only checks `super_admin_ids.contains(&uid)`, blocking server admins/moderators from reviewing words.

3. **Word Bank Listing: Guild Scope & Pagination Lost**
   - **Python:** `c_tutruong` (`bot.py:3081-3096`, aliases `wordbank`, `bank`) is **guild-scoped** (`db_get_words(gid, 60)` at `bot.py:1285-1292`, `db_word_count(gid)` at `bot.py:1206-1212`). `cmd_danhsachtu` / `slash_danhsachtu` (`bot.py:4880-4912`, aliases `khotudu`, `frombank`) is **global** and paginated (`trang: int = 1`, 10/page).
   - **Rust:** `danhsachtu` (`extra_cmds.rs:153-175`) merges `tutruong` and `wordbank` as aliases onto one command calling `pub_wordbank("", 20, 0)` (`db.rs:740-792`), which is **global** (no `WHERE guild_id = ?`) and **unpaginated** (hardcoded 20-word limit, no `trang` argument; `khotudu`/`frombank` aliases omitted).

4. **`gopthemtu` Validation & Alias Parity**
   - **Python:** `c_gopthemtu` (`bot.py:3003-3012`) has aliases `addword`, `gtt`, rejects empty input and `len(word) > 80`.
   - **Rust:** `gopthemtu` (`lib.rs:197-205`) has no aliases, no empty-string check after `wiki::normalize_word(&word)`, and no `len > 80` guard before calling `db.submit_pending`.

5. **`submit_pending` / `review_word` DB Behavior**
   - **Parity:** Schemas (`bot.py:843-863` vs `db.rs:100-119`) match (`COLLATE NOCASE`, `UNIQUE(guild_id, word)`), and both allow resubmitting `'rejected'` words (`bot.py:1148-1154` vs `db.rs:372-378`). `review_word` (`db.rs:431-461`) matches `db_review_word` (`bot.py:1181-1204`).
   - **Divergence:** `db.rs:382-385` maps *all* SQL errors on `INSERT/ON CONFLICT` to `"Từ này đã được gửi chờ duyệt trước đó!"` (masking real DB errors) and uses a follow-up `SELECT id` (`db.rs:386-397`) instead of `RETURNING id`. Also `pub_rejected` (`db.rs:794-850`) serves rejected words for the web API, with no Discord command in either codebase.

6. **Lobby Reaction Join Missing in Rust**
   - **Python:** `on_raw_reaction_add` (`bot.py:2608-2623`) lets players join a lobby by reacting `🙋` on `game.lobby_msg_id`.
   - **Rust:** `event_handler` (`lib.rs:473-547`) does not handle `FullEvent::ReactionAdd`; only in-game `✅` reaction on valid words exists (`noitu.rs:133`), and lobby joining is command-only (`thamgia` at `extra_cmds.rs:21`).

---

## Recommended Tests

1. **DB Unit Tests (`zenne-core/src/db.rs`):**
   - `submit_pending`: verify duplicate `'pending'` returns `Err`, `'rejected'` resets to `'pending'` and clears `reviewed_by`/`reviewed_at`, case-insensitive duplicate in `word_bank` returns `Err`.
   - `review_word`: verify `approve=true` inserts into `word_bank` and sets `status='approved'`; `approve=false` sets `status='rejected'` without inserting; re-reviewing non-pending ID returns `None`.
2. **Command Validation & Permission Tests (`zenne-discord`):**
   - `gopthemtu`: reject empty/whitespace-only and `>80`-char inputs.
   - `duyettu` / `kiemduyettu`: verify non-admin rejection and (once aligned) guild `administrator`/`manage_channels` acceptance.
   - `danhsachtu` / `tutruong`: verify guild-scoped filtering and page offset calculation (`trang >= 1`).
3. **Event / Interaction Tests:**
   - Verify `ReactionAdd` on lobby message adds user once (when implemented) and `ApproveView` button interactions disable buttons and reject double-review.
