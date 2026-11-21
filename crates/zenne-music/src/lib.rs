pub mod multiplay;

use dashmap::DashMap;
use futures_util::stream::{self, StreamExt};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use songbird::input::File as SongbirdFile;
use songbird::tracks::TrackHandle;
use songbird::Songbird;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;
use tracing::info;
use url::Url;

pub const SOUNDCLOUD_COLOR: u32 = 0xFF5500;
pub const UNSUPPORTED_LINK_MSG: &str =
    "❌ Link của bạn không hỗ trợ, chúng tôi chỉ hỗ trợ SoundCloud nhé!";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundcloudTrack {
    pub title: String,
    pub author: String,
    pub permalink_url: String,
    pub duration_ms: u64,
    pub artwork: Option<String>,
    pub stream_url: String,
    pub requester_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    Off = 0,
    Track = 1,
    Queue = 2,
}

#[derive(Default)]
pub struct GuildMusicState {
    pub queue: Vec<SoundcloudTrack>,
    pub current: Option<SoundcloudTrack>,
    pub loop_mode: u8,
    pub volume: f32,
    pub current_handle: Option<TrackHandle>,
}

#[derive(Clone)]
pub struct MusicManager {
    pub http: Client,
    pub client_id: String,
    pub sc_client_cache: Arc<tokio::sync::RwLock<(String, std::time::Instant)>>,
    pub states: Arc<DashMap<u64, Arc<Mutex<GuildMusicState>>>>,
}

impl MusicManager {
    pub fn new(http: Client, client_id: String) -> Self {
        let expires = std::time::Instant::now() + std::time::Duration::from_secs(86_400);
        Self {
            http,
            sc_client_cache: Arc::new(tokio::sync::RwLock::new((client_id.clone(), expires))),
            client_id,
            states: Arc::new(DashMap::new()),
        }
    }

