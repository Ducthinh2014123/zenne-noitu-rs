# Forensic Audit Report — ID 35

## 1. Startup Order & Process Architecture

### Python (`C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\`)
* **Multi-process Supervisor (`run_bots.py`)**:
  * Module load (`run_bots.py:26-37`): Reconfigures `sys.stdout`/`sys.stderr` to UTF-8 on `win32` (`26-31`) and calls `dotenv.load_dotenv()` (`33-37`).
  * Signal registration at import time (`run_bots.py:41-47`): Binds `_handle_signal` to `SIGINT` and `SIGTERM` (if available).
  * Entrypoint (`run_bots.py:149-163`): Reads `sys.argv[1]` (default `"all"`). Single-bot modes (`run_telegram`, `run_discord`, `run_twitch`, `run_revolt` at `129-147`) import the respective module in-process and call `.main()` (or `bot.bot.run(bot.DISCORD_TOKEN)` for Discord at `137`).
  * `run_all(auto_restart=True)` (`run_bots.py:49-96`): Spawns up to 4 OS child processes via `subprocess.Popen([python_bin, script], cwd=base_dir)`:
    1. `bot.py` — spawned unconditionally if file exists (`61-63`).
    2. `telegram_bot.py` — spawned unconditionally if file exists (`66-68`).
    3. `twitch_bot.py` — gated on non-empty `TWITCH_OAUTH_TOKEN` (`71-77`).
    4. `revolt_bot.py` — gated on non-empty `REVOLT_BOT_TOKEN` (`80-86`).
* **Discord Bot (`bot.py`)**:
  * Module-level side effects (`bot.py:2260-2295`): Instantiates `commands.Bot(command_prefix="!", intents=intents, help_command=None, proxy=DISCORD_PROXY)` (`2265`) with `message_content`, `guilds`, `members`, `presences` intents (`2260-2264`), then synchronously registers 25 mini-game modules + `register_music(bot)` (`2268-2293`).
  * Entrypoint (`bot.py:6353-6354`): Calls `bot.run(DISCORD_TOKEN)`.
* **Telegram Bot (`telegram_bot.py`)**:
  * Module load (`23-35`): UTF-8 reconfigure on `win32`, `load_dotenv()`, imports DB & game helpers from `bot.py` (`55-91` — triggering `bot.py` module-level bot/game registration as a side effect).
  * `main()` (`1921-1930`): Calls `init_db()` synchronously (`1923`) -> `create_telegram_app()` (`1926`, validates `TELEGRAM_BOT_TOKEN` at `1871-1879`, builds `Application` with `.post_init(post_init_setup)` at `1881`, registers 24+ handlers at `1884-1917`) -> `app.run_polling(allowed_updates=Update.ALL_TYPES)` (`1929`).
* **Twitch Bot (`twitch_bot.py`)**:
  * Module load (`27-62`): UTF-8 reconfigure, `load_dotenv()`, imports helpers from `bot.py` (`42-62`).
  * `main()` (`1011-1041`): Validates `TWITCH_OAUTH_TOKEN` (`1016-1026`, blocks on `input()` if missing), instantiates `TwitchBotClient()`, runs `asyncio.run(client.start())` (`1037`).
* **Revolt Bot (`revolt_bot.py`)**:
  * Module load (`27-62`): UTF-8 reconfigure, `load_dotenv()`, imports helpers from `bot.py` (`42-62`).
  * `main()` (`1006-1032`): Validates `REVOLT_BOT_TOKEN` (`1011-1018`, blocks on `input()` if missing), instantiates `RevoltBotClient(REVOLT_BOT_TOKEN)`, runs `asyncio.run(client.start())` (`1028`).

### Rust (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\`)
* **Single-process Multi-task Runtime (`src/main.rs`)**:
  * `#[tokio::main] async fn main()` (`src/main.rs:6-86`):
    1. `let _ = dotenvy::dotenv();` (`8`).
    2. `tracing_subscriber::fmt()` init (`10-14`).
    3. Parses `std::env::args().nth(1)` (`16-19`, default `"all"`).
    4. `let ctx = AppContext::init().await?;` (`25`).
    5. `let games = GameHub::default();` (`26`).
    6. Mode dispatch (`28-83`):
       * `"discord" | "dc"`: Spawns `zenne_api::start_web_api` (`31-33`) and awaits `zenne_discord::run_discord_bot(ctx, games)` (`34`).
       * `"telegram" | "twitch" | "revolt"`: Awaits respective runner directly (`36-44`).
       * `"all"` (default): Spawns 5 concurrent Tokio tasks (`h_api`, `h_dc`, `h_tg`, `h_tw`, `h_rv` at `46-73`) sharing cloned `Arc`-backed `AppContext` and `GameHub`.

