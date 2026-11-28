use std::{
    io,
    os::windows::process::CommandExt,
    process::Command,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use crate::native::CREATE_NO_WINDOW;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

enum Status {
    Idle,
    Checking,
    Current(String),
    Available {
        installed: String,
        tag: String,
        url: String,
    },
    Unavailable,
}

pub(super) struct Updates {
    status: Status,
    last_check: Option<Instant>,
    pending: Option<Receiver<Status>>,
    notified_tag: Option<String>,
}

impl Updates {
    pub(super) fn new() -> Self {
        Self {
            status: Status::Idle,
            last_check: None,
            pending: None,
            notified_tag: None,
        }
    }

    pub(super) fn check(&mut self, force: bool) {
        if self.pending.is_some()
            || (!force
                && self
                    .last_check
                    .is_some_and(|last| last.elapsed() < CHECK_INTERVAL))
        {
            return;
        }
        self.last_check = Some(Instant::now());
        self.status = Status::Checking;
        let (sender, receiver) = mpsc::channel();
        self.pending = Some(receiver);
        thread::spawn(move || {
            let result = std::env::current_exe()
                .and_then(|exe| {
                    Command::new(exe.with_file_name("tapsend.exe"))
                        .arg("update-check")
                        .creation_flags(CREATE_NO_WINDOW)
                        .output()
                })
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| parse_result(&String::from_utf8_lossy(&output.stdout)))
                .unwrap_or(Status::Unavailable);
            let _ = sender.send(result);
        });
    }

    pub(super) fn poll(&mut self) -> Option<String> {
        let status = match self.pending.as_ref()?.try_recv() {
            Ok(status) => status,
            Err(TryRecvError::Disconnected) => Status::Unavailable,
            Err(TryRecvError::Empty) => return None,
        };
        self.pending = None;
        self.status = status;
        match &self.status {
            Status::Available { tag, .. } if self.notified_tag.as_ref() != Some(tag) => {
                self.notified_tag = Some(tag.clone());
                Some(format!("TapSend {tag} is available · Press I for updates"))
            }
            _ => None,
        }
    }

    pub(super) fn available(&self) -> bool {
        matches!(self.status, Status::Available { .. })
    }

    pub(super) fn details(&self) -> (String, String, &'static str) {
        match &self.status {
            Status::Idle | Status::Checking => (
                "Checking GitHub Releases…".into(),
                String::new(),
                "[Esc] Close",
            ),
            Status::Current(installed) => (
                format!("TapSend v{installed}"),
                "No newer Windows installer found.".into(),
                "[R] Check again   [Esc] Close",
            ),
            Status::Available { installed, tag, .. } => (
                format!("TapSend {tag} is available"),
                format!("Installed v{installed}"),
                "[Enter] Open GitHub release   [R] Retry   [Esc] Close",
            ),
            Status::Unavailable => (
                "Could not reach GitHub Releases.".into(),
                "Try again when internet access is available.".into(),
                "[R] Check again   [Esc] Close",
            ),
        }
    }

    pub(super) fn open_release(&self) -> io::Result<()> {
        if let Status::Available { url, .. } = &self.status {
            crate::app::open_local_file(std::path::Path::new(url))?;
        }
        Ok(())
    }
}

fn parse_result(line: &str) -> Option<Status> {
    let fields: Vec<_> = line.trim().split('\t').collect();
    match fields.as_slice() {
        ["current", installed] if valid_version(installed) => {
            Some(Status::Current((*installed).into()))
        }
        ["available", installed, tag, url]
            if valid_version(installed)
                && tag.starts_with('v')
                && valid_version(&tag[1..])
                && valid_release_url(url, tag) =>
        {
            Some(Status::Available {
                installed: (*installed).into(),
                tag: (*tag).into(),
                url: (*url).into(),
            })
        }
        _ => None,
    }
}

fn valid_version(version: &str) -> bool {
    !version.is_empty()
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
}

fn valid_release_url(url: &str, tag: &str) -> bool {
    let Some(path) = url.strip_prefix("https://github.com/") else {
        return false;
    };
    let Some(repo) = path.strip_suffix(&format!("/releases/tag/{tag}")) else {
        return false;
    };
    let Some((owner, project)) = repo.split_once('/') else {
        return false;
    };
    [owner, project].iter().all(|part| {
        !part.is_empty()
            && *part != "."
            && *part != ".."
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_links_stay_on_github_release_pages() {
        let tag = "v1.0.3";
        assert!(matches!(
            parse_result(
                "available\t1.0.2\tv1.0.3\thttps://github.com/RDXFGXY1/TapSend/releases/tag/v1.0.3"
            ),
            Some(Status::Available { .. })
        ));
        assert!(!valid_release_url(
            "https://github.com.example.net/RDXFGXY1/TapSend/releases/tag/v1.0.3",
            tag,
        ));
        assert!(!valid_release_url(
            "https://github.com/RDXFGXY1/TapSend/releases/download/v1.0.3/setup.exe",
            tag,
        ));
    }
}
