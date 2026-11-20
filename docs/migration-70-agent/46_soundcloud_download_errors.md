# Forensic Audit Report — ID 46

## Scope & Inspected Files
- **Python**: `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\music_system.py` (1041 lines)
- **Rust Core**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-music\src\lib.rs` (481 lines)
- **Rust Discord**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\music.rs` (754 lines)

---

## 1. `client_id` Initialization & Renewal
| Aspect | Python (`music_system.py`) | Rust (`zenne-music/src/lib.rs`) |
|---|---|---|
| **Initial TTL** | `_sc_client_id_expires = time.time() + (86400 * 30)` (**30 days**, lines 26–27) | `Instant::now() + 86_400s` (**24 hours**, `MusicManager::new` lines 58–66) |
| **Force Refresh** | **None.** `get_soundcloud_client_id()` (lines 145–167) has no `force_refresh` arg; 401/403 in `resolve_soundcloud_stream` (lines 184–209) never triggers renewal, leaving a revoked ID cached for 30 days. | `get_soundcloud_client_id(&self, force_refresh: bool)` (lines 69–118) supports `force_refresh = true` on `401 \| 403` for `/resolve` (lines 200–209) and `/search/tracks` (lines 246–259). |
| **Scraping & Status Checks** | Scrapes `https://soundcloud.com` + `script_urls[-6:]` with Chrome `User-Agent` (lines 151–164), **missing `status_code` checks** on both HTML and JS fetches. | Scrapes `https://soundcloud.com` + `js_urls.iter().rev().take(6)` (lines 76–115), **missing `status().is_success()` checks** on both HTML (line 76) and JS (line 93) fetches, and **no `User-Agent` override** in `lib.rs`. |
| **Failure / Concurrency** | Falls back to `_sc_client_id or SOUNDCLOUD_CLIENT_ID` (line 167). | Falls back to stale `sc_client_cache` (line 117) without singleflight locking—concurrent `401/403`s trigger duplicate scrapes. `self.client_id` (line 52) is never updated after `new()`. |

---

## 2. Short Links (`on.soundcloud.com`) & URL Handling
- **Python (`music_system.py`)**: `is_soundcloud_url()` accepts `*.soundcloud.com` (including `on.soundcloud.com`), but does not strip `<...>` and passes `query` unexpanded to `/resolve`.
- **Rust (`zenne-music/src/lib.rs`)**: `clean_query()` strips `<` and `>`; `resolve_soundcloud_track()` pre-expands `on.soundcloud.com` and `m.soundcloud.com` and strips query/fragment params. **Gap**: Stripping all query params also strips `secret_token` on private SoundCloud links.

---

## 3. Playlists
- **Python (`music_system.py`)**: All playlists fail because `media` is absent (`line 194`).
- **Rust (`zenne-music/src/lib.rs:214-232`)**: Detects `kind == "playlist"`, extracts **only `tracks[0].id`**, and fetches `/tracks/{first_id}` without 401/403 `client_id` retry. Rest of playlist is dropped.

---

## 4. Go+ / Preview / Geo-Blocked Tracks & HLS vs Progressive Selection
- Neither Python nor Rust inspects `policy` (`"SNIP"`, `"BLOCK"`), `snipped`, `monetization_model`, or `streamable`, and neither skips unplayable items in `search/tracks` (`limit=5`, blindly takes index `0`).
- Neither prioritizes `progressive` vs `hls`. Python line 205 corrupts `tc_url` if `?` is already present; Rust line 284 handles `?` vs `&`.

---

## 5. `tc_url` 401/403 Retry
- **Python & Rust**: **No 401/403 retry** on `tc_url` (`music_system.py:205-209`, `lib.rs:285-295`).

---

## 6. `res.status()` Checks & Download Failure Handling
- **Python (`music_system.py`)**: Checks `res.status_code != 200` (lines 239–242); falls back to direct `FFmpegPCMAudio(stream_url)` on `None`.
- **Rust (`zenne-music/src/lib.rs`)**:
  - **Critical Bug in `download_audio_with_prefix()` (lines 346–352)**: `res.status()` is **never checked**. HTTP `403`/`404`/`410`/`429` error bodies are written directly to `{file_prefix}.mp3` and returned as `Ok(out_path)`.
  - **Queue `stream_url` Expiration**: Unlike Python (`lines 476-483`, re-resolves if `resolved_at >= 60s`), Rust never re-resolves `stream_url` for queued tracks.

---

## 7. M3U8 Segment Retry & Error Propagation
- **Rust (`zenne-music/src/lib.rs:385-413`)**: Retries segments 3 times with **no backoff/sleep**, returning `(idx, Vec::new())` on failure. If `init.mp4` fails, the `.m4a` is unplayable yet `Ok(out_path)` is returned if `full_bytes.len() >= 1024`.
- **Rust Error Propagation in Playback Loop (`zenne-discord/src/music.rs`)**:
  - **Critical Stall Bug**: `TrackEndNotifier::act()` (`lines 21-35`) and `WorkerTrackEndNotifier::act()` (`lines 47-61`) discard errors with `let _ = play_next_*`. Because `st.current` is assigned **before** `download_audio` / `play_local_file`, an error leaves `st.current = Some(...)` and permanently stalls the guild queue until `/skip` or `/mstop`.