---

## 2. Database & Cache Initialization

### Python
* **`bot.py` (`init_db()` at `835-1048`)**:
  * **Deferred initialization**: `init_db()` is **not** called before `bot.run()`. It is only invoked inside `@bot.event async def on_ready()` (`bot.py:2569-2571`) via `await asyncio.get_event_loop().run_in_executor(None, init_db)`.
  * **Sequence**: `_init_redis()` (`386-431`, line `837`) -> `_init_libsql()` (`709-729`, lines `838-839`) -> executes `_SCHEMA_SQL` (`841-979`), `_INDICES` (`981-994`), `_MIGRATIONS` (`996-1009`), and SQLite `PRAGMA table_info(web_users)` check (`1032-1041`). `_migrate_sqlite_to_libsql()` (`750`) is commented out at `1045`.
  * **Reconnect hazard**: Because `on_ready()` fires on every Discord gateway re-identify, `init_db()` re-runs on reconnects.
* **`telegram_bot.py`**: Calls `init_db()` synchronously in `main()` (`1923`) before starting polling.
* **`twitch_bot.py` & `revolt_bot.py` (Critical Gap)**: Neither file calls `init_db()`. Because `_db_is_libsql` defaults to `False` (`bot.py:498`) and `_init_libsql()` is only called inside `init_db()`, standalone `twitch_bot.py` and `revolt_bot.py` processes never initialize libSQL or Redis — `get_db()` (`bot.py:732-747`) silently falls back to local SQLite (`noitu_bot.db`), assuming tables already exist.

### Rust
* **`src/main.rs:25` -> `crates/zenne-core/src/lib.rs:44-60`**:
  * `AppContext::init().await?` runs **once** at startup before any bot or API task is spawned.
  * Initializes `AppConfig::from_env()` (`45`), `DbManager::new(&config).await?` (`46`), `CacheManager::new(&config.redis_url).await` (`47`), and shared `reqwest::Client` (`48-51`). All 4 bots and `zenne_api` share this single initialized pool/cache.

---

## 3. Background Tasks & Runtime Loops

### Python
* **`run_bots.py:100-112`**: Blocking supervisor loop (`while not _STOPPING`, `p.poll()`, `time.sleep(2)`). Auto-restarts crashed child processes up to 10 times (`107-111`) with a `3s` delay.
* **`bot.py` `on_ready()` (`2569-2593`)**:
  1. `init_db` in executor (`2571`).
  2. `await bot.tree.sync()` (`2574`).
  3. `await bot.change_presence(...)` (`2578-2580`).
  4. `asyncio.ensure_future(start_web_api())` (`2581`) -> `aiohttp.web.AppRunner` + `TCPSite("0.0.0.0", API_PORT)` (`6343-6347`). *(Hazard: `on_ready` reconnects will re-invoke `start_web_api()` and fail to bind `API_PORT`.)*
  5. `asyncio.create_task(asyncio.to_thread(get_wiki_session))` (`2583`).
  6. `await init_lavalink(bot)` (`2587`).
  7. `start_afk_task(bot, db_game_submit_score)` (`2591`) -> `@tasks.loop(seconds=45)` `_check_loop` (`afk_system.py:49-101`).
  8. Per-game turn timers via `asyncio.create_task(run_timeout(...))` (`2449`, `2671`, etc.).
* **`telegram_bot.py`**: `post_init_setup` (`1817-1849`) sets 24 `BotCommand`s; per-game `asyncio.create_task(_run_turn_timeout(...))` (`529`, `604`); `app.run_polling()` (`1929`).
* **`twitch_bot.py`**: `TwitchBotClient.start()` (`244-294`) spawns `_sender_task` (`_sender_loop` at `206-221`, 0.8s rate-limit sleep), runs `while self.running:` IRC SSL (`irc.chat.twitch.tv:6697`) reconnect loop (`5s` backoff at `294`), plus per-game/minigame timer tasks (`444`, `583`, `617`, `650`, `678`, `712`).
* **`revolt_bot.py`**: `RevoltBotClient.start()` (`208-261`) creates `aiohttp.ClientSession()`, validates token via `GET /users/@me` (`213-227`), runs `while self.running:` WebSocket reconnect loop (`5s` backoff at `257`), spawns `_ping_task` (`242`, cancelled in `finally:` at `258-260`), plus per-game/minigame timer tasks (`429`, `562`, `600`, `636`, `667`, `705`).

