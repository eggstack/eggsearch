use std::sync::Arc;
use std::time::{Duration, Instant};

use rmcp::model::{ProgressNotificationParam, ProgressToken};
use rmcp::service::RequestContext;
use rmcp::Peer;

pub const PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(250);
pub const PROGRESS_MAX_NOTIFICATIONS: u32 = 64;
pub const PROGRESS_MAX_BYTES: usize = 4 * 1024;
pub const PROGRESS_MESSAGE_MAX_BYTES: usize = 64;

trait ProgressEmitter: Send + Sync {
    fn emit(&self, params: ProgressNotificationParam);
}

struct PeerEmitter {
    peer: Peer<rmcp::RoleServer>,
}

impl ProgressEmitter for PeerEmitter {
    fn emit(&self, params: ProgressNotificationParam) {
        let peer = self.peer.clone();
        tokio::task::spawn(async move {
            let _ = peer.notify_progress(params).await;
        });
    }
}

#[cfg(any(test, feature = "mock"))]
struct NullEmitter;

#[cfg(any(test, feature = "mock"))]
impl ProgressEmitter for NullEmitter {
    fn emit(&self, _params: ProgressNotificationParam) {}
}

#[cfg(any(test, feature = "mock"))]
impl ProgressSink {
    pub fn for_test(token: ProgressToken) -> Self {
        Self {
            token: Some(token),
            emitter: Some(Arc::new(NullEmitter)),
            started: Instant::now(),
            last_emit: None,
            emitted: 0,
            bytes: 0,
            total: None,
        }
    }
}

#[cfg(test)]
struct RecordingEmitter {
    seen: std::sync::Mutex<Vec<ProgressNotificationParam>>,
}

#[cfg(test)]
impl ProgressEmitter for RecordingEmitter {
    fn emit(&self, params: ProgressNotificationParam) {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(params);
        }
    }
}

pub struct ProgressSink {
    token: Option<ProgressToken>,
    emitter: Option<Arc<dyn ProgressEmitter>>,
    started: Instant,
    last_emit: Option<Instant>,
    emitted: u32,
    bytes: usize,
    total: Option<f64>,
}

impl Default for ProgressSink {
    fn default() -> Self {
        Self::disabled()
    }
}

impl ProgressSink {
    pub fn disabled() -> Self {
        Self {
            token: None,
            emitter: None,
            started: Instant::now(),
            last_emit: None,
            emitted: 0,
            bytes: 0,
            total: None,
        }
    }

    pub fn from_context(context: &RequestContext<rmcp::RoleServer>) -> Self {
        match context.meta.get_progress_token() {
            Some(token) => Self {
                token: Some(token),
                emitter: Some(Arc::new(PeerEmitter {
                    peer: context.peer.clone(),
                })),
                started: Instant::now(),
                last_emit: None,
                emitted: 0,
                bytes: 0,
                total: None,
            },
            None => Self::disabled(),
        }
    }

    #[cfg(test)]
    fn recording(token: ProgressToken) -> (Self, Arc<RecordingEmitter>) {
        let emitter = Arc::new(RecordingEmitter {
            seen: std::sync::Mutex::new(Vec::new()),
        });
        let sink = Self {
            token: Some(token),
            emitter: Some(emitter.clone()),
            started: Instant::now(),
            last_emit: None,
            emitted: 0,
            bytes: 0,
            total: None,
        };
        (sink, emitter)
    }

    pub fn is_enabled(&self) -> bool {
        self.token.is_some() && self.emitter.is_some()
    }

    pub fn set_total(&mut self, total: f64) {
        self.total = Some(total);
    }

