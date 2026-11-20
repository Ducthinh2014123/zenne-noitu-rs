# Forensic Audit Report — ID 39

**Scope:** Legacy Python (`C:\Users\Ai Hoi\Downloads\gdrive-bot-telegram\`, plus older snapshot `C:\Users\Ai Hoi\Downloads\archive-2026-09-23T144146Z\`) vs. Rust rewrite (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\`).

---

## 1. Temp Files & Audio Downloads (`sc_track_*` vs `zenne_sc_*`)

### Python (`gdrive-bot-telegram\music_system.py`)
- **`download_soundcloud_audio(stream_url, guild_id)`** (`music_system.py:227-327`):
  - Resolves OS temp dir via `tempfile.gettempdir()` (`music_system.py:236`).
  - For M3U8/HLS playlists (`content_bytes.lstrip().startswith(b"#EXTM3U")`, line `246`), writes `sc_track_{guild_id}.{ext}` (`m4a` or `mp3`) at `music_system.py:265-266` via `open(out_path, "wb")` (`music_system.py:290`).
  - Downloads first 45 segments concurrently, then spawns a background task `append_remaining()` (`music_system.py:297-315`) that appends remaining segments via `open(out_path, "ab")` (`music_system.py:310`).
  - For progressive MP3, writes `sc_track_{guild_id}.mp3` (`music_system.py:320-322`) via `open(out_path, "wb")`.
  - **Forensic Gap:** **No cleanup/deletion** (`os.remove` / `os.unlink`) exists anywhere in `music_system.py`. Temp files persist on disk indefinitely after playback/disconnect, and skipping a track before `append_remaining()` finishes causes a write race on the same `sc_track_{guild_id}.{ext}` path.

### Rust (`zenne-noitu-rs\crates\zenne-music\src\lib.rs`)
- **`MusicManager::download_audio`** (`lib.rs:323-326`) -> prefix `zenne_sc_{guild_id}`.
- **`MusicManager::download_audio_for_worker`** (`lib.rs:328-339`) -> prefix `zenne_sc_w{worker_id}_{guild_id}`.
- **`MusicManager::download_audio_with_prefix`** (`lib.rs:341-430`):
  - Uses `std::env::temp_dir()` (`lib.rs:353`).
  - Checks `bytes.starts_with(b"#EXTM3U")` (`lib.rs:355` — note: does not `.lstrip()` leading whitespace unlike Python).
  - Buffers all M3U8 segments in memory (`full_bytes: Vec<u8>`, `lib.rs:406-409`), then atomically writes `{file_prefix}.{ext}` (`m4a` or `mp3`, `lib.rs:381-382`) via `tokio::fs::write(&out_path, &full_bytes)` (`lib.rs:414`). Progressive MP3 writes `{file_prefix}.mp3` via `tokio::fs::write(&out_path, &bytes)` (`lib.rs:424-425`). No background append race.
- **Explicit Cleanup:**
  - `cleanup_guild_audio` (`lib.rs:456-458`), `cleanup_worker_guild_audio` (`lib.rs:460-462`), and `cleanup_audio_prefix` (`lib.rs:464-472`) delete `.m4a` and `.mp3` via `tokio::fs::remove_file(&p)` (`lib.rs:469`).
  - Called in `TrackEndNotifier::act` (`crates\zenne-discord\src\music.rs:23`), `WorkerTrackEndNotifier::act` (`crates\zenne-discord\src\music.rs:49`), `stop` command (`crates\zenne-discord\src\music.rs:658`), `MultiPlayManager::release_session` (`crates\zenne-music\src\multiplay.rs:295`), `MultiPlayManager::release_worker_guild` (`multiplay.rs:303`), and `MultiPlayManager::shutdown_all` (`multiplay.rs:320`).

---

## 2. FFmpeg / Opus Paths & Audio Decoding

### Python (`gdrive-bot-telegram\music_system.py`)
- **FFmpeg:** Relies on `ffmpeg` in system `PATH` (no configurable binary path):
  - `play_next_track` (`music_system.py:440-580`) runs an `asyncio.create_subprocess_exec("ffmpeg", "-v", "error", "-i", local_audio_path, "-t", "0.1", "-f", "s16le", "-ar", "48000", "-ac", "2", "pipe:1", ...)` pre-check (`music_system.py:519-523`), then passes `local_audio_path` (or `stream_url` fallback) to `discord.FFmpegPCMAudio` (`music_system.py:532, 535`).
  - `cmd_mdebug` (`music_system.py:1004-1041`) checks `shutil.which("ffmpeg")` (`music_system.py:1010`).
- **Opus:** No explicit `discord.opus.load_opus(...)`; relies on `discord.py` auto-loading system `libopus`, checked via `discord.opus.is_loaded()` (`music_system.py:541, 1011`).

### Rust (`zenne-noitu-rs`)
- **FFmpeg:** **Not used.** `Dockerfile:1-5` installs only `ca-certificates curl libopus0 libssl3` (no `ffmpeg`).
- **Audio Decoding & Opus:**
  - `Cargo.toml:52-53` & `crates\zenne-music\Cargo.toml:20-21` use `songbird` (`0.6`, `builtin-queue`) + `symphonia` (`0.5`, features `["aac", "isomp4", "mp3"]`) for pure-Rust in-process decoding.
  - `MusicManager::play_local_file` (`crates\zenne-music\src\lib.rs:432-454`) wraps the temp file in `songbird::input::File::new(path)` (`lib.rs:445`) and plays via `handler.play_input(source.into())` (`lib.rs:446`).
  - Opus encoding uses system `libopus0` (`Dockerfile:4`) via Songbird's `audiopus` dependency.

---

## 3. `languages.json` Loading

### Python
- `gdrive-bot-telegram\bot.py:220-226` (and `archive-2026-09-23T144146Z\bot.py:152-157`):
  - Computes runtime path `_LANG_CATALOG_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "languages.json")`.
  - Reads at startup via `with open(_LANG_CATALOG_PATH, "r", encoding="utf-8") as _f: LANGUAGE_CATALOG = json.load(_f)`, falling back to a 2-entry `vi`/`en` list on exception.

### Rust
- `crates\zenne-core\src\i18n.rs:12-22`:
  - **Compile-time embedded** via `static LANG_CATALOG_RAW: &str = include_str!("../../../languages.json");` (`i18n.rs:12`), parsed lazily with `serde_json::from_str` (`i18n.rs:14-15`). Zero runtime disk I/O.

---

## 4. SQLite File Paths & Database Handling

### Python (`gdrive-bot-telegram\bot.py`)
- `DB_PATH = "noitu_bot.db"` (`bot.py:71`, relative to CWD; same in `archive-2026-09-23T144146Z\bot.py:38`).
- `get_db()` (`bot.py:726-741`): Prefers remote `libsql.connect(...)`, falls back to `sqlite3.connect(DB_PATH, check_same_thread=False)` (`bot.py:735`) with `PRAGMA journal_mode=WAL` (`bot.py:738`).
- `_migrate_sqlite_to_libsql()` (`bot.py:744-756`): Checks `os.path.exists(DB_PATH)` (`bot.py:746`) and opens `sqlite3.connect(DB_PATH)` (`bot.py:755`) to migrate local SQLite tables to remote libSQL.

### Rust (`zenne-noitu-rs`)
- `AppConfig` defines `sqlite_path: env_str("SQLITE_PATH", "noitu_bot.db")` (`crates\zenne-core\src\config.rs:11, 107`) and `Cargo.toml:46` declares `rusqlite = { version = "0.32", features = ["bundled"] }`.
- **Forensic Finding:** `DbManager::new` (`crates\zenne-core\src\db.rs:70-85`) **never uses `cfg.sqlite_path` or `rusqlite`**. It exclusively uses `libsql::Builder::new_remote(...)` (`db.rs:74`) and bails with `anyhow::bail!("Can DATABASE_URL va DATABASE_AUTH_TOKEN cho remote libSQL")` (`db.rs:79`) if remote credentials are absent. No local SQLite fallback or migration exists in Rust.

---

## 5. Other Disk I/O (`.env` & Scripts)

- **Python:**
  - `telegram_bot.py:32-33`, `revolt_bot.py:36-37`, `twitch_bot.py:36-37`, `run_bots.py:34-35` call `dotenv.load_dotenv()`.
  - `lay_token_twitch.py:153-180` reads/writes `.env` directly via `open(p, "r"/"w", encoding="utf-8")`.
  - `run_bots.py:57-111` checks script existence with `os.path.exists(...)` and spawns subprocesses with `cwd=base_dir`.
- **Rust:**
  - `src\main.rs:8` calls `let _ = dotenvy::dotenv();`. No other `.env` or config file disk I/O exists.

---

## 6. Discord & Telegram File / Embed / Audio Attachments

| Surface | Python Legacy | Rust (`zenne-noitu-rs`) |
|---|---|---|
| **Discord file attachments** | None (`discord.File` unused) | None (`CreateAttachment` unused) |
| **Discord embeds & artwork** | `discord.Embed` across games & music; external URL thumbnails via `set_thumbnail(url=...)` (`music_system.py:572, 657, 671, 825, 970`; `cadao_game_system.py:245`, `emoji_game_system.py:235`, `mastermind_system.py:160`, `sudoku_system.py:159`, `tamsao_system.py:192`, `vuatiengviet_system.py:232`, `whoami_system.py:222`, `wiki_quiz_system.py:173`) | `serenity::CreateEmbed::new()` (`crates\zenne-discord\src\lib.rs:29, 188`; `crates\zenne-discord\src\music.rs:124, 249, 407, 427, 501, 521, 700`) with external URL `.thumbnail(art)` (`music.rs:423, 517`) |
| **Discord voice audio** | Local temp file `sc_track_{guild_id}.{ext}` streamed via `discord.FFmpegPCMAudio` (`music_system.py:532`) | Local temp file `zenne_sc_{guild_id}.{ext}` / `zenne_sc_w{worker_id}_{guild_id}.{ext}` played via `songbird::input::File::new(path)` (`crates\zenne-music\src\lib.rs:445-446`) |
| **Telegram media/files** | Text-only (`reply_text`); zero `send_photo` / `send_audio` / `send_document` / `send_voice` / `InputFile` in `telegram_bot.py` | Text-only (`bot.send_message(...)` in `crates\zenne-telegram\src\lib.rs`, `minigames.rs`, `noitu.rs`); zero media/file uploads or downloads |
| **Web API / Revolt / Twitch** | Text/JSON only | Text/JSON/WebSocket only (`zenne-api`, `zenne-revolt`, `zenne-twitch` have zero file upload/download/multipart handlers) |
