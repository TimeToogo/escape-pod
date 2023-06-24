use std::{
    env,
    io::Read,
    path::{Path, PathBuf},
    process::{self, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use escapepod_common::nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};

pub fn workspace_dir() -> PathBuf {
    let output = process::Command::new(env::var("CARGO").unwrap())
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .unwrap()
        .stdout;
    let cargo_path = Path::new(std::str::from_utf8(&output).unwrap().trim());
    cargo_path.parent().unwrap().to_path_buf()
}

#[cfg(debug_assertions)]
pub fn cargo_profile() -> &'static str {
    "debug"
}

#[cfg(not(debug_assertions))]
pub fn cargo_profile() -> &'static str {
    "release"
}

pub fn target_dir() -> PathBuf {
    workspace_dir().join("target").join(cargo_profile())
}

pub fn escapepod_bin() -> String {
    target_dir().join("escapepod").to_string_lossy().to_string()
}

pub struct ChildWithStreamedOutput {
    pub proc: process::Child,
    pub stdout: Arc<Mutex<String>>,
    pub stderr: Arc<Mutex<String>>,
}

impl ChildWithStreamedOutput {
    pub fn signal(&mut self, signal: Signal) {
        signal::kill(Pid::from_raw(self.proc.id() as _), signal).unwrap();
    }
}

pub fn spawn(cmd: &mut process::Command) -> ChildWithStreamedOutput {
    let mut proc = cmd
        .env("RUST_LOG", "escapepod=trace")
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn");

    let stdout = Arc::new(Mutex::new("".to_string()));
    let stderr = Arc::new(Mutex::new("".to_string()));

    let pid = proc.id();
    let mut stdout_pipe = proc.stdout.take().unwrap();
    let mut stderr_pipe = proc.stderr.take().unwrap();

    thread::spawn({
        let stdout = stdout.clone();
        move || {
            let mut bbuf = [0u8; 1024];
            loop {
                let len = stdout_pipe.read(&mut bbuf).unwrap();
                if len == 0 {
                    break;
                }
                let out = String::from_utf8_lossy(&bbuf[..len]).to_string();
                print!("{}", out.replace("\n", &format!("\n [{pid}] ")));
                let mut buf = stdout.lock().unwrap();
                buf.push_str(out.as_str());
            }
        }
    });

    thread::spawn({
        let stderr = stderr.clone();
        move || {
            let mut bbuf = [0u8; 1024];
            loop {
                let len = stderr_pipe.read(&mut bbuf).unwrap();
                if len == 0 {
                    break;
                }
                let out = String::from_utf8_lossy(&bbuf[..len]).to_string();
                print!("{}", out.replace("\n", &format!("\n [{pid}] ")));
                let mut buf = stderr.lock().unwrap();
                buf.push_str(out.as_str());
            }
        }
    });

    ChildWithStreamedOutput {
        proc,
        stdout,
        stderr,
    }
}

pub fn wait_for_output(child: &ChildWithStreamedOutput, contents: &'static str) {
    loop {
        for out in [&child.stdout, &child.stderr] {
            let out = out.lock().unwrap();
            if out.contains(contents) {
                return;
            }
        }

        thread::sleep(Duration::from_millis(10));
    }
}
