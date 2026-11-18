# LegionZone 逆向笔记

本文件记录 LegionSwitch 实现过程中对 Lenovo LegionZone 的静态分析结果，供后续维护和兼容性适配使用。分析对象为本机已安装版本，不包含联想的源代码或二进制再分发。

## 1. 分析对象

安装根目录：

```text
C:\Program Files (x86)\Lenovo\LegionZone
```

版本目录：

```text
C:\Program Files (x86)\Lenovo\LegionZone\2.0.30.11033
```

关键文件与 SHA-256：

| 文件 | 版本 | SHA-256 |
|---|---|---|
| `LZMain.exe` | 2.0.28.8181 | `496E4D3032D8ACEBEA39F2E6D3A9598D2280B1FC3346AAD46A3DE31B7063EE26` |
| `LZService.exe` | 2.0.24.1221 | `960B8753AD3C5165DC43060C8CD27E4C14805BD364E18A2257337998BFA42778` |
| `LZTray.exe` | 2.1.24.9081 | `298A40B458F2B1136238383702B2FBF73613DE5401332B11FF4223BE9AB99F5A` |
| `plugin\phwcap.dll` | 2.0.29.9101 | `569308655CA3EB6EBF74E6ACEE324C24DC113E573BEED53A72D67A2259C34DF7` |

环境：

- 机型：Lenovo 83DG
- BIOS：NMCN21WW
- 系统：Windows 11 26200
- LegionZone：2.0.30.11033

## 2. 进程与职责

LegionZone 不是一个单一程序，而是多个进程/层的组合：

```text
LegionZone.exe (CEF/WebView UI, 多个进程)
  ├─ LZTray.exe (WPF 托盘, 高权限)
  │    ├─ LZTrayPlugin.dll (native, 模式切换/通知)
  │    └─ plugin\phwcap.dll (硬件能力/WMI/电源模式)
  ├─ LZStrategy.exe (策略/游戏识别)
  ├─ LZService.exe (Windows 服务, LocalSystem, 登录自启动/托盘拉起)
  └─ LZMain.exe (native 主程序/特权操作)
```

`LZService` 是 `LocalSystem` 的自动启动服务，负责登录后的托盘拉起等；它本身不是 WMI 模式切换的入口。`LZTray` 以高权限运行，直接完成大部分硬件操作，因此普通用户无法从自己的进程调用同样的 WMI 方法。实测 `LZTray.exe` 的令牌为 `elevated=1`、High Integrity（`S-1-16-12288`）。

LegionSwitch 不依赖这些进程，也不加载它们的 DLL；只复用 ACPI WMI 协议。

## 3. WMI 接口

命名空间：

```text
ROOT\WMI
```

类：

```text
LENOVO_GAMEZONE_DATA
```

本机可枚举到的模式相关方法（`Get-CimClass`）：

| 方法 | 参数 | 用途 |
|---|---|---|
| `SetSmartFanMode` | `Data: uint32` | 设置风扇/性能模式 |
| `GetSmartFanMode` | `Data: uint32` | 回读当前模式 |
| `GetThermalMode` | `Data: uint32` | 读取热模式/逻辑模式 |
| `IsSupportSmartFan` | `Data: uint32` | 是否支持 |
| `GetFanCount` | `Data: uint32` | 风扇数量 |
| `GetFan1Speed` / `GetFan2Speed` | `Data: uint32` | 风扇转速 |
| `GetCPUTemp` / `GetGPUTemp` | `Data: uint32` | 温度 |
| `GetPowerChargeMode` | `Data: uint32` | 充电模式 |
| `SetWinKeyStatus` / `SetTPStatus` | `Data: uint32` | Win 键/触控板 |
| `SetIGPUModeStatus` | `mode, Data` | 显卡模式（未使用） |

实际切换只需要 `SetSmartFanMode`，回读用 `GetSmartFanMode`。

