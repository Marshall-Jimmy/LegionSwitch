use windows_sys::Win32::Foundation::GetLastError;

use crate::modes::Mode;

#[derive(Debug, Clone, Copy, Default)]
pub struct Telemetry {
    pub cpu_temp: Option<u32>,
    pub gpu_temp: Option<u32>,
    pub fan1: Option<u32>,
    pub fan2: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Status {
    pub raw_mode: u32,
    pub mode: Option<Mode>,
    pub telemetry: Telemetry,
    pub overlay: Option<Mode>,
}

pub struct WmiSession {
    connection: wmi::WMIConnection,
    class: wmi::IWbemClassWrapper,
    /// `LENOVO_GAMEZONE_DATA` is a dynamic instance class. The methods are
    /// instance methods (the class definition has no `Static` qualifier), so
    /// `ExecMethod` must target the instance's `__Path`, not the class name.
    instance_path: String,
}

impl WmiSession {
    pub fn connect() -> Result<Self, String> {
        let connection = wmi::WMIConnection::with_namespace_path("ROOT\\WMI")
            .map_err(|error| map_wmi_error("连接 ROOT\\WMI", error))?;
        let class = connection
            .get_object("LENOVO_GAMEZONE_DATA")
            .map_err(|error| map_wmi_error("打开 LENOVO_GAMEZONE_DATA", error))?;

        // `GetSmartFanMode` / `SetSmartFanMode` are not static, so obtain the
        // singleton instance and keep its object path for all method calls.
        let mut instances = connection
            .exec_query("SELECT * FROM LENOVO_GAMEZONE_DATA")
            .map_err(|error| map_wmi_error("查询 LENOVO_GAMEZONE_DATA 实例", error))?;
        let instance = instances
            .next()
            .transpose()
            .map_err(|error| map_wmi_error("枚举 LENOVO_GAMEZONE_DATA 实例", error))?
            .ok_or_else(|| "未找到 LENOVO_GAMEZONE_DATA 实例".to_string())?;
        let instance_path = instance
            .path()
            .map_err(|error| map_wmi_error("读取 LENOVO_GAMEZONE_DATA.__Path", error))?;

        Ok(Self {
            connection,
            class,
            instance_path,
        })
    }

    pub fn set_mode(&self, mode: Mode) -> Result<(), String> {
        self.exec_with_data("SetSmartFanMode", Some(mode.smart_fan_value()))
            .map(|_| ())
    }

    pub fn get_raw_mode(&self) -> Result<u32, String> {
        self.read_u32("GetSmartFanMode")
    }

    pub fn get_telemetry(&self) -> Telemetry {
        Telemetry {
            cpu_temp: self.read_u32("GetCPUTemp").ok().filter(|value| *value > 0),
            gpu_temp: self.read_u32("GetGPUTemp").ok().filter(|value| *value > 0),
            fan1: self.read_u32("GetFan1Speed").ok(),
            fan2: self.read_u32("GetFan2Speed").ok(),
        }
    }

    pub fn status(&self) -> Result<Status, String> {
        let raw_mode = self.get_raw_mode()?;
        Ok(self.status_with_raw(raw_mode))
    }

    pub fn status_with_raw(&self, raw_mode: u32) -> Status {
        Status {
            raw_mode,
            mode: Mode::from_smart_fan_value(raw_mode),
            telemetry: self.get_telemetry(),
            overlay: crate::power::get_overlay(),
        }
    }

    fn read_u32(&self, method: &str) -> Result<u32, String> {
        let output = self.exec_with_data(method, None)?;
        let Some(output) = output else {
            return Err(format!("{method} 未返回结果"));
        };
        let value = output
            .get_property("Data")
            .or_else(|_| output.get_property("ReturnValue"))
            .map_err(|error| map_wmi_error(&format!("读取 {method}.Data"), error))?;
        variant_to_u32(value).ok_or_else(|| format!("{method} 返回了无法识别的数据类型"))
    }

    fn exec_with_data(
        &self,
        method: &str,
        input: Option<u32>,
    ) -> Result<Option<wmi::IWbemClassWrapper>, String> {
        let in_params = match input {
            Some(value) => {
                let signature = self
                    .class
                    .get_method(method)
                    .map_err(|error| map_wmi_error(&format!("读取 {method} 签名"), error))?
                    .ok_or_else(|| format!("{method} 没有输入参数签名"))?;
                let instance = signature
                    .spawn_instance()
                    .map_err(|error| map_wmi_error(&format!("创建 {method} 参数"), error))?;
                instance
                    .put_property("Data", value)
                    .map_err(|error| map_wmi_error(&format!("设置 {method}.Data"), error))?;
                Some(instance)
            }
            None => None,
        };

        self.connection
            .exec_method(&self.instance_path, method, in_params.as_ref())
            .map_err(|error| map_wmi_error(&format!("执行 {method}"), error))
    }
}

fn variant_to_u32(value: wmi::Variant) -> Option<u32> {
    match value {
        wmi::Variant::UI1(value) => Some(u32::from(value)),
        wmi::Variant::UI2(value) => Some(u32::from(value)),
        wmi::Variant::UI4(value) => Some(value),
        wmi::Variant::UI8(value) => u32::try_from(value).ok(),
        wmi::Variant::I1(value) => u32::try_from(value).ok(),
        wmi::Variant::I2(value) => u32::try_from(value).ok(),
        wmi::Variant::I4(value) => u32::try_from(value).ok(),
        wmi::Variant::I8(value) => u32::try_from(value).ok(),
        _ => None,
    }
}

pub fn map_wmi_error(context: &str, error: wmi::WMIError) -> String {
    let text = error.to_string();
    if text.contains("0x80041003")
        || text.to_ascii_lowercase().contains("access denied")
        || text.contains("拒绝访问")
        || text.contains("Access is denied")
    {
        format!("{context} 失败：需要管理员权限（{text}）")
    } else if text.contains("0x80041010") || text.contains("Invalid class") {
        format!("{context} 失败：此设备没有 Lenovo GameZone WMI 接口（{text}）")
    } else {
        format!("{context} 失败：{text}")
    }
}

pub fn last_os_error_text(prefix: &str) -> String {
    let code = unsafe { GetLastError() };
    format!("{prefix}（Win32 错误 {code}）")
}

#[cfg(test)]
mod tests {
    #[test]
    fn exec_query_exposes_an_instance_path() {
        // A normal, non-privileged WMI class verifies the same query-and-path
        // sequence used for the Lenovo singleton without touching firmware.
        let connection =
            wmi::WMIConnection::with_namespace_path("ROOT\\CIMV2").expect("connect to ROOT\\CIMV2");
        let mut query = connection
            .exec_query("SELECT * FROM Win32_OperatingSystem")
            .expect("query Win32_OperatingSystem");
        let instance = query
            .next()
            .expect("at least one instance")
            .expect("instance");
        let path = instance.path().expect("instance __Path");
        assert!(path.contains("Win32_OperatingSystem"));
    }
}
