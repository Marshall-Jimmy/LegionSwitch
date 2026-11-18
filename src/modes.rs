use windows_sys::core::GUID;

/// The three performance profiles exposed by Lenovo's GameZone WMI interface.
///
/// Values were confirmed by decompiling `LZTray.exe` (`GameMode`):
/// `Quiet = 1`, `Balance = 2`, `Beast = 3`. The same values are passed to
/// `LENOVO_GAMEZONE_DATA.SetSmartFanMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Quiet,
    Balanced,
    Beast,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Quiet, Mode::Balanced, Mode::Beast];

    pub const fn index(self) -> usize {
        match self {
            Mode::Quiet => 0,
            Mode::Balanced => 1,
            Mode::Beast => 2,
        }
    }

    pub const fn smart_fan_value(self) -> u32 {
        match self {
            Mode::Quiet => 1,
            Mode::Balanced => 2,
            Mode::Beast => 3,
        }
    }

    pub const fn from_smart_fan_value(value: u32) -> Option<Self> {
        match value {
            1 => Some(Mode::Quiet),
            2 => Some(Mode::Balanced),
            3 => Some(Mode::Beast),
            _ => None,
        }
    }

    pub const fn label_zh_full(self) -> &'static str {
        match self {
            Mode::Quiet => "安静模式",
            Mode::Balanced => "普通模式",
            Mode::Beast => "野兽模式",
        }
    }

    pub const fn label_en(self) -> &'static str {
        match self {
            Mode::Quiet => "QUIET",
            Mode::Balanced => "BALANCED",
            Mode::Beast => "BEAST",
        }
    }

    pub const fn glyph(self) -> &'static str {
        match self {
            Mode::Quiet => "静",
            Mode::Balanced => "衡",
            Mode::Beast => "兽",
        }
    }

    pub const fn accent_rgb(self) -> (u8, u8, u8) {
        match self {
            Mode::Quiet => (0x3C, 0x8D, 0xFF),
            Mode::Balanced => (0xD7, 0xDE, 0xE8),
            Mode::Beast => (0xE2, 0x3C, 0x48),
        }
    }

    pub const fn icon_resource_id(self) -> u16 {
        match self {
            Mode::Quiet => 2,
            Mode::Balanced => 3,
            Mode::Beast => 4,
        }
    }

    /// Windows 11 "Power mode" overlay GUID. These are the same GUIDs used by
    /// LegionZone's `phwcap.dll` (`PowerSetActiveOverlayScheme`).
    pub const fn overlay_guid(self) -> GUID {
        match self {
            Mode::Quiet => GUID {
                data1: 0x961c_c777,
                data2: 0x2547,
                data3: 0x4f9d,
                data4: [0x81, 0x74, 0x7d, 0x86, 0x18, 0x1b, 0x8a, 0x7a],
            },
            Mode::Balanced => GUID {
                data1: 0,
                data2: 0,
                data3: 0,
                data4: [0; 8],
            },
            Mode::Beast => GUID {
                data1: 0xded5_74b5,
                data2: 0x45a0,
                data3: 0x4f42,
                data4: [0x87, 0x37, 0x46, 0x34, 0x5c, 0x09, 0xc2, 0x38],
            },
        }
    }

    pub const fn overlay_name_zh(self) -> &'static str {
        match self {
            Mode::Quiet => "最佳能效",
            Mode::Balanced => "平衡",
            Mode::Beast => "最佳性能",
        }
    }

    pub const fn overlay_name_en(self) -> &'static str {
        match self {
            Mode::Quiet => "EFFICIENCY",
            Mode::Balanced => "BALANCED",
            Mode::Beast => "PERFORMANCE",
        }
    }
}

pub fn mode_from_overlay_guid(guid: &GUID) -> Option<Mode> {
    Mode::ALL
        .into_iter()
        .find(|mode| guid_eq(guid, &mode.overlay_guid()))
}

pub fn guid_eq(a: &GUID, b: &GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}

pub fn label_for_raw(raw: u32) -> &'static str {
    match raw {
        1 => "安静模式",
        2 => "普通模式",
        3 => "野兽模式",
        224 => "超神模式",
        255 => "自定义模式",
        _ => "未知模式",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_values_are_stable() {
        assert_eq!(Mode::Quiet.smart_fan_value(), 1);
        assert_eq!(Mode::Balanced.smart_fan_value(), 2);
        assert_eq!(Mode::Beast.smart_fan_value(), 3);
        for mode in Mode::ALL {
            assert_eq!(
                Mode::from_smart_fan_value(mode.smart_fan_value()),
                Some(mode)
            );
        }
        assert_eq!(Mode::from_smart_fan_value(224), None);
    }

    #[test]
    fn overlay_guids_are_distinct_and_round_trip() {
        let mut seen = Vec::new();
        for mode in Mode::ALL {
            let guid = mode.overlay_guid();
            assert!(!seen.iter().any(|other| guid_eq(&guid, other)));
            seen.push(guid);
            assert_eq!(mode_from_overlay_guid(&guid), Some(mode));
        }
    }

    #[test]
    fn raw_labels_cover_native_modes() {
        assert_eq!(label_for_raw(1), "安静模式");
        assert_eq!(label_for_raw(2), "普通模式");
        assert_eq!(label_for_raw(3), "野兽模式");
        assert_eq!(label_for_raw(224), "超神模式");
        assert_eq!(label_for_raw(255), "自定义模式");
    }
}