这两个方法没有 `Static` 限定符，是**实例方法**。因此不能把类名直接作为 `IWbemServices::ExecMethod` 的对象路径；需要先执行 `SELECT * FROM LENOVO_GAMEZONE_DATA` 取得唯一实例，再使用实例的 `__Path` 调用方法。LegionSwitch 的 `WmiSession` 就是这样实现的，并用一个非特权 WMI 类（`Win32_OperatingSystem`）的单元测试覆盖“查询 -> 取 `__Path`”这段逻辑。

### 3.1 模式取值

`LZTray.exe` 反编译出的 `LZTray.GameMode`：

```csharp
public enum GameMode
{
    Unknow = 0,
    Quiet = 1,
    Balance = 2,
    Beast = 3,
    Extreme = 224,
    Customize = 255
}
```

`LZTray.PluginDll.ChangeMode` 将枚举整数直接作为 `SetSmartFanMode` 的 `Data`：

```text
ChangeMode(GameMode.Beast)
  -> InvokePlugin("SetSmartFanMode", "3")
```

`LZTray.GlobalCurrentStatus.CurrentGameMode` 通过插件调用 `GetSmartFanMode`，读取返回 JSON 的 `ret` 字段并转换为 `GameMode`。这也与公开工具中 `Get-WmiObject -Class LENOVO_GAMEZONE_DATA; $gz.SetSmartFanMode(...)` 的实例调用方式一致。

因此 LegionSwitch 的映射为：

| UI | WMI `SetSmartFanMode.Data` |
|---|---|
| 安静 | 1 |
| 普通 | 2 |
| 野兽 | 3 |

本机 WMI 实例在普通权限下会返回：

```text
Get-CimInstance -Namespace root/WMI -ClassName LENOVO_GAMEZONE_DATA
-> 拒绝访问 (0x80041003)
```

这说明访问控制发生在 WMI 提供程序/ACPI 层，而不是应用层。LegionSwitch 因此需要管理员清单。

### 3.2 方法返回结构

`GetSmartFanMode` 的输出参数名为 `Data`。LegionSwitch 先读取 `Data`，若不存在再尝试 `ReturnValue`，以兼容不同 BIOS 的 MOF 定义。读取值为 `uint32`。

`SetSmartFanMode` 没有可靠的应用层返回值；判断是否成功应以回读 `GetSmartFanMode` 为准。LegionSwitch 在每次写入后回读，若回读值与请求不一致，会显示“模式已发送，但回读为 …”。

## 4. Windows 电源模式叠加层

LegionZone 的 `phwcap.dll` 中包含以下字符串：

```text
PowerSetActiveOverlayScheme
PowerGetActiveOverlayScheme
GetCurrentPowerMode: PowerGetActiveOverlayScheme function not found
SetCustomizedModeData_SetPowerMode_NB PowerSetActiveOverlayScheme is null
```

反编译/字符串分析确认它使用 Windows 11 的电源模式叠加层 GUID：

```text
最佳能效: 961cc777-2547-4f9d-8174-7d86181b8a7a
平衡:     00000000-0000-0000-0000-000000000000
最佳性能: ded574b5-45a0-4f42-8737-46345c09c238
```

这三个 GUID 在本机通过 `PowerSetActiveOverlayScheme` 实际写入并回读验证成功。

`phwcap.dll` 还包含：

```text
Quiet_Mode.pow
Balance_Mode.pow
Performance_Mode.pow
```

对应 LegionZone 自带的三个电源计划文件（`config\X80_PowerPlan`，内部名称分别为 `Legion Quiet Mode` / `Legion Balance Mode` / `Legion Performance Mode`）。本机当前活动电源计划是 Windows 的“高性能”，并未导入 Legion 计划。LegionSwitch 因此**不导入 `.pow`**，只同步叠加层；用户可在托盘菜单关闭同步。

## 5. 权限与完整性级别

