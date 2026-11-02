//! Voice-note transcription core: jobs, a bounded on-disk transcript store,
//! and language resolution. No I/O to any network here - the engine lives
//! behind the `Engine` trait so the module is testable without one.
//!
//! Design rules (from the client's scope notes): local speech services only,
//! never a cloud API; the transcript store is bounded and private; a global
//! default language with an optional per-message override.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// One voice note waiting for (or finishing) transcription.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    /// Stable message id the transcript belongs to.
    pub message_id: String,
    /// Chat the message lives in, for store partitioning.
    pub chat_id: String,
    /// Local audio file to transcribe.
    pub audio: PathBuf,
    /// Duration from the attachment metadata, when known.
    pub duration_secs: Option<f64>,
    /// Per-message language override; `None` uses the global default.
    pub language: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum JobState {
    #[default]
    Pending,
    Running,
    Done,
    Failed,
}

/// The speech engine. Implemented per local backend (whisper.cpp CLI,
/// Wyoming service, parakeet-mlx); never a cloud endpoint.
pub trait Engine {
    /// Transcribes one audio file. `language` is a BCP-47-ish code the
    /// engine understands ("fi", "en", ...). Errors are definitive
    /// failures; a missing binary or unreadable file is not retried.
    fn transcribe(&mut self, audio: &Path, language: &str) -> Result<String, String>;
}

/// FIFO of jobs. Bounded so a history sync cannot queue unbounded work;
/// the oldest pending jobs are dropped first, never running ones.
pub struct JobQueue {
    pending: VecDeque<Job>,
    running: Option<Job>,
    done: usize,
    cap: usize,
}

impl JobQueue {
    pub fn new(cap: usize) -> Self {
        Self {
            pending: VecDeque::new(),
            running: None,
            done: 0,
            cap: cap.max(1),
        }
    }

    /// Queues a job. Returns false when the same message is already queued
    /// or running; transcription of one voice note never duplicates.
    pub fn push(&mut self, job: Job) -> bool {
        let dup = self
            .pending
            .iter()
            .chain(self.running.iter())
            .any(|j| j.message_id == job.message_id);
        if dup {
            return false;
        }
        if self.pending.len() >= self.cap {
            self.pending.pop_front();
        }
        self.pending.push_back(job);
        true
    }

    /// Claims the next pending job as running.
    pub fn claim(&mut self) -> Option<Job> {
        let job = self.pending.pop_front()?;
        self.running = Some(job.clone());
        Some(job)
    }

    /// Completes the running job, successfully or not.
    pub fn finish(&mut self, ok: bool) {
        if self.running.take().is_some() && ok {
            self.done += 1;
        }
    }

