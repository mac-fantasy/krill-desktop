# Krill Desktop

Krill 个人账号用量助手，提供 Windows 与 macOS 两套原生客户端：

- **Windows**：C# / .NET 10 / WPF，桌面悬浮球，支持贴边收成进度竖条，点击展开详情。
- **macOS**：Swift 6 / AppKit / SwiftUI，菜单栏磷虾 Logo 与额度进度环，点击展开详情面板。macOS 14 起运行；macOS 26 上可为导航与工具按钮使用 Liquid Glass。

`0.2.1` 已恢复 C# / WPF 和 Swift / SwiftUI 客户端，并移除 GPUI 重构代码与构建工具。macOS 恢复 macOS 14+、Apple Silicon / Intel 通用包；已安装 `0.2.0` 的用户可检查更新返回原生版本，账号和设置沿用原位置。

两端均显示个人资料、套餐周期剩余、充值余额、福利余额、结转额度、今日请求、缓存率、思考 Tokens、请求速率、重置卡状态和公告，支持官网现有的任意登录方式（包括 Apple → GitHub 等二次登录），并可跳转官网管理账号。月卡额度用尽或到期后自动切换其他生效月卡，并将不可用卡片置灰。详情面板沿用相同的五个页签与深色薄荷绿配色。

macOS 已在 Apple Silicon / Apple Swift 6.4 环境通过测试、Release 编译和 ad-hoc 签名检查。应用使用 AppKit 生命周期，启动仅常驻菜单栏；详情面板使用 SwiftUI。真实账号登录、旧版 macOS 与 Intel 架构仍需验收，具体边界见 [macOS 开发文档](docs/macos.md)。

## macOS 安装

```bash
brew install --cask mac-fantasy/tap/krill-desktop
```

支持 macOS 14+、Apple Silicon 和 Intel；安装源由 [统一 Homebrew tap](https://github.com/mac-fantasy/homebrew-tap) 维护，每 6 小时检查本项目最新稳定版。应用也可以自行下载并安装更新；通过 Homebrew 手动升级时使用：

```bash
brew update
brew upgrade --cask --greedy mac-fantasy/tap/krill-desktop
```

普通卸载使用 `brew uninstall --cask mac-fantasy/tap/krill-desktop`，保留本机设置；加 `--zap` 会删除 `~/Library/Application Support/KrillDesktop` 中的偏好。钥匙串凭据不由该清理命令删除。若已手动安装同名应用，请先退出并移走旧 `.app` 后再安装。当前发行版未进行 Developer ID 签名和公证，首次打开可能需要按系统提示允许打开。

项目已由 `lljxww/krill-desktop` 迁入 `mac-fantasy/krill-desktop`。`0.1.5` 起的客户端使用组织的新更新地址；旧版严格校验原仓库地址，可能无法识别迁移后的更新，需要通过 Homebrew 或 [Release](https://github.com/mac-fantasy/krill-desktop/releases/latest) 安装一次新版。账号凭据和设置继续保存在原有位置。

## 项目结构

```text
apps/
  windows/
    src/Krill.Core/             C# 协议客户端、模型、额度与展示规则
    src/Krill.Desktop/          WPF 悬浮球、详情面板、登录与托盘
    tests/Krill.Core.Tests/     Windows 核心契约检查
  macos/
    Package.swift              Swift Package，原生 macOS 可执行目标
    Sources/KrillCore/         Swift 协议客户端、模型、额度与展示规则
    Sources/KrillMac/
      App/                    应用入口、菜单栏图标与弹出面板
      Services/               状态管理、钥匙串、浏览器 / WKWebView 登录
      Views/                  SwiftUI 五页详情面板与主题
    Tests/KrillCoreTests/      Swift 契约测试
    Tests/KrillMacTests/       菜单栏图标与 WebKit 回调回归测试
    Support/Info.plist         .app 包配置与菜单栏应用标识
    Support/Assets/            应用图标源文件、PNG 与 ICNS
contracts/fixtures/            两端共用的虚构 JSON 样例，无账号凭据
docs/                         平台开发、协议与迁移说明
scripts/windows/              Windows 构建验证与发布
scripts/macos/                macOS 验证与 .app 打包
.github/workflows/release.yml  推送版本 tag 后自动测试、打包与发布
VERSION                       本地构建的默认版本
Krill.Desktop.slnx             Windows 解决方案入口
Directory.Build.props         Windows .NET 公共构建配置
global.json                   Windows .NET SDK 选择
```

平台代码按原生语言分别维护；[协议与展示约定](docs/protocol.md)和 `contracts/fixtures` 用于对齐接口字段与金额计算。

## Windows 开发

从仓库根目录执行，需要 .NET 10 SDK：

```powershell
dotnet build Krill.Desktop.slnx -c Release
dotnet run --project apps/windows/src/Krill.Desktop -- --demo
.\scripts\verify.ps1
.\scripts\publish.ps1
```

原有解决方案入口与根目录 PowerShell 脚本入口继续保留。完整使用、登录、数据目录和发布说明见 [Windows 文档](docs/windows.md)。

## macOS 开发

将完整仓库复制到 Mac，安装 Xcode 与命令行工具，然后从仓库根目录执行：

```bash
# 执行 Swift 核心与 macOS 回归测试，再编译并检查应用入口
bash scripts/macos/verify.sh

# 创建本机架构的应用包
bash scripts/macos/build.sh debug

# 直接运行 Swift Package 的演示模式（不支持完整 .app 登录项行为）
swift run --package-path apps/macos KrillMac --demo
```

建议使用 Xcode 26 或更新版本，以编译 Liquid Glass 分支。也可以用 Xcode 打开 `apps/macos/Package.swift` 继续开发。具体的 .app 启动方式、签名说明和迁移验收见 [macOS 开发文档](docs/macos.md)。

## 数据与验证边界

macOS 会话输入支持 `⌘V`、`⌘A`，登录窗口还提供独立“粘贴”按钮；空剪贴板不会清空已有输入。

凭据只保存在本机：Windows 使用当前用户 DPAPI，macOS 使用钥匙串。两端默认提供系统浏览器登录，可复用已有 Krill / 第三方登录状态；官网目前没有桌面授权回传接口，因此登录后需按窗口指引手动导入一次会话。也可选择应用内临时浏览会话，支持第三方 HTTPS 跳转和共享会话的授权弹窗。自动读取只在精确的 `https://www.krill-code.com` 来源进行；自动读取和手动导入均调用 `/api/auth/me` 校验后才保存。接口请求不会自动跟随重定向；公告只显示原生文本摘要。

`--demo` 使用虚构数据和独立配置目录。`record.har` 仅作为本地协议参考，已忽略，不参与构建或打包。请不要将个人 HAR、会话、授权码或 token 提交到仓库。

两端在账号模式启动后自动检查 GitHub 最新正式 Release，之后每 6 小时检查一次；也可在设置页或菜单中手动检查，发现新版后点击“下载并重启更新”。推送 `vX.Y.Z` tag 会自动测试、构建并发布对应版本的 Windows x64 与 macOS 通用包，操作与运行限制见 [版本发布与应用更新](docs/releases.md)。

两端的验证结果与尚待完成的验收分别记录在平台文档中。接口来自官网当前实现，协议变化时需要同步更新两端客户端与共用样例。浏览器登录不会自动同步会话，跨设备会话同步尚未实现。
