//! Windows shell integration on a dedicated UI thread:
//! - the battery strip embedded in the taskbar (child window of `Shell_TrayWnd`);
//! - a hidden top-level window that receives system broadcasts: `TaskbarCreated` (Explorer
//!   restart), `WM_DEVICECHANGE` (USB plug/unplug), `WM_POWERBROADCAST` (resume) and
//!   `WM_SETTINGCHANGE` (light/dark taskbar).
//!
//! Because the strip's parent lives in Explorer, Windows attaches the input queues of both threads.
//! This thread therefore never blocks: it only draws. All device I/O runs on the manager thread.
pub mod render;

use render::{render_strip, StripItem};
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicIsize, Ordering},
        Arc, Mutex,
    },
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{LibraryLoader::GetModuleHandleW, Registry::*},
    UI::{Accessibility::*, Controls::*, HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
};

const WM_APP_RENDER: u32 = WM_APP + 1;
const WM_APP_RELAYOUT: u32 = WM_APP + 2;
const TIMER_EMBED: usize = 1;
const EMBED_RETRY_MS: u32 = 2000;
const EMBED_MAX_ATTEMPTS: u32 = 15;
const DBT_DEVNODES_CHANGED: usize = 0x0007;
const PBT_APMRESUMEAUTOMATIC: usize = 0x0012;
const MARGIN: f32 = 4.0;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct StripModel {
    /// Draw the strip at all (taskbar indicator mode).
    pub enabled: bool,
    pub items: Vec<StripItem>,
}

pub enum ShellEvent {
    /// The strip was clicked.
    Clicked,
    DevicesChanged,
    Resumed,
    /// Taskbar theme changed (true = light).
    ThemeChanged(bool),
    /// Whether the strip is currently embedded and drawn.
    StripAvailable(bool),
}

pub type EventSink = Box<dyn Fn(ShellEvent) + Send>;

pub struct Shell {
    owner: isize,
    model: Arc<Mutex<StripModel>>,
}

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Whether Windows uses the light theme for the taskbar.
pub fn taskbar_is_light() -> bool {
    let key = w(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let name = w("SystemUsesLightTheme");
    let mut value: u32 = 0;
    let mut size = 4u32;
    let rc = unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), &mut value as *mut _ as *mut _, &mut size) };
    rc == 0 && value == 1
}

impl Shell {
    pub fn start(sink: EventSink) -> Self {
        let model = Arc::new(Mutex::new(StripModel::default()));
        let (tx, rx) = std::sync::mpsc::channel();
        let m = model.clone();
        std::thread::Builder::new()
            .name("shell".into())
            .spawn(move || unsafe { run(m, sink, tx) })
            .expect("spawn shell thread");
        let owner = rx.recv().unwrap_or(0);
        Self { owner, model }
    }

    pub fn set_model(&self, model: StripModel) {
        if let Ok(mut m) = self.model.lock() {
            if *m == model {
                return;
            }
            *m = model;
        }
        unsafe { PostMessageW(self.owner as HWND, WM_APP_RENDER, 0, 0) };
    }

    pub fn shutdown(&self) {
        unsafe { PostMessageW(self.owner as HWND, WM_CLOSE, 0, 0) };
    }
}

struct State {
    owner: HWND,
    strip: HWND,
    tooltip: HWND,
    hook: HWINEVENTHOOK,
    taskbar_created: u32,
    model: Arc<Mutex<StripModel>>,
    sink: EventSink,
    light: bool,
    embed_attempts: u32,
    available: Option<bool>,
    /// (notify area left, taskbar rect, dpi, model) of the last layout, to skip redundant redraws.
    last: Option<(i32, RECT, u32, StripModel)>,
    tool_count: usize,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}
/// Handle of `TrayNotifyWnd`, read by the WinEvent callback.
static NOTIFY_HWND: AtomicIsize = AtomicIsize::new(0);
static TASKBAR_HWND: AtomicIsize = AtomicIsize::new(0);
static OWNER_HWND: AtomicIsize = AtomicIsize::new(0);

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().and_then(|mut s| s.as_mut().map(f)))
}

