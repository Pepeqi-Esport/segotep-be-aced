//! Require an elevated process. If launched without admin, relaunch via UAC
//! (`runas`) once; if that fails / is declined, show a clear popup and exit.

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

/// If not elevated: try `runas` relaunch, otherwise MessageBox + exit.
pub fn require_administrator() {
    if is_elevated() {
        return;
    }

    #[cfg(windows)]
    {
        if try_relaunch_elevated() {
            // UAC accepted (or prompted) — elevated child continues; this process exits.
            std::process::exit(0);
        }
        show_admin_required_popup();
    }

    std::process::exit(1);
}

#[cfg(windows)]
fn try_relaunch_elevated() -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let params = std::env::args().skip(1).collect::<Vec<_>>().join(" ");

    let exe_wide: Vec<u16> = exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let params_wide: Vec<u16> = std::ffi::OsStr::new(&params)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let runas: Vec<u16> = std::ffi::OsStr::new("runas")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(runas.as_ptr()),
            PCWSTR(exe_wide.as_ptr()),
            PCWSTR(params_wide.as_ptr()),
            None,
            SW_SHOWNORMAL,
        )
    };

    // Per ShellExecute docs, values > 32 mean success (UAC shown / process started).
    result.0 as usize > 32
}

#[cfg(windows)]
fn show_admin_required_popup() {
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
For login autostart without this popup, enable \"Launch on startup\" \
while running as Administrator (uses Task Scheduler with highest privileges).\n\n\
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