### Rust
* **`src/main.rs:46-75`**: Spawns 5 Tokio tasks (`h_api`, `h_dc`, `h_tg`, `h_tw`, `h_rv`). Note `let _ = (h_dc, h_tg, h_tw, h_rv);` (`75`) drops those `JoinHandle`s without awaiting or supervising them (no auto-restart if a bot task exits).
* **`zenne-discord/src/lib.rs:549-614`**: `run_discord_bot` creates `MusicManager`, `MultiPlayManager`, calls `workers::spawn_music_workers` (`565`), builds `poise::Framework` (`570-594`; `setup` calls `poise::builtins::register_globally` at `584`), registers `songbird` (`602`), and awaits `c.start().await` (`607`). `event_handler` (`473-547`) updates `app.live` on `Ready`/`CacheReady` (`480-510`) and handles AFK/Nối Từ on `Message` (`511-543`).
* **`zenne-discord/src/workers.rs:49-95`**: `spawn_music_workers` spawns one `tokio::spawn` task per `(worker_id, token)` (`61-94`) when `multiplay.enabled` and `tokens` non-empty. Each worker runs an isolated `serenity::Client` with `GUILDS | GUILD_VOICE_STATES` intents and `register_songbird()` (`64-74`), registers on `ready` (`15-26`), releases guild on `voice_state_update` (`28-45`), and calls `mp.unregister_worker(worker_id)` on exit (`92`).

---

## 4. Signal Handling & Graceful Shutdown

### Python
* **`run_bots.py`**: Handles `SIGINT` & `SIGTERM` (`41-47`) by setting `_STOPPING = True`. `finally:` (`113-127`) calls `p.terminate()`, waits `5s` (`119-120`), then `p.kill()` (`124`).
* **`bot.py`**: No custom signal handlers; relies on `discord.py`'s `bot.run()` (`6354`). `aiohttp.web.AppRunner` (`6343`) is never explicitly cleaned up (`runner.cleanup()` missing), and active game timeout tasks are not cancelled.
* **`telegram_bot.py`**: No custom signal handlers; relies on `app.run_polling()` (`1929`). Active `_NOITU_GAMES` timeout tasks are not cancelled on exit.
* **`twitch_bot.py` & `revolt_bot.py` (Event-loop Lifecycle Bug & Missing `SIGTERM`)**: Neither handles `SIGTERM`. Both wrap `asyncio.run(client.start())` in `try ... except KeyboardInterrupt:` and then call `asyncio.run(client.stop())` (`twitch_bot.py:1036-1040`, `revolt_bot.py:1027-1031`). Calling `client.stop()` in a **second** `asyncio.run()` loop attempts to cancel tasks (`_sender_task` / `_ping_task`) and close streams/sessions (`self.writer` / `self.ws` / `self.session`) bound to the first, already-closed event loop.

### Rust
* **`src/main.rs:76-81`**: In `"all"` mode, `tokio::select!` awaits `tokio::signal::ctrl_c()` or `h_api`. On `ctrl_c()`, `main()` returns `Ok(())` (`85`), dropping the Tokio runtime and **aborting** all bot tasks (`h_dc`, `h_tg`, `h_tw`, `h_rv`) and `workers.rs` tasks without graceful shard shutdown.
* **`zenne-discord/src/lib.rs:610`**: `mp_for_shutdown.shutdown_all().await` is only reached if `c.start().await` returns on its own (e.g., `"discord"` single-bot mode); in `"all"` mode, `src/main.rs:77-79` exits `main()` before line `610` can execute.

---

## 5. Healthcheck & Keepalive

| Component | Healthcheck Endpoint | Keepalive / Heartbeat |
|---|---|---|
| `bot.py` | `GET /health` -> `_ah_health` (`4199-4200`, `6298`) on `0.0.0.0:26035` | Discord gateway WS heartbeat (`discord.py`); Redis `health_check_interval=30` (`416`); Lavalink (`wavelink`) |
| `telegram_bot.py` | None | `app.run_polling()` HTTP long-polling (`1929`) |
| `twitch_bot.py` | None | Reactive IRC `PING` -> `PONG` (`265-270`) |
| `revolt_bot.py` | None | Proactive `_ping_loop()` WS `Ping` every `25s` (`196-206`) |
| `run_bots.py` | `p.poll()` every `2s` (`104`, `112`) | Auto-restart up to 10x (`107-111`) |
| Rust `zenne-api` (`crates/zenne-api/src/lib.rs`) | `GET /health` & `GET /api/health` (`60-61`) | `src/main.rs:80` exits process if `h_api` terminates |
| Rust `zenne-discord` & `workers.rs` | None (in-memory `app.live` updated at `lib.rs:485-509`) | Serenity internal shard WS heartbeat |