unsafe fn run(model: Arc<Mutex<StripModel>>, sink: EventSink, ready: std::sync::mpsc::Sender<isize>) {
    let inst = GetModuleHandleW(std::ptr::null());
    let owner_cls = w("WhiteHubShellOwner");
    let strip_cls = w("WhiteHubBatteryStrip");
    RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(owner_proc), hInstance: inst, lpszClassName: owner_cls.as_ptr(), ..std::mem::zeroed() });
    RegisterClassW(&WNDCLASSW {
        lpfnWndProc: Some(strip_proc),
        hInstance: inst,
        lpszClassName: strip_cls.as_ptr(),
        hCursor: LoadCursorW(std::ptr::null_mut(), IDC_HAND),
        ..std::mem::zeroed()
    });
    // A hidden *top-level* window: message-only windows do not receive broadcasts.
    let owner = CreateWindowExW(WS_EX_TOOLWINDOW, owner_cls.as_ptr(), w("WhiteHub shell").as_ptr(), WS_POPUP, 0, 0, 0, 0, std::ptr::null_mut(), std::ptr::null_mut(), inst, std::ptr::null());
    OWNER_HWND.store(owner as isize, Ordering::Relaxed);
    let taskbar_created = RegisterWindowMessageW(w("TaskbarCreated").as_ptr());
    STATE.with(|s| {
        *s.borrow_mut() = Some(State {
            owner,
            strip: std::ptr::null_mut(),
            tooltip: std::ptr::null_mut(),
            hook: std::ptr::null_mut(),
            taskbar_created,
            model,
            sink,
            light: taskbar_is_light(),
            embed_attempts: 0,
            available: None,
            last: None,
            tool_count: 0,
        })
    });
    let _ = ready.send(owner as isize);
    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    STATE.with(|s| s.borrow_mut().take());
    log::info!("[shell] stopped");
}

impl State {
    fn report(&mut self, ok: bool) {
        if self.available != Some(ok) {
            self.available = Some(ok);
            log::info!("[shell] taskbar strip {}", if ok { "embedded" } else { "unavailable" });
            (self.sink)(ShellEvent::StripAvailable(ok));
        }
    }

    unsafe fn destroy_strip(&mut self) {
        if !self.hook.is_null() {
            UnhookWinEvent(self.hook);
            self.hook = std::ptr::null_mut();
        }
        if !self.tooltip.is_null() {
            DestroyWindow(self.tooltip);
            self.tooltip = std::ptr::null_mut();
        }
        if !self.strip.is_null() && IsWindow(self.strip) != 0 {
            DestroyWindow(self.strip);
        }
        self.strip = std::ptr::null_mut();
        self.last = None;
        self.tool_count = 0;
    }

