use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SendState {
    Publishing,
    WaitingForPhone,
    WaitingForChoice,
    Declined,
    Revoked,
    Available,
    Downloading(u8),
    AwaitingReceipt,
    Saved,
    Unconfirmed,
    Interrupted,
    PublishFailed(String),
}

pub(crate) struct QueueRow {
    pub path: PathBuf,
    pub bytes: u64,
    pub state: SendState,
    pub recipient: String,
}

struct Share {
    request: u64,
    path: PathBuf,
    file: Option<String>,
    bytes: u64,
    error: Option<String>,
    decision: Option<SendState>,
    target: Option<(String, String)>,
    receivers: HashMap<String, Receiver>,
}

#[derive(Default)]
struct Receiver {
    active: HashMap<u64, Option<(u64, u64)>>,
    served: Vec<(u64, u64)>,
    empty_served: bool,
    interrupted: bool,
    saved: bool,
    unconfirmed: bool,
}

// Use byte ranges rather than summing requests. The browser downloads in
// parallel and can fetch the same range again after a retry.
fn merge_ranges(mut ranges: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    ranges.sort_unstable();
    let mut merged: Vec<(u64, u64)> = Vec::new();
    for (start, end) in ranges {
        if let Some(last) = merged.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
}

impl Share {
    fn state(&self) -> SendState {
        if let Some(error) = &self.error {
            return SendState::PublishFailed(error.clone());
        }
        if self.file.is_none() {
            return SendState::Publishing;
        }
        if self.receivers.values().any(|receiver| receiver.saved) {
            return SendState::Saved;
        }
        if let Some(decision) = &self.decision {
            return decision.clone();
        }
        let mut best = None;
        for receiver in self
            .receivers
            .values()
            .filter(|receiver| !receiver.active.is_empty())
        {
            let mut ranges = receiver.served.clone();
            ranges.extend(receiver.active.values().filter_map(|range| *range));
            let done: u64 = merge_ranges(ranges)
                .into_iter()
                .map(|(start, end)| end - start)
                .sum();
            let percent = (done.saturating_mul(100) / self.bytes.max(1)).min(100) as u8;
            best = Some(best.unwrap_or(0).max(percent));
        }
        if let Some(percent) = best {
            return SendState::Downloading(percent);
        }
        if self.receivers.values().any(|receiver| receiver.unconfirmed) {
            return SendState::Unconfirmed;
        }
        if self.receivers.values().any(|receiver| {
            !receiver.interrupted
                && (receiver.empty_served || receiver.served.first() == Some(&(0, self.bytes)))
        }) {
            return SendState::AwaitingReceipt;
        }
        if self.receivers.values().any(|receiver| receiver.interrupted) {
            return SendState::Interrupted;
        }
        SendState::Available
    }
}

#[derive(Default)]
pub(super) struct SendQueue {
    next: u64,
    shares: Vec<Share>,
    published: VecDeque<PathBuf>,
}

impl SendQueue {
    // None means the file is already published or awaiting a reply.
    #[cfg(test)]
    pub fn stage(&mut self, path: &Path, bytes: u64) -> Option<u64> {
        self.stage_to(path, bytes, None)
    }

    pub fn stage_to(
        &mut self,
        path: &Path,
        bytes: u64,
        recipient: Option<(&str, &str)>,
    ) -> Option<u64> {
        let matches_target = |entry: &Share| {
            entry.path == path
                && entry.target.as_ref().map(|(id, _)| id.as_str()) == recipient.map(|(id, _)| id)
        };
        if let Some(entry) = self.shares.iter().find(|entry| {
            matches_target(entry)
                && !matches!(
                    entry.state(),
                    SendState::Declined
                        | SendState::Revoked
                        | SendState::Saved
                        | SendState::Unconfirmed
                        | SendState::PublishFailed(_)
                )
        }) {
            if entry.file.is_some() {
                self.published.push_back(entry.path.clone());
            }
            return None;
        }
        self.next += 1;
        let share = Share {
            request: self.next,
            path: path.into(),
            file: None,
            bytes,
            error: None,
            decision: None,
            target: recipient.map(|(id, name)| (id.into(), name.into())),
            receivers: HashMap::new(),
        };
        if let Some(entry) = self.shares.iter_mut().find(|entry| matches_target(entry)) {
            *entry = share;
        } else {
            self.shares.push(share);
        }
        Some(self.next)
    }

    pub fn fail(&mut self, request: u64, error: String) {
        if let Some(share) = self
            .shares
            .iter_mut()
            .find(|share| share.request == request)
        {
            share.error = Some(error);
        }
    }

    pub fn take_published(&mut self) -> Vec<PathBuf> {
        self.published.drain(..).collect()
    }

    pub fn rows(&self) -> Vec<QueueRow> {
        self.shares
            .iter()
            .map(|share| QueueRow {
                path: share.path.clone(),
                bytes: share.bytes,
                state: share.state(),
                recipient: share
                    .target
                    .as_ref()
                    .map(|(_, name)| name.clone())
                    .unwrap_or_default(),
            })
            .collect()
    }

    pub fn apply(&mut self, line: &str) {
        let Some((kind, body)) = line.split_once(' ') else {
            return;
        };
        if !matches!(
            kind,
            "TUI_SHARE" | "TUI_JOB" | "TUI_RECEIPT" | "TUI_OFFER" | "TUI_RESTORE"
        ) {
            return;
        }
        let fields: HashMap<_, _> = body
            .split_whitespace()
            .filter_map(|field| field.split_once('='))
            .collect();
        let get = |key| fields.get(key).copied().unwrap_or("");
        let state = get("state");
        if kind == "TUI_RESTORE" {
            let (Some(path), Some(peer), Some(name), Ok(bytes)) = (
                crate::app::hex_decode(get("path")),
                crate::app::hex_decode(get("peer")),
                crate::app::hex_decode(get("name")),
                get("size").parse(),
            ) else {
                return;
            };
            let file = get("file");
            if file.is_empty()
                || self
                    .shares
                    .iter()
                    .any(|share| share.file.as_deref() == Some(file))
            {
                return;
            }
            self.shares.push(Share {
                request: 0,
                path: path.into(),
                file: Some(file.into()),
                bytes,
                error: None,
                target: Some((peer, name.chars().filter(|ch| !ch.is_control()).collect())),
                decision: match state {
                    "waiting" => Some(SendState::WaitingForPhone),
                    "pending" => Some(SendState::WaitingForChoice),
                    "declined" => Some(SendState::Declined),
                    "revoked" => Some(SendState::Revoked),
                    _ => None,
                },
                receivers: HashMap::new(),
            });
            if matches!(state, "received" | "unconfirmed" | "failed" | "cancelled") {
                let receiver = self
                    .shares
                    .last_mut()
                    .unwrap()
                    .receivers
                    .entry(get("peer").into())
                    .or_default();
                receiver.saved = state == "received";
                receiver.unconfirmed = state == "unconfirmed";
                receiver.interrupted = matches!(state, "failed" | "cancelled");
            }
            return;
        }
        if kind == "TUI_SHARE" {
            let Ok(request) = get("request").parse() else {
                return;
            };
            let Some(share) = self
                .shares
                .iter_mut()
                .find(|share| share.request == request)
            else {
                return;
            };
            if matches!(state, "available" | "pending" | "waiting") && !get("file").is_empty() {
                share.file = Some(get("file").into());
                share.decision = match state {
                    "pending" => Some(SendState::WaitingForChoice),
                    "waiting" => Some(SendState::WaitingForPhone),
                    _ => None,
                };
                share.bytes = get("size").parse().unwrap_or(share.bytes);
                self.published.push_back(share.path.clone());
            } else if state == "failed" {
                share.error = Some(decode_error(get("error")));
            }
            return;
        }
        let file = get("file");
        let peer = get("peer");
        if file.is_empty() || peer.is_empty() {
            return;
        }
        let Some(share) = self
            .shares
            .iter_mut()
            .find(|share| share.file.as_deref() == Some(file))
        else {
            return;
        };
        if let Some((target, _)) = &share.target
            && target
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                != peer
        {
            return;
        }
        if kind == "TUI_OFFER" {
            share.decision = match state {
                "seen" => Some(SendState::WaitingForChoice),
                "accepted" => None,
                "declined" => Some(SendState::Declined),
                "revoked" => Some(SendState::Revoked),
                "changed" => Some(SendState::PublishFailed(
                    "Original file changed or moved".into(),
                )),
                _ => return,
            };
            return;
        }
        let receiver = share.receivers.entry(peer.into()).or_default();
        if kind == "TUI_RECEIPT" {
            match state {
                "received" => receiver.saved = true,
                "unconfirmed" => receiver.unconfirmed = true,
                "failed" | "cancelled" => {
                    receiver.active.clear();
                    receiver.interrupted = true;
                }
                _ => {}
            }
            return;
        }
        if get("dir") != "TX" {
            return;
        }
        let Ok(job) = get("id").parse() else { return };
        let done = get("done").parse::<u64>().unwrap_or(0);
        let range = get("offset").parse::<u64>().ok().and_then(|offset| {
            let end = offset.saturating_add(done).min(share.bytes);
            (offset < end).then_some((offset, end))
        });
        if state == "active" {
            receiver.interrupted = false;
            receiver.active.insert(job, range);
        } else {
            receiver.active.remove(&job);
            if state == "served" || state == "interrupted" {
                if state == "served" && share.bytes == 0 {
                    receiver.empty_served = true;
                }
                if let Some(range) = range {
                    receiver.served.push(range);
                    receiver.served = merge_ranges(std::mem::take(&mut receiver.served));
                }
            }
            if matches!(state, "interrupted" | "failed") {
                receiver.interrupted = true;
            }
        }
    }
}

fn decode_error(hex: &str) -> String {
    let bytes = hex
        .as_bytes()
        .chunks_exact(2)
        .filter_map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect::<Vec<_>>();
    if bytes.is_empty() {
        "Server could not share the file".into()
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_only_the_right_file_and_receipt_across_parallel_ranges() {
        let mut queue = SendQueue::default();
        assert_eq!(queue.stage(Path::new("a/file.txt"), 100), Some(1));
        assert_eq!(queue.stage(Path::new("b/file.txt"), 100), Some(2));
        queue.apply("TUI_SHARE request=1 file=aa size=100 state=available");
        queue.apply("TUI_SHARE request=2 file=bb size=100 state=available");
        assert_eq!(queue.take_published().len(), 2);
        queue.apply("TUI_JOB id=1 dir=TX file=aa peer=123 offset=0 done=50 state=served");
        queue.apply("TUI_JOB id=2 dir=TX file=aa peer=123 offset=25 done=25 state=served");
        queue.apply("TUI_JOB id=3 dir=TX file=aa peer=123 offset=50 done=25 state=active");
        assert_eq!(queue.rows()[0].state, SendState::Downloading(75));
        queue.apply("TUI_JOB id=3 dir=TX file=aa peer=123 offset=50 done=50 state=served");
        assert_eq!(queue.rows()[0].state, SendState::AwaitingReceipt);
        queue.apply("TUI_RECEIPT file=aa peer=123 state=unconfirmed");
        assert_eq!(queue.rows()[0].state, SendState::Unconfirmed);
        queue.apply("TUI_RECEIPT file=aa peer=123 state=received");
        assert_eq!(queue.rows()[0].state, SendState::Saved);
        assert_eq!(queue.rows()[1].state, SendState::Available);
    }

    #[test]
    fn tracks_only_the_selected_devices_decision_and_save() {
        let mut queue = SendQueue::default();
        let path = Path::new("file.txt");
        assert_eq!(queue.stage_to(path, 100, Some(("a", "Phone A"))), Some(1));
        assert_eq!(queue.stage_to(path, 100, Some(("b", "Phone B"))), Some(2));
        queue.apply("TUI_SHARE request=1 file=aa size=100 state=pending");
        queue.apply("TUI_SHARE request=2 file=bb size=100 state=pending");
        queue.apply("TUI_OFFER file=aa peer=62 state=accepted");
        assert_eq!(queue.rows()[0].state, SendState::WaitingForChoice);
        queue.apply("TUI_OFFER file=aa peer=61 state=declined");
        queue.apply("TUI_OFFER file=bb peer=62 state=accepted");
        queue.apply("TUI_JOB id=2 dir=TX file=bb peer=62 offset=0 done=37 state=active");
        assert_eq!(queue.rows()[0].state, SendState::Declined);
        assert_eq!(queue.rows()[1].state, SendState::Downloading(37));
        queue.apply("TUI_JOB id=2 dir=TX file=bb peer=62 offset=0 done=100 state=served");
        assert_eq!(queue.rows()[1].state, SendState::AwaitingReceipt);
        queue.apply("TUI_RECEIPT file=bb peer=61 state=received");
        assert_eq!(queue.rows()[1].state, SendState::AwaitingReceipt);
        queue.apply("TUI_RECEIPT file=bb peer=62 state=received");
        assert_eq!(queue.rows()[1].state, SendState::Saved);
    }

    #[test]
    fn restored_outbox_waits_for_the_phone_then_tracks_its_response() {
        let mut queue = SendQueue::default();
        queue.apply(
            "TUI_RESTORE file=aa peer=61 name=50686f6e65 path=766964656f size=50 state=waiting",
        );
        assert_eq!(queue.rows()[0].state, SendState::WaitingForPhone);
        queue.apply("TUI_OFFER file=aa peer=61 state=seen");
        assert_eq!(queue.rows()[0].state, SendState::WaitingForChoice);
        queue.apply("TUI_OFFER file=aa peer=61 state=accepted");
        assert_eq!(queue.rows()[0].state, SendState::Available);
    }

    #[test]
    fn failed_publication_can_be_retried_without_a_second_queue_row() {
        let mut queue = SendQueue::default();
        let path = Path::new("file.txt");
        assert_eq!(queue.stage(path, 50), Some(1));
        assert_eq!(queue.stage(path, 50), None);
        queue.apply("TUI_SHARE request=1 state=failed error=6572726f72");
        assert_eq!(
            queue.rows()[0].state,
            SendState::PublishFailed("error".into())
        );
        assert_eq!(queue.stage(path, 50), Some(2));
        assert_eq!(queue.rows().len(), 1);
    }
}
