# Changelog

## 1.0.0 — 2026-11-18

首个产品化版本，作者 jimmma。

- 面板内设置页：开机自启动、Windows 电源模式同步、遥测开关。
- 设置页展示版本、作者 `jimmma`、MIT License 和 GitHub 项目入口。
- 托盘菜单新增“设置 / 关于”和“GitHub 项目主页”。
- 新增 `diagnose` CLI 诊断报告。
- 开机自启动状态改为后台探测，打开设置页不再阻塞 UI。
- 新增 `packaging/install.ps1` / `uninstall.ps1`：Program Files 安装、开始菜单、应用和功能卸载项。
- 新增 `packaging/package.ps1`：生成便携 ZIP 和 SHA-256 校验文件。
- 新增 GitHub Actions Windows 构建/测试工作流。
- 版本资源、作者信息、版权和项目链接更新为 1.0.0 / jimmma。

## 0.2.0 — 2026-11-18

LegionZone 风格 UI 与点击修复。

- 重做面板：金属圆环模式按钮、深色金属渐变、LegionZone 式状态栏与热键提示。
- 三枚模式图标按 LegionZone 的圆形双箭头视觉语言重新绘制，不直接打包其素材。
- 修复面板卡片点击被 `ReleaseCapture` 同步触发的 `WM_CAPTURECHANGED` 清空的问题。
- 模式强调色调整为 LegionZone 的蓝 / 银 / 红。
- 应用/托盘图标同步更新配色。
- 版本号更新为 0.2.0。

## 0.1.0 — 2026-11-18

首个可构建版本。

- 原生 Win32 单进程托盘应用。
- 安静 / 普通 / 野兽模式切换：`LENOVO_GAMEZONE_DATA.SetSmartFanMode`。
- `GetSmartFanMode` 回读校验，回读不一致时提示。
- 可选同步 Windows 11 电源模式叠加层。
- 全局热键 `Ctrl+Alt+1/2/3`、`Ctrl+Alt+L`。
- 托盘菜单、温度/风扇显示、单实例、开机自启动计划任务。
- CLI：`status`、`quiet`、`balanced`、`beast`、`startup-install`、`startup-remove`。
- WMI 调用放在独立线程，避免 UI 卡顿。
- 应用图标、模式托盘图标、DPI 缩放、深色自绘面板。
- 逆向笔记与 README。
