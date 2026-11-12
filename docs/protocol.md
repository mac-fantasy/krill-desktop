# 两端协议与展示约定

网站来源固定为 `https://www.krill-code.com`。这份约定来自当前 Windows 实现与已有 HAR 分析；并非网站对外承诺的稳定 API。

## 请求

| 用途 | 方法与路径 | 认证 | 请求体 |
|---|---|---|---|
| 确认身份 | `GET /api/auth/me` | Krill 会话 | 无 |
| 套餐、额度、余额 | `GET /api/subscription` | Krill 会话 | 无 |
| 今日统计 | `POST /api/request-logs/stats` | Krill 会话 | `start_time`、`end_time`，UTC ISO8601 |
| 重置卡状态、上限与适用订阅 | `GET /api/reset-cards` | Krill 会话 | 无 |
| 公告 | `GET /api/public/announcements` | 无 | 无 |

已认证请求同时携带 `Authorization: Bearer …` 与 `Cookie: krill_jwt=…`，沿用网站现有要求。语言头为 `X-Language: zh-CN`、`Accept-Language: zh-CN`。公共公告请求不携带账号会话。客户端只提供固定接口与官网跳转枚举，不接受外部传入的任意凭据目标地址。

响应外层为 `{ "success": true, "code": 200, "data": ... }`。HTTP 401 或外层业务 `code: 401` 视为会话失效；403、429、网络错误和数据格式错误分别处理。拒绝自动 HTTP 重定向，每份响应最多读取 8 MiB。不要在异常、日志或调试描述中包含 token 或完整响应。

## 金额与额度

- 服务端金额可能为 JSON number、数字字符串或 null；Windows 使用 `decimal`，Swift 使用 `Decimal`。缺失值为未知，显示 `—`。
- 当前周期使用 `quota.daily_limit_usd`、`used_usd`、`remaining_usd`；即使是周 / 月套餐，额度上限字段也可能仍叫 `daily_limit_usd`。不要改用订阅生命周期的 `total_*` 字段。
- `billing_type` 为 `usd_daily` / `usd_weekly` / `usd_monthly` 时分别显示本日 / 本周 / 本月额度；其他金额型周期显示“当前周期额度”。
- `request_count` 使用 `limit_requests` / `used_requests` / `remaining_requests`，单位为次。积分套餐使用 `limit_credits`（缺失时回退到 `plan.total_credits`）、`used_credits` / `remaining_credits`。未知字段不推断额度。
- 剩余百分比 = `remaining / limit × 100`，限制在 0–100；缺失剩余或上限不大于 0 时百分比未知。只在绘制时用空进度表示未知，文字仍显示 `—`。
- `forwarded_remaining_usd` 单独列为结转，不并入当前周期剩余百分比。充值余额和福利余额分别展示。
- 套餐按 `consume_priority`、套餐 ID 升序排列；检查 `status == active`、生效时间已到、到期时间未到。月卡包括 `usd_monthly` 和 `duration_days` 为 28–31 天的套餐；其当前周期可识别剩余额度不大于零、且没有可用的金额结转时视为“额度已用尽”。未知额度不当作零，日 / 周套餐的展示规则保持原样。
- 保留仍可用的手动选择。所选月卡用尽或到期后，优先切换另一张已生效且可用的月卡；没有可用月卡时再按消费顺序选择其他有效套餐。自动切换会保存新 ID，旧卡额度恢复后不会抢回展示。若服务端已移除旧月卡，也会保留“优先另一张月卡”的选择规则。
- 过期、用尽、未生效、冻结及其他不可用套餐保留在列表中并置灰，分别显示状态，排除在选择器外。已知的生效 / 到期边界由本地计时检查触发，即使当时没有网络响应也会更新展示；额度消耗仍以接口刷新结果为准。没有有效套餐时显示充值余额，macOS 菜单栏使用金额标志，设置中可同时显示余额数字。
- 重置和到期时间来自服务端 ISO8601 字段，转换为本机时区。今日统计范围为本机当天零点到当前时间，按各时刻自己的时区偏移转换为 UTC，覆盖夏令时变化。

## 扩展账号与统计数据

