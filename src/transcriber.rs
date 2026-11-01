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
    pub fn next(&mut self) -> Option<Job> {
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

    fn file_for(&self, chat_id: &str, message_id: &str) -> PathBuf {
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
            .join(safe(chat_id))
            .join(format!("{}.txt", safe(message_id)))
    }

    pub fn get(&self, chat_id: &str, message_id: &str) -> Option<String> {
        std::fs::read_to_string(self.file_for(chat_id, message_id)).ok()
    }

    /// Writes one transcript and evicts oldest files past the cap.
    pub fn put(&self, chat_id: &str, message_id: &str, text: &str) -> std::io::Result<()> {
        let file = self.file_for(chat_id, message_id);
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
    fn evict(&self) -> std::io::Result<()> {
        let mut files: Vec<(u64, std::time::SystemTime, PathBuf)> = Vec::new();
        for chat in std::fs::read_dir(&self.root)?.flatten() {
            for entry in std::fs::read_dir(chat.path())?.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        files.push((
                            meta.len(),
                            meta.modified().unwrap_or(std::time::UNIX_EPOCH),
                            entry.path(),
                        ));
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
        let claimed = q.next().unwrap();
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
        assert_eq!(q.next().unwrap().message_id, "m2");
    }

    #[test]
    fn crash_requeues_running_job_once_at_front() {
        let mut q = JobQueue::new(10);
        q.push(job("m1"));
        q.push(job("m2"));
        q.next();
        q.recover();
        assert_eq!(q.next().unwrap().message_id, "m1");
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
        store.put("chat 9/4", "m!1", "hei maailma").unwrap();
        assert_eq!(store.get("chat 9/4", "m!1").as_deref(), Some("hei maailma"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_evicts_oldest_past_cap() {
        let dir = std::env::temp_dir().join(format!("tr-evict-{}", std::process::id()));
        let store = Store::new(dir.clone(), 10).unwrap();
        store.put("c", "old", "12345678").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        store.put("c", "new", "12345678").unwrap();
        assert_eq!(store.get("c", "old"), None);
        assert_eq!(store.get("c", "new").as_deref(), Some("12345678"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn store_files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("tr-perms-{}", std::process::id()));
        let store = Store::new(dir.clone(), 1 << 20).unwrap();
        store.put("c", "m", "secret words").unwrap();
        let mode = std::fs::metadata(store.file_for("c", "m"))
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