    /// Creates the strip inside the current taskbar. Returns false if the taskbar is not ready.
    unsafe fn embed(&mut self) -> bool {
        let taskbar = FindWindowW(w("Shell_TrayWnd").as_ptr(), std::ptr::null());
        let notify = if taskbar.is_null() { std::ptr::null_mut() } else { FindWindowExW(taskbar, std::ptr::null_mut(), w("TrayNotifyWnd").as_ptr(), std::ptr::null()) };
        if taskbar.is_null() || notify.is_null() {
            return false;
        }
        self.destroy_strip();
        let inst = GetModuleHandleW(std::ptr::null());
        let strip = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w("WhiteHubBatteryStrip").as_ptr(),
            w("WhiteHub battery").as_ptr(),
            WS_POPUP,
            0, 0, 1, 1,
            std::ptr::null_mut(), std::ptr::null_mut(), inst, std::ptr::null(),
        );
        if strip.is_null() {
            log::error!("[shell] cannot create strip window: {}", GetLastError());
            return false;
        }
        SetWindowLongPtrW(strip, GWL_STYLE, (WS_CHILD | WS_CLIPSIBLINGS) as isize);
        if SetParent(strip, taskbar).is_null() {
            log::error!("[shell] SetParent into the taskbar failed: {}", GetLastError());
            DestroyWindow(strip);
            return false;
        }
        self.strip = strip;
        TASKBAR_HWND.store(taskbar as isize, Ordering::Relaxed);
        NOTIFY_HWND.store(notify as isize, Ordering::Relaxed);
        self.tooltip = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            TOOLTIPS_CLASSW,
            std::ptr::null(),
            WS_POPUP | TTS_ALWAYSTIP | TTS_NOPREFIX,
            CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT,
            strip, std::ptr::null_mut(), inst, std::ptr::null(),
        );
        SendMessageW(self.tooltip, TTM_SETMAXTIPWIDTH, 0, 480);
        // Follow the notification area when its width changes (language bar, new tray icons).
        let mut pid = 0u32;
        let tid = GetWindowThreadProcessId(taskbar, &mut pid);
        self.hook = SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE, std::ptr::null_mut(), Some(win_event), pid, tid, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);
        true
    }

    unsafe fn try_embed(&mut self) {
        if self.embed() {
            self.embed_attempts = 0;
            KillTimer(self.owner, TIMER_EMBED);
            self.render(true);
        } else {
            self.embed_attempts += 1;
            if self.embed_attempts >= EMBED_MAX_ATTEMPTS {
                KillTimer(self.owner, TIMER_EMBED);
                self.report(false);
            } else {
                SetTimer(self.owner, TIMER_EMBED, EMBED_RETRY_MS, None);
            }
        }
    }

    unsafe fn render(&mut self, force: bool) {
        let model = self.model.lock().map(|m| m.clone()).unwrap_or_default();
        if !model.enabled {
            if !self.strip.is_null() {
                self.destroy_strip();
            }
            return;
        }
        if self.strip.is_null() || IsWindow(self.strip) == 0 || GetParent(self.strip) != TASKBAR_HWND.load(Ordering::Relaxed) as HWND {
            self.strip = std::ptr::null_mut();
            if self.embed_attempts == 0 {
                self.try_embed();
            }
            return;
        }
        let taskbar = TASKBAR_HWND.load(Ordering::Relaxed) as HWND;
        let notify = NOTIFY_HWND.load(Ordering::Relaxed) as HWND;
        let (mut tr, mut nr) = (std::mem::zeroed::<RECT>(), std::mem::zeroed::<RECT>());
        if GetWindowRect(taskbar, &mut tr) == 0 || GetWindowRect(notify, &mut nr) == 0 {
            self.strip = std::ptr::null_mut();
            self.try_embed();
            return;
        }
        let dpi = GetDpiForWindow(taskbar).max(96);
        let key = (nr.left, tr, dpi, model.clone());
        if !force && self.last.as_ref().is_some_and(|l| l.0 == key.0 && rect_eq(&l.1, &key.1) && l.2 == key.2 && l.3 == key.3) {
            return;
        }
        let scale = dpi as f32 / 96.0;
        if model.items.is_empty() {
            ShowWindow(self.strip, SW_HIDE);
            self.last = Some(key);
            return;
        }
        let fg = if self.light { [0x1a, 0x1a, 0x1a] } else { [0xff, 0xff, 0xff] };
        let height = (tr.bottom - tr.top).max(1) as u32;
        let Some(r) = render_strip(&model.items, height, scale, fg) else {
            self.report(false);
            return;
        };
        let x = nr.left - tr.left - r.w as i32 - (MARGIN * scale) as i32;
        SetWindowPos(self.strip, HWND_TOP, x, 0, r.w as i32, r.h as i32, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        if !blit(self.strip, &r.bgra, r.w, r.h) {
            log::error!("[shell] UpdateLayeredWindow failed: {}", GetLastError());
            self.report(false);
            return;
        }
        self.update_tooltips(&model.items, &r.regions, r.h as i32);
        self.last = Some(key);
        self.report(true);
    }

    unsafe fn update_tooltips(&mut self, items: &[StripItem], regions: &[(i32, i32)], h: i32) {
        if self.tooltip.is_null() {
            return;
        }
        let mut ti: TTTOOLINFOW = std::mem::zeroed();
        ti.cbSize = std::mem::size_of::<TTTOOLINFOW>() as u32;
        ti.hwnd = self.strip;
        for id in 0..self.tool_count {
            ti.uId = id + 1;
            SendMessageW(self.tooltip, TTM_DELTOOLW, 0, &ti as *const _ as LPARAM);
        }
        for (i, (it, (x0, x1))) in items.iter().zip(regions).enumerate() {
            let mut text = w(&it.tooltip);
            ti.uFlags = TTF_SUBCLASS;
            ti.uId = i + 1;
            ti.rect = RECT { left: *x0, top: 0, right: *x1, bottom: h };
            ti.lpszText = text.as_mut_ptr();
            SendMessageW(self.tooltip, TTM_ADDTOOLW, 0, &ti as *const _ as LPARAM);
        }
        self.tool_count = items.len();
    }
}

