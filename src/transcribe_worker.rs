//! The transcription worker: one background thread owning the engine and
//! the store, fed voice-note jobs over a channel, reporting per-message
//! status the conversation view reads each frame. Audio never leaves the
//! machine: the engine is a local service or binary behind `Engine`.
//!
//! Deletion semantics are immediate, not queued: remove_chat/clear mark
//! shared tombstones and purge statuses on the calling thread, so an
//! in-flight engine's late result can never store or show deleted text,
//! and a dropped (respawned) worker's result never lands either.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use crate::transcriber::{Engine, Hit, Job, JobQueue, Store, resolve_language};

/// What the bubble shows for one message's transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Running,
    /// The transcript text, hot in memory; the store keeps the durable copy.
    Ready(String),
    Failed(String),
}

/// Status map key: (account_id, chat_id, message_id). Message ids are
/// unique within a chat, chat ids within an account; the account half
/// keeps same-shaped ids of two accounts from colliding in memory.
type Key = (String, String, String);

/// Deletion markers shared between the calling thread and the worker
/// thread: written immediately by remove_chat/clear, checked when an
/// in-flight job finishes. A fresh transcribe for a re-added chat lifts
/// its marker.
#[derive(Default)]
pub struct Tombstones {
    chats: HashSet<(String, String)>,
    clears: u64,
}

/// Privacy commands ride the same channel so they order behind queued
/// transcription work for the affected chat; the tombstones above make
/// their effect immediate anyway.
pub enum Command {
    Transcribe {
        job: Job,
        account_id: String,
        global_language: Option<String>,
    },
    RemoveChat {
        account_id: String,
        chat_id: String,
    },
    Clear,
    Shutdown,
}

pub struct Worker {
    tx: Sender<Command>,
    /// (account_id, chat_id, message_id) -> status, read by the view.
    pub status: Arc<Mutex<HashMap<Key, Status>>>,
    /// Deletion markers checked when in-flight work finishes.
    tombstones: Arc<Mutex<Tombstones>>,
    /// False once the worker is dropped (respawn on a settings change):
    /// the old thread's late result is dropped instead of stored.
    alive: Arc<AtomicBool>,
    /// The store search runs against; `None` on stub workers.
    store: Option<Store>,
    /// Search memo: (store version, query, hits).
    search_cache: SearchCache,
    /// What a stub was told, for tests; always empty on a live worker.
    recorded: Arc<Mutex<Vec<String>>>,
}

/// Search memo: (store version, query, hits).
type SearchCache = Arc<Mutex<Option<(u64, String, Vec<Hit>)>>>;

