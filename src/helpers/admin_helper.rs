//! Require an elevated process — show a clear popup if not running as Administrator.

#[cfg(windows)]
use windows::Win32::Foundation::CloseHandle;
#[cfg(windows)]
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        unsafe {
            let mut token = windows::Win32::Foundation::HANDLE::default();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
                return false;
            }
            let mut elevation = TOKEN_ELEVATION::default();
            let mut size = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                Some(std::ptr::addr_of_mut!(elevation).cast()),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            );
            let _ = CloseHandle(token);
            ok.is_ok() && elevation.TokenIsElevated != 0
        }
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// If not elevated: show a MessageBox and exit the process.
pub fn require_administrator() {
    if is_elevated() {
        return;
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};

        let title: Vec<u16> = std::ffi::OsStr::new("BeAced")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let body: Vec<u16> = std::ffi::OsStr::new(
            "BeAced must be run as Administrator.\n\n\
Right-click segotep-be-aced.exe → Run as administrator.\n\n\
Administrator rights are required for CPU temperature / power sensors.",
        )
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

        unsafe {
            let _ = MessageBoxW(
                None,
                PCWSTR(body.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONWARNING,
            );
        }
    }

    std::process::exit(1);
}
