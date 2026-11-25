//! Small facts about the Windows installation.
use std::path::PathBuf;
use windows_sys::Win32::System::{
    Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ},
    SystemInformation::GetSystemWindowsDirectoryW,
};

/// The Windows folder as reported by the system (not the environment, which a user can change).
pub fn windows_dir() -> PathBuf {
    let mut buf = [0u16; 260];
    let n = unsafe { GetSystemWindowsDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if n == 0 || n >= buf.len() {
        return PathBuf::from(r"C:\Windows");
    }
    PathBuf::from(String::from_utf16_lossy(&buf[..n]))
}

pub fn windows_build() -> String {
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (key, name) = (wide(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"), wide("CurrentBuildNumber"));
    let mut buf = [0u16; 32];
    let mut size = std::mem::size_of_val(&buf) as u32;
    let rc = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut size) };
    if rc != 0 {
        return "unknown".into();
    }
    String::from_utf16_lossy(&buf[..(size as usize / 2).saturating_sub(1)])
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_windows() {
        assert!(super::windows_dir().join("explorer.exe").exists());
        assert!(super::windows_build().parse::<u32>().unwrap() >= 22000);
    }
}
