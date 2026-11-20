# Forensic Audit Report — ID 47

**Scope Audited:**
- `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-music\src\multiplay.rs`
- `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\workers.rs`
- `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\music.rs`
- (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-music\src\lib.rs`)

---

## 1. `allocate_or_reuse` Under `Reserved` / `Connecting` States

### Findings
1. **Premature `Reuse` while `sb.join` is still in flight (Error + Stuck `current`):**
   - In `MultiPlayManager::allocate_or_reuse` (`zenne-music/src/multiplay.rs:221-235`), `SessionState::Reserved` and `SessionState::Connecting` are treated as `active` (`multiplay.rs:223-226`), returning `AllocationResult::Reuse { worker_id }` immediately after `guild_lock` is dropped (`multiplay.rs:217-218, 233`).
   - Concurrent Task B receives `Reuse` (`is_new_join = false`) while Task A is still awaiting `sb.join` (`music.rs:344`). Task B can call `play_next_for_worker` (`music.rs:386`), pop its track into `st.current` (`music.rs:183`), and fail `play_local_file` because `songbird.get(guild_id)` is `None`. Because `is_new_join == false`, Task B leaves `st.current = Some(track)` with `current_handle = None`, blocking future auto-starts.
2. **Cascading session destruction on `is_new_join` failure:**
   - If Task A fails (`music.rs:339-343, 344-352, 365-374, 397-405`), Task A unconditionally calls `sb.remove` and `mp.release_session`, destroying Task B's valid queued/playing session.
3. **Concurrent `should_start` double-playback race:**
   - Two concurrent `play` calls on the same session can both observe `st.current.is_none()` (`music.rs:379-383`) and both call `play_next_for_worker` (`music.rs:386`).
4. **`set_session_state` & cancellation gaps:**
   - `set_session_state` (`multiplay.rs:273-277`) mutates `channel_sessions` without `guild_lock`. Task cancellation between `Reserved` and `Playing` leaks the worker slot.

---

## 2. Idle Timer ABA / TOCTOU Races

In `play_next_for_worker` (`zenne-discord/src/music.rs:199-217`):
1. **TOCTOU between `is_idle` check and `sb.remove` / `release_session`:** `gs.lock().await` is dropped at `music.rs:209` before `sb.remove` (`212`) and `release_session` (`214`).
2. **TOCTOU during `resolve_soundcloud_track` (`music.rs:359-383`):** During HTTP resolution before `st.queue.push_back`, `st.current.is_none() && st.queue.is_empty()` remains `true`.
3. **Classic ABA race across sessions & voice channels:** Timer carries no session generation token and does not verify `channel_sessions` / `worker_guild_assignments` still match `(worker_id, voice_channel_id)`.
4. **Unbounded timer accumulation & `guild_states` re-creation.**

---

## 3. `release_worker_guild` Locking Order

In `MultiPlayManager::release_worker_guild` (`zenne-music/src/multiplay.rs:299-305`):
1. **Locking / Mutation Order Inversion:** Mutates `worker_guild_assignments.remove(&(guild_id, worker_id))` **outside `guild_lock`**, then calls `release_session(guild_id, vc_id)` which acquires `guild_lock`.
2. **Double-allocation & Cross-VC Session Hijack:** Between `multiplay.rs:300` and `281`, `allocate_or_reuse` can reassign `worker_id` to `vc_2`, and `release_session(guild_id, vc_1)` then deletes `vc_2`'s assignment, wipes `slot.guild_states[guild_id]`, and deletes `zenne_sc_w{worker_id}_{guild_id}`.

---

## 4. `worker_guild_assignments` Leaks (and Related State Leaks)

1. **Confirmed leak in `allocate_or_reuse` cleanup path (`multiplay.rs:221-237`):** Removes `channel_sessions` (`237`) without removing `(guild_id, wid)` from `worker_guild_assignments`.
2. **Lock-free race between `unregister_worker` and `allocate_or_reuse` (`multiplay.rs:165-184`).**
3. **Unbounded growth of `WorkerSlot::guild_states` and `MultiPlayManager::guild_locks`.**

---

## 5. `shutdown_all`

In `MultiPlayManager::shutdown_all` (`zenne-music/src/multiplay.rs:307-322`):
1. **No allocation gate / shutdown flag.**
2. **Orphaned `worker_guild_assignments` not cleaned.**
3. **Sequential `await` per session.**

---

## 6. Queue Isolation

1. **Cross-VC queue & audio destruction via `release_worker_guild` (`multiplay.rs:299-305`).**
2. **Stale `WorkerTrackEndNotifier` firing after worker reassignment (`music.rs:37-62`).**
3. **Mode-switch isolation gap (`ready_worker_count() > 0` check in `music.rs:312, 546, 621`).**
4. **`loop_mode` and `volume` bleed across sessions (`multiplay.rs:286-293`).**