    /// A crash with a job running requeues it once, at the front: audio is
    /// idempotent to re-read, unlike a sent message.
    pub fn recover(&mut self) {
        if let Some(job) = self.running.take() {
            self.pending.push_front(job);
        }
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

/// Per-message language resolution: an explicit per-message choice wins,
/// then the global default, then the engine's own auto-detection.
pub fn resolve_language(per_message: Option<&str>, global: Option<&str>) -> Option<String> {
    per_message
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .or_else(|| global.map(str::trim).filter(|l| !l.is_empty()))
        .map(str::to_string)
}

/// On-disk transcripts: one text file per message under a per-chat dir,
/// bounded in total size with oldest-first eviction. The directory is
/// created 0700 and files 0600 - transcripts are message content.
pub struct Store {
    root: PathBuf,
    /// Total bytes the store may hold before evicting oldest.
    max_bytes: u64,
}

impl Store {
    pub fn new(root: PathBuf, max_bytes: u64) -> std::io::Result<Self> {
        std::fs::create_dir_all(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self { root, max_bytes })
    }

    fn file_for(&self, account_id: &str, chat_id: &str, message_id: &str) -> PathBuf {
        let safe = |s: &str| {
            s.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        };
        self.root
            .join(safe(account_id))
            .join(safe(chat_id))
            .join(format!("{}.txt", safe(message_id)))
    }

    pub fn get(&self, account_id: &str, chat_id: &str, message_id: &str) -> Option<String> {
        std::fs::read_to_string(self.file_for(account_id, chat_id, message_id)).ok()
    }

    /// Removes one transcript, e.g. when its message is deleted.
    pub fn remove(&self, account_id: &str, chat_id: &str, message_id: &str) -> std::io::Result<()> {
        match std::fs::remove_file(self.file_for(account_id, chat_id, message_id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Privacy control: removes every transcript of one chat.
    pub fn remove_chat(&self, account_id: &str, chat_id: &str) -> std::io::Result<()> {
        let file = self.file_for(account_id, chat_id, "_");
        let dir = file.parent().map(Path::to_path_buf).unwrap_or(file);
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Privacy control: removes every transcript of every chat.
    pub fn clear(&self) -> std::io::Result<()> {
        for entry in std::fs::read_dir(&self.root)?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                std::fs::remove_dir_all(&path)?;
            }
        }
        Ok(())
    }

    /// Writes one transcript and evicts oldest files past the cap.
    pub fn put(
        &self,
        account_id: &str,
        chat_id: &str,
        message_id: &str,
        text: &str,
    ) -> std::io::Result<()> {
        let file = self.file_for(account_id, chat_id, message_id);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        std::fs::write(&file, text)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600))?;
        }
        self.evict()
    }

    /// Evicts oldest-modified transcript files until under the cap.
    /// Symlinks are never followed or removed through: symlink_metadata
    /// keeps a planted link from pointing the store outside its root.
    fn evict(&self) -> std::io::Result<()> {
        let mut files: Vec<(u64, std::time::SystemTime, PathBuf)> = Vec::new();
        for account in std::fs::read_dir(&self.root)?.flatten() {
            if !account.path().is_dir() {
                continue;
            }
            for chat in std::fs::read_dir(account.path())?.flatten() {
                if !chat.path().is_dir() {
                    continue;
                }
                for entry in std::fs::read_dir(chat.path())?.flatten() {
                    // DirEntry::file_type never follows links, so a symlink
                    // is excluded before any metadata call touches its target.
                    if let Ok(file_type) = entry.file_type()
                        && file_type.is_file()
                    {
                        if let Ok(meta) = entry.metadata() {
                            files.push((
                                meta.len(),
                                meta.modified().unwrap_or(std::time::UNIX_EPOCH),
                                entry.path(),
                            ));
                        }
                    }
                }
            }
        }
        let mut total: u64 = files.iter().map(|(len, _, _)| len).sum();
        files.sort_by_key(|(_, modified, _)| *modified);
        for (len, _, path) in files {
            if total <= self.max_bytes {
                break;
            }
            std::fs::remove_file(&path)?;
            total = total.saturating_sub(len);
        }
        Ok(())
    }
}

/// Converts mono f32 samples (-1..=1) at `from_rate` into Wyoming's wire
/// format: 16 kHz mono signed 16-bit little-endian. The app's voice
/// decode already yields mono 48 kHz, so the usual path is a clean 3:1
/// linear resample; other rates get the same linear treatment.
pub fn wyoming_pcm_from_mono(samples: &[f32], from_rate: u32) -> Vec<u8> {
    const TARGET: u32 = 16_000;
    if samples.is_empty() || from_rate == 0 {
        return Vec::new();
    }
    let out_len = (samples.len() as u64 * TARGET as u64 / from_rate as u64).max(1) as usize;
    let mut pcm = Vec::with_capacity(out_len * 2);
    for i in 0..out_len {
        // Map the output index back to a source position and interpolate.
        let pos = i as f64 * from_rate as f64 / TARGET as f64;
        let base = (pos as usize).min(samples.len() - 1);
        let next = (base + 1).min(samples.len() - 1);
        let frac = (pos - base as f64) as f32;
        let sample = samples[base] + (samples[next] - samples[base]) * frac;
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        pcm.extend_from_slice(&value.to_le_bytes());
    }
    pcm
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str) -> Job {
        Job {
            message_id: id.into(),
            chat_id: "c1".into(),
            audio: PathBuf::from("/tmp/x.ogg"),
            duration_secs: Some(3.0),
            language: None,
        }
    }

    #[test]
    fn queue_rejects_duplicate_message() {
        let mut q = JobQueue::new(10);
        assert!(q.push(job("m1")));
        assert!(!q.push(job("m1")));
        let claimed = q.claim().unwrap();
        assert_eq!(claimed.message_id, "m1");
        assert!(!q.push(job("m1"))); // running still counts
    }

    #[test]
    fn queue_drops_oldest_pending_past_cap() {
        let mut q = JobQueue::new(2);
        q.push(job("m1"));
        q.push(job("m2"));
        q.push(job("m3"));
        assert_eq!(q.pending_len(), 2);
        assert_eq!(q.claim().unwrap().message_id, "m2");
    }

    #[test]
    fn crash_requeues_running_job_once_at_front() {
        let mut q = JobQueue::new(10);
        q.push(job("m1"));
        q.push(job("m2"));
        q.claim();
        q.recover();
        assert_eq!(q.claim().unwrap().message_id, "m1");
    }

    #[test]
    fn language_prefers_message_then_global_then_none() {
        assert_eq!(
            resolve_language(Some("fi"), Some("en")).as_deref(),
            Some("fi")
        );
        assert_eq!(resolve_language(None, Some("en")).as_deref(), Some("en"));
        assert_eq!(
            resolve_language(Some("  "), Some("en")).as_deref(),
            Some("en")
        );
        assert_eq!(resolve_language(None, None), None);
    }

