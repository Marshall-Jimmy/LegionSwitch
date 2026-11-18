use crate::modes::{Mode, label_for_raw};
use crate::wmi::WmiSession;
use crate::{autostart, config, power};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    Tray { show: bool },
    Apply(Mode),
    Status,
    StartupInstall,
    StartupRemove,
    Help,
    Version,
    Invalid(String),
}

pub fn parse_args<I, S>(args: I) -> Launch
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_ascii_lowercase())
        .collect();

    if args.is_empty() {
        return Launch::Tray { show: true };
    }

    if args.len() > 1 {
        return Launch::Invalid(args.join(" "));
    }

    match args[0].as_str() {
        "--tray" | "tray" | "--startup" => Launch::Tray { show: false },
        "--show" | "show" | "--ui" => Launch::Tray { show: true },
        "--status" | "status" => Launch::Status,
        "--quiet" | "quiet" => Launch::Apply(Mode::Quiet),
        "--balanced" | "balanced" | "--normal" | "normal" => Launch::Apply(Mode::Balanced),
        "--beast" | "beast" | "--performance" | "performance" => Launch::Apply(Mode::Beast),
        "--startup-install" | "startup-install" => Launch::StartupInstall,
        "--startup-remove" | "startup-remove" => Launch::StartupRemove,
        "--help" | "-h" | "help" => Launch::Help,
        "--version" | "-v" | "version" => Launch::Version,
        other => Launch::Invalid(other.to_string()),
    }
}

pub fn run_cli(launch: Launch) -> i32 {
    crate::console::attach();

    match launch {
        Launch::Help => {
            print_help();
            0
        }
        Launch::Version => {
            crate::console::out(&format!("LegionSwitch {}", env!("CARGO_PKG_VERSION")));
            0
        }
        Launch::Invalid(argument) => {
            crate::console::err(&format!("无法识别的参数：{argument}"));
            print_help();
            2
        }
        Launch::Tray { .. } => 0,
        Launch::Status => status(),
        Launch::Apply(mode) => apply(mode),
        Launch::StartupInstall => startup_install(),
        Launch::StartupRemove => startup_remove(),
    }
}

fn status() -> i32 {
    match WmiSession::connect().and_then(|session| session.status()) {
        Ok(status) => {
            crate::console::out(&format!(
                "模式: {} ({})",
                label_for_raw(status.raw_mode),
                status.raw_mode
            ));
            if let Some(overlay) = status.overlay {
                crate::console::out(&format!("Windows 电源模式: {}", overlay.overlay_name_zh()));
            }
            if let Some(value) = status.telemetry.cpu_temp {
                crate::console::out(&format!("CPU: {value} °C"));
            }
            if let Some(value) = status.telemetry.gpu_temp {
                crate::console::out(&format!("GPU: {value} °C"));
            }
            if let Some(value) = status.telemetry.fan1 {
                crate::console::out(&format!("风扇 1: {value} RPM"));
            }
            if let Some(value) = status.telemetry.fan2 {
                crate::console::out(&format!("风扇 2: {value} RPM"));
            }
            0
        }
        Err(error) => {
            crate::console::err(&error);
            1
        }
    }
}

fn apply(mode: Mode) -> i32 {
    let cfg = config::Config::load();
    let result = (|| -> Result<String, String> {
        let session = WmiSession::connect()?;
        session.set_mode(mode)?;
        let raw = session.get_raw_mode().unwrap_or(mode.smart_fan_value());
        let mut message = format!("已切换到{}（{raw}）", mode.label_zh_full());
        if cfg.sync_overlay {
            if let Err(error) = power::set_overlay(mode) {
                message.push_str(&format!("；Windows 电源模式同步失败：{error}"));
            }
        }
        Ok(message)
    })();

    match result {
        Ok(message) => {
            crate::console::out(&message);
            0
        }
        Err(error) => {
            crate::console::err(&error);
            1
        }
    }
}

fn startup_install() -> i32 {
    match autostart::enable() {
        Ok(()) => {
            crate::console::out("已启用开机自启动（计划任务 LegionSwitch，最高权限）");
            0
        }
        Err(error) => {
            crate::console::err(&error);
            1
        }
    }
}

fn startup_remove() -> i32 {
    match autostart::disable() {
        Ok(()) => {
            crate::console::out("已关闭开机自启动");
            0
        }
        Err(error) => {
            crate::console::err(&error);
            1
        }
    }
}

fn print_help() {
    crate::console::out(
        "LegionSwitch - 轻量级联想 Legion 性能模式切换\n\
         \n\
         用法:\n\
           LegionSwitch.exe                 打开面板\n\
           LegionSwitch.exe --tray          仅驻留托盘（自启动用）\n\
           LegionSwitch.exe quiet           切换到安静模式\n\
           LegionSwitch.exe balanced        切换到普通模式\n\
           LegionSwitch.exe beast           切换到野兽模式\n\
           LegionSwitch.exe status          输出当前模式/温度/风扇\n\
           LegionSwitch.exe startup-install 安装开机自启动\n\
           LegionSwitch.exe startup-remove  移除开机自启动\n\
           LegionSwitch.exe --help          显示帮助\n\
         \n\
         热键:\n\
           Ctrl+Alt+1  安静模式\n\
           Ctrl+Alt+2  普通模式\n\
           Ctrl+Alt+3  野兽模式\n\
           Ctrl+Alt+L  显示/隐藏面板\n\
         \n\
         模式切换需要管理员权限（WMI ACPI 接口限制）。",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ui_and_actions() {
        assert_eq!(
            parse_args(Vec::<String>::new()),
            Launch::Tray { show: true }
        );
        assert_eq!(parse_args(["--tray"]), Launch::Tray { show: false });
        assert_eq!(parse_args(["beast"]), Launch::Apply(Mode::Beast));
        assert_eq!(parse_args(["normal"]), Launch::Apply(Mode::Balanced));
        assert_eq!(parse_args(["--status"]), Launch::Status);
        assert!(matches!(parse_args(["nope"]), Launch::Invalid(_)));
    }
}