fn rect_eq(a: &RECT, b: &RECT) -> bool {
    (a.left, a.top, a.right, a.bottom) == (b.left, b.top, b.right, b.bottom)
}

/// Pushes premultiplied BGRA pixels to a layered child window (keeps its current position).
unsafe fn blit(hwnd: HWND, bgra: &[u8], w: u32, h: u32) -> bool {
    let screen = GetDC(std::ptr::null_mut());
    let mem = CreateCompatibleDC(screen);
    let mut bi: BITMAPINFO = std::mem::zeroed();
    bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = w as i32;
    bi.bmiHeader.biHeight = -(h as i32);
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    let mut bits = std::ptr::null_mut();
    let dib = CreateDIBSection(screen, &bi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
    let mut ok = false;
    if !dib.is_null() && !bits.is_null() {
        std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());
        let old = SelectObject(mem, dib);
        let size = SIZE { cx: w as i32, cy: h as i32 };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
        // pptDst = null: for child windows a destination point would be parent-relative.
        ok = UpdateLayeredWindow(hwnd, screen, std::ptr::null(), &size, mem, &src, 0, &blend, ULW_ALPHA) != 0;
        SelectObject(mem, old);
        DeleteObject(dib);
    }
    DeleteDC(mem);
    ReleaseDC(std::ptr::null_mut(), screen);
    ok
}

unsafe extern "system" fn win_event(_: HWINEVENTHOOK, _: u32, hwnd: HWND, id_object: i32, _: i32, _: u32, _: u32) {
    if id_object == OBJID_WINDOW && (hwnd as isize == NOTIFY_HWND.load(Ordering::Relaxed) || hwnd as isize == TASKBAR_HWND.load(Ordering::Relaxed)) {
        PostMessageW(OWNER_HWND.load(Ordering::Relaxed) as HWND, WM_APP_RELAYOUT, 0, 0);
    }
}

unsafe extern "system" fn owner_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let taskbar_created = with_state(|s| s.taskbar_created).unwrap_or(0);
    match msg {
        WM_APP_RENDER => {
            with_state(|s| s.render(true));
            0
        }
        WM_APP_RELAYOUT => {
            // Coalesce bursts of location events.
            let mut m: MSG = std::mem::zeroed();
            while PeekMessageW(&mut m, hwnd, WM_APP_RELAYOUT, WM_APP_RELAYOUT, PM_REMOVE) != 0 {}
            with_state(|s| s.render(false));
            0
        }
        m if m == taskbar_created && m != 0 => {
            log::info!("[shell] taskbar was recreated (Explorer restart)");
            with_state(|s| {
                s.strip = std::ptr::null_mut();
                s.hook = std::ptr::null_mut();
                s.tooltip = std::ptr::null_mut();
                s.embed_attempts = 0;
                SetTimer(s.owner, TIMER_EMBED, EMBED_RETRY_MS, None);
            });
            0
        }
        WM_TIMER if wp == TIMER_EMBED => {
            with_state(|s| {
                let enabled = s.model.lock().map(|m| m.enabled).unwrap_or(false);
                if enabled { s.try_embed() } else { KillTimer(s.owner, TIMER_EMBED); }
            });
            0
        }
        WM_DEVICECHANGE => {
            if wp == DBT_DEVNODES_CHANGED {
                with_state(|s| (s.sink)(ShellEvent::DevicesChanged));
            }
            1
        }
        WM_POWERBROADCAST => {
            if wp == PBT_APMRESUMEAUTOMATIC {
                with_state(|s| (s.sink)(ShellEvent::Resumed));
            }
            1
        }
        WM_SETTINGCHANGE => {
            let light = taskbar_is_light();
            with_state(|s| {
                if s.light != light {
                    s.light = light;
                    (s.sink)(ShellEvent::ThemeChanged(light));
                    s.render(true);
                } else {
                    s.render(false);
                }
            });
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            with_state(|s| s.render(true));
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_CLOSE => {
            with_state(|s| s.destroy_strip());
            DestroyWindow(hwnd);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe extern "system" fn strip_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_LBUTTONUP | WM_RBUTTONUP => {
            with_state(|s| (s.sink)(ShellEvent::Clicked));
            0
        }
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
