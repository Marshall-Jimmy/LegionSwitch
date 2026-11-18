use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::modes::Mode;
use crate::power;
use crate::wmi::{Status, WmiSession};

pub const WM_APP_EVENT: u32 = 0x8000 + 10;

#[derive(Debug, Clone, Copy)]
pub enum Command {
    Refresh,
    Apply { mode: Mode, sync_overlay: bool },
}

#[derive(Debug)]
pub struct ApplyOutcome {
    pub status: Status,
    pub overlay_warning: Option<String>,
}

#[derive(Debug)]
pub enum Event {
    Status(Result<Status, String>),
    Applied {
        requested: Mode,
        result: Result<ApplyOutcome, String>,
    },
}

/// One background thread owns the COM/WMI connection. The UI thread only sends
/// commands and receives results through window messages, so opening the panel
/// never blocks on ACPI/WMI.
pub fn spawn(hwnd: HWND) -> Sender<Command> {
    let (sender, receiver) = mpsc::channel::<Command>();
    let hwnd_bits = hwnd as usize;

    let _ = thread::Builder::new()
        .name("LegionSwitch-WMI".to_string())
        .spawn(move || worker_loop(hwnd_bits, receiver));

    sender
}

fn worker_loop(hwnd_bits: usize, receiver: Receiver<Command>) {
    let mut session: Option<WmiSession> = None;

    while let Ok(command) = receiver.recv() {
        let event = match command {
            Command::Refresh => {
                let result = with_session(&mut session, |session| session.status());
                Event::Status(result)
            }
            Command::Apply { mode, sync_overlay } => {
                let result = with_session(&mut session, |session| {
                    apply_with_session(session, mode, sync_overlay)
                });
                Event::Applied {
                    requested: mode,
                    result,
                }
            }
        };

        let hwnd = hwnd_bits as HWND;
        let pointer = Box::into_raw(Box::new(event)) as isize;
        let posted = unsafe { PostMessageW(hwnd, WM_APP_EVENT, 0, pointer) };
        if posted == 0 {
            // The window disappeared while the worker was running.
            unsafe {
                drop(Box::from_raw(pointer as *mut Event));
            }
            break;
        }
    }
}

fn with_session<T>(
    slot: &mut Option<WmiSession>,
    action: impl FnOnce(&WmiSession) -> Result<T, String>,
) -> Result<T, String> {
    if slot.is_none() {
        *slot = Some(WmiSession::connect()?);
    }
    action(slot.as_ref().expect("session initialized above"))
}

fn apply_with_session(
    session: &WmiSession,
    mode: Mode,
    sync_overlay: bool,
) -> Result<ApplyOutcome, String> {
    session.set_mode(mode)?;

    let status = match session.get_raw_mode() {
        Ok(raw) => session.status_with_raw(raw),
        Err(_) => Status {
            raw_mode: mode.smart_fan_value(),
            mode: Some(mode),
            telemetry: session.get_telemetry(),
            overlay: power::get_overlay(),
        },
    };

    let overlay_warning = if sync_overlay {
        power::set_overlay(mode).err()
    } else {
        None
    };

    Ok(ApplyOutcome {
        status,
        overlay_warning,
    })
}
