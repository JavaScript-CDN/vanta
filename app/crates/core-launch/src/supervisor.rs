//! Process supervision: spawn with a clean environment, contain in a Job Object (Windows),
//! detect exit/crash, stop gracefully. One engine process tree per profile.
use crate::args::{LaunchSpec, build_args};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct ExitEvent { pub profile_id: String, pub code: Option<i32>, pub crashed: bool }

struct Running {
    pid: u32,
    kill: tokio::sync::oneshot::Sender<()>,
    #[cfg(windows)] _job: crate::job::Job,
}

#[derive(Clone)]
pub struct Supervisor { inner: Arc<Mutex<HashMap<String, Running>>>, events: mpsc::UnboundedSender<ExitEvent> }

/// Environment variables passed through to the engine; everything else is dropped (file 00 §5.3).
const ENV_ALLOW: &[&str] = &["SYSTEMROOT", "SYSTEMDRIVE", "WINDIR", "TEMP", "TMP", "USERPROFILE", "LOCALAPPDATA", "APPDATA",
    "PROGRAMDATA", "PROGRAMFILES", "PROGRAMFILES(X86)", "COMMONPROGRAMFILES", "HOMEDRIVE", "HOMEPATH", "PATH", "PATHEXT",
    "NUMBER_OF_PROCESSORS", "PROCESSOR_ARCHITECTURE", "HOME", "DISPLAY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"];

impl Supervisor {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ExitEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { inner: Arc::default(), events: tx }, rx)
    }

    pub fn is_running(&self, id: &str) -> bool { self.inner.lock().unwrap().contains_key(id) }
    pub fn running_count(&self) -> usize { self.inner.lock().unwrap().len() }
    pub fn pid(&self, id: &str) -> Option<u32> { self.inner.lock().unwrap().get(id).map(|r| r.pid) }

    pub fn spawn(&self, profile_id: &str, spec: &LaunchSpec, ram_limit_mb: Option<u32>, keep_alive: Box<dyn Send + 'static>) -> std::io::Result<u32> {
        if self.is_running(profile_id) { return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "running")); }
        let mut cmd = Command::new(&spec.engine_exe);
        cmd.args(build_args(spec)).env_clear().kill_on_drop(true);
        for (k, v) in std::env::vars_os() {
            if let Some(ks) = k.to_str() { if ENV_ALLOW.contains(&ks.to_ascii_uppercase().as_str()) { cmd.env(&k, v); } }
        }
        let mut child: Child = cmd.spawn()?;
        let pid = child.id().unwrap_or(0);
        #[cfg(windows)]
        let job = crate::job::Job::new(ram_limit_mb).and_then(|j| { j.assign(&child)?; Ok(j) })?;
        #[cfg(not(windows))]
        let _ = ram_limit_mb;
        let (ktx, krx) = tokio::sync::oneshot::channel::<()>();
        self.inner.lock().unwrap().insert(profile_id.to_string(), Running { pid, kill: ktx, #[cfg(windows)] _job: job });
        let (inner, events, id) = (self.inner.clone(), self.events.clone(), profile_id.to_string());
        tokio::spawn(async move {
            let _keep = keep_alive; // forwarder lives exactly as long as the engine
            let status = tokio::select! {
                s = child.wait() => s.ok(),
                _ = krx => { let _ = child.start_kill(); child.wait().await.ok() }
            };
            inner.lock().unwrap().remove(&id);
            let code = status.and_then(|s| s.code());
            let crashed = matches!(code, Some(c) if c != 0) && code != Some(1);
            let _ = events.send(ExitEvent { profile_id: id, code, crashed });
        });
        Ok(pid)
    }

    /// Stop: the Job Object (Windows) / kill_on_drop ends the whole tree.
    pub fn stop(&self, profile_id: &str) -> bool {
        if let Some(r) = self.inner.lock().unwrap().remove(profile_id) { let _ = r.kill.send(()); true } else { false }
    }

    pub fn stop_all(&self) {
        let all: Vec<_> = self.inner.lock().unwrap().drain().collect();
        for (_, r) in all { let _ = r.kill.send(()); }
    }
}
