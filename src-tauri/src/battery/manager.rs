//! Polling manager: one background thread that owns the HID API and all providers.
//!
//! The thread sleeps in `recv_timeout` until the next device is due or a command arrives — no
//! busy loops, no async runtime. Providers are polled one after another; a provider that fails or
//! panics only affects its own snapshot.
use super::{
    now_ms,
    provider::{EventSource, Found, Outcome, Poll, Provider},
    report,
    scanner::{connection_of, Reading, Scanned, Scanner},
    DeviceKind, DeviceSnapshot, Status,
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
const MAX_SLOTS: usize = 64;
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
    /// Drop a discovered device that is not connected.
    Forget(String),
    /// Describe connected hardware for a device request.
    Report(Sender<String>),
    /// A device event listener stopped (device unplugged).
    ListenerEnded(&'static str),
    Shutdown,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Connected { id: String, name: String },
    Disconnected { id: String, name: String },
    /// `level` is ready to show, e.g. "12%" or "Low".
    Low { id: String, name: String, level: String },
}

pub type Publish = Box<dyn Fn(&[DeviceSnapshot], Vec<Event>) + Send>;

pub struct Manager {
    tx: Sender<Cmd>,
    snapshots: Arc<Mutex<Vec<DeviceSnapshot>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Manager {
    pub fn start(providers: Vec<Box<dyn Provider>>, scanners: Vec<Box<dyn Scanner>>, settings: &Settings, publish: Publish) -> Self {
        let interval = interval_of(settings);
        let mut slots: Vec<Slot> = providers.into_iter().map(|p| Slot::new(p, settings)).collect();
        // Devices remembered from earlier runs stay listed until they are seen again or forgotten.
        for (id, cfg) in &settings.devices {
            if let (Some(_), Some(known)) = (connection_of(id), &cfg.known) {
                slots.push(Slot::discovered(id.clone(), known.name.clone(), known.kind, cfg.clone()));
            }
        }
        let snapshots = Arc::new(Mutex::new(slots.iter().map(|s| s.snap.clone()).collect()));
        let settings = settings.clone();
        let (tx, rx) = channel();
        let shared = snapshots.clone();
        let worker_tx = tx.clone();
        let thread = std::thread::Builder::new()
            .name("battery-manager".into())
            .spawn(move || {
                let mut w = Worker { slots, scanners, settings, scan_due: Some(Instant::now()), interval, api: None, rescan_at: None, publish, shared, tx: worker_tx };
                loop {
                    let now = Instant::now();
                    let wake = w.slots.iter().filter_map(|s| s.next_due).chain(w.rescan_at).chain(w.scan_due).min();
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
    /// `None` for devices found by a scanner.
    provider: Option<Box<dyn Provider>>,
    detected_kind: DeviceKind,
    default_connection: &'static str,
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
        let mut slot = Self::blank(provider.id().into(), provider.default_name().into(), provider.kind(), provider.connection(), cfg);
        slot.next_due = slot.cfg.enabled.then(Instant::now);
        slot.provider = Some(provider);
        slot
    }

    fn discovered(id: String, name: String, kind: DeviceKind, cfg: DeviceSettings) -> Self {
        let connection = connection_of(&id).unwrap_or("");
        let mut slot = Self::blank(id, name, kind, connection, cfg);
        slot.snap.known = true;
        slot.snap.discovered = true;
        if slot.cfg.enabled {
            slot.snap.status = Status::Disconnected;
        }
        slot
    }

    fn blank(id: String, default_name: String, kind: DeviceKind, connection: &'static str, cfg: DeviceSettings) -> Self {
        let snap = DeviceSnapshot {
            id,
            name: cfg.name.clone().unwrap_or_else(|| default_name.clone()),
            default_name,
            known: cfg.known.is_some(),
            discovered: false,
            kind: cfg.kind.unwrap_or(kind),
            connection,
            status: if cfg.enabled { Status::Unknown } else { Status::Disabled },
            percent: None,
            level: None,
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
        Self { provider: None, detected_kind: kind, default_connection: connection, cfg, snap, next_due: None, last_poll: None, failures: 0, linked: None, low_alerted: false, signature: String::new(), listening: false }
    }

    /// Folds a poll result into the snapshot; returns notification events and the next delay.
    fn apply(&mut self, poll: Poll, now: u64, interval: Duration) -> (Vec<Event>, Duration) {
        let s = &mut self.snap;
        let before = (s.status, s.percent, s.level);
        s.last_attempt = Some(now);
        if let Some(f) = &poll.found {
            s.hardware = Some(f.hardware.clone());
            s.product = Some(f.product.clone());
            s.source = f.source.clone();
            s.connection = f.connection.unwrap_or(self.default_connection);
            s.known = true;
        }
        let delay = match &poll.outcome {
            Outcome::Battery { percent, charging, millivolts } => {
                s.status = Status::Online;
                s.percent = Some(*percent);
                s.level = None;
                s.charging = *charging;
                s.millivolts = *millivolts;
                s.last_update = Some(now);
                s.error = None;
                self.failures = 0;
                interval
            }
            Outcome::Step(level) => {
                s.status = Status::Online;
                s.percent = None;
                s.level = Some(*level);
                s.charging = false;
                s.last_update = Some(now);
                s.error = None;
                self.failures = 0;
                interval
            }
            Outcome::NotFound => {
                s.status = Status::Disconnected;
                s.connection = self.default_connection;
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
        if (s.status, s.percent, s.level) != before {
            match (s.status, s.percent) {
                (Status::Online, None) => log::info!("[{}] battery {:?}", s.id, s.level),
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
                    Event::Connected { id: s.id.clone(), name: name.clone() }
                } else {
                    Event::Disconnected { id: s.id.clone(), name: name.clone() }
                });
            }
            self.linked = Some(linked);
        }
        // (is low, has recovered, text)
        let state = match &poll.outcome {
            Outcome::Battery { percent, charging, .. } => {
                let threshold = self.cfg.low_threshold;
                Some((*percent <= threshold && !charging, *charging || *percent >= threshold.saturating_add(LOW_HYSTERESIS), format!("{percent}%")))
            }
            Outcome::Step(level) => Some((level.is_low(), !level.is_low(), level.label().to_owned())),
            _ => None,
        };
        if let (Some((low, recovered, level)), true) = (state, self.cfg.notify_low) {
            if low {
                if !self.low_alerted {
                    events.push(Event::Low { id: s.id.clone(), name, level });
                }
                self.low_alerted = true;
            } else if recovered {
                self.low_alerted = false;
            }
        }
        (events, delay)
    }

    /// Follows a name or type that changed in Windows.
    fn rename(&mut self, name: &str, kind: DeviceKind) {
        self.detected_kind = kind;
        self.snap.kind = self.cfg.kind.unwrap_or(kind);
        if self.snap.default_name != name {
            self.snap.default_name = name.to_owned();
            self.snap.name = self.cfg.name.clone().unwrap_or_else(|| name.to_owned());
        }
    }

    fn configure(&mut self, cfg: DeviceSettings, interval: Duration, now: Instant) {
        let was_enabled = self.cfg.enabled;
        if cfg.product_id != self.cfg.product_id {
            if let Some(p) = &mut self.provider {
                p.set_product_filter(parse_pid(&cfg.product_id));
                self.next_due = Some(now);
            }
        }
        if cfg.low_threshold != self.cfg.low_threshold {
            self.low_alerted = false;
        }
        self.snap.name = cfg.name.clone().unwrap_or_else(|| self.snap.default_name.clone());
        self.snap.kind = cfg.kind.unwrap_or(self.detected_kind);
        self.snap.known |= cfg.known.is_some();
        self.snap.enabled = cfg.enabled;
        match (was_enabled, cfg.enabled) {
            (true, false) => {
                self.snap.status = Status::Disabled;
                self.next_due = None;
                self.linked = None;
            }
            (false, true) => {
                self.snap.status = Status::Unknown;
                // Discovered devices are refreshed by the next scan.
                self.next_due = self.provider.is_some().then_some(now);
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
    scanners: Vec<Box<dyn Scanner>>,
    scan_due: Option<Instant>,
    /// Current settings, needed for devices that appear later.
    settings: Settings,
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
                    if s.provider.is_some() {
                        s.next_due = Some(now);
                    } else {
                        self.scan_due = Some(now);
                    }
                }
                if id.is_none() {
                    self.scan_due = Some(now);
                }
            }
            Cmd::RefreshIfStale(age) => {
                // Reading what Windows already knows is cheap and sends nothing to the devices.
                self.scan_due = Some(now);
                for s in self.slots.iter_mut().filter(|s| s.cfg.enabled && s.provider.is_some()) {
                    if s.last_poll.map_or(true, |t| now.duration_since(t) >= age) {
                        s.next_due = Some(now);
                    }
                }
            }
            Cmd::DevicesChanged => self.rescan_at = Some(now + RESCAN_DEBOUNCE),
            Cmd::Resumed => {
                log::info!("[battery] system resumed, re-reading devices");
                self.api = None; // handles may be stale after sleep; enumerate from scratch
                self.scan_due = Some(now + RESUME_DELAY);
                for s in self.slots.iter_mut().filter(|s| s.cfg.enabled && s.provider.is_some()) {
                    s.next_due = Some(now + RESUME_DELAY);
                }
            }
            Cmd::Configure(settings) => {
                self.interval = interval_of(&settings);
                for s in &mut self.slots {
                    let cfg = settings.device(&s.snap.id);
                    s.configure(cfg, self.interval, now);
                }
                self.settings = *settings;
                self.scan_due = Some(now);
                self.publish(Vec::new());
            }
            Cmd::RefreshIn(id, delay) => {
                if let Some(s) = self.slots.iter_mut().find(|s| s.cfg.enabled && s.snap.id == id) {
                    let at = now + delay;
                    s.next_due = Some(s.next_due.map_or(at, |d| d.min(at)));
                }
            }
            Cmd::Forget(id) => {
                self.slots.retain(|s| s.snap.id != id || s.provider.is_some() || s.snap.status == Status::Online);
                if !self.slots.iter().any(|s| s.snap.id == id) {
                    self.settings.devices.remove(&id);
                }
                self.publish(Vec::new());
            }
            Cmd::Report(reply) => {
                let hid = match self.ensure_api() {
                    Ok(()) => self.api.as_ref().map(report::describe_hid).unwrap_or_default(),
                    Err(e) => format!("HID unavailable: {e}\n"),
                };
                let found: Vec<Scanned> = self.scan_all();
                let _ = reply.send(hid + &report::describe_scanned(&found));
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
            // A Bluetooth device or controller may have come or gone as well.
            self.scan_due = Some(now);
            if self.ensure_api().is_ok() {
                let api = self.api.as_ref().expect("api");
                for s in &mut self.slots {
                    let Some(sig) = s.provider.as_ref().map(|p| p.signature(api)) else { continue };
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
        let scan = self.scan_due.is_some_and(|t| t <= now);
        if due.is_empty() && !scan {
            return;
        }
        let interval = self.interval;
        let mut events = Vec::new();
        if scan {
            self.scan_due = Some(now + interval);
            events.extend(self.scan());
        }
        if due.is_empty() {
            self.publish(events);
            return;
        }
        match self.ensure_api() {
            Ok(_) => {
                for i in due {
                    let api = self.api.as_ref().expect("api");
                    let slot = &mut self.slots[i];
                    let Some(provider) = &mut slot.provider else {
                        slot.next_due = None;
                        continue;
                    };
                    let started = Instant::now();
                    let poll = catch_unwind(AssertUnwindSafe(|| provider.poll(api))).unwrap_or_else(|_| {
                        log::error!("[{}] provider panicked", slot.snap.id);
                        Poll { found: None, outcome: Outcome::Failed("internal provider error".into()) }
                    });
                    log::debug!("[{}] poll took {} ms: {:?}", slot.snap.id, started.elapsed().as_millis(), poll.outcome);
                    let (ev, delay) = slot.apply(poll, now_ms(), interval);
                    slot.last_poll = Some(Instant::now());
                    slot.next_due = Some(Instant::now() + delay);
                    events.extend(ev);
                    let Some(provider) = &slot.provider else { continue };
                    slot.signature = provider.signature(api);
                    if !slot.listening && matches!(slot.snap.status, Status::Online | Status::Off) {
                        if let Some(src) = provider.event_source(api) {
                            slot.listening = spawn_listener(provider.id(), src, self.tx.clone());
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

    fn scan_all(&mut self) -> Vec<Scanned> {
        let mut all = Vec::new();
        for s in &mut self.scanners {
            match catch_unwind(AssertUnwindSafe(|| s.scan(&all))) {
                Ok(found) => all.extend(found),
                Err(_) => log::error!("[{}] scanner panicked", s.prefix()),
            }
        }
        all
    }

    /// Updates discovered devices; devices that show a battery level for the first time are added.
    fn scan(&mut self) -> Vec<Event> {
        let started = Instant::now();
        let found = self.scan_all();
        log::debug!("[scan] {} devices in {} ms", found.len(), started.elapsed().as_millis());
        for f in found.iter().filter(|f| f.connected && f.reading.is_some()) {
            if self.slots.len() < MAX_SLOTS && !self.slots.iter().any(|s| s.snap.id == f.id) {
                log::info!("[scan] new device {} ({:?})", f.id, f.kind);
                self.slots.push(Slot::discovered(f.id.clone(), f.name.clone(), f.kind, self.settings.device(&f.id)));
            }
        }
        let mut events = Vec::new();
        for slot in self.slots.iter_mut().filter(|s| s.provider.is_none() && s.cfg.enabled) {
            let source = self.scanners.iter().find(|s| slot.snap.id.starts_with(s.prefix())).map_or("", |s| s.source());
            let poll = match found.iter().find(|f| f.id == slot.snap.id) {
                Some(f) if f.connected => {
                    slot.rename(&f.name, f.kind);
                    let outcome = match f.reading {
                        Some(Reading::Percent(percent)) => Outcome::Battery { percent, charging: false, millivolts: None },
                        Some(Reading::Step(level)) => Outcome::Step(level),
                        None => Outcome::Asleep,
                    };
                    Poll { found: Some(Found { product: f.name.clone(), hardware: String::new(), source: source.into(), connection: None }), outcome }
                }
                _ => Poll::not_found(),
            };
            events.extend(slot.apply(poll, now_ms(), self.interval).0);
            slot.snap.hardware = None;
        }
        events
    }

    fn publish(&self, events: Vec<Event>) {
        let mut snaps: Vec<DeviceSnapshot> = self.slots.iter().map(|s| s.snap.clone()).collect();
        snaps.sort_by(|a, b| (a.kind, a.discovered, &a.id).cmp(&(b.kind, b.discovered, &b.id)));
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

    struct FakeScanner(Arc<Mutex<Vec<Scanned>>>);
    impl Scanner for FakeScanner {
        fn prefix(&self) -> &'static str {
            "bt:"
        }
        fn source(&self) -> &'static str {
            "test"
        }
        fn scan(&mut self, _: &[Scanned]) -> Vec<Scanned> {
            self.0.lock().unwrap().clone()
        }
    }

    fn worker(found: Arc<Mutex<Vec<Scanned>>>) -> Worker {
        Worker {
            slots: Vec::new(),
            scanners: vec![Box::new(FakeScanner(found))],
            scan_due: None,
            settings: Settings::default(),
            interval: MIN5,
            api: None,
            rescan_at: None,
            publish: Box::new(|_, _| {}),
            shared: Arc::new(Mutex::new(Vec::new())),
            tx: channel().0,
        }
    }

    fn earbuds(connected: bool, reading: Option<Reading>) -> Scanned {
        Scanned { id: "bt:1".into(), name: "Earbuds".into(), kind: DeviceKind::Headset, connected, reading }
    }

    #[test]
    fn discovered_devices_come_and_go() {
        let found = Arc::new(Mutex::new(vec![earbuds(false, Some(Reading::Percent(70)))]));
        let mut w = worker(found.clone());
        w.scan();
        assert!(w.slots.is_empty(), "a device that is not connected is not added");
        *found.lock().unwrap() = vec![earbuds(true, Some(Reading::Percent(70)))];
        assert!(w.scan().is_empty(), "no notification for the first sight");
        assert_eq!((w.slots[0].snap.status, w.slots[0].snap.percent, w.slots[0].snap.known), (Status::Online, Some(70), true));
        *found.lock().unwrap() = Vec::new();
        assert!(matches!(w.scan().as_slice(), [Event::Disconnected { .. }]));
        assert_eq!((w.slots[0].snap.status, w.slots[0].snap.percent), (Status::Disconnected, Some(70)), "last value is kept");
        *found.lock().unwrap() = vec![earbuds(true, None)];
        w.scan();
        assert_eq!((w.slots[0].snap.status, w.slots[0].snap.percent), (Status::Asleep, Some(70)), "no level means no invented value");
    }

    #[test]
    fn only_absent_discovered_devices_can_be_forgotten() {
        let found = Arc::new(Mutex::new(vec![earbuds(true, Some(Reading::Percent(70)))]));
        let mut w = worker(found.clone());
        w.slots.push(slot());
        w.scan();
        w.handle(Cmd::Forget("bt:1".into()));
        w.handle(Cmd::Forget("wlmouse".into()));
        assert_eq!(w.slots.len(), 2, "connected and built-in devices stay");
        found.lock().unwrap().clear();
        w.scan();
        w.handle(Cmd::Forget("bt:1".into()));
        assert_eq!(w.slots.len(), 1);
    }

    #[test]
    fn coarse_levels_alert_once() {
        use crate::battery::Level;
        let mut s = slot();
        let low = |s: &mut Slot, l| s.apply(outcome(Outcome::Step(l)), 0, MIN5).0.len();
        assert_eq!(low(&mut s, Level::Medium), 0);
        assert_eq!(low(&mut s, Level::Low), 1);
        assert_eq!(low(&mut s, Level::Empty), 0);
        assert_eq!(low(&mut s, Level::Full), 0);
        assert_eq!((s.snap.percent, s.snap.level), (None, Some(Level::Full)));
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