/// The loop, factored out for tests: drain commands until Shutdown or the
/// channel closes. `wake` repaints the view after each status change.
#[allow(clippy::too_many_arguments)]
pub fn run(
    rx: Receiver<Command>,
    mut engine: Box<dyn Engine + Send>,
    store: Store,
    status: Arc<Mutex<HashMap<Key, Status>>>,
    tombstones: Arc<Mutex<Tombstones>>,
    alive: Arc<AtomicBool>,
    wake: impl Fn(),
) {
    let mut queue = JobQueue::new(64);
    let gone = |tombstones: &Arc<Mutex<Tombstones>>, account: &str, chat: &str, clears: u64| {
        let marked = tombstones.lock().unwrap_or_else(|p| p.into_inner());
        marked.clears != clears
            || marked
                .chats
                .contains(&(account.to_owned(), chat.to_owned()))
    };
    while let Ok(command) = rx.recv() {
        match command {
            Command::Transcribe {
                job,
                account_id,
                global_language,
            } => {
                if !queue.push(job) {
                    continue;
                }
                // One at a time: engines are local models; parallel runs
                // would thrash the machine for no latency win.
                while let Some(job) = queue.claim() {
                    let key = (
                        account_id.clone(),
                        job.chat_id.clone(),
                        job.message_id.clone(),
                    );
                    let clears_at_claim =
                        tombstones.lock().unwrap_or_else(|p| p.into_inner()).clears;
                    if gone(&tombstones, &account_id, &job.chat_id, clears_at_claim) {
                        // Claimed after its chat was deleted: skip silently.
                        queue.finish(true);
                        continue;
                    }
                    let language =
                        resolve_language(job.language.as_deref(), global_language.as_deref());
                    set(&status, &key, Status::Running);
                    wake();
                    let result = engine.transcribe(&job.audio, language.as_deref().unwrap_or(""));
                    // The decisive check, after the engine's work: a delete
                    // or clear that landed while it ran, or this worker's
                    // own drop, discards the late result entirely.
                    if !alive.load(Ordering::Relaxed)
                        || gone(&tombstones, &account_id, &job.chat_id, clears_at_claim)
                    {
                        drop_status(&status, &key);
                        queue.finish(true);
                        wake();
                        continue;
                    }
                    let result = result.and_then(|text| {
                        store
                            .put(&account_id, &job.chat_id, &job.message_id, &text)
                            .map_err(|e| format!("transcript store: {e}"))?;
                        Ok(text)
                    });
                    let state = match result {
                        Ok(text) => Status::Ready(text),
                        Err(error) => Status::Failed(error),
                    };
                    let done = matches!(state, Status::Ready(_));
                    set(&status, &key, state);
                    queue.finish(done);
                    wake();
                }
            }
            Command::RemoveChat {
                account_id,
                chat_id,
            } => {
                // The tombstone and the status purge already landed on the
                // calling thread; this is the durable half.
                let _ = store.remove_chat(&account_id, &chat_id);
                status
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .retain(|(account, chat, _), _| !(account == &account_id && chat == &chat_id));
                wake();
            }
            Command::Clear => {
                let _ = store.clear();
                status.lock().unwrap_or_else(|p| p.into_inner()).clear();
                wake();
            }
            Command::Shutdown => return,
        }
    }
}

fn set(status: &Arc<Mutex<HashMap<Key, Status>>>, key: &Key, state: Status) {
    status
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(key.clone(), state);
}

fn drop_status(status: &Arc<Mutex<HashMap<Key, Status>>>, key: &Key) {
    status.lock().unwrap_or_else(|p| p.into_inner()).remove(key);
}

impl Worker {
    /// Spawns the worker thread. `wake` must repaint the conversation view.
    pub fn spawn(
        engine: Box<dyn Engine + Send>,
        store: Store,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (tx, rx) = channel();
        let status = Arc::new(Mutex::new(HashMap::new()));
        let tombstones = Arc::new(Mutex::new(Tombstones::default()));
        let alive = Arc::new(AtomicBool::new(true));
        let thread_status = Arc::clone(&status);
        let thread_tombstones = Arc::clone(&tombstones);
        let thread_alive = Arc::clone(&alive);
        let thread_store = store.clone();
        std::thread::Builder::new()
            .name("transcribe".to_string())
            .spawn(move || {
                run(
                    rx,
                    engine,
                    thread_store,
                    thread_status,
                    thread_tombstones,
                    thread_alive,
                    wake,
                )
            })
            .expect("transcribe worker thread");
        Self {
            tx,
            status,
            tombstones,
            alive,
            store: Some(store),
            search_cache: Arc::new(Mutex::new(None)),
            recorded: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Queues one voice note. Duplicate message ids are dropped in the
    /// queue; a Ready status short-circuits before a job is even sent. A
    /// fresh request for a chat lifts its deletion marker: the chat is
    /// back and the user asked again.
    pub fn transcribe(&self, job: Job, account_id: String, global_language: Option<String>) {
        self.tombstones
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .chats
            .remove(&(account_id.clone(), job.chat_id.clone()));
        let known = self
            .status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&(
                account_id.clone(),
                job.chat_id.clone(),
                job.message_id.clone(),
            ))
            .cloned();
        if matches!(known, Some(Status::Ready(_)) | Some(Status::Running)) {
            return;
        }
        let _ = self.tx.send(Command::Transcribe {
            job,
            account_id,
            global_language,
        });
    }

    /// Deletes one chat's transcripts, effective now: the marker goes up
    /// and the statuses purge on this thread, before any in-flight engine
    /// for the chat can finish.
    pub fn remove_chat(&self, account_id: String, chat_id: String) {
        self.tombstones
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .chats
            .insert((account_id.clone(), chat_id.clone()));
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|(account, chat, _), _| !(account == &account_id && chat == &chat_id));
        let _ = self.tx.send(Command::RemoveChat {
            account_id,
            chat_id,
        });
    }

