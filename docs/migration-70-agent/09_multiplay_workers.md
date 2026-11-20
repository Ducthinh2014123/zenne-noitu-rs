# Audit Report `09`

- **ID:** `09`
- **Scope:** `crates/zenne-music/src/multiplay.rs`, `crates/zenne-discord/src/workers.rs`, plus integration paths in `crates/zenne-discord/src/music.rs` and `crates/zenne-discord/src/lib.rs`.
- **Status:** **ISSUES FOUND** (Critical / High concurrency, timer cancellation, and shutdown gaps)

## Findings

1. **[High] Uncancelled Voice Idle Timer & TOCTOU Race on New Track Start**
   - `crates/zenne-discord/src/music.rs:199-218`
   - When the queue empties, `play_next_for_worker` spawns a detached `tokio::spawn` sleeping for `idle_timeout_seconds` (`music.rs:202-203`) with **no `AbortHandle`, `CancellationToken`, or generation counter**.
   - **ABA bug:** If the worker is stopped/reused and finishes a new track right around when the old timer expires, the stale timer wakes up, sees `st.current.is_none() && st.queue.is_empty()`, and immediately disconnects the worker with 0s idle time.
   - **TOCTOU bug:** The `gs.lock().await` guard is dropped at `music.rs:209` before `sb.remove` (`212`) and `release_session` (`214`). A concurrent `!play` acquiring `gs` in that window has its newly started track/session killed.

2. **[High] Worker Allocation / Reuse Races Across `Reserved` & `Connecting` States**
   - `crates/zenne-music/src/multiplay.rs:207-271` & `crates/zenne-discord/src/music.rs:313-405`
   - `allocate_or_reuse` holds `guild_lock` only during map lookup/insert (`multiplay.rs:217-270`). Once `SessionState::Reserved` / `Connecting` is set, a second `!play` in the same VC receives `AllocationResult::Reuse` (`music.rs:326`) while the first caller is still awaiting `sb.join` (`344`) and `resolve_soundcloud_track` (`361`).
   - Caller 2 can call `play_next_for_worker` (`386`) before `sb.join` finishes (failing `play_local_file` without releasing the session because `is_new_join == false`), or Caller 1's failure cleanup (`345`, `370`, `401`) can tear down Caller 2's active session.

3. **[High] `worker_guild_assignments` Leak on Stale/Unready Session Cleanup**
   - `crates/zenne-music/src/multiplay.rs:221-237`
   - In `allocate_or_reuse`, if an existing `channel_sessions` entry is no longer active or its worker is `!is_ready()`, line `237` removes `(guild_id, voice_channel_id)` from `channel_sessions`, **but does not remove `(guild_id, wid)` from `worker_guild_assignments`**, permanently leaking that worker slot for that guild.

4. **[High] `release_worker_guild` Mutates `worker_guild_assignments` Outside `guild_lock`**
   - `crates/zenne-music/src/multiplay.rs:299-305` (called by `crates/zenne-discord/src/workers.rs:40-42`)
   - `release_worker_guild` removes `(guild_id, worker_id)` at `multiplay.rs:300` *before* calling `release_session` (which acquires `guild_lock` at `280-281`). A concurrent `allocate_or_reuse` can reassign `worker_id` to a new `vc_id_2` in that window, and `release_session` will then clear `(guild_id, worker_id)` again, corrupting the new assignment.

5. **[Medium] Voice Channel Move & Empty-VC Disconnect Not Handled**
   - `crates/zenne-discord/src/workers.rs:28-45`
   - `WorkerHandler::voice_state_update` only cleans up when `new.channel_id.is_none()` (`workers.rs:38`). If a moderator drags the worker to another VC (`new.channel_id == Some(other_vc)`), `channel_sessions` and `worker_guild_assignments` remain bound to the old VC, breaking `!mstop`/`!queue`/`!skip` routing.
   - Non-bot `voice_state_update` events are ignored (`35`), so workers remain in empty VCs until the queue/loop finishes plus `idle_timeout_seconds`.

6. **[Medium] Incomplete Graceful Shutdown of Worker Clients & `unregister_worker` Cleanup**
   - `crates/zenne-discord/src/workers.rs:49-95`, `crates/zenne-music/src/multiplay.rs:165-184, 307-322`, `crates/zenne-discord/src/lib.rs:607-611`
   - `spawn_music_workers` spawns detached `tokio::spawn` tasks (`workers.rs:63`) without storing `JoinHandle`s or each worker's `ShardManager`.
   - `shutdown_all` (`multiplay.rs:307-322`) disconnects Songbird and clears `channel_sessions`, but does not shut down worker Gateway shards, does not mark workers `ready = false`, and in `lib.rs:610` only runs if `c.start().await` returns (no `ctrl_c`/signal hook).
   - `unregister_worker` (`multiplay.rs:165-184`) does not clear `slot.guild_states` or call `cleanup_worker_guild_audio`.

## Recommended Tests
1. **Idle timer cancellation (ABA & TOCTOU):** Start idle timer (`idle_timeout_seconds = 1`), enqueue/start a new track before expiry, wait past the first deadline, and assert the worker remains connected and `SessionState::Playing`.
2. **Concurrent `play` during `Reserved`/`Connecting`:** Simulate two concurrent `!play` calls on the same `(guild_id, vc_id)` where the first `sb.join` is delayed or fails; verify the second neither plays before join completes nor loses its session silently.
3. **Stale session cleanup:** Set a session to `SessionState::Failed` (or mark worker unready) without calling `release_session`, call `allocate_or_reuse`, and assert `worker_guild_assignments` does not leak `wid`.
4. **`release_worker_guild` vs `allocate_or_reuse` race:** Concurrently invoke `release_worker_guild(wid, gid)` and `allocate_or_reuse(gid, vc2, tc)` under `tokio::join!` and verify `worker_guild_assignments` and `channel_sessions` remain consistent.
5. **Worker VC move & shutdown:** Test `voice_state_update` with `Some(new_vc_id)` and verify `shutdown_all` marks workers unready and terminates worker shard managers cleanly.
