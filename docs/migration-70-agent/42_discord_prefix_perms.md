# Forensic Audit Report — ID 42

**Scope:** Discord prefix-command parsing & permission parity audit between Python (`discord.py` `@bot.command` in `gdrive-bot-source/bot.py`, `gdrive-bot-source/music_system.py`, `gdrive-bot-source/gdrive-bot/*_system.py`) and Rust (`poise` `#[poise::command(prefix_command, ...)]` in `zenne-noitu-rs/crates/zenne-discord/src/*.rs`).

---

## 1. Prefix & Command Dispatch Setup
- **Python (`C:\Users\Ai Hoi\Downloads\gdrive-bot-source\bot.py`):**
  - `bot = commands.Bot(command_prefix="!", intents=intents, help_command=None, proxy=DISCORD_PROXY)` (`L2259`).
  - `on_command_error` (`L2591-2604`) handles `CommandNotFound` (with typo suggestions) and `MissingRequiredArgument`.
- **Rust (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\lib.rs`):**
  - `poise::PrefixFrameworkOptions { prefix: Some("!".into()), ..Default::default() }` (`L573-576`).
  - No custom `on_error` or `command_check` hook is configured in `poise::FrameworkOptions` (`L571-581`).

---

## 2. Multi-Word Argument Parsing (`#[rest]`, `*`, `|` Delimiter)

### 2.1 `#[rest]` vs Python Keyword-Only Consume-Rest (`*, arg`)
- **Parity preserved (`#[rest]` present):**
  - `!tratu` (`lib.rs:138`), `!gopthemtu` (`lib.rs:198`), `!ask` (`lib.rs:224`), `!play` (`music.rs:282-286`), `!afk` (`lib.rs:250`).
- **Parity breaks (Missing `#[rest]` in Rust):**
  - `!contukhong` (`extra_cmds.rs:89`): required and lacks `#[rest]`.
  - `!taoroom` (`extra_cmds.rs:179`): `name: Option<String>` lacks `#[rest]`.
  - Minigame inline prefix arguments (`vtv`, `quiz`, `wordle`, `anagram`, `cadao`, `chinhta`, `emoji`, `flag`, `history`, `whoami`, `typing`, `catword`, `tamsao`) all lack `#[rest]`, truncating multi-word Vietnamese answers to the first word unless quoted.

### 2.2 `|` Delimiter (`!tochucgiaidau`)
- **Python (`bot.py:3405-3436`):** `c_tochucgiaidau(ctx, *, args=None)` splits raw `args` on `"|"`.
- **Rust (`extra_cmds.rs:244-268`):** `tochucgiaidau(ctx, name: String, prize: Option<String>)` does not use `#[rest]` and does not split on `|`.

### 2.3 Other Music Parameter/Name Divergences
- `!volume`: Python (`music_system.py:976`) makes `level` optional (shows current volume); Rust (`music.rs:710`) requires `level: u32`.
- `!loop` vs `!rloop`: Python uses `loop` (`music_system.py:990`); Rust uses `rloop` (`music.rs:733`) with no `"loop"` alias.

---

## 3. Permission Parity (`is_admin`, `manage_channels`, `super_admin_ids`)

### 3.1 `is_admin` & `manage_channels`
- **Python (`bot.py:2386-2398`):** Enforces `is_admin(ctx)` (`super_admin` or `administrator` or `guild.owner_id` or `manage_channels`) on `c_setnoitu`, `c_setngonngu`, `c_tochucgiaidau`, `c_ketthucgiaidau`, and host-or-admin on `c_batdautro`, `c_stop`, `c_batdaulai`.
- **Rust (`zenne-discord/src/*.rs`):** **Zero `is_admin` or `manage_channels` checks exist anywhere in `zenne-discord/src/*.rs`**. Any guild member can run `setnoitu`, `setlang`, `tochucgiaidau`, `ketthucgiaidau`, `stop`, and `batdaulai`.

### 3.2 `super_admin_ids`
- **ID Mismatch:** Python `SUPER_ADMIN = 1395221255589986374` (`bot.py:67`) vs Rust default `"1177130080259219507"` (`config.rs:64`).
- `revoke_2fa`: Python (`bot.py:5215`) allows Super Admin or Guild Administrator; Rust (`extra_cmds.rs:321`) allows only `super_admin_ids`.

---

## 4. DM vs Guild Checks
- **Python (`bot.py:2626-2630`):** All `@bot.command` prefix commands are globally blocked in DMs (`if not message.guild: return`).
- **Rust:** Music commands reject DMs via `ctx.guild_id().ok_or(...)`. All other commands in `lib.rs` and `extra_cmds.rs` fall back to `guild_id = 0` and execute in DMs, allowing DM users to mutate `guild_id = 0` DB rows (`setnoitu`, `setlang`, `setuilang`, `tochucgiaidau`, `ketthucgiaidau`).

---

## 5. Cooldowns
- Neither Python nor Rust applies `@commands.cooldown` / `cooldown` on Discord prefix commands.
