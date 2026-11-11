# Krill Desktop

Krill 个人账号用量助手。Windows 和 macOS 共用一套 **Rust 业务逻辑与 GPUI 界面**，快速查看套餐剩余、余额、今日用量、重置卡及公告。

- Windows x64：托盘和桌面悬浮球；支持拖动、置顶、贴边收缩、悬停展开、多屏 DPI 和开机启动。
- macOS 15+、Apple Silicon：菜单栏进度环、可选百分比、点击展开面板、登录项。macOS 26+ 使用原生 Liquid Glass，15 及以上提供模糊回退。
- 共享五页界面：概览、套餐、用量、公告、设置。用量图按本机时区显示最近 48 个时段，悬停整列可查看费用和请求数。
- 登录可选默认浏览器导入会话或应用内临时浏览会话；支持第三方 HTTPS 授权弹窗。JWT 验证成功后才保存到 Keychain / DPAPI。
- GitHub Release 更新支持大小、SHA-256、包身份校验和安装失败回滚。

## 安装与升级

从 [GitHub Releases](https://github.com/mac-fantasy/krill-desktop/releases/latest) 下载对应平台 ZIP。

Windows：完整解压后运行 `Krill.Desktop.exe`，无需 .NET。应用内登录需要 Microsoft Edge WebView2 Runtime，系统浏览器登录不依赖它。建议使用 Windows 10 / 11 x64、支持 DirectX 11 的显卡驱动。

macOS：将 `Krill Desktop.app` 放入应用程序目录，打开后常驻菜单栏。**0.2.0 起仅支持 Apple Silicon，最低 macOS 15。** 首次从 0.1.x 升级到 Rust 版本须手动安装一次，因为旧更新器只识别通用包；账号凭据与设置沿用原位置。默认发布包采用 ad-hoc 签名，尚未进行 Developer ID 公证，首次打开可能需要在系统设置中允许。

Homebrew 安装源已同步到 0.2.0：

```bash
brew install --cask mac-fantasy/tap/krill-desktop
# 已有 Homebrew 安装升级
brew update
brew upgrade --cask --greedy mac-fantasy/tap/krill-desktop
```

[统一 tap](https://github.com/mac-fantasy/homebrew-tap) 已同步 arm64 附件、校验值及系统要求。0.1.x 的 macOS 用户可通过上述 Homebrew 命令或 Release 手动迁移一次；具体约束见 [版本发布](docs/releases.md)。

## 使用

点击菜单栏图标或悬浮球打开面板。在“套餐”页选择显示的有效套餐，在“设置”页调整刷新间隔、固定面板与系统集成。面板失焦自动收起，固定后保持显示；`Esc` 和关闭按钮收起面板，托盘菜单或设置页可退出程序。

默认浏览器登录可以使用官网支持的任意方式和二次验证。登录后按应用指引，在 Krill 官网控制台运行“复制会话导出脚本”提供的脚本，再将弹出的会话粘贴回客户端。macOS 支持 `⌘V`、`⌘A`，Windows 支持 `Ctrl+V`、`Ctrl+A`；遮罩输入框禁止复制会话。会话等同登录凭据，请勿分享。

常驻默认每 60 秒刷新；面板展开时最多每 30 秒同步，公告约每 10 分钟检查。失败后退避并保留当前账号的已有数据；唤醒后及时同步。会话失效后清除本机凭据并提示重新连接。演示模式只使用虚构数据，数据目录与账号模式分开：

```bash
cargo run --locked -- --demo --show
```

## 开发与验证

需要 Rust 1.92+。依赖由 `Cargo.lock` 固定，GPUI Kit 0.9.1 / GPUI 0.3.12；Windows 需 MSVC C++ 工具链及 Windows SDK，macOS 需 Xcode Command Line Tools。仅在对应原生平台构建客户端。

```bash
cargo test --workspace --locked
cargo run --locked -- --show

# macOS：回归、安装器测试、真实事件循环/WebKit及五页渲染截图
bash scripts/verify.sh
rustup target add aarch64-apple-darwin
bash scripts/build-macos.sh release
```

```powershell
# Windows：回归、安装器测试、原生窗口/WebView2及渲染截图
./scripts/verify.ps1
./scripts/build-windows.ps1
```

`native-qa` 是验收专用功能，生成 `artifacts/rust-native/verification.json` 和页面 PNG；正式包不启用。测试包含整页签点击、图表空白区域悬停、登录输入快捷键、套餐选择与本地到期、接口合约、更新包校验、原生 OAuth 弹窗与关闭行为。原生冒烟使用合成页面和演示数据，不访问个人账号，也不能代替真实服务商登录验收。

## 目录

```text
crates/krill-core/       共享协议、模型、展示规则、账号状态和更新合约
crates/krill-desktop/    GPUI 五页界面、系统托盘、窗口、WebView、凭据和更新服务
tools/krill-package/     Rust 构建工具，生成 Windows 升级清单与旧更新器兼容元数据
resources/              图标、Info.plist、Windows 清单及安装脚本
contracts/              不含凭据的合成协议样例与更新合约
scripts/                原生验证、打包和安装器回归
```

[协议与展示约定](docs/protocol.md) · [Windows](docs/windows.md) · [macOS](docs/macos.md) · [发布与更新](docs/releases.md) · [迁移验收](docs/rust-migration.md)

凭据只保存在本机系统存储中，不写入设置或诊断日志。正常模式数据位于 macOS `~/Library/Application Support/KrillDesktop/Account` 或 Windows `%LOCALAPPDATA%\KrillDesktop\Account`；演示目录为同级 `Demo`。退出账号清除本机会话与当前面板数据，不撤销官网的登录。客户端仅向固定 Krill API 来源发送凭据，禁止接口重定向；公告以纯文本呈现。
