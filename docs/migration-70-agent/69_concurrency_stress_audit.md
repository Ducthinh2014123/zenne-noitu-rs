# Forensic Audit Report — ID 69

**Scope:** Bounded concurrency, lock contention, unbounded `DashMap` growth, N+1 DB/HTTP queries, and stress-test design across `zenne-music`, `zenne-core`, `zenne-api`, and `zenne-telegram`.

---

## 1. Lock Contention & Concurrency Bottlenecks
1. **`MultiPlayManager` (`multiplay.rs:279-322`):** `release_session` awaits `cleanup_worker_guild_audio` while holding `guild_lock`; `release_worker_guild` removes `worker_guild_assignments` outside `guild_lock`; `shutdown_all` runs sequentially.
2. **`CacheManager` (`cache.rs:14, 50-60, 87-94, 133-147`):** Global `Mutex<ConnectionManager>` serializes all Redis calls (`ConnectionManager` is `Clone` and multiplexed); `get` takes a write lock (`mem.remove`) on every miss; `set` does `mem.clear()` above 50k; `rate_limited` has a read-drop-insert TOCTOU race.
3. **`ws_game.rs` (`179-194, 533-536, 339-408`):** Split-brain `rooms.get` -> `.await` -> `rooms.insert` race; `rooms.remove` race; 3 separate lock acquisitions per `"word"`; unbounded `mpsc::UnboundedSender`.
4. **`noitu_rooms` (`noitu.rs:110-137, 538-579, 1246-1266, 1454-1728`):** Holds `room_arc.lock().await` across Telegram HTTP `.await`s; 8+ lock acquisitions per word message.
5. **`pub_games.rs` (`140-196, 266-334`):** Holds `DashMap` `RefMut` shard guard across minimax/flood-fill/RNG.

## 2. Unbounded `DashMap` Growth
`WorkerSlot::guild_states`, `MultiPlayManager::guild_locks`, `CacheManager::mem` (via `rate_limited`), `ApiState::{used_nonces, ttt_sessions, ms_sessions, ws_rooms (pre-join)}`, `TelegramState::{room_tokens, mp_wizard, active_*}`.

## 3. N+1 DB Queries & Sequential Fan-Out
`pub_leaderboard` (`1 + N` `resolve_display_user`), `pub_game_leaderboard` (`1 + N`), `enrich_discord_users` (sequential Discord REST calls), `ah_review_bulk` (`3 * N`), `lb_add_game` (`N` inserts), `stats_summary` (8 sequential counts), `get_public_rooms` (write-on-read), `cache_exists` (2 round-trips).

## 4. Concrete Bounded-Concurrency Stress Test Designs
1. **MultiPlay Allocation/Release Stress (`Semaphore::new(64)`):** 2,000 tasks across 200 guilds × 12 VCs verifying `channel_sessions.len() == worker_guild_assignments.len()` and single-VC-per-worker invariant.
2. **`CacheManager` Burst (`Semaphore::new(128)`):** 60,000 `rate_limited` keys + 5,000 hot-key bursts asserting `mem.len() <= 50_000` and exact `<= 1000` allowed hits.
3. **`ws_rooms` Split-Brain & Leak (`Semaphore::new(50)`):** 50 concurrent joins to one room + 500 pre-join disconnects verifying `ws_rooms.len() == 0`.
4. **`noitu_rooms` Lock-Across-Await (`Semaphore::new(32)`):** Concurrent `nt_getlink` + `cmd_diem` + `handle_text_message` + `cmd_stop` against a delayed mock Telegram API + `room_tokens.len() == 0`.
5. **Leaderboard & Bulk Review Benchmark (`Semaphore::new(16)`):** 100 concurrent `GET /pub/leaderboard?limit=100` & `POST /api/review/bulk`.
