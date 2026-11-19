use crate::{GuildMusicState, MusicManager};
use dashmap::DashMap;
use reqwest::Client;
use songbird::Songbird;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Reserved,
    Connecting,
    Playing,
    Disconnecting,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationResult {
    Allocated { worker_id: usize },
    Reuse { worker_id: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MultiPlayError {
    #[error("MultiPlay đang tắt.")]
    Disabled,
    #[error("Hiện chưa có Zenne-Music worker nào sẵn sàng.")]
    NoReadyWorkers,
    #[error("Tất cả {0} bot nhạc trong server đều đang bận phát ở các kênh thoại khác! Vui lòng đợi hoặc dùng lại sau.")]
    AllWorkersBusy(usize),
    #[error("Không tìm thấy phiên phát nhạc nào trong kênh thoại này.")]
    SessionNotFound,
}

#[derive(Debug, Clone)]
pub struct ActiveVoiceSession {
    pub guild_id: u64,
    pub voice_channel_id: u64,
    pub text_channel_id: u64,
    pub worker_id: usize,
    pub state: SessionState,
}

pub struct WorkerSlot {
    pub worker_id: usize,
    pub name: String,
    pub ready: AtomicBool,
    pub bot_user_id: AtomicU64,
    pub songbird: Mutex<Option<Arc<Songbird>>>,
    pub http: Mutex<Option<Arc<serenity::http::Http>>>,
    /// Mỗi guild chỉ có tối đa 1 voice connection trên mỗi worker
    pub guild_states: DashMap<u64, Arc<Mutex<GuildMusicState>>>,
}

impl WorkerSlot {
    pub fn new(worker_id: usize) -> Self {
        Self {
            worker_id,
            name: format!("Zenne-Music {}", worker_id),
            ready: AtomicBool::new(false),
            bot_user_id: AtomicU64::new(0),
            songbird: Mutex::new(None),
            http: Mutex::new(None),
            guild_states: DashMap::new(),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub fn get_guild_state(&self, guild_id: u64) -> Arc<Mutex<GuildMusicState>> {
        self.guild_states
            .entry(guild_id)
            .or_insert_with(|| {
                Arc::new(Mutex::new(GuildMusicState {
                    volume: 1.0,
                    ..Default::default()
                }))
            })
            .clone()
    }
}

#[derive(Clone)]
pub struct MultiPlayManager {
    pub enabled: bool,
    pub max_workers: usize,
    pub auto_select: bool,
    pub idle_timeout_seconds: u64,
    pub audio: MusicManager,
    /// worker_id (1..=10) -> WorkerSlot
    pub workers: Arc<DashMap<usize, Arc<WorkerSlot>>>,
    /// (guild_id, voice_channel_id) -> ActiveVoiceSession
    pub channel_sessions: Arc<DashMap<(u64, u64), ActiveVoiceSession>>,
    /// (guild_id, worker_id) -> voice_channel_id (đảm bảo 1 worker chỉ giữ 1 VC trong mỗi guild)
    pub worker_guild_assignments: Arc<DashMap<(u64, usize), u64>>,
    /// Khóa nguyên tử theo guild_id để chống race condition khi nhiều người cùng !play
    guild_locks: Arc<DashMap<u64, Arc<Mutex<()>>>>,
}

impl MultiPlayManager {
    pub fn new(
        http: Client,
        soundcloud_client_id: String,
        enabled: bool,
        max_workers: usize,
        auto_select: bool,
        idle_timeout_seconds: u64,
    ) -> Self {
        let workers = DashMap::new();
        for id in 1..=max_workers {
            workers.insert(id, Arc::new(WorkerSlot::new(id)));
        }
        Self {
            enabled,
            max_workers,
            auto_select,
            idle_timeout_seconds,
            audio: MusicManager::new(http, soundcloud_client_id),
            workers: Arc::new(workers),
            channel_sessions: Arc::new(DashMap::new()),
            worker_guild_assignments: Arc::new(DashMap::new()),
            guild_locks: Arc::new(DashMap::new()),
        }
    }

    fn guild_lock(&self, guild_id: u64) -> Arc<Mutex<()>> {
        self.guild_locks
            .entry(guild_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Đăng ký worker sau khi Gateway Ready (hoặc trong test)
    pub async fn register_worker(
        &self,
        worker_id: usize,
        bot_user_id: u64,
        songbird: Option<Arc<Songbird>>,
        http: Option<Arc<serenity::http::Http>>,
    ) {
        let slot = self
            .workers
            .entry(worker_id)
            .or_insert_with(|| Arc::new(WorkerSlot::new(worker_id)))
            .clone();
        slot.bot_user_id.store(bot_user_id, Ordering::Release);
        if let Some(sb) = songbird {
            *slot.songbird.lock().await = Some(sb);
        }
        if let Some(h) = http {
            *slot.http.lock().await = Some(h);
        }
        slot.ready.store(true, Ordering::Release);
        info!(
            "[MultiPlay] {} (ID: {}) da san sang nhan lenh phat nhac!",
            slot.name, bot_user_id
        );
    }

    pub fn unregister_worker(&self, worker_id: usize) {
        if let Some(slot) = self.workers.get(&worker_id) {
            slot.ready.store(false, Ordering::Release);
        }
        let keys_to_remove: Vec<(u64, usize)> = self
            .worker_guild_assignments
            .iter()
            .filter(|e| e.key().1 == worker_id)
            .map(|e| *e.key())
            .collect();
        for (gid, wid) in keys_to_remove {
            if let Some((_, vc_id)) = self.worker_guild_assignments.remove(&(gid, wid)) {
                self.channel_sessions.remove(&(gid, vc_id));
            }
        }
        warn!(
            "[MultiPlay] Zenne-Music {} da ngat ket noi va giai phong session.",
            worker_id
        );
    }

    pub fn ready_worker_count(&self) -> usize {
        (1..=self.max_workers)
            .filter(|id| self.workers.get(id).map(|w| w.is_ready()).unwrap_or(false))
            .count()
    }

    pub fn get_worker(&self, worker_id: usize) -> Option<Arc<WorkerSlot>> {
        self.workers.get(&worker_id).map(|w| w.clone())
    }

    pub fn get_session_worker(&self, guild_id: u64, voice_channel_id: u64) -> Option<usize> {
        let sess = self.channel_sessions.get(&(guild_id, voice_channel_id))?;
        match sess.state {
            SessionState::Reserved | SessionState::Connecting | SessionState::Playing => {
                Some(sess.worker_id)
            }
            _ => None,
        }
    }

    /// Cấp phát nguyên tử 1 worker rảnh trong `guild_id`, hoặc tái sử dụng worker đang ở `voice_channel_id`
    pub async fn allocate_or_reuse(
        &self,
        guild_id: u64,
        voice_channel_id: u64,
        text_channel_id: u64,
    ) -> Result<AllocationResult, MultiPlayError> {
        if !self.enabled {
            return Err(MultiPlayError::Disabled);
        }

        let lock = self.guild_lock(guild_id);
        let _guard = lock.lock().await;

        // 1. Tái sử dụng session đang hoạt động ở đúng voice_channel_id
        if let Some(existing) = self.channel_sessions.get(&(guild_id, voice_channel_id)) {
            let wid = existing.worker_id;
            let active = matches!(
                existing.state,
                SessionState::Reserved | SessionState::Connecting | SessionState::Playing
            );
            let worker_ready = self
                .workers
                .get(&wid)
                .map(|w| w.is_ready())
                .unwrap_or(false);
            if active && worker_ready {
                return Ok(AllocationResult::Reuse { worker_id: wid });
            }
        }
        // Dọn session hỏng cũ nếu có
        self.channel_sessions.remove(&(guild_id, voice_channel_id));

        // 2. Tìm worker sẵn sàng đầu tiên chưa bận trong guild_id
        let mut ready_count = 0usize;
        for wid in 1..=self.max_workers {
            let Some(slot) = self.workers.get(&wid) else {
                continue;
            };
            if !slot.is_ready() {
                continue;
            }
            ready_count += 1;
            if !self.worker_guild_assignments.contains_key(&(guild_id, wid)) {
                self.worker_guild_assignments
                    .insert((guild_id, wid), voice_channel_id);
                self.channel_sessions.insert(
                    (guild_id, voice_channel_id),
                    ActiveVoiceSession {
                        guild_id,
                        voice_channel_id,
                        text_channel_id,
                        worker_id: wid,
                        state: SessionState::Reserved,
                    },
                );
                return Ok(AllocationResult::Allocated { worker_id: wid });
            }
        }

        if ready_count == 0 {
            Err(MultiPlayError::NoReadyWorkers)
        } else {
            Err(MultiPlayError::AllWorkersBusy(ready_count))
        }
    }

    pub fn set_session_state(&self, guild_id: u64, voice_channel_id: u64, new_state: SessionState) {
        if let Some(mut sess) = self.channel_sessions.get_mut(&(guild_id, voice_channel_id)) {
            sess.state = new_state;
        }
    }

    pub async fn release_session(&self, guild_id: u64, voice_channel_id: u64) {
        let lock = self.guild_lock(guild_id);
        let _guard = lock.lock().await;
        if let Some((_, sess)) = self.channel_sessions.remove(&(guild_id, voice_channel_id)) {
            self.worker_guild_assignments
                .remove(&(guild_id, sess.worker_id));
            if let Some(slot) = self.workers.get(&sess.worker_id) {
                if let Some(gs) = slot.guild_states.get(&guild_id) {
                    let mut st = gs.lock().await;
                    st.queue.clear();
                    st.current = None;
                    if let Some(h) = st.current_handle.take() {
                        let _ = h.stop();
                    }
                }
            }
            MusicManager::cleanup_worker_guild_audio(sess.worker_id, guild_id).await;
        }
    }

    pub async fn release_worker_guild(&self, worker_id: usize, guild_id: u64) {
        if let Some((_, vc_id)) = self.worker_guild_assignments.remove(&(guild_id, worker_id)) {
            self.release_session(guild_id, vc_id).await;
        } else {
            MusicManager::cleanup_worker_guild_audio(worker_id, guild_id).await;
        }
    }

    pub async fn shutdown_all(&self) {
        let sessions: Vec<(u64, u64, usize)> = self
            .channel_sessions
            .iter()
            .map(|e| (e.key().0, e.key().1, e.value().worker_id))
            .collect();
        for (gid, vc_id, wid) in sessions {
            if let Some(slot) = self.workers.get(&wid) {
                let sb_opt = slot.songbird.lock().await.clone();
                if let Some(sb) = sb_opt {
                    let _ = sb.remove(serenity::model::id::GuildId::new(gid)).await;
                }
            }
            self.release_session(gid, vc_id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SoundcloudTrack;

    async fn setup_manager(num_ready: usize) -> MultiPlayManager {
        let mgr = MultiPlayManager::new(
            Client::new(),
            "test_client_id".to_string(),
            true,
            10,
            true,
            300,
        );
        for id in 1..=num_ready {
            mgr.register_worker(id, 1000 + id as u64, None, None).await;
        }
        mgr
    }

    fn dummy_track(title: &str) -> SoundcloudTrack {
        SoundcloudTrack {
            title: title.to_string(),
            author: "Tester".to_string(),
            permalink_url: "https://soundcloud.com/test/track".to_string(),
            duration_ms: 120_000,
            artwork: None,
            stream_url: "https://example.com/stream".to_string(),
            requester_name: "User".to_string(),
        }
    }

    #[tokio::test]
    async fn test_1_worker_allocation_scenarios_a_and_b() {
        let mgr = setup_manager(10).await;
        let a = mgr.allocate_or_reuse(1, 101, 501).await.unwrap();
        assert_eq!(a, AllocationResult::Allocated { worker_id: 1 });

        let b = mgr.allocate_or_reuse(1, 102, 502).await.unwrap();
        assert_eq!(b, AllocationResult::Allocated { worker_id: 2 });
    }

    #[tokio::test]
    async fn test_2_existing_voice_session_reuse_scenario_c() {
        let mgr = setup_manager(10).await;
        let a = mgr.allocate_or_reuse(1, 101, 501).await.unwrap();
        assert_eq!(a, AllocationResult::Allocated { worker_id: 1 });
        mgr.set_session_state(1, 101, SessionState::Playing);

        let reuse = mgr.allocate_or_reuse(1, 101, 501).await.unwrap();
        assert_eq!(reuse, AllocationResult::Reuse { worker_id: 1 });
        assert_eq!(mgr.worker_guild_assignments.len(), 1);
    }

    #[tokio::test]
    async fn test_3_all_10_workers_occupied_scenario_d() {
        let mgr = setup_manager(10).await;
        for ch in 1..=10 {
            let res = mgr.allocate_or_reuse(99, ch, 500 + ch).await.unwrap();
            assert_eq!(
                res,
                AllocationResult::Allocated {
                    worker_id: ch as usize
                }
            );
        }
        let err = mgr.allocate_or_reuse(99, 11, 511).await.unwrap_err();
        assert_eq!(err, MultiPlayError::AllWorkersBusy(10));
    }

    #[tokio::test]
    async fn test_4_concurrent_requests_different_and_same_channel() {
        let mgr = setup_manager(10).await;
        let mut handles = Vec::new();
        for ch in 1..=10 {
            let m = mgr.clone();
            handles.push(tokio::spawn(async move {
                m.allocate_or_reuse(77, ch, 900).await.unwrap()
            }));
        }
        let mut worker_ids = Vec::new();
        for h in handles {
            if let AllocationResult::Allocated { worker_id } = h.await.unwrap() {
                worker_ids.push(worker_id);
            }
        }
        worker_ids.sort();
        assert_eq!(worker_ids, (1..=10).collect::<Vec<_>>());

        // Same channel concurrent requests -> 1 allocated, 9 reused, never 2 workers
        let mgr2 = setup_manager(10).await;
        let mut same_handles = Vec::new();
        for _ in 0..10 {
            let m = mgr2.clone();
            same_handles.push(tokio::spawn(async move {
                m.allocate_or_reuse(88, 200, 900).await.unwrap()
            }));
        }
        let mut allocated_cnt = 0;
        let mut reused_cnt = 0;
        for h in same_handles {
            match h.await.unwrap() {
                AllocationResult::Allocated { worker_id } => {
                    assert_eq!(worker_id, 1);
                    allocated_cnt += 1;
                }
                AllocationResult::Reuse { worker_id } => {
                    assert_eq!(worker_id, 1);
                    reused_cnt += 1;
                }
            }
        }
        assert_eq!(allocated_cnt, 1);
        assert_eq!(reused_cnt, 9);
    }

    #[tokio::test]
    async fn test_5_failed_connection_releases_reservation() {
        let mgr = setup_manager(2).await;
        assert_eq!(
            mgr.allocate_or_reuse(10, 1, 1).await.unwrap(),
            AllocationResult::Allocated { worker_id: 1 }
        );
        // Simulate connection failure -> release_session
        mgr.set_session_state(10, 1, SessionState::Failed("timeout".into()));
        mgr.release_session(10, 1).await;

        // Worker 1 is free again
        assert_eq!(
            mgr.allocate_or_reuse(10, 2, 2).await.unwrap(),
            AllocationResult::Allocated { worker_id: 1 }
        );
    }

    #[tokio::test]
    async fn test_6_worker_disconnection_cleanup() {
        let mgr = setup_manager(3).await;
        mgr.allocate_or_reuse(5, 50, 500).await.unwrap();
        mgr.unregister_worker(1);
        assert_eq!(mgr.ready_worker_count(), 2);
        assert!(mgr.get_session_worker(5, 50).is_none());

        // Next request picks Worker 2 seamlessly
        assert_eq!(
            mgr.allocate_or_reuse(5, 50, 500).await.unwrap(),
            AllocationResult::Allocated { worker_id: 2 }
        );
    }

    #[tokio::test]
    async fn test_7_independent_queues_per_worker_and_guild() {
        let mgr = setup_manager(2).await;
        let w1 = mgr.get_worker(1).unwrap();
        let w2 = mgr.get_worker(2).unwrap();

        w1.get_guild_state(1)
            .lock()
            .await
            .queue
            .push(dummy_track("Track A"));
        w2.get_guild_state(1)
            .lock()
            .await
            .queue
            .push(dummy_track("Track B"));
        w1.get_guild_state(2)
            .lock()
            .await
            .queue
            .push(dummy_track("Track C"));

        assert_eq!(w1.get_guild_state(1).lock().await.queue[0].title, "Track A");
        assert_eq!(w2.get_guild_state(1).lock().await.queue[0].title, "Track B");
        assert_eq!(w1.get_guild_state(2).lock().await.queue[0].title, "Track C");
    }

    #[tokio::test]
    async fn test_8_per_guild_availability_scenario_e() {
        let mgr = setup_manager(1).await;
        // Worker 1 serves Guild A
        assert_eq!(
            mgr.allocate_or_reuse(111, 1, 10).await.unwrap(),
            AllocationResult::Allocated { worker_id: 1 }
        );
        // Same Worker 1 may also serve Guild B concurrently!
        assert_eq!(
            mgr.allocate_or_reuse(222, 1, 20).await.unwrap(),
            AllocationResult::Allocated { worker_id: 1 }
        );
        // But Guild A cannot allocate Worker 1 for a 2nd channel
        assert_eq!(
            mgr.allocate_or_reuse(111, 2, 11).await.unwrap_err(),
            MultiPlayError::AllWorkersBusy(1)
        );
    }

    #[tokio::test]
    async fn test_9_disabled_multiplay_and_graceful_shutdown() {
        let mgr = MultiPlayManager::new(Client::new(), "cid".into(), false, 10, true, 300);
        assert_eq!(
            mgr.allocate_or_reuse(1, 1, 1).await.unwrap_err(),
            MultiPlayError::Disabled
        );

        let mgr2 = setup_manager(3).await;
        mgr2.allocate_or_reuse(1, 10, 100).await.unwrap();
        mgr2.allocate_or_reuse(2, 20, 200).await.unwrap();
        mgr2.shutdown_all().await;
        assert!(mgr2.channel_sessions.is_empty());
        assert!(mgr2.worker_guild_assignments.is_empty());
    }
}