    pub fn phase(&mut self, message: &str) {
        if !self.is_enabled() {
            return;
        }
        if self.emitted >= PROGRESS_MAX_NOTIFICATIONS {
            return;
        }
        if let Some(last) = self.last_emit {
            if last.elapsed() < PROGRESS_MIN_INTERVAL {
                return;
            }
        }
        let message = bounded_message(message);
        let bytes = message.len() + 16;
        if self.bytes + bytes > PROGRESS_MAX_BYTES {
            return;
        }
        let Some(token) = self.token.clone() else {
            return;
        };
        let Some(emitter) = self.emitter.clone() else {
            return;
        };
        self.emitted += 1;
        self.bytes += bytes;
        self.last_emit = Some(Instant::now());
        let mut params =
            ProgressNotificationParam::new(token, f64::from(self.emitted)).with_message(message);
        if let Some(total) = self.total {
            params = params.with_total(total);
        }
        emitter.emit(params);
    }

    pub fn dispatch(&mut self, completed: usize, total: usize) {
        self.phase(&format!("dispatch {completed}/{total}"));
    }

    pub fn emitted_count(&self) -> u32 {
        self.emitted
    }

    pub fn emitted_bytes(&self) -> usize {
        self.bytes
    }

    pub fn elapsed_since_start(&self) -> Duration {
        self.started.elapsed()
    }
}

fn bounded_message(message: &str) -> String {
    let mut out = String::with_capacity(PROGRESS_MESSAGE_MAX_BYTES);
    for ch in message.chars() {
        if !ch.is_ascii_graphic() && ch != ' ' {
            continue;
        }
        if out.len() + 1 > PROGRESS_MESSAGE_MAX_BYTES {
            break;
        }
        out.push(ch);
    }
    if out.is_empty() {
        "working".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token() -> ProgressToken {
        ProgressToken(rmcp::model::NumberOrString::String("tok".into()))
    }

    #[test]
    fn disabled_sink_emits_nothing() {
        let mut sink = ProgressSink::disabled();
        sink.phase("plan");
        assert!(!sink.is_enabled());
        assert_eq!(sink.emitted_count(), 0);
    }

    #[test]
    fn sink_requires_a_token() {
        let (mut sink, emitter) = ProgressSink::recording(token());
        assert!(sink.is_enabled());
        sink.phase("plan");
        assert_eq!(sink.emitted_count(), 1);
        let seen = emitter.seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].message.as_deref(), Some("plan"));
        assert_eq!(seen[0].progress, 1.0);
    }

    #[test]
    fn rate_limit_coalesces_to_bounded_notifications() {
        let (mut sink, emitter) = ProgressSink::recording(token());
        for i in 0..1000 {
            sink.phase(&format!("dispatch {i}/1000"));
        }
        assert!(sink.emitted_count() <= 1);
        assert_eq!(emitter.seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn messages_are_ascii_and_bounded() {
        let (mut sink, emitter) = ProgressSink::recording(token());
        let hostile = "plan \u{1b}[31mhttps://secret.example/path?token=abc\u{7f}".repeat(4);
        sink.phase(&hostile);
        let seen = emitter.seen.lock().unwrap();
        let message = seen[0].message.as_deref().unwrap();
        assert!(message.len() <= PROGRESS_MESSAGE_MAX_BYTES);
        assert!(message.is_ascii());
        assert!(!message.contains('\u{1b}'));
    }

    #[test]
    fn total_bytes_and_count_are_capped() {
        let (mut sink, emitter) = ProgressSink::recording(token());
        for i in 0..PROGRESS_MAX_NOTIFICATIONS + 16 {
            sink.last_emit = None;
            sink.phase(&format!("dispatch {i}/{}", PROGRESS_MAX_NOTIFICATIONS + 16));
        }
        assert_eq!(sink.emitted_count(), PROGRESS_MAX_NOTIFICATIONS);
        assert!(sink.emitted_bytes() <= PROGRESS_MAX_BYTES);
        assert_eq!(
            emitter.seen.lock().unwrap().len(),
            PROGRESS_MAX_NOTIFICATIONS as usize
        );
    }

    #[test]
    fn dispatch_phase_uses_fixed_vocabulary() {
        let (mut sink, emitter) = ProgressSink::recording(token());
        sink.dispatch(2, 8);
        let seen = emitter.seen.lock().unwrap();
        assert_eq!(seen[0].message.as_deref(), Some("dispatch 2/8"));
    }
}
