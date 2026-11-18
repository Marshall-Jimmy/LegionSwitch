use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    /// Also set the Windows 11 power-mode overlay (best efficiency / balanced /
    /// best performance) when switching the Legion EC mode.
    pub sync_overlay: bool,
    /// Read temperatures and fan speeds while the panel is open.
    pub show_telemetry: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sync_overlay: true,
            show_telemetry: true,
        }
    }
}

impl Config {
    pub fn dir() -> PathBuf {
        std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("LOCALAPPDATA"))
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("LegionSwitch")
    }

    pub fn path() -> PathBuf {
        Self::dir().join("config.ini")
    }

    pub fn load() -> Self {
        let Ok(text) = std::fs::read_to_string(Self::path()) else {
            return Self::default();
        };
        let mut config = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "sync_overlay" => config.sync_overlay = parse_bool(value, config.sync_overlay),
                "show_telemetry" => {
                    config.show_telemetry = parse_bool(value, config.show_telemetry)
                }
                _ => {}
            }
        }
        config
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = Self::dir();
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("无法创建配置目录 {}：{error}", dir.display()))?;
        let text = format!(
            "# LegionSwitch configuration\nsync_overlay={}\nshow_telemetry={}\n",
            if self.sync_overlay { 1 } else { 0 },
            if self.show_telemetry { 1 } else { 0 },
        );
        std::fs::write(Self::path(), text)
            .map_err(|error| format!("无法写入配置 {}：{error}", Self::path().display()))
    }
}

fn parse_bool(value: &str, fallback: bool) -> bool {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_parsing_is_forgiving() {
        assert!(parse_bool("1", false));
        assert!(parse_bool("TRUE", false));
        assert!(parse_bool("on", false));
        assert!(!parse_bool("0", true));
        assert!(!parse_bool("no", true));
        assert!(parse_bool("garbage", true));
    }
}