    /// Tự động lấy hoặc gia hạn SoundCloud `client_id` từ JS bundle (cache 24h, giống `music_system.py`).
    pub async fn get_soundcloud_client_id(&self, force_refresh: bool) -> String {
        if !force_refresh {
            let guard = self.sc_client_cache.read().await;
            if std::time::Instant::now() < guard.1 && !guard.0.is_empty() {
                return guard.0.clone();
            }
        }
        if let Ok(html) = self.http.get("https://soundcloud.com").send().await {
            if let Ok(body) = html.text().await {
                static RE_JS: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
                static RE_CID: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
                let re_js = RE_JS.get_or_init(|| {
                    Regex::new(r#"https://a-v2\.sndcdn\.com/assets/[a-zA-Z0-9\-_.]+\.js"#).unwrap()
                });
                let re_cid = RE_CID
                    .get_or_init(|| Regex::new(r#"client_id[:=]"([a-zA-Z0-9]{32})""#).unwrap());
                let js_urls: Vec<String> = re_js
                    .find_iter(&body)
                    .map(|m| m.as_str().to_string())
                    .collect();
                for js_url in js_urls.iter().rev().take(6) {
                    if let Ok(resp) = self.http.get(js_url).send().await {
                        if let Ok(js_txt) = resp.text().await {
                            if let Some(caps) = re_cid.captures(&js_txt) {
                                if let Some(cid) = caps.get(1) {
                                    let new_cid = cid.as_str().to_string();
                                    let mut guard = self.sc_client_cache.write().await;
                                    *guard = (
                                        new_cid.clone(),
                                        std::time::Instant::now()
                                            + std::time::Duration::from_secs(86_400),
                                    );
                                    tracing::info!(
                                        "[SoundCloud] Đã tự động làm mới client_id thành công!"
                                    );
                                    return new_cid;
                                }
                            }
                        }
                    }
                }
            }
        }
        self.sc_client_cache.read().await.0.clone()
    }

    pub fn get_state(&self, guild_id: u64) -> Arc<Mutex<GuildMusicState>> {
        self.states
            .entry(guild_id)
            .or_insert_with(|| {
                Arc::new(Mutex::new(GuildMusicState {
                    volume: 1.0,
                    ..Default::default()
                }))
            })
            .clone()
    }

    pub fn clean_query(s: &str) -> String {
        s.trim()
            .trim_matches(|c| c == '<' || c == '>')
            .trim()
            .to_string()
    }

    pub fn is_url(s: &str) -> bool {
        let low = Self::clean_query(s).to_lowercase();
        low.starts_with("http://") || low.starts_with("https://")
    }

    pub fn is_soundcloud_url(raw: &str) -> bool {
        let cleaned = Self::clean_query(raw);
        let Ok(parsed) = Url::parse(&cleaned) else {
            return false;
        };
        let Some(host) = parsed.host_str() else {
            return false;
        };
        let h = host.to_lowercase();
        h == "soundcloud.com" || h.ends_with(".soundcloud.com") || h == "on.soundcloud.com"
    }

    pub fn format_ms(ms: u64) -> String {
        let total_secs = ms / 1000;
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, mins, secs)
        } else {
            format!("{:02}:{:02}", mins, secs)
        }
    }

    async fn resolve_track_stream(
        &self,
        track_data: &serde_json::Value,
        cid: &mut String,
        fallback_url: &str,
        requester_name: &str,
    ) -> Result<SoundcloudTrack, String> {
        let transcodings = track_data["media"]["transcodings"]
            .as_array()
            .ok_or_else(|| "Bài hát không có luồng âm thanh khả dụng.".to_string())?;

        let mut stream_url = None;
        for tc in transcodings {
            let Some(tc_url) = tc["url"].as_str() else {
                continue;
            };
            let sep = if tc_url.contains('?') { '&' } else { '?' };
            let req_url = format!("{}{}client_id={}", tc_url, sep, cid);
            if let Ok(mut r) = self.http.get(&req_url).send().await {
                if r.status().as_u16() == 401 || r.status().as_u16() == 403 {
                    let fresh_cid = self.get_soundcloud_client_id(true).await;
                    if !fresh_cid.is_empty() {
                        *cid = fresh_cid;
                        let retry_url = format!("{}{}client_id={}", tc_url, sep, cid);
                        if let Ok(r2) = self.http.get(&retry_url).send().await {
                            r = r2;
                        }
                    }
                }
                if r.status().is_success() {
                    if let Ok(j) = r.json::<serde_json::Value>().await {
                        if let Some(u) = j["url"].as_str() {
                            stream_url = Some(u.to_string());
                            break;
                        }
                    }
                }
            }
        }

        let stream_url =
            stream_url.ok_or_else(|| "Không thể giải mã stream_url từ SoundCloud.".to_string())?;

        Ok(SoundcloudTrack {
            title: track_data["title"]
                .as_str()
                .unwrap_or("SoundCloud Track")
                .to_string(),
            author: track_data["user"]["username"]
                .as_str()
                .unwrap_or("SoundCloud Artist")
                .to_string(),
            permalink_url: track_data["permalink_url"]
                .as_str()
                .unwrap_or(fallback_url)
                .to_string(),
            duration_ms: track_data["duration"].as_u64().unwrap_or(0),
            artwork: track_data["artwork_url"].as_str().map(|s| s.to_string()),
            stream_url,
            requester_name: requester_name.to_string(),
        })
    }

    pub async fn resolve_soundcloud_tracks(
        &self,
        query: &str,
        is_url_flag: bool,
        requester_name: &str,
    ) -> Result<(Option<String>, Vec<SoundcloudTrack>), String> {
        let cleaned = Self::clean_query(query);
        let mut cid = self.get_soundcloud_client_id(false).await;
        if is_url_flag {
            // Mở rộng link rút gọn (on.soundcloud.com) & bỏ query tracking (?si=..., ?ref=...)
            let mut target_url = cleaned.clone();
            if let Ok(parsed) = Url::parse(&cleaned) {
                let host = parsed.host_str().unwrap_or("").to_lowercase();
                if host == "on.soundcloud.com" || host == "m.soundcloud.com" {
                    if let Ok(r) = self.http.get(&cleaned).send().await {
                        target_url = r.url().to_string();
                    }
                }
            }
            if let Ok(mut u) = Url::parse(&target_url) {
                u.set_query(None);
                u.set_fragment(None);
                target_url = u.to_string();
            }

            let mut res = self
                .http
                .get("https://api-v2.soundcloud.com/resolve")
                .query(&[("url", target_url.as_str()), ("client_id", cid.as_str())])
                .send()
                .await
                .map_err(|e| format!("Lỗi kết nối SoundCloud: {}", e))?;
            if matches!(res.status().as_u16(), 401 | 403) {
                cid = self.get_soundcloud_client_id(true).await;
                res = self
                    .http
                    .get("https://api-v2.soundcloud.com/resolve")
                    .query(&[("url", target_url.as_str()), ("client_id", cid.as_str())])
                    .send()
                    .await
                    .map_err(|e| format!("Lỗi kết nối SoundCloud: {}", e))?;
            }
            if !res.status().is_success() {
                return Err(format!("SoundCloud API trả về HTTP {}", res.status()));
            }
            let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            if matches!(json["kind"].as_str(), Some("playlist" | "system-playlist"))
                || json["tracks"].is_array()
            {
                let playlist_title = json["title"].as_str().map(|s| s.to_string());
                let raw_tracks = json["tracks"]
                    .as_array()
                    .ok_or_else(|| "Playlist SoundCloud trống.".to_string())?;

                let mut full_items: Vec<serde_json::Value> = Vec::new();
                let mut stub_ids: Vec<u64> = Vec::new();
                for item in raw_tracks.iter().take(50) {
                    if item["media"]["transcodings"].as_array().is_some() {
                        full_items.push(item.clone());
                    } else if let Some(id) = item["id"].as_u64() {
                        stub_ids.push(id);
                    }
                }

                if !stub_ids.is_empty() {
                    for chunk in stub_ids.chunks(50) {
                        let ids_param = chunk
                            .iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>()
                            .join(",");
                        let mut tr_res = self
                            .http
                            .get("https://api-v2.soundcloud.com/tracks")
                            .query(&[("ids", ids_param.as_str()), ("client_id", cid.as_str())])
                            .send()
                            .await;
                        if let Ok(ref r) = tr_res {
                            if matches!(r.status().as_u16(), 401 | 403) {
                                cid = self.get_soundcloud_client_id(true).await;
                                tr_res = self
                                    .http
                                    .get("https://api-v2.soundcloud.com/tracks")
                                    .query(&[
                                        ("ids", ids_param.as_str()),
                                        ("client_id", cid.as_str()),
                                    ])
                                    .send()
                                    .await;
                            }
                        }
                        if let Ok(r) = tr_res {
                            if r.status().is_success() {
                                if let Ok(arr) = r.json::<Vec<serde_json::Value>>().await {
                                    full_items.extend(arr);
                                }
                            }
                        }
                    }
                }

                let mut resolved = Vec::new();
                for item in &full_items {
                    if let Ok(t) = self
                        .resolve_track_stream(item, &mut cid, &target_url, requester_name)
                        .await
                    {
                        resolved.push(t);
                    }
                }
                if resolved.is_empty() {
                    return Err("Không thể tải bài hát nào trong playlist SoundCloud.".to_string());
                }
                return Ok((playlist_title, resolved));
            }

            let t = self
                .resolve_track_stream(&json, &mut cid, &target_url, requester_name)
                .await?;
            Ok((None, vec![t]))
        } else {
            let mut res = self
                .http
                .get("https://api-v2.soundcloud.com/search/tracks")
                .query(&[
                    ("q", cleaned.as_str()),
                    ("client_id", cid.as_str()),
                    ("limit", "5"),
                ])
                .send()
                .await
                .map_err(|e| format!("Lỗi tìm kiếm SoundCloud: {}", e))?;
            if matches!(res.status().as_u16(), 401 | 403) {
                cid = self.get_soundcloud_client_id(true).await;
                res = self
                    .http
                    .get("https://api-v2.soundcloud.com/search/tracks")
                    .query(&[
                        ("q", cleaned.as_str()),
                        ("client_id", cid.as_str()),
                        ("limit", "5"),
                    ])
                    .send()
                    .await
                    .map_err(|e| format!("Lỗi tìm kiếm SoundCloud: {}", e))?;
            }
            if !res.status().is_success() {
                return Err(format!("SoundCloud Search API lỗi HTTP {}", res.status()));
            }
            let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            let track_data = body["collection"]
                .as_array()
                .and_then(|arr| arr.first().cloned())
                .ok_or_else(|| {
                    format!(
                        "Không tìm thấy bài hát nào trên SoundCloud cho: `{}`",
                        query
                    )
                })?;
            let t = self
                .resolve_track_stream(&track_data, &mut cid, query, requester_name)
                .await?;
            Ok((None, vec![t]))
        }
    }

    pub async fn resolve_soundcloud_track(
        &self,
        query: &str,
        is_url_flag: bool,
        requester_name: &str,
    ) -> Result<SoundcloudTrack, String> {
        let (_pl, tracks) = self
            .resolve_soundcloud_tracks(query, is_url_flag, requester_name)
            .await?;
        tracks
            .into_iter()
            .next()
            .ok_or_else(|| "Không tìm thấy bài hát SoundCloud.".to_string())
    }

    /// Tải và ghép init.mp4 + các phân đoạn .m4s (hoặc mp3) thành file cục bộ
    /// để phát qua Songbird cực nhẹ, không treo FFmpeg HLS.
    pub async fn download_audio(&self, stream_url: &str, guild_id: u64) -> Result<PathBuf, String> {
        self.download_audio_with_prefix(stream_url, &format!("zenne_sc_{}", guild_id))
            .await
    }

    pub async fn download_audio_for_worker(
        &self,
        stream_url: &str,
        worker_id: usize,
        guild_id: u64,
    ) -> Result<PathBuf, String> {
        self.download_audio_with_prefix(
            stream_url,
            &format!("zenne_sc_w{}_{}", worker_id, guild_id),
        )
        .await
    }

    async fn download_audio_with_prefix(
        &self,
        stream_url: &str,
        file_prefix: &str,
    ) -> Result<PathBuf, String> {
        let res = self
            .http
            .get(stream_url)
            .send()
            .await
            .map_err(|e| format!("Lỗi tải playlist SoundCloud: {}", e))?;
        if !res.status().is_success() {
            return Err(format!("SoundCloud trả về HTTP {}", res.status()));
        }
        let bytes = res.bytes().await.map_err(|e| e.to_string())?;
        let tmp_dir = std::env::temp_dir();

        if bytes.starts_with(b"#EXTM3U") {
            let text = String::from_utf8_lossy(&bytes);
            let base = Url::parse(stream_url).map_err(|e| e.to_string())?;
            let mut urls = Vec::new();

            static MAP_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
            let map_re = MAP_RE.get_or_init(|| Regex::new(r#"#EXT-X-MAP:URI="([^"]+)""#).unwrap());
            let is_fmp4 = map_re.is_match(&text) || text.contains(".m4s");
            if let Some(cap) = map_re.captures(&text) {
                if let Ok(u) = base.join(&cap[1]) {
                    urls.push(u.to_string());
                }
            }

            for line in text.lines() {
                let l = line.trim();
                if !l.is_empty() && !l.starts_with('#') {
                    if let Ok(u) = base.join(l) {
                        urls.push(u.to_string());
                    }
                }
            }

            if urls.is_empty() {
                return Err("Playlist M3U8 rỗng.".to_string());
            }

            let ext = if is_fmp4 { "m4a" } else { "mp3" };
            let out_path = tmp_dir.join(format!("{}.{}", file_prefix, ext));

            let http = self.http.clone();
            let mut ordered: Vec<(usize, Vec<u8>)> = stream::iter(urls.into_iter().enumerate())
                .map(|(idx, u)| {
                    let c = http.clone();
                    async move {
                        for _ in 0..3 {
                            if let Ok(r) = c.get(&u).send().await {
                                if r.status().is_success() {
                                    if let Ok(b) = r.bytes().await {
                                        return (idx, b.to_vec());
                                    }
                                }
                            }
                        }
                        (idx, Vec::new())
                    }
                })
                .buffer_unordered(12)
                .collect()
                .await;

            ordered.sort_by_key(|(idx, _)| *idx);
            let mut full_bytes = Vec::new();
            for (_, chunk) in ordered {
                full_bytes.extend_from_slice(&chunk);
            }

            if full_bytes.len() < 1024 {
                return Err("Dữ liệu âm thanh tải về quá nhỏ.".to_string());
            }
            fs::write(&out_path, &full_bytes)
                .await
                .map_err(|e| format!("Lỗi ghi file tạm: {}", e))?;
            info!(
                "[Music] Đã tải SoundCloud -> {:?} ({} KB)",
                out_path,
                full_bytes.len() / 1024
            );
            Ok(out_path)
        } else {
            let out_path = tmp_dir.join(format!("{}.mp3", file_prefix));
            fs::write(&out_path, &bytes)
                .await
                .map_err(|e| format!("Lỗi ghi file mp3: {}", e))?;
            Ok(out_path)
        }
    }

    pub async fn play_local_file(
        &self,
        songbird: &Arc<Songbird>,
        guild_id: u64,
        path: PathBuf,
        volume: f32,
        loop_single: bool,
    ) -> Result<TrackHandle, String> {
        let call = songbird
            .get(serenity::model::id::GuildId::new(guild_id))
            .ok_or_else(|| "Bot chưa kết nối vào kênh thoại!".to_string())?;

        let mut handler = call.lock().await;
        let source = SongbirdFile::new(path);
        let handle = handler.play_input(source.into());
        let _ = handle.set_volume(volume);
        if loop_single {
            let _ = handle.enable_loop();
        } else {
            let _ = handle.disable_loop();
        }
        Ok(handle)
    }

    pub async fn cleanup_guild_audio(guild_id: u64) {
        Self::cleanup_audio_prefix(&format!("zenne_sc_{}", guild_id)).await;
    }

    pub async fn cleanup_worker_guild_audio(worker_id: usize, guild_id: u64) {
        Self::cleanup_audio_prefix(&format!("zenne_sc_w{}_{}", worker_id, guild_id)).await;
    }

    async fn cleanup_audio_prefix(file_prefix: &str) {
        let tmp_dir = std::env::temp_dir();
        for ext in ["m4a", "mp3"] {
            let p = tmp_dir.join(format!("{}.{}", file_prefix, ext));
            if tokio::fs::try_exists(&p).await.unwrap_or(false) {
                let _ = tokio::fs::remove_file(&p).await;
            }
        }
    }

    pub fn toggle_loop_handle(handle: &TrackHandle, loop_single: bool) {
        if loop_single {
            let _ = handle.enable_loop();
        } else {
            let _ = handle.disable_loop();
        }
    }
}
