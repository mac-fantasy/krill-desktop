# 版本发布与应用更新

Windows 和 macOS 使用公开仓库 `mac-fantasy/krill-desktop` 的最新稳定 GitHub Release。账号模式启动后检查，之后每 6 小时检查；演示模式只允许手动检查。更新提示出现在详情面板及设置页。

## 发布

根目录 `VERSION` 和 Cargo workspace 版本为开发默认版本。正式发布使用 `vX.Y.Z` tag，三个分量均为 0–65535，不接受前导零和预发布后缀。tag 版本通过 `KRILL_VERSION` 写入原生程序、Windows 版本资源和包清单、macOS Info.plist、ZIP 名称。

```bash
git tag -a v0.2.0 -m "Krill Desktop 0.2.0"
git push origin v0.2.0
```

[Release 工作流](../.github/workflows/release.yml) 在 Windows 和 macOS 分别运行 Rust 回归、原生 GPUI/WebView 验证与安装器回滚测试，再构建 release 包。两个平台成功后先创建含完整附件的 draft，再公开 Release。CI 使用仓库 `GITHUB_TOKEN`，客户端和安装包不包含 GitHub 凭据。

附件固定命名：

- `Krill.Desktop-X.Y.Z-win-x64.zip`
- `Krill.Desktop-X.Y.Z-macos-arm64.zip`

macOS 仅 Apple Silicon、macOS 15+；不再生成 universal 包。Windows 为本机 Rust 程序，无 .NET 运行依赖。默认包没有 Windows Authenticode 或 Apple Developer ID 公证；macOS 使用 ad-hoc 签名，可在本地指定 `KRILL_SIGN_IDENTITY`。

## 0.1.x 迁移

Windows 旧更新器强制读取 `Krill.Desktop.dll` 的程序集版本。`tools/krill-package` 用 Rust 生成只含 Assembly 名称和版本的兼容 DLL；不含 C# 业务或托管入口，新程序不加载它。打包脚本实际用 PowerShell `AssemblyName.GetAssemblyName` 验证版本。旧更新器替换同名 EXE 后可运行 Rust 程序，新更新器改用包清单验证。

macOS 旧更新器只接受 `macos-universal.zip`。不能将 arm64 包冒充 universal；Apple Silicon 用户从 Release 手动安装一次 0.2.0，Intel 或 macOS 14 用户继续使用 0.1.x。保留旧 bundle ID、内部可执行名、Keychain 身份和偏好路径，因此手动替换不要求重新配置账号。钥匙串访问是否需要系统确认取决于旧签名与本机权限。

[Homebrew tap](https://github.com/mac-fantasy/homebrew-tap) 的 cask 与更新脚本也需改为 `macos-arm64.zip`、`depends_on arch: :arm64` 和 macOS Sequoia；在 tap 迁移前用 Release 手动安装。项目仓库迁移前的 0.1.4 及更早版本还受旧下载来源限制，同样需要手动安装。

## 本地打包

```bash
KRILL_VERSION=0.2.0 bash scripts/build-macos.sh release
```

```powershell
./scripts/build-windows.ps1 -Version 0.2.0
```

macOS debug 只生成签名 `.app`；release 生成 `.app` 和 ZIP。Windows 打包生成 native EXE、兼容元数据 DLL、`krill-package.json` 和说明。

## 更新校验与回滚

检查更新的请求不跟随重定向，最多读取 1 MiB。仅接受更高稳定版本、精确仓库地址和当前平台附件；摘要必须是 GitHub 提供的 `sha256:`，大小必须有效。下载仅跟随有限次 HTTPS 跳转，流式核对大小和 SHA-256。解压限制条目数与总大小，拒绝路径穿越、路径别名和符号链接。

macOS 额外验证 `com.krill.desktop.macos`、`Contents/MacOS/KrillMac`、版本、arm64 架构和代码签名；Windows 验证包清单与原生 EXE 存在。源码运行、App Translocation、只读或无权限目录不支持原位更新；将应用安装到当前用户可写的固定目录。

安装脚本等待当前进程退出，带原启动参数重启。Windows 逐文件替换并保留不相关文件；macOS 在同卷准备新包后重命名替换。替换或重启失败时尝试恢复旧版。凭据和设置位于包外，不随升级删除。安装失败日志在系统临时目录 `krill-update.log`。

合成测试覆盖替换、参数引用、不相关文件保留和回滚，原生测试覆盖包签名与身份拒绝。真实 GitHub 跨版本在线下载/重启、服务商登录和不同实体设备仍需发布后的实际环境验收。
