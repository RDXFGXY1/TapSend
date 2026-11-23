use std::{
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
};

pub struct ServerProcess {
    child: Child,
    logs: Arc<Mutex<Vec<String>>>,
    shared: Vec<PathBuf>,
}
impl ServerProcess {
    pub fn start(files: &[PathBuf]) -> io::Result<Self> {
        let binary = std::env::var_os("TAPSEND_SERVER").unwrap_or_else(|| "tapsend-server".into());
        let mut command = Command::new(binary);
        for file in files {
            command.arg("--file").arg(file);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("server log unavailable"))?;
        let logs = Arc::new(Mutex::new(Vec::new()));
        let thread_logs = Arc::clone(&logs);
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Ok(mut values) = thread_logs.lock() {
                    values.push(line);
                    if values.len() > 200 {
                        values.remove(2);
                    }
                }
            }
        });
        Ok(Self {
            child,
            logs,
            shared: files.to_vec(),
        })
    }
    pub fn logs(&self) -> Vec<String> {
        self.logs
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }
    pub fn control(&mut self, line: &str) -> io::Result<()> {
        let mut parts = line.splitn(3, ' ');
        let action = parts.next().unwrap_or("");
        if ![
            "devices",
            "rename",
            "recipient",
            "approve",
            "deny",
            "revoke",
            "stop",
            "resume",
            "message",
            "pairing",
            "add",
        ]
        .contains(&action)
        {
            return Err(io::Error::other(
                "Commands: pairing | add PATH | message TEXT | stop | resume | devices | approve ID | deny ID | rename ID NAME | recipient ID | revoke ID",
            ));
        }
        let id = parts.next().unwrap_or("");
        let name = if action == "message" || action == "add" {
            line.strip_prefix(&format!("{action} ")).unwrap_or("")
        } else {
            parts.next().unwrap_or("")
        };
        let quote = |s: &str| {
            s.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .chars()
                .filter(|c| !c.is_control())
                .collect::<String>()
        };
        let input = self
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::other("Server input closed"))?;
        writeln!(
            input,
            "{{\"Action\":\"{}\",\"ID\":\"{}\",\"Name\":\"{}\"}}",
            quote(action),
            quote(id),
            quote(name)
        )
    }
    pub fn add_file(&mut self, path: &Path) -> io::Result<()> {
        let value = path
            .to_str()
            .ok_or_else(|| io::Error::other("File name is not valid UTF-8"))?;
        if value.chars().any(char::is_control) {
            return Err(io::Error::other("File name contains a control character"));
        }
        self.control(&format!("add {value}"))?;
        if !self.shared.iter().any(|existing| existing == path) {
            self.shared.push(path.to_path_buf());
        }
        Ok(())
    }
    pub fn shared_files(&self) -> &[PathBuf] {
        &self.shared
    }
    pub fn pairing_url(&self) -> Option<String> {
        self.logs().into_iter().rev().find_map(|line| {
            line.split_once("TapSend pairing URL: ")
                .map(|(_, value)| value.trim().to_string())
        })
    }
    pub fn qr_code(&self) -> Vec<String> {
        static QR_CACHE: Mutex<(String, Vec<String>)> = Mutex::new((String::new(), Vec::new()));
        let Some(url) = self.pairing_url() else {
            return Vec::new();
        };
        let mut cache = QR_CACHE.lock().unwrap();
        if cache.0 == url {
            return cache.1.clone();
        }
        let rows: Vec<String> = Command::new("qrencode")
            .args(["-t", "UTF8", "-m", "1", &url])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        *cache = (url, rows.clone());
        rows
    }
}
impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
