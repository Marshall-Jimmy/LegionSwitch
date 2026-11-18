use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

const TASK_NAME: &str = "LegionSwitch";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn task_exists() -> bool {
    Command::new("schtasks.exe")
        .args(["/Query", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn enable() -> Result<(), String> {
    let exe =
        std::env::current_exe().map_err(|error| format!("无法获取 LegionSwitch 路径：{error}"))?;
    let task_run = format!("\"{}\" --tray", exe.display());

    let output = Command::new("schtasks.exe")
        .args([
            "/Create", "/SC", "ONLOGON", "/TN", TASK_NAME, "/TR", &task_run, "/RL", "HIGHEST",
            "/IT", "/F",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("无法调用 schtasks.exe：{error}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "创建计划任务失败：{}",
            process_message(&output.stdout, &output.stderr)
        ))
    }
}

pub fn disable() -> Result<(), String> {
    let output = Command::new("schtasks.exe")
        .args(["/Delete", "/TN", TASK_NAME, "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("无法调用 schtasks.exe：{error}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "删除计划任务失败：{}",
            process_message(&output.stdout, &output.stderr)
        ))
    }
}

fn process_message(stdout: &[u8], stderr: &[u8]) -> String {
    let text = if stderr.is_empty() { stdout } else { stderr };
    let text = String::from_utf8_lossy(text);
    let text = text.trim();
    if text.is_empty() {
        "schtasks 返回非零退出码".to_string()
    } else {
        text.to_string()
    }
}
