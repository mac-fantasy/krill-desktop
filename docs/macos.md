# Krill Desktop · macOS

0.2.0 起使用 Rust / GPUI，与 Windows 共用业务和界面。发布架构仅为 **arm64 / Apple Silicon**，最低 **macOS 15**。菜单栏、钥匙串、登录项和材质由 Rust 平台适配层调用原生系统 API。

菜单栏图标显示 Krill 标志和剩余额度进度环，支持可选百分比/余额及原生提示。点击展开五页详情，右键提供刷新、更新、官网和退出。面板定位在菜单栏图标所在屏幕，并限制在该屏幕工作区；失焦收起，固定面板后保持显示。应用不显示 Dock 图标。

macOS 26+ 使用 `NSGlassEffectView`；15+ 回退系统模糊。可在设置中关闭材质。登录项使用 `SMAppService.mainApp`，需从固定目录的打包应用运行；若系统要求授权，设置页提供登录项系统设置入口。

## 构建与验证

```bash
rustup target add aarch64-apple-darwin
bash scripts/verify.sh
bash scripts/build-macos.sh debug
bash scripts/build-macos.sh release
```

输出位于 `dist/Krill.Desktop-X.Y.Z-macos-arm64-{debug,release}/Krill Desktop.app`，release 额外生成 `dist/Krill.Desktop-X.Y.Z-macos-arm64.zip`。版本默认读取根目录 `VERSION`，可用 `KRILL_VERSION=X.Y.Z` 覆盖。默认 ad-hoc 签名，可通过 `KRILL_SIGN_IDENTITY` 指定签名身份；公证需要维护者另行配置。

`cargo run --locked -- --demo --show` 可运行共享界面。`bash scripts/verify.sh` 包含共享回归、真实 macOS 事件循环、五页渲染图及真实 WKWebView 的合成 OAuth 测试。`native-qa` 功能仅用于截图验收，生产构建不启用。测试宿主可能为 Intel，但发布脚本强制 `aarch64-apple-darwin` 并校验签名；包校验测试使用真实签名 arm64 可执行文件。

## 登录和旧版迁移

默认浏览器登录完成后按指引导入 Krill 会话，输入框支持 `⌘V`、`⌘A`，并禁止从遮罩框复制会话。应用内 WKWebView 使用非持久数据存储，OAuth 子窗口保留 `window.opener` 和本次私有会话，各窗口独立关闭。只在精确 Krill HTTPS 来源读取 token，再调用 `/api/auth/me` 验证身份后保存。

保留 bundle ID `com.krill.desktop.macos`、内部可执行名称 `KrillMac`、钥匙串 service `com.krill.desktop.macos.account` / account `krill-session`、原 `Account/settings.json` 路径和偏好字段。0.1.x 更新器只识别 universal 附件，因此升级到 0.2.0 必须手动安装一次；Intel 或 macOS 14 用户应留在旧版。Homebrew tap 须先完成附件及系统要求迁移，详见 [发布文档](releases.md)。

真实 Krill / GitHub / Apple 登录、第三方风控页面、实际跨版本联网下载重启仍需要使用真实账号和对应环境验收，合成 WebKit 测试不会声称覆盖这些服务行为。
