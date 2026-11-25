//! Polling manager: one background thread that owns the HID API and all providers.
//!
//! The thread sleeps in `recv_timeout` until the next device is due or a command arrives — no
//! busy loops, no async runtime. Providers are polled one after another; a provider that fails or
//! panics only affects its own snapshot.
use super::{
    now_ms,
    provider::{EventSource, Outcome, Poll, Provider},
    DeviceSnapshot, Status,
};
use crate::settings::{DeviceSettings, Settings};
use hidapi::HidApi;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        mpsc::{channel, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const RETRY_STEPS: [u64; 5] = [30, 60, 120, 300, 600];
const RESCAN_DEBOUNCE: Duration = Duration::from_millis(1500);
const RESUME_DELAY: Duration = Duration::from_secs(5);
const IDLE_WAIT: Duration = Duration::from_secs(3600);
/// Low-battery alerts re-arm only after the level climbs this far above the threshold.
const LOW_HYSTERESIS: u8 = 5;

pub enum Cmd {
    /// Poll now: one device, or all enabled devices.
    Refresh(Option<String>),
    /// Poll devices whose last attempt is older than the given age (popup opened).
    RefreshIfStale(Duration),
    /// USB device nodes changed (debounced presence check).
    DevicesChanged,
    /// System resumed from sleep/hibernation.
    Resumed,
    Configure(Box<Settings>),
    /// Poll one device after a short delay (pushed device event).
    RefreshIn(&'static str, Duration),
    /// A device event listener stopped (device unplugged).
    ListenerEnded(&'static str),
    Shutdown,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Connected { id: &'static str, name: String },
    Disconnected { id: &'static str, name: String },
    Low { id: &'static str, name: String, percent: u8 },
}

pub type Publish = Box<dyn Fn(&[DeviceSnapshot], Vec<Event>) + Send>;

pub struct Manager {
    tx: Sender<Cmd>,
    snapshots: Arc<Mutex<Vec<DeviceSnapshot>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Manager {
    pub fn start(providers: Vec<Box<dyn Provider>>, settings: &Settings, publish: Publish) -> Self {
        let interval = interval_of(settings);
        let slots: Vec<Slot> = providers.into_iter().map(|p| Slot::new(p, settings)).collect();
        let snapshots = Arc::new(Mutex::new(slots.iter().map(|s| s.snap.clone()).collect()));
        let (tx, rx) = channel();
        let shared = snapshots.clone();
        let worker_tx = tx.clone();
        let thread = std::thread::Builder::new()
            .name("battery-manager".into())
            .spawn(move || {
                let mut w = Worker { slots, interval, api: None, rescan_at: None, publish, shared, tx: worker_tx };
                loop {
                    let now = Instant::now();
                    let wake = w.slots.iter().filter_map(|s| s.next_due).chain(w.rescan_at).min();
                    let timeout = wake.map_or(IDLE_WAIT, |t| t.saturating_duration_since(now));
                    match rx.recv_timeout(timeout) {
                        Ok(Cmd::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                        Ok(cmd) => w.handle(cmd),
                        Err(RecvTimeoutError::Timeout) => {}
                    }
                    w.tick();
                }
                log::info!("[battery] manager stopped");
            })
            .expect("spawn battery manager");
        Self { tx, snapshots, thread: Mutex::new(Some(thread)) }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }

    pub fn snapshots(&self) -> Vec<DeviceSnapshot> {
        self.snapshots.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Stops the thread; waits at most for the poll in progress (≤ ~2 s).
    pub fn shutdown(&self) {
        let _ = self.tx.send(Cmd::Shutdown);
        if let Some(t) = self.thread.lock().ok().and_then(|mut t| t.take()) {
            let _ = t.join();
        }
    }
}

fn interval_of(s: &Settings) -> Duration {
    Duration::from_secs(u64::from(s.poll_interval_minutes) * 60)
}

fn parse_pid(s: &Option<String>) -> Option<u16> {
    s.as_deref().and_then(|p| u16::from_str_radix(p, 16).ok())
}

/// Delay before retrying a device that did not deliver a reading.
pub fn retry_delay(failures: u32, interval: Duration) -> Duration {
    let step = RETRY_STEPS[(failures.max(1) as usize - 1).min(RETRY_STEPS.len() - 1)];
    Duration::from_secs(step).min(interval)
}

struct Slot {
    provider: Box<dyn Provider>,
    cfg: DeviceSettings,
    snap: DeviceSnapshot,
    next_due: Option<Instant>,
    last_poll: Option<Instant>,
    failures: u32,
    linked: Option<bool>,
    low_alerted: bool,
    /// Matching USB devices at the last check (see `Provider::signature`).
    signature: String,
    /// A device event listener thread is running.
    listening: bool,
}

impl Slot {
    fn new(mut provider: Box<dyn Provider>, settings: &Settings) -> Self {
        let cfg = settings.device(provider.id());
        provider.set_product_filter(parse_pid(&cfg.product_id));
        let snap = DeviceSnapshot {
            id: provider.id(),
            name: cfg.name.clone().unwrap_or_else(|| provider.default_name().into()),
            kind: provider.kind(),
            connection: provider.connection(),
            status: if cfg.enabled { Status::Unknown } else { Status::Disabled },
            percent: None,
            charging: false,
            millivolts: None,
            last_update: None,
            last_attempt: None,
            source: String::new(),
            hardware: None,
            product: None,
            error: None,
            enabled: cfg.enabled,
        };
        let next_due = cfg.enabled.then(Instant::now);
        Self { provider, cfg, snap, next_due, last_poll: None, failures: 0, linked: None, low_alerted: false, signature: String::new(), listening: false }
    }

    /// Folds a poll result into the snapshot; returns notification events and the next delay.
    fn apply(&mut self, poll: Poll, now: u64, interval: Duration) -> (Vec<Event>, Duration) {
        let s = &mut self.snap;
        let before = (s.status, s.percent);
        s.last_attempt = Some(now);
        if let Some(f) = &poll.found {
            s.hardware = Some(f.hardware.clone());
            s.product = Some(f.product.clone());
            s.source = f.source.clone();
            s.connection = f.connection.unwrap_or_else(|| self.provider.connection());
        }
        let delay = match &poll.outcome {
            Outcome::Battery { percent, charging, millivolts } => {
                s.status = Status::Online;
                s.percent = Some(*percent);
                s.charging = *charging;
                s.millivolts = *millivolts;
                s.last_update = Some(now);
                s.error = None;
                self.failures = 0;
                interval
            }
            Outcome::NotFound => {
                s.status = Status::Disconnected;
                s.connection = self.provider.connection();
                s.charging = false;
                s.hardware = None;
                s.error = None;
                self.failures = 0;
                // Plug-in is detected through device-change notifications, not by polling faster.
                interval
            }
            other => {
                s.status = match other {
                    Outcome::Asleep => Status::Asleep,
                    Outcome::Off => Status::Off,
                    _ => Status::Error,
                };
                s.charging = false;
                s.error = match other {
                    Outcome::Failed(e) => Some(e.clone()),
                    _ => None,
                };
                self.failures += 1;
                retry_delay(self.failures, interval)
            }
        };
        if (s.status, s.percent) != before {
            match (s.status, s.percent) {
                (Status::Online, Some(p)) => log::info!("[{}] battery {p}%{}", s.id, if s.charging { " (charging)" } else { "" }),
                (Status::Error, _) => log::warn!("[{}] error: {}", s.id, s.error.as_deref().unwrap_or("")),
                (st, _) => log::info!("[{}] status {st:?}", s.id),
            }
        }

        let mut events = Vec::new();
        let name = s.name.clone();
        if let Some(linked) = s.status.linked() {
            if self.linked.is_some_and(|was| was != linked) {
                events.push(if linked {
                    Event::Connected { id: s.id, name: name.clone() }
                } else {
                    Event::Disconnected { id: s.id, name: name.clone() }
                });
            }
            self.linked = Some(linked);
        }
        if let (Outcome::Battery { percent, charging, .. }, true) = (&poll.outcome, self.cfg.notify_low) {
            let threshold = self.cfg.low_threshold;
            if *percent <= threshold && !charging {
                if !self.low_alerted {
                    events.push(Event::Low { id: s.id, name, percent: *percent });
                }
                self.low_alerted = true;
            } else if *charging || *percent >= threshold.saturating_add(LOW_HYSTERESIS) {
                self.low_alerted = false;
            }
        }
        (events, delay)
    }

    fn configure(&mut self, cfg: DeviceSettings, interval: Duration, now: Instant) {
        let was_enabled = self.cfg.enabled;
        if cfg.product_id != self.cfg.product_id {
            self.provider.set_product_filter(parse_pid(&cfg.product_id));
            self.next_due = Some(now);
        }
        if cfg.low_threshold != self.cfg.low_threshold {
            self.low_alerted = false;
        }
        self.snap.name = cfg.name.clone().unwrap_or_else(|| self.provider.default_name().into());
        self.snap.enabled = cfg.enabled;
        match (was_enabled, cfg.enabled) {
            (true, false) => {
                self.snap.status = Status::Disabled;
                self.next_due = None;
                self.linked = None;
            }
            (false, true) => {
                self.snap.status = Status::Unknown;
                self.next_due = Some(now);
            }
            (true, true) if self.failures == 0 => {
                // Interval change: keep the cadence relative to the last poll.
                if let (Some(last), Some(_)) = (self.last_poll, self.next_due) {
                    self.next_due = Some((last + interval).max(now));
                }
            }
            _ => {}
        }
        self.cfg = cfg;
    }
}

struct Worker {
    slots: Vec<Slot>,
    interval: Duration,
    api: Option<HidApi>,
    rescan_at: Option<Instant>,
    publish: Publish,
    shared: Arc<Mutex<Vec<DeviceSnapshot>>>,
    tx: Sender<Cmd>,
}

/// Delay between a pushed event and the poll, so the device has settled (e.g. charge state).
const EVENT_SETTLE: Duration = Duration::from_millis(1500);

/// A device that floods events must not flood the log and the command queue.
const EVENT_MIN_GAP: Duration = Duration::from_secs(1);

/// Whether an event at `now` may be handled, given the last handled one.
fn event_due(last: Option<Instant>, now: Instant) -> bool {
    last.map_or(true, |t| now.saturating_duration_since(t) >= EVENT_MIN_GAP)
}

/// Blocks on the device's input reports (no CPU while idle) and asks for a poll when the device
/// reports a change. Ends when the device goes away. Returns whether the thread started.
fn spawn_listener(id: &'static str, src: EventSource, tx: Sender<Cmd>) -> bool {
    std::thread::Builder::new()
        .name(format!("{id}-events"))
        .spawn(move || {
            log::info!("[{id}] listening for device events");
            let mut buf = [0u8; 64];
            let mut last = None;
            loop {
                match src.device.read_timeout(&mut buf, -1) {
                    // Dropping an event within the gap is safe: the poll it would request is
                    // already due after EVENT_SETTLE, which is longer than the gap.
                    Ok(n) if n > 0 && (src.is_change)(&buf[..n]) && event_due(last, Instant::now()) => {
                        last = Some(Instant::now());
                        log::info!("[{id}] device event {}", super::hex(&buf[..n.min(6)]));
                        if tx.send(Cmd::RefreshIn(id, EVENT_SETTLE)).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::info!("[{id}] event listener stopped: {e}");
                        break;
                    }
                }
            }
            let _ = tx.send(Cmd::ListenerEnded(id));
        })
        .is_ok()
}

impl Worker {
    fn handle(&mut self, cmd: Cmd) {
        let now = Instant::now();
        match cmd {
            Cmd::Refresh(id) => {
                for s in self.slots.iter_mut().filter(|s| s.cfg.enabled && id.as_deref().map_or(true, |id| id == s.snap.id)) {
                    s.next_due = Some(now);
                }
            }
            Cmd::RefreshIfStale(age) => {
                for s in self.slots.iter_mut().filter(|s| s.cfg.enabled) {
                    if s.last_poll.map_or(true, |t| now.duration_since(t) >= age) {
                        s.next_due = Some(now);
                    }
                }
            }
            Cmd::DevicesChanged => self.rescan_at = Some(now + RESCAN_DEBOUNCE),
            Cmd::Resumed => {
                log::info!("[battery] system resumed, re-reading devices");
                self.api = None; // handles may be stale after sleep; enumerate from scratch
                for s in self.slots.iter_mut().filter(|s| s.cfg.enabled) {
                    s.next_due = Some(now + RESUME_DELAY);
                }
            }
            Cmd::Configure(settings) => {
                self.interval = interval_of(&settings);
                for s in &mut self.slots {
                    let cfg = settings.device(s.snap.id);
                    s.configure(cfg, self.interval, now);
                }
                self.publish(Vec::new());
            }
            Cmd::RefreshIn(id, delay) => {
                if let Some(s) = self.slots.iter_mut().find(|s| s.cfg.enabled && s.snap.id == id) {
                    let at = now + delay;
                    s.next_due = Some(s.next_due.map_or(at, |d| d.min(at)));
                }
            }
            Cmd::ListenerEnded(id) => {
                if let Some(s) = self.slots.iter_mut().find(|s| s.snap.id == id) {
                    s.listening = false;
                }
            }
            Cmd::Shutdown => {}
        }
    }

    /// Makes sure an HID API exists and its device list is current.
    fn ensure_api(&mut self) -> Result<(), String> {
        match self.api.as_mut() {
            Some(api) => api.refresh_devices().map_err(|e| e.to_string()),
            None => {
                self.api = Some(HidApi::new().map_err(|e| e.to_string())?);
                Ok(())
            }
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        if self.rescan_at.is_some_and(|t| t <= now) {
            self.rescan_at = None;
            if self.ensure_api().is_ok() {
                let api = self.api.as_ref().expect("api");
                let sigs: Vec<String> = self.slots.iter().map(|s| s.provider.signature(api)).collect();
                for (s, sig) in self.slots.iter_mut().zip(sigs) {
                    // Receiver plugged/removed, or the mouse appeared on its charging cable.
                    if s.cfg.enabled && (sig != s.signature || s.snap.status == Status::Unknown) {
                        log::info!("[{}] USB devices changed: [{}] -> [{}]", s.snap.id, s.signature, sig);
                        s.next_due = Some(now);
                    }
                    s.signature = sig;
                }
            }
        }
        let due: Vec<usize> = (0..self.slots.len()).filter(|&i| self.slots[i].next_due.is_some_and(|t| t <= now)).collect();
        if due.is_empty() {
            return;
        }
        let interval = self.interval;
        let mut events = Vec::new();
        match self.ensure_api() {
            Ok(_) => {
                for i in due {
                    let api = self.api.as_ref().expect("api");
                    let slot = &mut self.slots[i];
                    let started = Instant::now();
                    let poll = catch_unwind(AssertUnwindSafe(|| slot.provider.poll(api))).unwrap_or_else(|_| {
                        log::error!("[{}] provider panicked", slot.snap.id);
                        Poll { found: None, outcome: Outcome::Failed("internal provider error".into()) }
                    });
                    log::debug!("[{}] poll took {} ms: {:?}", slot.snap.id, started.elapsed().as_millis(), poll.outcome);
                    let (ev, delay) = slot.apply(poll, now_ms(), interval);
                    slot.last_poll = Some(Instant::now());
                    slot.next_due = Some(Instant::now() + delay);
                    slot.signature = slot.provider.signature(api);
                    events.extend(ev);
                    if !slot.listening && matches!(slot.snap.status, Status::Online | Status::Off) {
                        if let Some(src) = slot.provider.event_source(api) {
                            slot.listening = spawn_listener(slot.snap.id, src, self.tx.clone());
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("[battery] HID API unavailable: {e}");
                self.api = None;
                for i in due {
                    let (ev, delay) = self.slots[i].apply(Poll { found: None, outcome: Outcome::Failed(format!("HID unavailable: {e}")) }, now_ms(), interval);
                    self.slots[i].next_due = Some(Instant::now() + delay);
                    events.extend(ev);
                }
            }
        }
        self.publish(events);
    }

    fn publish(&self, events: Vec<Event>) {
        let snaps: Vec<DeviceSnapshot> = self.slots.iter().map(|s| s.snap.clone()).collect();
        if let Ok(mut shared) = self.shared.lock() {
            *shared = snaps.clone();
        }
        (self.publish)(&snaps, events);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::{provider::Found, DeviceKind};

    struct Fake;
    impl Provider for Fake {
        fn id(&self) -> &'static str {
            "wlmouse"
        }
        fn default_name(&self) -> &'static str {
            "Fake Mouse"
        }
        fn kind(&self) -> DeviceKind {
            DeviceKind::Mouse
        }
        fn connection(&self) -> &'static str {
            "test"
        }
        fn set_product_filter(&mut self, _: Option<u16>) {}
        fn signature(&self, _: &HidApi) -> String {
            String::new()
        }
        fn poll(&mut self, _: &HidApi) -> Poll {
            unreachable!("not used by slot tests")
        }
    }

    const MIN5: Duration = Duration::from_secs(300);

    fn slot() -> Slot {
        Slot::new(Box::new(Fake), &Settings::default())
    }
    fn found() -> Option<Found> {
        Some(Found { product: "X".into(), hardware: "36A7:A860".into(), source: "test".into(), connection: None })
    }
    fn battery(p: u8, charging: bool) -> Poll {
        Poll { found: found(), outcome: Outcome::Battery { percent: p, charging, millivolts: None } }
    }
    fn outcome(o: Outcome) -> Poll {
        Poll { found: found(), outcome: o }
    }

    #[test]
    fn keeps_last_value_when_unreachable() {
        let mut s = slot();
        s.apply(battery(64, false), 1000, MIN5);
        let (_, delay) = s.apply(outcome(Outcome::Asleep), 2000, MIN5);
        assert_eq!(s.snap.status, Status::Asleep);
        assert_eq!(s.snap.percent, Some(64));
        assert_eq!(s.snap.last_update, Some(1000));
        assert_eq!(s.snap.last_attempt, Some(2000));
        assert_eq!(delay, Duration::from_secs(30));
        s.apply(Poll::not_found(), 3000, MIN5);
        assert_eq!(s.snap.status, Status::Disconnected);
        assert_eq!(s.snap.percent, Some(64));
    }

    #[test]
    fn backoff_grows_and_is_capped_by_interval() {
        assert_eq!(retry_delay(1, MIN5), Duration::from_secs(30));
        assert_eq!(retry_delay(2, MIN5), Duration::from_secs(60));
        assert_eq!(retry_delay(3, MIN5), Duration::from_secs(120));
        assert_eq!(retry_delay(4, MIN5), MIN5);
        assert_eq!(retry_delay(9, Duration::from_secs(3600)), Duration::from_secs(600));
        let mut s = slot();
        let delays: Vec<_> = (0..3).map(|i| s.apply(outcome(Outcome::Off), i, MIN5).1.as_secs()).collect();
        assert_eq!(delays, [30, 60, 120]);
        assert_eq!(s.apply(battery(50, false), 9, MIN5).1, MIN5);
        assert_eq!(s.failures, 0);
    }

    #[test]
    fn errors_are_reported_and_cleared() {
        let mut s = slot();
        s.apply(outcome(Outcome::Failed("boom".into())), 1, MIN5);
        assert_eq!((s.snap.status, s.snap.error.as_deref()), (Status::Error, Some("boom")));
        s.apply(battery(10, true), 2, MIN5);
        assert_eq!((s.snap.status, s.snap.error.clone()), (Status::Online, None));
    }

    #[test]
    fn connect_events_only_on_transitions() {
        let mut s = slot();
        assert!(s.apply(battery(80, false), 1, MIN5).0.is_empty(), "no event for the initial state");
        assert!(s.apply(outcome(Outcome::Asleep), 2, MIN5).0.is_empty(), "sleeping is not a disconnect");
        let (ev, _) = s.apply(Poll::not_found(), 3, MIN5);
        assert!(matches!(ev.as_slice(), [Event::Disconnected { .. }]));
        assert!(s.apply(Poll::not_found(), 4, MIN5).0.is_empty());
        let (ev, _) = s.apply(battery(80, false), 5, MIN5);
        assert!(matches!(ev.as_slice(), [Event::Connected { .. }]));
    }

    #[test]
    fn low_battery_alerts_once_with_hysteresis() {
        let mut s = slot(); // threshold 20
        let low = |s: &mut Slot, p, c| s.apply(battery(p, c), 0, MIN5).0.iter().filter(|e| matches!(e, Event::Low { .. })).count();
        assert_eq!(low(&mut s, 50, false), 0);
        assert_eq!(low(&mut s, 20, false), 1);
        assert_eq!(low(&mut s, 18, false), 0, "same low state is not repeated");
        assert_eq!(low(&mut s, 22, false), 0, "inside hysteresis band");
        assert_eq!(low(&mut s, 19, false), 0);
        assert_eq!(low(&mut s, 25, false), 0, "re-armed");
        assert_eq!(low(&mut s, 15, false), 1);
        assert_eq!(low(&mut s, 15, true), 0, "charging re-arms, no alert");
        assert_eq!(low(&mut s, 14, false), 1);
        assert_eq!(low(&mut s, 100, false), 0, "never notify at full charge");
    }

    #[test]
    fn event_floods_are_limited() {
        let t = Instant::now();
        assert!(event_due(None, t));
        assert!(!event_due(Some(t), t + Duration::from_millis(999)));
        assert!(event_due(Some(t), t + EVENT_MIN_GAP));
        assert!(EVENT_SETTLE > EVENT_MIN_GAP, "a dropped event is covered by the pending poll");
    }

    #[test]
    fn disabled_alerts_are_suppressed() {
        let mut settings = Settings::default();
        settings.devices.get_mut("wlmouse").unwrap().notify_low = false;
        let mut s = Slot::new(Box::new(Fake), &settings);
        assert!(s.apply(battery(5, false), 0, MIN5).0.is_empty());
    }

    #[test]
    fn disabling_and_enabling() {
        let mut s = slot();
        let now = Instant::now();
        let mut cfg = s.cfg.clone();
        cfg.enabled = false;
        cfg.name = Some("My mouse".into());
        s.configure(cfg.clone(), MIN5, now);
        assert_eq!((s.snap.status, s.next_due, s.snap.name.as_str()), (Status::Disabled, None, "My mouse"));
        cfg.enabled = true;
        s.configure(cfg, MIN5, now);
        assert_eq!((s.snap.status, s.next_due), (Status::Unknown, Some(now)));
    }

    #[test]
    fn interval_change_reschedules_relative_to_last_poll() {
        let mut s = slot();
        let now = Instant::now();
        s.last_poll = Some(now);
        s.next_due = Some(now + Duration::from_secs(3600));
        s.configure(s.cfg.clone(), MIN5, now);
        assert_eq!(s.next_due, Some(now + MIN5));
        s.configure(s.cfg.clone(), Duration::from_secs(1800), now);
        assert_eq!(s.next_due, Some(now + Duration::from_secs(1800)));
    }
}