- 重置卡来自 `/api/reset-cards` 的 `items`：`id`、`plan_name`、`status`（0 未使用 / 1 已使用 / 2 已作废）、`expired`、`expires_at`、`cap_usd`、`granted_usd`、`used_at`、`note` 和 `usable_subscriptions[].subscription_id`。已使用 / 作废优先于到期状态，未使用卡片结合服务端标记与本地时间判定过期。只有未过期、未使用且适用订阅非空的卡才计为可用；未知状态与缺失适用订阅不推断可用。应用只查询状态，使用与预览跳转官网。
- 狂蹬卡等 `request_count` 套餐展示订阅响应顶层 `request_count_quota` 的共享限额：`used_5h` / `limit_5h`、`used_weekly` / `limit_weekly`、`used_monthly` / `limit_monthly`，以及 `cycle_start` / `cycle_end`。这些是账号共享统计，不乘以卡片数。概览、菜单栏 / 悬浮球显示已知有效窗口中剩余比例最低的一档。没有完整使用数据时仅展示 `plan.rate_limit_5h` / `rate_limit_weekly` / `rate_limit_monthly` 的上限；不根据上限推断已用或剩余，既有 `quota.limit_requests` 等字段仍兼容。
- 今日统计新增 `failed_requests`、`cache_read_input_tokens`、`cache_creation_input_tokens`、`reasoning_tokens`、`rpm`、`tpm` 和 `channel_cache_rates[].channel_name` / `cache_rate`。渠道缓存率为 0–1 比例，界面转为百分数。
- 总缓存率优先使用 `cache_rate_read_tokens / cache_rate_eligible_tokens`；这两个专用字段未同时提供时，只有输入、缓存写入、缓存读取三个计数均已知才回退到 `cache_read_input_tokens / (input_tokens + cache_creation_input_tokens + cache_read_input_tokens)`。零分母、负数、不一致计数或缺失数据均显示 `—`；已知 0 命中且分母大于零显示 `0.0%`。不使用总 Tokens 或请求数充当缓存率分母。
- 重置卡在展开详情后随刷新加载，成功后至少间隔 30 秒再请求；接口暂不可用时保留当前账号上次成功结果，独立显示错误，不阻断套餐、统计和公告。401 沿用全局会话失效处理。切换账号 / 退出会清空扩展数据并拒绝旧代次回写；已知重置卡到期时间也在本地更新。

## 刷新与会话

默认后台 60 秒；展开详情时核心数据至多间隔 30 秒。统计在详情展开后加载；公告约每 10 分钟检查一次。失败按次数增加等待，最长约 300 秒；失败时保留当前进程中上次成功的数据，并标记离线。退出 / 切换账号时增加会话代次并取消旧任务，旧响应不得写入新账号面板。

登录沿用网站的 `/login` 页面，不限定身份提供商，支持邮箱、验证码、Google、GitHub、Linux.do 及服务商继续委托的 Apple / SSO 等流程。客户端不注册自己的 OAuth 应用，不接收或保存服务商密码。

默认入口使用系统浏览器，保留该浏览器的已有登录状态。当前官网公开前端没有桌面授权回传协议，无法只靠打开浏览器自动取回会话；用户需按登录窗口指引，在精确 Krill 来源运行只读取 `localStorage.krill_jwt` 并弹出复制框的脚本，再显式粘贴导入。脚本不发送网络请求，也不会在其他来源读取存储。导入只接受有长度上限的 JWT 文本（兼容 `Bearer ` 前缀），拒绝 URL、脚本和额外请求头；会话经 `/api/auth/me` 校验后才写入系统凭据存储。应用不扫描浏览器数据文件、不读取其 cookie，也不轮询剪贴板。

应用内登录作为可选入口：Windows 使用 WebView2 InPrivate；macOS 使用 `WKWebsiteDataStore.nonPersistent()`。顶层导航允许有效 HTTPS 页面（默认 443 端口，无 user info），使第三方及二次授权可以继续；初始 `about:blank` 只用于授权弹窗，其他非 HTTPS 顶层跳转被拒绝。弹窗保留提供商原有的 `window.opener`、共享 cookie / 存储和 `window.close()` 行为，最多同时打开 5 个。主窗口和弹窗均只在精确 Krill 来源读取会话，同时检查浏览器 URL 和脚本内 `location.origin`，再通过账号接口确认身份。主登录窗口关闭时取消校验并关闭所有授权弹窗。禁止嵌入式浏览器的提供商可使用系统浏览器入口。

客户端持久保存的登录会话只存在 DPAPI / Keychain 中；偏好 JSON 不存 token。演示模式不加载真实会话、不请求个人账号接口。两个操作系统的会话与设置分别维护。

## 共用样例

`contracts/fixtures/subscriptions.json`、`stats.json`、`subscription-selection.json`、`request-count.json`、`reset-cards.json` 为手工创建的虚构数据，不含真实账号。Windows 契约程序和 Swift `ProtocolTests` 读取同一组文件，验证：金额字符串、当前周期与生命周期区分、结转独立、按次 / 积分额度、未知值、统计小金额精度，以及月卡用尽 / 到期 / 未生效 / 结转 / 自动选择边界。

修改展示规则时同时更新两端实现、共用样例和断言。Swift 测试读取仓库内的相对路径，迁移时应复制完整仓库。
