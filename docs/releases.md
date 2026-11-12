# 版本发布与应用更新

Windows 和 macOS 均从 `mac-fantasy/krill-desktop` 的 GitHub Release 检测更新。发布仓库必须公开，才能使用匿名更新；当前仓库由维护者改为公开后启用，不在应用或安装包中保存 GitHub token。账号模式启动后自动检查，此后每 6 小时检查一次；菜单栏 / 托盘右键菜单和设置页均提供“检查更新”。演示模式不启动后台检查，但可以手动检查。

仓库由 `lljxww` 转入 `mac-fantasy` 后，`0.1.5` 起的 Windows 和 macOS 客户端均使用新地址。旧版严格校验旧仓库的下载 URL，可能将新位置的附件视为不匹配；需要从新 Release 或通过 macOS Homebrew Cask 安装一次 `0.1.5` 或更新版本。迁移不改变 bundle ID、钥匙串标识或偏好目录。

发现更新后，详情面板显示更新提示。点击“下载并重启更新”后，应用下载对应平台的 ZIP，校验附件大小、SHA-256 和包内版本，再退出、替换程序并带原启动参数重启。检测或下载失败时可重试，替换或启动失败时安装组件会尝试恢复原版本。会话与偏好保存在应用包以外，继续使用原有数据目录。

## 推送 tag 发布指定版本

提交代码后创建并推送带版本的 tag，例如：

```bash
git add .
git commit -m "feat: add automatic updates"
git tag -a v0.1.5 -m "Krill Desktop 0.1.5"
git push origin HEAD v0.1.5
```

推送 `vX.Y.Z` 触发 [Release 工作流](../.github/workflows/release.yml)。三个版本分量均为 0–65535 的整数，不接受前导零或预发布后缀，如 `v01.2.3`、`v1.2.3-beta`。tag 是本次发布版本的来源，打包时自动写入 Windows 程序程序集、macOS `Info.plist` 和压缩包名称，无需先修改源码中的版本。

工作流依次执行：

1. 校验 tag 格式。
2. 在 Windows 执行核心测试、更新安装组件测试与 WPF 验证，打包自包含 x64 程序。
3. 在 macOS 执行 Swift 测试、更新安装组件测试与 Release 编译，打包 Intel + Apple Silicon 通用应用。
4. 两端均成功后，创建 GitHub Release、生成发布说明并上传三个 ZIP。附件上传完成后再公开 Release。

`0.2.1` 恢复 C# / WPF 与 Swift / SwiftUI。正式安装包为 Windows x64 与 macOS universal；另发布由相同 Swift 代码编译的真实 arm64 包，使已安装 `0.2.0` 的更新器能识别并切回 SwiftUI。Windows 包内 `krill-package.json` 仅描述版本、平台和可执行文件，供该版本更新器校验；包内实际程序集和程序均为 .NET / WPF。

发布附件的固定命名：

- `Krill.Desktop-X.Y.Z-win-x64.zip`
- `Krill.Desktop-X.Y.Z-macos-universal.zip`
- `Krill.Desktop-X.Y.Z-macos-arm64.zip`（相同 SwiftUI 客户端的 Apple Silicon 更新包）

发布任务只使用仓库提供的 `GITHUB_TOKEN`；工作流的 `publish` job 声明 `contents: write`。仓库需启用 GitHub Actions，并允许该任务的写入权限。没有配置额外凭据、PAT 或分发服务器。已发布版本不覆盖重发；修复后应使用更高的新 tag。两端更新客户端忽略草稿、预发布、相同版本和较低版本。

## 本地打包

仓库根目录 [VERSION](../VERSION) 是本地开发 / 打包的默认版本。可以指定本次构建版本：

```bash
KRILL_VERSION=0.1.5 bash scripts/macos/build.sh release --universal
```

```powershell
.\scripts\publish.ps1 -Version 0.1.5
```

macOS Release 打包同时生成 ZIP；Debug 打包仅生成 `.app`。Windows 默认自包含；`-FrameworkDependent` 包不作为自动更新附件，后续更新会使用自包含正式包。

## 更新校验与运行限制

客户端通过匿名的 [GitHub Releases API](https://docs.github.com/en/rest/releases/releases#get-the-latest-release) 查询 `/releases/latest`，仅接受仓库精确下载地址、平台名称和 `sha256:` 摘要均匹配的附件。GitHub 提供的附件摘要用于校验下载内容；没有摘要或缺少对应平台附件时，客户端显示包未就绪并保留当前应用。

macOS 从打包后的 `.app` 执行更新，并额外使用 `codesign --verify --strict` 校验下载的应用。请先将应用移动到可写的固定目录（如用户可写的 Applications 目录）；App Translocation 临时路径和只读磁盘镜像不支持原位更新。默认 CI 包沿用 ad-hoc 签名，尚未配置 Developer ID、公证或 Windows Authenticode 签名；首次安装仍按原有平台方式打开。要分发签名版本，可扩展工作流中的签名步骤；macOS 本地构建继续支持 `KRILL_SIGN_IDENTITY`。

Windows 使用独立 PowerShell 安装组件等待主进程退出，再逐文件替换，保留安装目录中的其他文件；失败时根据备份恢复已替换文件。应用应放在当前用户可写目录。macOS 在应用所在卷准备新包，重命名替换并保留回滚副本，直到重启请求成功。

安装失败的诊断文件位于系统临时目录 `krill-update.log`。源码运行模式和现有旧版应用不会自动获得更新能力，需先安装含本功能的版本。首次 GitHub 实际发布、跨版本在线下载重启、Windows 原生 GUI 以及不同签名 / 旧系统上的更新，仍需在对应环境验收。