在分析会话中，工作区启动的 Rust 进程被系统标记为 Low Integrity（`S-1-16-4096`），而 `%TEMP%` 下的 C# 进程为 Medium（`S-1-16-8192`）。Low Integrity 下：

- `PowerSetActiveOverlayScheme` 返回 `5`（`ERROR_ACCESS_DENIED`）。
- WMI 模式方法返回 `0x80041003`（`WBEM_E_ACCESS_DENIED`）。

把同一个 Rust 探测程序复制到 `%TEMP%` 后，Medium Integrity 下 `PowerSetActiveOverlayScheme` 返回 `0`，GUID 回读正确。这说明：

1. 电源叠加层的调用本身不需要管理员，但需要正常的中等完整性令牌。
2. WMI 模式方法仍然需要管理员。
3. 正式版 LegionSwitch 使用 `requireAdministrator`，所以两个调用都在足够权限下运行。

## 6. 关键反编译点

工具：

- `ilspycmd`（带 .NET 6 runtime roll-forward）反编译 `LZTray.exe`。
- `dumpbin` 查看 `phwcap.dll` 导出函数。
- `strings -el` 提取 UTF-16 字符串。
- PowerShell `Get-CimClass` / `Get-CimInstance` 枚举 WMI。
- `Add-Type` P/Invoke 验证 `powrprof.dll` 叠加层 API。

`phwcap.dll` 相关导出：

```text
SetPowerMode
GetHWWinCurInfo_Tray
GetHWWinCurInfo_MCP
GetCustomizedModeData_ByPerf
RefreshOcParameter_Tray
```

`LZTrayPlugin.dll` 中与模式切换/热键相关字符串：

```text
GameInHandle SetSmartFanMode:%d
GameExitHandle Back SetSmartFanMode:%d
CurCustomModeConfigID = 2. SetSmartFanMode(0xE0)
HWStatusAssistant::SmartFanModeChangedCallBack : %d
LZWmiMethod::SetSmartFanMode dwFanMode : %d
```

其中 `0xE0`（224）是“超神模式”。自定义模式使用 `255`，两者都不在 LegionSwitch 的三模式切换范围内。

`phwcap.dll` 的 `SetPowerModeByCap` 会：

- 检查 `PowerModeSwitch` 配置；
- 读取/写入 `PowerSetActiveOverlayScheme`；
- 对自定义模式直接跳过；
- 失败时记录返回值。

LegionSwitch 的 `power.rs` 采用同样的核心逻辑，但去掉了 LegionZone 的配置/自定义模式分支，并提供关闭开关。

## 7. 与 LegionSwitch 的差异

LegionSwitch 刻意只做最小闭环：

| 项目 | LegionZone | LegionSwitch |
|---|---|---|
| UI | CEF + WPF | 原生 Win32 + GDI |
| 常驻进程 | 多个，含 CEF 子进程 | 1 个托盘进程 |
| 安装体积 | 数百 MB | 约 0.5 MB |
| 模式接口 | 厂商插件 + WMI | 直接 WMI |
| 电源模式 | `.pow`/overlay/配置 | 仅可选 overlay |
| 自定义模式 | 支持 | 不支持 |
| 驱动/服务 | 多个 | 无 |
| 网络/账号 | 有 | 无 |

## 8. 复现与验证

只读枚举：

```powershell
Get-CimClass -Namespace root/WMI -ClassName LENOVO_GAMEZONE_DATA |
  Select-Object -ExpandProperty CimClassMethods |
  Select-Object Name
```

在管理员 PowerShell 中读取：

```powershell
$c = Get-CimInstance -Namespace root/WMI -ClassName LENOVO_GAMEZONE_DATA
Invoke-CimMethod -InputObject $c -MethodName GetSmartFanMode
```

> 不建议在没有恢复方案的情况下直接写 `SetSmartFanMode`。LegionSwitch 会在托盘/CLI 中给出回读结果。

LegionSwitch 侧验证：

