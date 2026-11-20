# Forensic Audit Report — ID 57

Audit of Tokio task cancellation, shutdown handling, `DashMap` unbounded growth, and temp file leaks in `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs`.

---

## 1. `tokio::select!` Task Abort on `ctrl_c` & Unawaited `JoinHandle`s
1. **`src/main.rs:45-85`:** Drops `h_dc`, `h_tg`, `h_tw`, `h_rv` (`L75`). On `ctrl_c()`, returns `Ok(())` without signalling shutdown, so `mp_for_shutdown.shutdown_all().await` (`zenne-discord/src/lib.rs:610`) is never reached in `"all"` mode. `zenne_api::start_web_api` (`zenne-api/src/lib.rs:159`) lacks `.with_graceful_shutdown(...)`.
2. **`zenne-discord/src/workers.rs:49-95`:** Worker Serenity tasks are spawned via detached `tokio::spawn` with no `JoinHandle` or `ShardManager` shutdown handle.
3. **`zenne-discord/src/music.rs:202-217`:** Idle-timeout `tokio::spawn` is detached and unawaited.
4. **`zenne-api/src/ws_game.rs:171-177, 558`:** `writer_task.abort()` is not awaited; if `writer_task` exits early, read loop stays open.

---

## 2. Orphaned `timeout_handle`s (`zenne-telegram/src/noitu.rs` & `state.rs`)
- `execute_create_public_room` (`noitu.rs:832`) overwrites `st.noitu_rooms` without calling `cancel_timeout()` on any existing room.
- `nt_cancel` (`noitu.rs:1290-1313`) removes `noitu_rooms` without calling `cancel_timeout()` or cleaning `st.room_tokens`.

---

## 3. `DashMap` Unbounded Growth
1. **`used_nonces` (`zenne-api/src/lib.rs:27, 505-513`):** Never evicted or expired.
2. **`ttt_sessions` & `ms_sessions` (`zenne-api/src/pub_games.rs:39-40, 140, 266`):** Never removed on game completion and have no TTL/LRU eviction.
3. **`ws_rooms` (`ws_game.rs:191, 520, 535`):** Inserted before `"join"` succeeds; cleanup is gated on `if joined`, leaking empty rooms on failed handshakes.
4. **`guild_states` (`multiplay.rs:55`, `zenne-music/src/lib.rs:54`) & `guild_locks` (`multiplay.rs:102`):** Never removed on `release_session` or `shutdown_all`.
5. **`room_tokens` (`zenne-telegram/src/state.rs:160`):** Never removed on stop/cancel/timeout.

---

## 4. Temp File Leaks (`zenne-music/src/lib.rs`, `multiplay.rs`, `zenne-discord/src/music.rs`)
1. `MultiPlayManager::unregister_worker` (`multiplay.rs:165-184`) does not call `cleanup_worker_guild_audio`.
2. `play_next_in_guild` (`music.rs:102-105`) and `play_next_for_worker` (`music.rs:221-228`) leak the downloaded temp file if `play_local_file` fails via `?`.
3. No temp-dir cleanup on `Ctrl+C` or single-bot shutdown.
