# Forensic Audit Report — ID 43

## Scope & Inspected Files
- **Python (FFmpeg/VoiceClient variant)**: `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\music_system.py` (1041 lines; identical to `gdrive-bot-telegram\music_system.py`)
- **Python (Wavelink/Lavalink variant)**: `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\music_system.py` (694 lines)
- **Rust Discord Bot & Commands**:
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\lib.rs` (739 lines)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\music.rs` (754 lines)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\workers.rs` (95 lines)
- **Rust Music & MultiPlay Core**:
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-music\src\multiplay.rs` (536 lines)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-music\src\lib.rs` (481 lines)

---

## 1. Python `music_system.py` Audit (`on_voice_state_update`, `on_wavelink_*`)

### `on_voice_state_update`
- **Absent in both Python variants** (confirmed across all `.py` files under `gdrive-bot*`). Neither variant listens for bot kicks/disconnects, moderator channel moves, or empty-VC events.
- In `gdrive-bot-source\music_system.py`:
  - `GuildMusicState.__init__` (`L347-357`) defines `self.disconnect_task: Optional[asyncio.Task] = None` (`L357`), but it is **dead code**—never scheduled, awaited, or cancelled.
  - `play_next_track` (`L440-469`) checks `if not vc or not vc.is_connected()` (`L444`), resetting `is_playing`/`current` only when the next track is attempted. When the queue empties (`L460-469`), it sends a message and stays in VC indefinitely.
  - `handle_play` (`L605-628`) reconnects if `not vc or not vc.is_connected()`, cleans up stale `existing_vc` with `force=True` (`L614-618`), and connects with `reconnect=True` (`L619`). Channel move only occurs on `!play` when `not vc.is_playing()` via `await vc.move_to(voice_channel)` (`L624-628`).
  - Manual stops (`MusicControlView.stop_player` `L414-423`, `sl_stop_music` `L767-777`, `cmd_mstop` `L912-922`) clear `queue`, reset `current`/`is_playing`, and call `await vc.disconnect(force=True)`—but do not clear `state.voice_client` or delete temp files (`sc_track_{guild_id}.*`).

### `on_wavelink_*`
- **FFmpeg variant (`gdrive-bot-source\music_system.py`)**: No `on_wavelink_*` handlers exist; `init_lavalink` (`L680-701`) is a compatibility stub that attaches `_DaveFilter` to `discord.gateway` and `discord.voice_state` loggers (`L689-699`).
- **Wavelink variant (`gdrive-bot-source\gdrive-bot\music_system.py`)**:
  - `register_music` (`L232-275`) registers only:
    - `on_wavelink_node_ready` (`L236-237`): logs node readiness.
    - `on_wavelink_track_start` (`L240-263`): posts "Now Playing" embed + `MusicControlView` to `player.home`.
    - `on_wavelink_track_exception` (`L266-274`): sends error notice to `player.home`.
  - **Missing**: `on_wavelink_track_end`, `on_wavelink_track_stuck`, `on_wavelink_websocket_closed`. Auto-disconnect relies solely on `wavelink.Node(..., inactive_channel_tokens=1)` (`L223`).
  - Channel move in `handle_play` (`L304-308`): moves via `await player.move_to(voice_channel)` only if `not player.playing`.

---

## 2. Rust Comparison: Single-Bot vs. MultiPlay Workers

| Capability / Event | Rust Single-Bot (`lib.rs`, `music.rs`) | Rust MultiPlay Workers (`workers.rs`, `music.rs`, `multiplay.rs`) |
|---|---|---|
| **Voice Disconnect (Kick / Forced DC)** | **UNHANDLED.** `event_handler` (`lib.rs:473-547`) only matches `Ready`, `CacheReady`, `Message`, `_` (`L544`). `FullEvent::VoiceStateUpdate` is ignored despite `GUILD_VOICE_STATES` intent (`L598`). | **HANDLED.** `WorkerHandler::voice_state_update` (`workers.rs:28-45`) checks `new.user_id.get() == self_id` and `new.channel_id.is_none()` (`L35-38`), calling `multiplay.release_worker_guild(worker_id, gid.get())` (`L40-42`). |
| **Guild State Cleanup on Disconnect** | **BROKEN.** On forced DC, `MusicManager.states` (`zenne-music/src/lib.rs:54`) retains `queue`, `current`, and `current_handle`; `cleanup_guild_audio` is not called. Next `!play` (`music.rs:480`) sees `st.current.is_some()`, so `should_start == false` and playback stalls permanently until `!mstop` (`music.rs:646-658`). | **COMPLETE on VC DC.** `release_worker_guild` (`multiplay.rs:299-305`) -> `release_session` (`L279-297`) removes `channel_sessions` & `worker_guild_assignments`, clears `queue`, `current`, stops `current_handle`, and calls `cleanup_worker_guild_audio` (`zenne-music/src/lib.rs:460-462`).<br>**Partial gap on Gateway crash:** `unregister_worker` (`multiplay.rs:165-184`, called in `workers.rs:92`) clears `ready`, `worker_guild_assignments`, and `channel_sessions`, but does **not** clear `slot.guild_states` or temp audio files. |
| **Channel Move (User or Mod)** | **UNHANDLED.** `play` (`music.rs:452-462`) only joins if `manager.get(guild_id).is_none()`; if already in another VC, it neither moves nor rejects. Mod moves are ignored. | **BUG / UNHANDLED.** `WorkerHandler::voice_state_update` (`workers.rs:38`) only acts when `new.channel_id.is_none()`. If a mod moves a worker from VC A to VC B, `channel_sessions` and `worker_guild_assignments` still record VC A. `resolve_target_state` (`music.rs:548`) then fails to find the worker for users in VC B. |
| **Empty-VC Auto-Leave** | **UNHANDLED.** `play_next_in_guild` (`music.rs:93-100`) notifies queue end and stays in VC forever. No `VoiceStateUpdate` listener tracks non-bot members. | **NO Empty-VC Check (Idle-Queue Timeout Only).** `WorkerHandler::voice_state_update` (`workers.rs:35-37`) returns early for non-self users. Instead, `play_next_for_worker` (`music.rs:199-217`) spawns an `idle_timeout_seconds` (300s) sleep after queue end; if `current.is_none() && queue.is_empty()` (`L208`), it calls `sb.remove` and `release_session` (`L211-214`). |
| **Songbird Reconnect / Error Events** | **ONLY `TrackEvent::End`.** `play_next_in_guild` (`music.rs:107-116`) registers `TrackEndNotifier` (`L12-35`). **No** `CoreEvent::DriverDisconnect`, `CoreEvent::DriverReconnect`, `CoreEvent::ClientDisconnect`, or `TrackEvent::Error` handlers exist. | **ONLY `TrackEvent::End`.** `play_next_for_worker` (`music.rs:230-240`) registers `WorkerTrackEndNotifier` (`L37-62`). **No** `CoreEvent` or `TrackEvent::Error` handlers exist on worker `Call`s or `TrackHandle`s. |
| **Shutdown Cleanup** | **NONE.** `run_discord_bot` (`lib.rs:607-611`) does not clean up single-bot `music` states or Songbird calls on exit. | **HANDLED.** `lib.rs:610` calls `mp_for_shutdown.shutdown_all().await` (`multiplay.rs:307-322`), which removes all Songbird calls and releases all sessions. |

---

## 3. Key Findings & Risk Summary
1. **Single-bot state lockup after kick**: Because `lib.rs:event_handler` ignores `FullEvent::VoiceStateUpdate` and `Songbird` `CoreEvent::DriverDisconnect` is unregistered, kicking the single-bot leaves `st.current = Some(...)`, breaking subsequent `!play` invocations (`music.rs:480`).
2. **MultiPlay desync on moderator channel move**: `workers.rs:38` only handles `new.channel_id.is_none()`, leaving `(guild_id, old_vc_id)` stale in `channel_sessions` when a worker is moved to a new VC.
3. **No Songbird driver/track error observability**: Neither single-bot nor MultiPlay registers `CoreEvent::DriverDisconnect`, `CoreEvent::DriverReconnect`, or `TrackEvent::Error`, making voice gateway/UDP drops silent at the application layer.
4. **No empty-VC detection anywhere**: Python's `disconnect_task` (`music_system.py:357`) is unused; Rust single-bot has no auto-leave; Rust MultiPlay only has a post-queue idle timer (`music.rs:199-217`), meaning a worker playing a long track/loop in an empty VC will never auto-leave.
