# Krill Desktop · Windows

0.2.0 起使用 Rust / GPUI，与 macOS 共用业务和界面。面向 Windows 10 / 11 x64，需要支持 DirectX 11 的驱动；应用内登录另需 Microsoft Edge WebView2 Runtime。程序本身无需 .NET。

完整解压 ZIP 后运行 `Krill.Desktop.exe`。默认启动显示详情，`--background` 只常驻。相同数据目录的重复启动会唤出已有面板。`--demo` 运行独立演示账号，`--data-dir PATH` 可指定本地数据目录。

## 悬浮球和面板

- 88 个逻辑像素的圆球显示所选有效套餐剩余百分比；无有效套餐时显示充值余额，未知数据显示 `—`。
- 左键拖动移动，点击切换面板，右键显示原生菜单。悬浮球不激活、不占任务栏，可设置置顶。
- 开启贴边后靠左/右边缘收缩为 16 个逻辑像素宽的竖条；移入展开，移开约 650 毫秒后收缩。保存展开位置，按目标屏幕工作区与 DPI 定位。
- 详情支持固定、失焦隐藏、`Esc` 隐藏及标题区域拖动。托盘提供打开面板、显示/隐藏悬浮球、刷新、检查更新、官网和退出。
- 系统启动项使用当前用户注册表 `Run/KrillDesktop`，带 `--background`。设置操作不请求管理员权限。

共享用量图最近 48 个时段按本机时区排序，悬停整列及柱子上方空白均可读取时段、费用、请求数。上方读数区高度固定，移出恢复提示，零和未知费用仍可悬停。金额使用 Rust Decimal，不经浮点数解析；详细规则见 [协议](protocol.md)。

## 登录与本地数据

默认浏览器登录可复用已有网站登录状态，按指引导入会话；输入框支持 `Ctrl+V` / `Ctrl+A`，遮罩会话不能复制。应用内 WebView2 使用 InPrivate，共享本次授权弹窗的环境并保留 opener，关闭主登录窗口清理子窗口。只在精确 Krill 来源读取 token，并在验证身份成功后保存。

账号目录 `%LOCALAPPDATA%\KrillDesktop\Account`，演示目录为同级 `Demo`。`session.dat` 沿用旧版当前用户 DPAPI 及 entropy `Krill.Desktop/session/v1`；`settings.json` 兼容旧 PascalCase 字段。设置文件不包含 token，演示不会加载/保存真实凭据。

## 开发、验证与打包

安装 Rust 1.92+ MSVC 工具链、Visual Studio C++ Build Tools、Windows SDK。

```powershell
cargo test --workspace --locked
cargo run --locked -- --demo --show
./scripts/verify.ps1
./scripts/build-windows.ps1 -Configuration release
```

`verify.ps1` 运行合约、GPUI 点击/悬停/快捷键测试、DPAPI 测试、更新安装回滚测试、原生悬浮球窗口和 WebView2 OAuth 合成验证，并生成原生页面截图与 `artifacts/rust-native/verification.json`。

打包脚本生成 `dist/Krill.Desktop-X.Y.Z-win-x64.zip`，可用 `-Version X.Y.Z` 覆盖默认 `VERSION`。程序嵌入图标、文件版本、PerMonitorV2 DPI 清单。构建用二进制沿用 `KrillMac` 名称，发布复制为 `Krill.Desktop.exe`。

旧更新器通过程序集 DLL 读取版本，因此 Rust 构建工具额外生成仅含版本元数据的 `Krill.Desktop.dll`，打包时用 PowerShell `AssemblyName` 读取验证。客户端不加载此 DLL，也不依赖 .NET；新更新器使用 `krill-package.json`、附件大小与 SHA-256 校验包。

原生 CI 可验证窗口样式、左右贴边尺寸和锚点、调整窗口不抢焦点、真实 WebView2 弹窗和私有环境。多台实体显示器、不同驱动、真实第三方登录和跨版本联网安装仍需在实际用户环境验收。
