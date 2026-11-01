//! Local speech engines behind the transcriber's Engine trait. Every
//! backend runs on the user's own machines; there is no cloud path.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::transcriber::Engine;

/// whisper.cpp's CLI (`whisper-cli`, formerly `main`): one process per
/// voice note, text on stdout. Voice notes are seconds long, so process
/// startup dominates; a bounded wall-clock cap keeps a wedged model from
/// stalling the queue.
pub struct WhisperCpp {
    /// Path to the whisper.cpp binary.
    pub binary: PathBuf,
    /// Path to the ggml/gguf model file.
    pub model: PathBuf,
    /// Wall-clock cap per voice note.
    pub timeout: Duration,
}

impl WhisperCpp {
    pub fn new(binary: PathBuf, model: PathBuf) -> Self {
        Self {
            binary,
            model,
            timeout: Duration::from_secs(120),
        }
    }
}

impl Engine for WhisperCpp {
    fn transcribe(&mut self, audio: &Path, language: &str) -> Result<String, String> {
        let mut cmd = Command::new(&self.binary);
        cmd.arg("-m")
            .arg(&self.model)
            .arg("-f")
            .arg(audio)
            .arg("--no-timestamps")
            .arg("--output-txt")
            .arg("-of")
            .arg("/dev/stdout")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // Auto-detect when no language is pinned; whisper.cpp wants no
        // -l flag at all for that.
        if !language.is_empty() {
            cmd.arg("-l").arg(language);
        }
        let started = Instant::now();
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("whisper.cpp not runnable at {}: {e}", self.binary.display()))?;
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => {
                    let mut out = String::new();
                    use std::io::Read;
                    child.stdout.take().map(|mut s| s.read_to_string(&mut out));
                    let text = out.trim().to_string();
                    if text.is_empty() {
                        return Err("whisper.cpp produced no transcript".into());
                    }
                    return Ok(text);
                }
                Ok(Some(status)) => return Err(format!("whisper.cpp exited {status}")),
                Ok(None) => {
                    if started.elapsed() > self.timeout {
                        let _ = child.kill();
                        return Err("whisper.cpp timed out".into());
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(format!("whisper.cpp wait failed: {e}")),
            }
        }
    }
}

/// The household Whisper behind the Wyoming protocol: TCP, one JSON
/// describe/transcribe handshake, audio streamed as WAV. Wyoming is the
/// user's measured accuracy winner and already runs on the Studio, so a
/// voice note can reuse it without loading a second model.
pub struct Wyoming {
    /// host:port of the Wyoming Whisper service.
    pub addr: String,
    pub timeout: Duration,
}

impl Wyoming {
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            addr: addr.into(),
            timeout: Duration::from_secs(120),
        }
    }
}

impl Engine for Wyoming {
    fn transcribe(&mut self, _audio: &Path, _language: &str) -> Result<String, String> {
        // The Wyoming event protocol needs audio resampled to 16 kHz mono
        // PCM; that conversion lands with the audio pipeline work. The
        // adapter is declared now so daemon wiring has both engines.
        Err("wyoming adapter not yet implemented".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A fake whisper.cpp: copies a fixture file to stdout so the adapter
    /// is tested for process plumbing, timeouts, and error mapping
    /// without a model.
    fn fake_whisper(body: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("fake-whisper-{}", std::process::id()));
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn whisper_cpp_returns_stdout_text() {
        let bin = fake_whisper("#!/bin/sh\nprintf 'hei maailma\\n'\n");
        let mut engine = WhisperCpp::new(bin.clone(), PathBuf::from("/tmp/model.bin"));
        let text = engine.transcribe(Path::new("/tmp/note.ogg"), "fi").unwrap();
        assert_eq!(text, "hei maailma");
        std::fs::remove_file(bin).ok();
    }

    #[test]
    fn whisper_cpp_maps_missing_binary() {
        let mut engine = WhisperCpp::new(PathBuf::from("/no/such/binary"), PathBuf::from("/m"));
        let err = engine.transcribe(Path::new("/a.ogg"), "fi").unwrap_err();
        assert!(err.contains("not runnable"), "got: {err}");
    }

    #[test]
    fn whisper_cpp_maps_failure_exit() {
        let bin = fake_whisper("#!/bin/sh\nexit 3\n");
        let mut engine = WhisperCpp::new(bin.clone(), PathBuf::from("/m"));
        let err = engine.transcribe(Path::new("/a.ogg"), "fi").unwrap_err();
        assert!(err.contains("exited"), "got: {err}");
        std::fs::remove_file(bin).ok();
    }

    #[test]
    fn whisper_cpp_maps_empty_output() {
        let bin = fake_whisper("#!/bin/sh\ntrue\n");
        let mut engine = WhisperCpp::new(bin.clone(), PathBuf::from("/m"));
        let err = engine.transcribe(Path::new("/a.ogg"), "fi").unwrap_err();
        assert!(err.contains("no transcript"), "got: {err}");
        std::fs::remove_file(bin).ok();
    }

    #[test]
    fn whisper_cpp_timeout_kills() {
        let bin = fake_whisper("#!/bin/sh\nsleep 60\n");
        let mut engine = WhisperCpp::new(bin.clone(), PathBuf::from("/m"));
        engine.timeout = Duration::from_millis(300);
        let started = Instant::now();
        let err = engine.transcribe(Path::new("/a.ogg"), "fi").unwrap_err();
        assert!(err.contains("timed out"), "got: {err}");
        assert!(started.elapsed() < Duration::from_secs(5));
        std::fs::remove_file(bin).ok();
    }

    #[test]
    fn wyoming_adapter_declared_but_unimplemented() {
        let mut engine = Wyoming::new("192.0.2.1:10300");
        let err = engine.transcribe(Path::new("/a.ogg"), "fi").unwrap_err();
        assert!(err.contains("not yet implemented"));
    }
}
