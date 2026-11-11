# Rust / GPUI 迁移验收

目标：共享 Rust 业务、共享 GPUI 界面和 Rust 平台适配替换 C# / WPF 与 Swift / SwiftUI。macOS 仅 Apple Silicon，最低 macOS 15。旧实现的完整快照保留在 `v0.1.8` tag。

## 功能对照

| 范围 | Rust 实现与验证 |
|---|---|
| 依赖与构建 | Cargo workspace / lock；GPUI Kit 0.9.1、GPUI 0.3.12；两端原生构建，macOS 发布固定 arm64 |
| 协议 | 数值字符串、精确 Decimal、未知值、8 MiB 上限、固定来源、禁止重定向、错误分类；共享合约测试 |
| 套餐 | 当前周期金额/积分/按次、共享窗口、结转/余额、有效性、月卡自动切换及本地到期；合约与状态测试 |
| 统计 | 本机今日范围、缓存口径、RPM/TPM、成功失败/思考 Tokens、渠道、最近 48 时段；整列空白悬停与固定读数区的 GPUI 事件测试 |
| 重置卡与公告 | 状态、恢复上限/历史、适用订阅、纯文本摘要、未读及部分失败保留；共享合约测试 |
| 状态 | 默认60秒、详情30秒、公告600秒、退避、唤醒后刷新、账号代次隔离、演示隔离 |
| 共享界面 | 五页、欢迎页、登录、固定/关闭、失焦、Esc、退出账号确认；五页整区域点击与原生渲染图 |
| 会话导入 | 系统浏览器、脚本导出、JWT 格式与 API 验证后保存；遮罩输入支持两端粘贴/全选并禁止复制 |
| 应用内登录 | Wry；实际检查私有引擎后才导航；HTTPS 来源限制、OAuth opener/共享临时会话、postMessage、独立关闭/父关闭清理；真实引擎的合成页面测试 |
| macOS | 进度环、百分比、提示、右键、隐藏Dock、图标所在屏幕定位/工作区限制、登录项；26+原生玻璃与15+模糊 |
| Windows | 托盘、无激活悬浮球、原生拖动、置顶、88/16逻辑像素、650ms悬停展开/收缩、两侧锚点/多屏DPI、原生提示、开机启动、重复启动唤出已有面板 |
| 本地兼容 | 原偏好路径与字段别名、DPAPI entropy、Keychain service/account；原子设置写入；凭据不进入偏好和日志 |
| 更新 | 版本/精确来源/大小/SHA-256、解压路径/数量/大小、包身份/架构/签名；安装替换/回滚/参数引用测试 |
| 打包发布 | Rust验证/打包脚本和GitHub工作流；Windows程序集元数据兼容旧更新器，macOS旧版手动迁移及Homebrew调整说明 |

## 证据与运行方式

```bash
cargo test --workspace --locked
bash scripts/verify.sh
bash scripts/build-macos.sh release
```

```powershell
./scripts/verify.ps1
./scripts/build-windows.ps1
```

本机 Rust 测试30项（共享核心18、桌面11、包工具1），发布/安装器Python测试4项。Windows桌面测试还包含原生DPAPI和单实例检查。原生冒烟实际运行GPUI事件循环、GPU渲染与WKWebView/WebView2；`native-qa`生成五页PNG和`artifacts/rust-native/verification.json`，正式包不包含该功能。

macOS原生回归、五页渲染、WebKit弹窗和Apple Silicon签名打包已多轮通过。Windows在原生CI验证；曾发现并修复测试专用窗口API误用、平台条件编译遗漏，以及GPUI与应用重复嵌入清单的链接冲突。最终以分支最新 [Rust 工作流](https://github.com/mac-fantasy/krill-desktop/actions/workflows/rust.yml) 结果为准。

## 升级约束与验证边界

Windows旧更新器检查.NET程序集DLL。构建工具仅生成版本元数据标记，运行时不加载DLL、不依赖.NET；PowerShell实际读取AssemblyName以验证旧更新器的兼容路径。新程序通过清单、摘要和包身份校验。

macOS旧更新器只识别universal附件。0.2.0只发布arm64，需手动安装一次；不得将arm64冒充universal。bundle ID、内部可执行名、钥匙串身份和偏好路径保留。Homebrew tap需同步架构、系统要求与附件名。

合成测试不需要真实账号。真实Krill/第三方登录服务商风控、跨版本联网下载重启、实体多屏热插拔和不同显卡驱动仍是实际环境验收范围，不能用合成页面通过来代替。

## 收尾

- [ ] 最新Windows和macOS原生CI均通过。
- [ ] 删除旧`apps/`、C#/Swift工程和专用脚本，只保留共享资源与安装器回归。
- [ ] 清理旧产物与中间缓存，保留当前可审阅的构建结果，检查工作区与git diff。
