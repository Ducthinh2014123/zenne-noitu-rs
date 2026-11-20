# Forensic Audit Report — ID `08`

**Scope:**  
Audit of `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\music_system.py` (SoundCloud resolution, `client_id` scraping, playlist handling, transcoding fallback, temp file cleanup, queue & control buttons/commands) against `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-music\src\lib.rs` (+ `multiplay.rs` `release_session`) and `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\music.rs`.

**Status:** `PARTIAL / GAPS FOUND`  
Core SoundCloud resolution, `client_id` scraping, M3U8 fMP4/MP3 download, and temp file cleanup are implemented in Rust (with several improvements over Python, such as 401/403 retry, short-URL expansion, and temp file deletion), but **missing interactive control buttons, `/nowplaying`, `/stop` & `/loop` command parity, playlist multi-track queueing, queued `stream_url` re-resolution, and HTTP status checks on download**.

---

### Findings

1. **`get_soundcloud_client_id` & 401/403 Refresh**
   - `lib.rs:58-66`: `MusicManager::new` caches the initial `client_id` with a 24h TTL (`Instant::now() + 86_400`). If the configured/default `client_id` is invalid at startup, `get_soundcloud_client_id(false)` (`lib.rs:69-75`) returns the stale ID until a 401/403 triggers `get_soundcloud_client_id(true)`.
   - `lib.rs:76-116` vs `music_system.py:145-167`: Both scrape `https://soundcloud.com` for `https://a-v2.sndcdn.com/assets/*.js` (last 6 scripts) and match `client_id[:=]"([a-zA-Z0-9]{32})"`. Note: `lib.rs:76-82` omits a browser `User-Agent` header (present in `music_system.py:151-153`).
   - `lib.rs:200-209, 246-259`: Rust retries `/resolve` and `/search/tracks` once on HTTP 401/403 with `force_refresh = true` (improvement over `music_system.py:184-192`). **Gap:** `lib.rs:285-295` does **not** retry transcoding `tc_url` requests on 401/403.

2. **Playlist Handling (`resolve_soundcloud_track`)**
   - `lib.rs:214-232`: Unlike `music_system.py:183-195` (which fails on playlists because `media` is absent), Rust detects `kind == "playlist"`, extracts `tracks[0].id`, and fetches `/tracks/{first_id}`.
   - **Gaps:** Only the **first track** is resolved; remaining playlist tracks are silently dropped instead of queued. If `tracks[0]` is geo-blocked/Go+ or `/tracks/{first_id}` fails, `json` remains the playlist object and fails at `lib.rs:275-277` instead of falling back to subsequent tracks.

3. **Transcoding Fallback & Audio Download**
   - `lib.rs:280-296` vs `music_system.py:201-209`: Both iterate `media.transcodings` in order and take the first HTTP 200 JSON `url`, without prioritizing `progressive` vs `hls` or filtering unsupported/encrypted protocols.
   - `lib.rs:346-352`: `download_audio_with_prefix` does **not** check `res.status().is_success()` (unlike `music_system.py:240`). An HTTP 403/404/5xx non-M3U8 response is written directly as `.mp3` (`lib.rs:423-428`).
   - `lib.rs:389-399`: Failed M3U8 segment downloads after 3 retries silently return `Vec::new()`, causing missing audio frames/corrupt fMP4 instead of failing or retrying.
   - `music.rs:102, 221` vs `music_system.py:533-535`: No fallback to direct URL streaming (or auto-skipping to the next track) if `download_audio` fails; `TrackEndNotifier::act` (`music.rs:24-32`) discards the `Result` (`let _ = ...`), silently halting the queue on error.

4. **Temp File Cleanup**
   - `lib.rs:456-472`: `cleanup_guild_audio` / `cleanup_worker_guild_audio` remove `zenne_sc_{guild_id}.{m4a,mp3}` and `zenne_sc_w{worker_id}_{guild_id}.{m4a,mp3}` (improvement over `music_system.py`, which never deletes `sc_track_{guild_id}.*`).
   - Invoked in `TrackEndNotifier::act` (`music.rs:23`), `WorkerTrackEndNotifier::act` (`music.rs:49`), single-bot `mstop` (`music.rs:658`), and MultiPlay `release_session` (`multiplay.rs:295`).
   - **Risk (`lib.rs:324, 336`):** Fixed per-guild/per-worker filenames create a race if `skip` / `TrackEndNotifier` overlaps with `download_audio` for the next track (old `TrackEndNotifier` can delete the newly downloaded file, or Windows can lock the open file).

5. **Control Buttons, Commands & Queue Parity**
   - **Missing Interactive Buttons:** `music_system.py:372-434` (`MusicControlView`: `⏯️`, `⏭️`, `⏹️`, `🔁` with same-VC `interaction_check`) is attached to `/play`, `!play`, `/nowplaying`, `!np`, and auto-next embeds (`music_system.py:576, 732, 827, 877, 972`). `zenne-discord/src/music.rs` has **no** UI buttons or component interaction handler.
   - **Missing Commands:** `nowplaying` / `np` (`music_system.py:806-828, 951-973`) and `mdebug` (`music_system.py:1004-1041`) are absent in `music.rs`.
   - **Command Name Mismatch:** `music.rs:617-618` uses `mstop` (missing `/stop` from `music_system.py:767`); `music.rs:733-734` uses `rloop` without a `loop` alias (`music_system.py:844, 990`).
   - **Stale Queued `stream_url`:** `music_system.py:475-483` re-resolves `permalink_url` if `resolved_at >= 60s`. `music.rs:102, 221` uses the cached `track.stream_url` indefinitely, risking CDN token expiration on queued tracks.
   - **Single-Bot VC Check:** `music.rs:556` (`resolve_target_state`) does not verify the caller is in the bot's voice channel in single-bot mode (unlike MultiPlay mode at `music.rs:547-554`).
   - **Loop+Skip & Queue Embed:** `music.rs:75-76, 169-170` replays the same track on `skip` when `loop_mode == 1` (same as `music_system.py:452-454`). `music.rs:666-706` (`queue`) omits elapsed time, requester, loop mode, volume, and overflow count (`> 10` tracks) present in `music_system.py:786-803`.

---

### Recommended Tests
1. **`client_id` Scraping & 401/403 Recovery:** Unit/mock-HTTP test initializing `MusicManager::new` with an invalid `client_id`, verifying 401/403 on `/resolve`, `/search/tracks`, and transcoding `tc_url` triggers `get_soundcloud_client_id(true)` and succeeds on retry.
2. **Playlist Resolution:** Test `/resolve` returning `kind: "playlist"` (both single-track and multi-track, plus a geo-blocked `tracks[0]` fallback).
3. **Download Error & Segment Failure:** Test `download_audio_with_prefix` receiving HTTP 403/404 (must return `Err`, not write HTML to `.mp3`) and M3U8 with a failing segment.
4. **Temp File Cleanup & Skip Race:** Test rapid `skip` and `mstop` (both single-bot and MultiPlay) ensuring no leftover `zenne_sc_*` files and no premature deletion of the next track's file.
5. **Queue Expiration & Controls:** Test queued track playback after `stream_url` expiry (>60s), `rloop`/`loop` + `skip` behavior, and single-bot VC membership enforcement on `pause`/`skip`/`mstop`/`volume`/`rloop`.
