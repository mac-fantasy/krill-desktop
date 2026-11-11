# Rust / GPUI migration

目标：用共享 Rust 业务、共享 GPUI UI 和 Rust 平台适配完整替换 C# / WPF 与 Swift / SwiftUI。macOS 仅支持 Apple Silicon，最低系统允许提高。功能等价验证后删除旧实现、过时脚本和构建产物。

## 验收清单

- [ ] 固定可构建的 GPUI / 组件版本，macOS arm64 与 Windows x64 原生构建。
- [ ] 协议：数值字符串、十进制、未知值、8 MiB 响应限制、禁止重定向、固定凭据来源、错误分类。
- [ ] 套餐：当前周期金额、积分、共享按次窗口、结转、余额、有效性、自动月卡切换及本地到期。
- [ ] 统计：今日时区范围、缓存率、RPM/TPM、成功失败/思考 Tokens、渠道、最近48时段和悬停。
- [ ] 重置卡、公告纯文本/未读、部分失败保留数据。
- [ ] 状态：默认60秒、详情30秒、公告600秒、退避、唤醒、会话代次隔离、演示独立。
- [ ] 共享五页界面、欢迎页、登录、固定/关闭面板、Esc、快捷键、退出确认。
- [ ] 系统浏览器登录、脚本导出、JWT校验后保存；应用内临时WebView、OAuth共享弹窗/opener/独立关闭/来源限制。
- [ ] macOS 菜单栏动态进度环、百分比、提示、右键、隐藏Dock、面板定位/失焦、登录项、材质设置。
- [ ] Windows 托盘、无任务栏悬浮球、拖动、置顶、贴边收缩、悬停展开、多屏DPI、开机启动。
- [ ] 兼容旧偏好、DPAPI entropy和Keychain身份；凭据不进入偏好和日志。
- [ ] GitHub更新：版本/来源/大小/SHA-256/包身份、安装/回滚/重启，旧版更新兼容路径；保留旧mac包内部身份。
- [ ] Rust构建/测试/打包脚本、GitHub工作流、macOS arm64发布和Homebrew迁移说明。
- [ ] macOS实际运行和截图检查；Windows CI原生构建/行为检查。
- [ ] 删除旧C#/Swift实现和专用工程/脚本；整理资源、文档、忽略规则；清理工作空间并检查git diff。

## 已知迁移约束

Windows 0.1.8 更新器强制检查.NET程序集DLL；纯Rust包必须设计兼容包或过渡升级，不可假定同名EXE就能更新。macOS旧更新器仅识别macos-universal附件并检查`com.krill.desktop.macos`和`Contents/MacOS/KrillMac`；新平台仅arm64时仍需明确旧用户路径。

现有协议定义与合成样例为验收依据：`docs/protocol.md`、`contracts/fixtures`、`contracts/updates`。旧实现暂时作为对照保留，替代功能尚未验证时不可删除。

## 进展

- 2026-11-11：检查当前v0.1.8代码、工具链和既有功能。Rust 1.99.1可用，开始建立Rust workspace与共享核心。
- 首批实现：固定GPUI Kit 0.9.1；共享十进制模型、协议客户端、展示规则、套餐自动切换、账号状态和旧偏好别名；五页界面、默认浏览器会话导入、后台刷新与代次隔离；Keychain / DPAPI兼容代码；macOS菜单栏进度图标、右键菜单、弹出面板、登录项适配；Apple Silicon .app打包。
- 本机验证：18项共享核心测试及5项客户端测试通过，包括真实GPUI事件分派下的五页签整区点击、整列空白悬停/移出、固定图表布局，以及遮罩输入框Command-V/Command-A/禁止复制行为。macOS原生debug构建和ad-hoc签名包通过，演示进程启动运行。系统界面自动化未获应用访问权限，尚未取得桌面截图。
- 尚待实现/验证：Windows悬浮球/贴边/多屏和本机凭据适配；应用内WebView及OAuth弹窗；更新服务与旧版升级路径；材质设置；两端原生验收、正式工作流/文档替换；旧实现删除和工作空间清理。不能将当前里程碑视为完成重构。
- 后续实现：共享更新服务（检查、受限下载、SHA-256、ZIP路径与大小、macOS包签名/架构/身份、Windows清单、原安装组件回滚/重启）；macOS 26+原生NSGlassEffectView与15+模糊回退；标准编辑菜单、登录项设置、Esc/关闭隐藏面板。macOS原生冒烟测试实际启动并依次切换五页，`artifacts/rust-native/verification.json`报告通过。
- 远端原生CI已启动：分支`rust-gpui-refactor`，初始运行42975016672和修复后运行42975048187；以当前GitHub结果为准。测试宿主使用`gpui-pre`的test-support以避免GPUI Kit可选Fast测试依赖对未同步proptest版本的解析失败。
- 下一阶段：完成Windows悬浮球（原88px/16px、650ms、无激活、多屏DPI、托盘显示隐藏）及设置；应用内临时WebView/OAuth；升级兼容包/发布脚本；补齐原生验证后再删除旧目录。当前CLI二进制暂名`KrillMac`，Windows发布需明确`Krill.Desktop.exe`名称和清单。
- 当前证据：提交969a268的原生CI运行42975048187在macos-26、windows-latest均成功（这是更新服务接入前的快照）。更新服务接入后，本机`cargo test --workspace --locked`共26项通过，包括签名arm64应用的正向包校验、错误版本拒绝、ZIP穿越/路径别名拒绝。最新本机debug包构建与签名检查通过。
- 原生冒烟命令：`target/debug/KrillMac --smoke-test --data-dir /tmp/krill-rust-smoke-data --output artifacts/rust-native`。该模式只用演示数据、逐页切换并自动退出；不等价于截图和鼠标人工验收。GPUI交互单测另行覆盖点击、悬停、快捷键。
