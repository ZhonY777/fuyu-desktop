# 浮屿 · Windows 桌面助手

浮屿是基于 **DeskBox** 和 **Daozai（岛仔）** 的非官方改编版。它把桌面分区、应用分类和顶部灵动岛整合到同一个 Windows 应用中，灵动岛设置也已并入浮屿主设置窗口。

- 感谢 **朱天雨（Tianyu Zhu）** 和 DeskBox 贡献者。浮屿的桌面分区、应用管理和 WinUI 设置窗口来自 DeskBox，基于 `037c88fc9333266e20a8f649bdfef3b7c02a67e0` 改编。[DeskBox 原项目](https://github.com/Tianyu199509/DeskBox)
- 感谢 **小唐的Token** 和 Daozai 贡献者。浮屿的灵动岛、音乐、专注、邮件、日程和 AI 额度功能来自岛仔，基于 `39413b3c65d0c91d598507bc6d1ab3be0c6d1105` 改编。[Daozai 原项目](https://github.com/Renly1994/xiaotang-daozai)

浮屿的改动和维护由本仓库负责。感谢两位作者及各位贡献者公开源码。若本项目的改编、署名或展示有冒犯或不当之处，请通过本仓库 Issues 联系维护者。收到后会核实，并按请求更正或删除相关内容。[联系维护者](https://github.com/ZhonY777/fuyu-desktop/issues/new)

整合版按 [GPL-3.0-only](LICENSE) 发布。DeskBox 原许可证、Daozai 的 [MIT 许可证](island/LICENSE) 和[第三方声明](island/THIRD_PARTY_NOTICES.txt) 随源码保留。第三方依赖和素材沿用各自的许可与权利声明。

## 构建和运行

在 Windows x64 上准备 .NET SDK 10.0.402、Rust 1.96 或更新的稳定版、Visual Studio C++ Build Tools 和 Windows SDK。从仓库根目录在 PowerShell 中运行：

```powershell
.\build.ps1
```

构建输出位于 `dist\win-x64`。运行其中的 `DeskBox.exe --open-settings`，在主设置左侧选择「灵动岛」。首次启动使用当前 Windows 用户的本地配置。邮箱、在线日历和 AI 服务在应用内按需连接。