    /// Deletes every transcript, effective now: same immediate semantics
    /// as remove_chat.
    pub fn clear(&self) {
        self.tombstones
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clears += 1;
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        let _ = self.tx.send(Command::Clear);
    }

    /// Local transcript search over the store, newest first. Cached on the
    /// store version so repaints don't rescan disk; a stub has no store
    /// and finds nothing. Callers scope the hits to their account.
    pub fn search(&self, query: &str) -> Vec<Hit> {
        let Some(store) = &self.store else {
            return Vec::new();
        };
        let key = query.trim().to_owned();
        if key.is_empty() {
            return Vec::new();
        }
        let version = store.version();
        {
            let cache = self.search_cache.lock().unwrap_or_else(|p| p.into_inner());
            if let Some((cached_version, cached_key, hits)) = &*cache
                && *cached_version == version
                && *cached_key == key
            {
                return hits.clone();
            }
        }
        let hits = store.search(&key, 50);
        *self.search_cache.lock().unwrap_or_else(|p| p.into_inner()) =
            Some((version, key, hits.clone()));
        hits
    }

    /// A worker with no engine behind it, for demos and tests: the given
    /// statuses are what the view sees; queued work is only recorded.
    pub fn stub(pairs: impl IntoIterator<Item = (Key, Status)>) -> Self {
        let (tx, rx) = channel();
        let recorded = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&recorded);
        std::thread::spawn(move || {
            while let Ok(command) = rx.recv() {
                let line = match &command {
                    Command::Transcribe {
                        job,
                        account_id,
                        global_language,
                    } => format!(
                        "transcribe {} in {} as {account_id} ({global_language:?})",
                        job.message_id, job.chat_id
                    ),
                    Command::RemoveChat {
                        account_id,
                        chat_id,
                    } => format!("remove-chat {chat_id} of {account_id}"),
                    Command::Clear => "clear".to_owned(),
                    Command::Shutdown => "shutdown".to_owned(),
                };
                log.lock().unwrap_or_else(|p| p.into_inner()).push(line);
            }
        });
        Self {
            tx,
            status: Arc::new(Mutex::new(pairs.into_iter().collect())),
            tombstones: Arc::new(Mutex::new(Tombstones::default())),
            alive: Arc::new(AtomicBool::new(true)),
            store: None,
            search_cache: Arc::new(Mutex::new(None)),
            recorded,
        }
    }

    /// A stub whose store is real, for search tests: no engine behind it,
    /// but the given transcripts are searchable.
    #[cfg(test)]
    pub fn stub_with_store(store: Store) -> Self {
        let mut worker = Self::stub([]);
        worker.store = Some(store);
        worker
    }

    /// What a stub was told, for tests. The draining thread parks on the
    /// channel; callers poll until the expectation holds. Available in
    /// every build so the field is used outside test targets too.
    pub fn recorded(&self) -> Vec<String> {
        self.recorded
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Status for one message in one chat of one account, for the bubble.
    pub fn get(&self, account_id: &str, chat_id: &str, message_id: &str) -> Option<Status> {
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&(
                account_id.to_owned(),
                chat_id.to_owned(),
                message_id.to_owned(),
            ))
            .cloned()
    }
}

impl Drop for Worker {
    /// A respawned worker (the service address changed) stops meaning
    /// anything: its in-flight engine's result is dropped, not stored.
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    struct EchoEngine;
    impl Engine for EchoEngine {
        fn transcribe(
            &mut self,
            audio: &std::path::Path,
            language: &str,
        ) -> Result<String, String> {
            Ok(format!("{language}:{}", audio.display()))
        }
    }