    #[test]
    fn store_roundtrip_and_unsafe_ids() {
        let dir = std::env::temp_dir().join(format!("tr-store-{}", std::process::id()));
        let store = Store::new(dir.clone(), 1 << 20).unwrap();
        store
            .put("acc-1", "chat 9/4", "m!1", "hei maailma")
            .unwrap();
        assert_eq!(
            store.get("acc-1", "chat 9/4", "m!1").as_deref(),
            Some("hei maailma")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_evicts_oldest_past_cap() {
        let dir = std::env::temp_dir().join(format!("tr-evict-{}", std::process::id()));
        let store = Store::new(dir.clone(), 10).unwrap();
        store.put("a", "c", "old", "12345678").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        store.put("a", "c", "new", "12345678").unwrap();
        assert_eq!(store.get("a", "c", "old"), None);
        assert_eq!(store.get("a", "c", "new").as_deref(), Some("12345678"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_isolates_accounts_and_chats() {
        let dir = std::env::temp_dir().join(format!("tr-iso-{}", std::process::id()));
        let store = Store::new(dir.clone(), 1 << 20).unwrap();
        store.put("acc-1", "c", "m", "one").unwrap();
        store.put("acc-2", "c", "m", "two").unwrap();
        store.put("acc-1", "d", "m", "three").unwrap();
        assert_eq!(store.get("acc-1", "c", "m").as_deref(), Some("one"));
        assert_eq!(store.get("acc-2", "c", "m").as_deref(), Some("two"));
        assert_eq!(store.get("acc-1", "d", "m").as_deref(), Some("three"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_delete_controls() {
        let dir = std::env::temp_dir().join(format!("tr-del-{}", std::process::id()));
        let store = Store::new(dir.clone(), 1 << 20).unwrap();
        store.put("a", "c1", "m1", "x").unwrap();
        store.put("a", "c1", "m2", "y").unwrap();
        store.put("a", "c2", "m3", "z").unwrap();
        store.remove("a", "c1", "m1").unwrap();
        assert_eq!(store.get("a", "c1", "m1"), None);
        assert_eq!(store.get("a", "c1", "m2").as_deref(), Some("y"));
        store.remove_chat("a", "c1").unwrap();
        assert_eq!(store.get("a", "c1", "m2"), None);
        assert_eq!(store.get("a", "c2", "m3").as_deref(), Some("z"));
        store.clear().unwrap();
        assert_eq!(store.get("a", "c2", "m3"), None);
        // Deleting what is absent is not an error.
        store.remove("a", "c1", "m1").unwrap();
        store.remove_chat("a", "gone").unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn store_eviction_ignores_symlinks() {
        let dir = std::env::temp_dir().join(format!("tr-sym-{}", std::process::id()));
        let store = Store::new(dir.clone(), 10).unwrap();
        store.put("a", "c", "real", "12345678").unwrap();
        // A planted symlink inside the store pointing outside it: eviction
        // must neither follow nor delete through it.
        let outside = std::env::temp_dir().join(format!("tr-outside-{}", std::process::id()));
        std::fs::write(&outside, "keep me").unwrap();
        std::os::unix::fs::symlink(&outside, store.file_for("a", "c", "link")).unwrap();
        store.put("a", "c", "new", "12345678").unwrap();
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep me");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_file(&outside).ok();
    }

    #[cfg(unix)]
    #[test]
    fn store_files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("tr-perms-{}", std::process::id()));
        let store = Store::new(dir.clone(), 1 << 20).unwrap();
        store.put("a", "c", "m", "secret words").unwrap();
        let mode = std::fs::metadata(store.file_for("a", "c", "m"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn wyoming_pcm_downsamples_48k_to_16k() {
        // 48 samples at 48 kHz = 1 ms of audio -> 16 samples at 16 kHz.
        let samples: Vec<f32> = (0..48).map(|i| i as f32 / 48.0).collect();
        let pcm = wyoming_pcm_from_mono(&samples, 48_000);
        assert_eq!(pcm.len(), 16 * 2);
        // Little-endian i16 of the first sample (0.0) and a later one.
        assert_eq!(&pcm[0..2], &0i16.to_le_bytes());
        let mid = i16::from_le_bytes([pcm[20], pcm[21]]);
        assert!(mid > 0);
    }

    #[test]
    fn wyoming_pcm_clamps_and_handles_edge_cases() {
        let loud = wyoming_pcm_from_mono(&[2.0; 48], 48_000);
        assert_eq!(&loud[0..2], &i16::MAX.to_le_bytes());
        let quiet = wyoming_pcm_from_mono(&[-3.0; 48], 48_000);
        assert_eq!(&quiet[0..2], &(-i16::MAX).to_le_bytes());
        assert!(wyoming_pcm_from_mono(&[], 48_000).is_empty());
        assert!(wyoming_pcm_from_mono(&[0.5], 0).is_empty());
        // Same rate in and out still converts to s16le.
        let same = wyoming_pcm_from_mono(&[0.5, -0.5], 16_000);
        assert_eq!(same.len(), 4);
    }

    struct EchoEngine;
    impl Engine for EchoEngine {
        fn transcribe(&mut self, audio: &Path, language: &str) -> Result<String, String> {
            Ok(format!("{}:{}", language, audio.display()))
        }
    }

    #[test]
    fn engine_trait_object_is_usable() {
        let mut engine: Box<dyn Engine> = Box::new(EchoEngine);
        assert_eq!(
            engine.transcribe(Path::new("/a.ogg"), "fi").unwrap(),
            "fi:/a.ogg"
        );
    }
}