```powershell
$env:LEGIONSWITCH_PREVIEW = "1"
cargo test
cargo build --release
```

正式版：

```powershell
Remove-Item Env:LEGIONSWITCH_PREVIEW
cargo build --release
```

## 9. 未验证项与风险

- 分析会话本身未在管理员环境中执行真实 `SetSmartFanMode` 写入；代码路径已经到达 `LENOVO_GAMEZONE_DATA` 实例枚举，非管理员返回 0x80041003，与预期一致。目标机上用户已通过托盘菜单实机切换成功，说明 WMI 写入路径可用。
- 面板点击曾因 `ReleaseCapture` 同步触发 `WM_CAPTURECHANGED` 清空按下状态而失效；0.2.0 已改为先快照按下状态再释放捕获，并用窗口消息复测三张模式按钮。
- 未在所有 Legion 代数上验证 WMI 兼容性。部分型号可能使用不同类名/方法名，或需要先设置 `SetThermalMode`。
- 未测试 LegionZone 同时运行时的竞争条件；建议只保留一个模式控制方。
- 未验证自定义模式（255）和超神模式（224）的完整语义；LegionSwitch 只显示、不切换。
- 未对 BIOS/EC 做任何直接操作，因此不存在刷写风险；但模式切换会改变风扇/功耗策略，长时间高负载仍需注意散热。

## 10. UI 素材观察

LegionZone 的本地前端有两套模式视觉：

- `ui/main.html` + `ui/images/`：托盘顶栏使用 96×96 的金属圆环双箭头按钮：
  - `mode_q_n.png` / `mode_q_h.png`：安静（蓝）
  - `mode_n_n.png` / `mode_n_h.png`：普通/均衡（银）
  - `mode_a_n.png` / `mode_a_h.png`：野兽（红）
  - `switch_*`：电源/联动开关；`top-banner-bg.png`：1016×224 顶栏背景（原图 2032×446）。
  - 布局参数：顶栏 1016×224；模式按钮 104×106，图标显示 50×50；模式弹层 106 宽、条目 30 高。
  - 配色：文字 `#EAF7FF`，弹层 `#1E242B`，悬停 `rgba(57,64,77,0.45)`。
- `localhtml/img/perf-mode-*.png`：另一套 96×96 仪表风格图标，安静为蓝色仪表、均衡为白色双向箭头、野兽为红色仪表。

LegionSwitch 按这套“金属圆环 + 暗玻璃内盘 + 蓝/银/红双箭头”的语言，用 Python/Pillow 重新绘制了 48/96/144/192 多尺寸 ICO（`resources/make_mode_icons.py`），**没有直接打包或运行时加载联想的图片素材**。UI 其余部分也是 GDI 自绘。

## 11. 参考

- Linux 内核 Lenovo WMI GameZone 文档（同类 ACPI WMI 协议说明）：<https://raw.githubusercontent.com/torvalds/linux/master/Documentation/wmi/devices/lenovo-wmi-gamezone.rst>
- fancontrol 项目的 Lenovo 后端（用 `Get-WmiObject -Class LENOVO_GAMEZONE_DATA` 取实例后调用方法，与本文的实例调用结论一致）：<https://github.com/pjt222/fancontrol>
- Windows 电源模式叠加层 API 的 GUID 与调用方式来自本机 `phwcap.dll` 字符串/反编译结果，并用 PowerShell P/Invoke 实测。

## 12. 结论

对本机 LegionZone 2.0.30.11033 的分析确认：

- 三个原生模式由 `LENOVO_GAMEZONE_DATA.SetSmartFanMode(1/2/3)` 控制。
- 回读使用 `GetSmartFanMode`。
- Windows 电源模式叠加层使用三个标准 GUID。
- 管理员权限是 WMI ACPI 接口的硬性要求。
- 不需要驱动、服务、WebView 或网络即可完成核心切换。

LegionSwitch 就是在这四点之上实现的最小化替代品。