    struct FailEngine;
    impl Engine for FailEngine {
        fn transcribe(
            &mut self,
            _audio: &std::path::Path,
            _language: &str,
        ) -> Result<String, String> {
            Err("engine down".into())
        }
    }

    /// A slow engine so tests can delete the chat mid-transcription.
    struct SlowEngine;
    impl Engine for SlowEngine {
        fn transcribe(
            &mut self,
            audio: &std::path::Path,
            language: &str,
        ) -> Result<String, String> {
            std::thread::sleep(Duration::from_millis(300));
            Ok(format!("{language}:{}", audio.display()))
        }
    }

    fn job(id: &str) -> Job {
        Job {
            message_id: id.into(),
            chat_id: "c1".into(),
            audio: PathBuf::from("/tmp/note.ogg"),
            duration_secs: Some(2.0),
            language: None,
        }
    }

    fn store() -> (Store, PathBuf) {
        let dir = std::env::temp_dir().join(format!("tw-store-{}", std::process::id()));
        (Store::new(dir.clone(), 1 << 20).unwrap(), dir)
    }

    fn wait_for(worker: &Worker, account: &str, chat: &str, id: &str) -> Status {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match worker.get(account, chat, id) {
                Some(s @ (Status::Ready(_) | Status::Failed(_))) => return s,
                _ if Instant::now() > deadline => panic!("no final status for {id}"),
                _ => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    }

    fn wait_running(worker: &Worker, account: &str, chat: &str, id: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while worker.get(account, chat, id) != Some(Status::Running) {
            assert!(Instant::now() <= deadline, "job never started");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn transcribe_ready_and_stored() {
        let (s, dir) = store();
        let worker = Worker::spawn(Box::new(EchoEngine), s.clone(), || {});
        worker.transcribe(job("m1"), "acc-1".into(), Some("fi".into()));
        assert_eq!(
            wait_for(&worker, "acc-1", "c1", "m1"),
            Status::Ready("fi:/tmp/note.ogg".into())
        );
        // The durable copy landed under account/chat/message isolation.
        assert_eq!(
            s.get("acc-1", "c1", "m1").as_deref(),
            Some("fi:/tmp/note.ogg")
        );
        worker.clear();
        let deadline = Instant::now() + Duration::from_secs(5);
        while worker.get("acc-1", "c1", "m1").is_some() {
            assert!(Instant::now() <= deadline, "clear never landed");
            std::thread::sleep(Duration::from_millis(10));
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn failure_reported_not_retried_forever() {
        let (s, dir) = store();
        let worker = Worker::spawn(Box::new(FailEngine), s, || {});
        worker.transcribe(job("m2"), "acc-1".into(), None);
        assert_eq!(
            wait_for(&worker, "acc-1", "c1", "m2"),
            Status::Failed("engine down".into())
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn duplicate_transcribe_is_dropped() {
        let (s, dir) = store();
        let worker = Worker::spawn(Box::new(EchoEngine), s, || {});
        worker.transcribe(job("m3"), "acc-1".into(), None);
        let first = wait_for(&worker, "acc-1", "c1", "m3");
        worker.transcribe(job("m3"), "acc-1".into(), None); // Ready short-circuits
        assert_eq!(worker.get("acc-1", "c1", "m3"), Some(first));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn global_language_used_when_job_has_none() {
        let (s, dir) = store();
        let worker = Worker::spawn(Box::new(EchoEngine), s, || {});
        worker.transcribe(job("m4"), "acc-1".into(), Some("en".into()));
        assert_eq!(
            wait_for(&worker, "acc-1", "c1", "m4"),
            Status::Ready("en:/tmp/note.ogg".into())
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn accounts_sharing_ids_stay_separate() {
        let (s, dir) = store();
        let worker = Worker::spawn(Box::new(EchoEngine), s.clone(), || {});
        // Same chat id, same message id, two accounts: no collision in
        // memory, statuses, or the store.
        worker.transcribe(job("m5"), "acc-1".into(), Some("fi".into()));
        worker.transcribe(job("m5"), "acc-2".into(), Some("en".into()));
        assert_eq!(
            wait_for(&worker, "acc-1", "c1", "m5"),
            Status::Ready("fi:/tmp/note.ogg".into())
        );
        assert_eq!(
            wait_for(&worker, "acc-2", "c1", "m5"),
            Status::Ready("en:/tmp/note.ogg".into())
        );
        assert_eq!(
            s.get("acc-1", "c1", "m5").as_deref(),
            Some("fi:/tmp/note.ogg")
        );
        assert_eq!(
            s.get("acc-2", "c1", "m5").as_deref(),
            Some("en:/tmp/note.ogg")
        );
        // Deleting the chat in one account leaves the other's intact.
        worker.remove_chat("acc-1".into(), "c1".into());
        let deadline = Instant::now() + Duration::from_secs(5);
        while s.get("acc-1", "c1", "m5").is_some() {
            assert!(Instant::now() <= deadline, "remove never landed");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(worker.get("acc-1", "c1", "m5"), None);
        assert!(matches!(
            worker.get("acc-2", "c1", "m5"),
            Some(Status::Ready(_))
        ));
        assert!(s.get("acc-2", "c1", "m5").is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remove_chat_is_immediate_and_drops_the_late_result() {
        let (s, dir) = store();
        let worker = Worker::spawn(Box::new(SlowEngine), s.clone(), || {});
        worker.transcribe(job("m6"), "acc-1".into(), None);
        wait_running(&worker, "acc-1", "c1", "m6");
        worker.remove_chat("acc-1".into(), "c1".into());
        // Immediate: the status is gone before the engine finishes, not
        // when the queued command is reached.
        assert_eq!(worker.get("acc-1", "c1", "m6"), None);
        // Adversarial: the engine finishes long after the delete landed.
        std::thread::sleep(Duration::from_millis(700));
        assert_eq!(worker.get("acc-1", "c1", "m6"), None, "late status landed");
        assert_eq!(s.get("acc-1", "c1", "m6"), None, "late result stored");
        // A fresh request for the re-added chat still transcribes.
        worker.transcribe(job("m6"), "acc-1".into(), Some("fi".into()));
        assert_eq!(
            wait_for(&worker, "acc-1", "c1", "m6"),
            Status::Ready("fi:/tmp/note.ogg".into())
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clear_during_in_flight_drops_the_late_result() {
        let (s, dir) = store();
        let worker = Worker::spawn(Box::new(SlowEngine), s.clone(), || {});
        worker.transcribe(job("m7"), "acc-1".into(), None);
        wait_running(&worker, "acc-1", "c1", "m7");
        worker.clear();
        assert_eq!(worker.get("acc-1", "c1", "m7"), None);
        std::thread::sleep(Duration::from_millis(700));
        assert_eq!(worker.get("acc-1", "c1", "m7"), None, "late status landed");
        assert_eq!(s.get("acc-1", "c1", "m7"), None, "late result stored");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dropped_workers_late_result_never_lands() {
        let (s, dir) = store();
        let worker = Worker::spawn(Box::new(SlowEngine), s.clone(), || {});
        worker.transcribe(job("m8"), "acc-1".into(), None);
        wait_running(&worker, "acc-1", "c1", "m8");
        // The settings respawn drops the worker mid-engine.
        drop(worker);
        std::thread::sleep(Duration::from_millis(700));
        assert_eq!(
            s.get("acc-1", "c1", "m8"),
            None,
            "a dropped worker's result was stored"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_uses_the_store_and_caches() {
        let (s, dir) = store();
        s.put("acc-1", "c1", "m9", "hei maailma").unwrap();
        let worker = Worker::stub_with_store(s.clone());
        let hits = worker.search("maailma");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].message_id, "m9");
        // A new transcript invalidates the cache through the version bump.
        s.put("acc-1", "c1", "m10", "maailma again").unwrap();
        assert_eq!(worker.search("maailma").len(), 2);
        // A plain stub has no store and finds nothing.
        assert!(Worker::stub([]).search("maailma").is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
