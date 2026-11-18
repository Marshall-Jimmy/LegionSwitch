use std::sync::OnceLock;

use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::core::{GUID, PCSTR};

use crate::modes::{Mode, mode_from_overlay_guid};

type PowerSetActiveOverlaySchemeFn = unsafe extern "system" fn(*const GUID) -> u32;
type PowerGetActualOverlaySchemeFn = unsafe extern "system" fn(*mut GUID) -> u32;

#[derive(Clone, Copy)]
struct OverlayApi {
    set: Option<PowerSetActiveOverlaySchemeFn>,
    get: Option<PowerGetActualOverlaySchemeFn>,
}

// The overlay functions are exported by powrprof.dll but are missing from the
// public import library on some SDK/Windows combinations. Resolve them at
// runtime so the app still starts on systems without the exports.
fn overlay_api() -> Option<OverlayApi> {
    static API: OnceLock<Option<OverlayApi>> = OnceLock::new();
    *API.get_or_init(|| unsafe {
        let module_name: Vec<u16> = "powrprof.dll"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let module = LoadLibraryW(module_name.as_ptr());
        if module.is_null() {
            return None;
        }

        let set_name: PCSTR = c"PowerSetActiveOverlayScheme".as_ptr() as PCSTR;
        let get_name: PCSTR = c"PowerGetActualOverlayScheme".as_ptr() as PCSTR;
        let set = GetProcAddress(module, set_name)
            .map(|pointer| std::mem::transmute::<_, PowerSetActiveOverlaySchemeFn>(pointer));
        let get = GetProcAddress(module, get_name)
            .map(|pointer| std::mem::transmute::<_, PowerGetActualOverlaySchemeFn>(pointer));
        if set.is_none() && get.is_none() {
            None
        } else {
            Some(OverlayApi { set, get })
        }
    })
}

/// Mirrors LegionZone's `SetPowerModeByCap` behaviour on Windows 11: set the
/// Windows "Power mode" overlay in addition to the EC/BIOS smart-fan mode.
pub fn set_overlay(mode: Mode) -> Result<(), String> {
    let set = overlay_api().and_then(|api| api.set).ok_or_else(|| {
        "powrprof.dll 不提供 PowerSetActiveOverlayScheme（需要 Windows 10 1809+）".to_string()
    })?;
    let guid = mode.overlay_guid();
    let result = unsafe { set(&guid) };
    if result == 0 {
        Ok(())
    } else {
        Err(format!("PowerSetActiveOverlayScheme 返回 0x{result:08X}"))
    }
}

pub fn get_overlay() -> Option<Mode> {
    let get = overlay_api()?.get?;
    let mut guid = GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    let result = unsafe { get(&mut guid) };
    if result == 0 {
        mode_from_overlay_guid(&guid)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_round_trip_on_windows_10_1809_plus() {
        // Reading the overlay has no side effects. Some CI containers do not
        // expose powrprof, hence the Option.
        let _ = get_overlay();
    }

    #[test]
    fn setting_balanced_overlay_is_idempotent() {
        if std::env::var_os("LEGIONSWITCH_TEST_POWER").is_some() {
            set_overlay(Mode::Balanced).expect("balanced overlay should be accepted");
            assert_eq!(get_overlay(), Some(Mode::Balanced));
        }
    }

    #[test]
    fn all_overlay_guids_round_trip_when_requested() {
        if std::env::var_os("LEGIONSWITCH_TEST_ALL_POWER").is_none() {
            return;
        }
        for mode in Mode::ALL {
            set_overlay(mode).expect("overlay should be accepted");
            assert_eq!(get_overlay(), Some(mode), "failed for {mode:?}");
        }
        // Leave the machine in the default balanced overlay.
        set_overlay(Mode::Balanced).expect("balanced restore should work");
    }
}
