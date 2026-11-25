//! Start with Windows through the Run key of the current user, which needs no administrator rights.
//! The uninstaller removes the entry.
use crate::system::wide;
use windows_sys::Win32::{
    Foundation::ERROR_SUCCESS,
    System::Registry::{RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ},
};

const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "WhiteHub";

fn command() -> std::io::Result<String> {
    Ok(format!("\"{}\" {}", std::env::current_exe()?.display(), crate::app::AUTOSTART_ARG))
}

fn current() -> Option<String> {
    let (key, name) = (wide(KEY), wide(VALUE));
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let rc = unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut size) };
    (rc == ERROR_SUCCESS).then(|| String::from_utf16_lossy(&buf[..(size as usize / 2).saturating_sub(1)]))
}

/// Enabled and pointing at this executable.
pub fn is_enabled() -> bool {
    command().is_ok_and(|c| current().is_some_and(|v| v.eq_ignore_ascii_case(&c)))
}

pub fn set(enabled: bool) -> Result<(), String> {
    let (key, name) = (wide(KEY), wide(VALUE));
    let rc = if enabled {
        let data = wide(&command().map_err(|e| e.to_string())?);
        unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32) }
    } else if current().is_some() {
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) }
    } else {
        ERROR_SUCCESS
    };
    if rc == ERROR_SUCCESS { Ok(()) } else { Err(format!("registry error {rc}")) }
}
