# rust_learn_my_api 鉴权改造 —— 进度记录、Review 与后续路线

> 记录时间：2026-09-30　最近更新：2026-10-07
> 项目只读，代码由本人手动修改
> 前置：已完成 rustlings，按 plan.md 学习 axum web api；本次为 JWT 鉴权 + auth 体系改造

---

## 〇、当前状态速览（2026-10-07）

- 鉴权改造已完成并通过 review，工作树干净，已提交：
  - `d90abc2 基本完成鉴权部分的改造`
  - `621604c 增加现阶段AI审计和建议`
- 自 09-30 review 后 `src/` 未再变更（11 个模块文件，无未提交改动）。
- **结论：当前项目的核心目标已基本达成**——axum 的核心概念已用掉大半，再做两项收尾（见第三节）即可转场，不必按 plan.md 一路走到 Docker/CI。

---

## 一、改造总览

原有项目是无鉴权的裸 CRUD（`State<SqlitePool>` + 统一响应/错误处理）。本次改造**没有动依赖和数据库结构**（`argon2` 0.6、`jsonwebtoken` 11.1、`password_hash` 列早已就位），重点全在代码结构上。

### 定稿的接口清单

```
POST   /auth/register     注册（email, password, name）→ 邮箱占用 409，成功 201
POST   /auth/login        登录 → 返回 token + 用户信息
GET    /me                当前用户信息（CurrentUser 提取器）
PATCH  /me                改资料（name, message，均可选）
DELETE /me                注销
PUT    /me/password       修改密码（old_password, new_password）→ 必须登录
GET    /users/{id}        公开读（模拟"公开主页"），无鉴权
GET    /health            健康检查
```

### 核心设计决策（含理由，防止将来忘记为什么）

| 决策 | 理由 |
|---|---|
| **提取器而非中间件做鉴权**（`CurrentUser`） | 类型即权限：handler 签名里有没有 `CurrentUser` 一眼可辨；编译器保证不会漏。中间件方案留给将来 `/admin` 的角色校验 |
| **轻版提取器**：`CurrentUser { id: i64 }`，只验签不查库 | 认证（JWT）与取数（数据库）职责分离；与窄依赖原则同构；避免双查询；可测性更好 |
| **`/me` 替代 `PUT /users/{id}` 自助接口** | 从结构上消灭越权可能（路径里没有 id，"用不可变的结构替代可遗忘的检查"） |
| **`/admin/*` 前缀留给将来** | 角色校验用 `nest` + `layer` 在路由层批量生效，`403 Forbidden` 在那时登场 |
| **PATCH /me 部分更新，PUT /me/password 凭据替换** | PATCH = 部分字段更新（Option 字段）；密码是单值凭据整体替换 + 需验证旧凭据，属"动作"，用 PUT |
| **JWT 只存 id（`sub`），资料必查库** | payload 是 Base64 可读，不放 PII；id 不可变；资料以库为准防陈旧 |
| **实体/DTO 分离**：`UserEntity`（全列含 hash）+ `UserResponse`（无 hash）+ `From` 转换 | 查询函数只按"查找方式"增减；暴露字段的变化只动 DTO 层 |
| **`UserEntity` 只派生 `Clone`**（无 Serialize） | 结构上杜绝 hash 被序列化出网；出门唯一路径是 `From → UserResponse` |
| **service 层窄依赖**：收 `pool` / `encoding_key` / `ttl` 等具体参数，不收 `&AppState` | `Zero To Production` 风格；签名即依赖清单；换容器零成本。窄依赖只约束 service 层以下，handler 收 `&AppState` 天经地义 |
| **service 返回完整业务产物**（`UserEntity` / `LoginResponse`） | 业务动词闭环；复用方免粘合；事务边界将来收在 service 内 |
| **service 收 owned 请求体**（如 `update_me` 收 `UpdateUserRequest` 按值） | 要"用掉"字段就收值（直接 move 免 clone），只读才收 `&` |
| **错误翻译边界**：sql.rs 只出 `sqlx::Error` → auth.rs 翻译成 `ApiError` | 每层只翻译下一层的语言；UNIQUE 冲突 → 409 的翻译发生在 auth.rs |
| **请求体按接口建模**（`UserRegister` / `UserLogin`），不复用表结构 | 密码只在注册/登录/改密时出现；请求 DTO 只派生 `Deserialize` |
| **`hash_password` / `verify_password` 为自由函数**（非方法） | 无状态通用操作优先自由函数；DTO 回归纯数据载体 |
| **argon2 0.6 单参数 `hash_password(pwd)`** | 0.6 新签名：内部自动生成随机盐（PHC 字符串自包含盐与参数），verify 无需传盐 |
| **JWT_SECRET fail-fast**（`expect`，无默认值） | 密钥缺失要启动即炸，不能静默回退到公开字符串带病运行 |
| **登录失败统一 401** | 防邮箱枚举；SQL 真故障 → 500（`ApiError::Sql`），不伪装成 404 |

### 落地的分层职责

```
jwt.rs     → Claims + sign/validate（HS256，exp 自动校验）
state.rs   → AppState { pool, access_ttl: TimeDelta, encoding_key, decoding_key }
             （Config 与 AppState 是接力关系：运行时用的进 state，启动期用完即弃）
auth.rs    → CurrentUser 提取器 + 密码工具函数 + 6 个 service 函数
             （register / login / update_me / delete_me / update_password_me）
handler.rs → 三行壳：提取参数 → 调 service → .into() + ApiResponse
             （Json 永远放参数最后；get_user_id 公开读不走 service）
sql.rs     → 只按"查找方式"提供函数，全部返回 UserEntity 或操作结果
             （UPDATE 部分更新用 COALESCE(?, message)）
models.rs  → UserEntity / UserResponse / 请求体 DTO / LoginResponse
error.rs   → ApiError（含 Conflict/Password 等）+ IntoResponse 统一映射
```

---

## 二、Review 结论（2026-09-30）

**整体通过，无编译级问题，架构原则基本全部落地。** 亮点：分层闭环完整、`UserEntity` 无 `Serialize` 的类型级防线、错误翻译位置正确、`jwt.rs` 修掉了 `Ok(?)` 冗余。

### 遗留点（按重要性）

1. **`message` 永远无法被清空**（语义缺口，当前可接受）
   - 三态模型缺一态：`Some(x)`=设值、`None`=不动，没有"清空回 NULL"的表达
   - 将来解法引子：JSON Merge Patch 三态（缺席/null/值），Rust 里需 `Option<Option<T>>` 或手工解析——留作练习
2. **`get_me` 绕过 service 层**（对称性）
   - handler 里直接查库 + `if let`；建议下沉为 `auth::get_me(pool, id)`
   - 顺带：`get_user_id` 的 `if let Some/else` 可收敛成 `.ok_or(ApiError::NotFound(..))?`
3. **`sql::get_all_users` 死代码**
   - 无调用者，binary crate 会报 dead_code warning；建议先删，admin 里程碑时复活
4. **`DELETE /me` 返回 200 + `data: null`**
   - 可选练习：改成 **204 No Content**，体会"统一信封的合法例外"（返回类型换 `StatusCode`）
5. 小项：`old_password` 可改名 `current_password`；config 的 ttl `unwrap_or` 静默吞错（已知，暂不管）；DTO 分居 auth.rs / models.rs 两处（有意为之，可接受）

### 验收清单（改动后手动跑一遍）

```
1. POST /auth/register            → 201，返回用户 JSON（无 password_hash 字段）
2. 同邮箱再注册                    → 409
3. POST /auth/login               → 200，拿到 token
4. 密码错登录                      → 401，提示不区分"用户不存在/密码错"
5. GET  /me（无 token）            → 401
6. GET  /me（带 Bearer）          → 200
7. PATCH /me 只改 name            → 200，email/message 不变
8. PUT  /me/password 旧密码错      → 401；旧密码对 → 200，新密码可登录
9. DELETE /me 后带旧 token 访问    → 404（"注销后 token 靠查库兜底"现场验证）
```

---

## 三、当前项目的收手标准（2026-10-07 新增）

### 3.1 判断标准

这个项目的使命不是"做完一个生产级后端"，而是：**把 axum 的核心概念各用一遍 + 用 Rust 写一遍类型/所有权密集的代码**。

按这个标准盘点，axum 版图里只剩**两块没碰到**，做完就可以收手：

| 收尾项 | 为什么值得做 | 会练到 |
|---|---|---|
| **admin + role**（建议做） | 中间件 / `tower::Layer` 是唯一还没碰的 axum 核心概念 | `Router::nest` + `layer`、`403`、增量迁移、role 写进 Claims 的权衡 |
| **集成测试**（建议做） | "会写玩具"和"会写项目"的分水岭，也是窄依赖设计兑现红利的地方 | `#[tokio::test]`、内存 sqlite、AAA 结构、测试数据构造 |
| refresh token（可选） | 业务设计题而非语言题，但能练状态设计与迁移 | token 版本号 / 黑名单、token 轮换 |

### 3.2 建议降级或跳过

| 项目 | 判断 |
|---|---|
| CORS | 一个 `CorsLayer` 的事，真接前端时顺手加，不值得单独立项 |
| tracing | 学的是工程化不是 Rust，下个项目（Agent）里自然会用到 |
| 阶段 10 目录化分层（routes/services/repositories） | 纯体力活，思想已在 auth.rs / sql.rs 用上，收益低于成本 |
| Docker / CI / 部署 / HTTPS | 运维知识，与 Rust 无关，**学习期性价比最低——plan.md 后半段这些直接跳** |
| gRPC / 微服务 / 消息队列 | 同上，等有真实需求再说 |

> 一句话：**再做 admin+role 和一组集成测试，这个项目就够本了**；之后它作为"资产"保留，下一个项目会直接复用它。

---

## 四、下一个项目：Rust Coding Agent（2026-10-07 定案）

### 4.1 结论

**采纳"做 Agent"这条路线，但附加三个限定条件**：不用现成 Agent 框架、先做非流式 + CLI、在隔离目录里跑。

理由：
- 和当前项目**天然衔接**——Axum/SQLx/鉴权/分层这些已攒下的能力，在 V2 阶段直接复用（agent 的 HTTP 层 + 会话持久化 + 多用户隔离）。
- 会真正逼你练到 Rust 特有的东西：**trait object 动态分发**、**serde tagged enum 建模工具参数**、**async 取消与超时**、错误类型设计。
- 产出是**可用的东西**（一个能帮你读代码、跑 cargo 的 CLI），比再写一个 Todo List 有意思。
- 本地模型条件已具备：llama.cpp / CUDA 环境在手，可直接打本地 OpenAI 兼容端点，无需云端 key。

### 4.2 它真在练什么（分两栏看，避免预期错位）

| 真在练 Rust | 换 Python/TS 写是同一件事 |
|---|---|
| `Tool` trait 动态分发（`Box<dyn Tool>` + 注册表） | 调 LLM 的 HTTP 请求 |
| serde tagged enum 表达工具参数（**最能体现 Rust 表达力**） | prompt 拼装 |
| 错误类型设计（工具失败也要回灌给模型） | JSON Schema 描述 |
| `Arc` / 共享状态（工具注册表、会话状态） | 流式 chunk 的字符串处理 |
| 取消与超时（`CancellationToken` + `tokio::select!`） | |
| async 生命周期、跨 await 的借用 | |

### 4.3 三处要打折的地方（避免踩坑）

1. **"核心代码也就这么一个循环"是极度乐观的说法。** 那 20 行是骨架，真正的活在外面：
   - 流式：手写 SSE 解析，`tool_calls` 分片跨 chunk 累积（最麻烦的一块）
   - 可靠性：重试 / 限流 / 超时 / 可中断（Ctrl-C 要能停）
   - 上下文：历史裁剪、token 计数、assistant/tool 消息配对协议
   - 时间预期按 **两周以上** 算，不是"一个周末"
2. **本地小模型的 tool calling 可靠性差。** 7B 级模型多轮 tool call 容易不听话，会分不清"我循环写错了"还是"模型不肯调工具"。对策：先用云端强模型验证循环正确性，再切本地；或不依赖原生 tool call，让模型输出 JSON 代码块自己解析（早期 agent 的做法，本身是很好的练习）。
3. **框架生态的名字不重要，别照抄教程。** 各家 agent crate 更新极快，写之前先 `cargo search` / docs.rs 核对当前版本 API，或干脆不用。

### 4.4 必须自己立起来的安全边界（最容易被忽略）

Agent 能 `write_file` + `run_command`，等于把"任意文件写入 + 任意命令执行"交给一个概率模型。学习版必须：

- **只在专门的 scratch 目录里跑**，不指向真实项目（练习项目本身是只读的，另开一个玩具目录给它折腾）
- **写操作与 shell 走人工确认**：先打印 diff / 待执行命令，回车才执行——这个"人机确认"也是真实 agent 的标准设计
- **工具白名单**，不做万能 shell；命令限定在 `cargo check` / `cargo test` 这类
- 结果输出要**截断**，别把整个文件塞回上下文

### 4.5 备选方向 B：反向代理 / 负载均衡器

| | 方向 A：Agent | 方向 B：负载均衡器 |
|---|---|---|
| Rust 浓度 | 中（一半时间在 LLM 管道） | 高，纯 Rust |
| 与当前项目关系 | 直接复用（同一项目长出来） | 同一技术栈的下一层（tokio/hyper/tower） |
| 最硬的知识点 | trait object、流式并发、状态机 | tokio 并发、`Arc`/`RwLock`、手写 `tower::Service`、故障转移 |
| 反馈速度 | 慢（外部依赖） | 快（改一个文件就能看到效果） |
| 产出价值 | 有，可自用 | 无，纯练习 |

**取舍**：要"产出 + 覆盖面"选 A；要"把并发练到手"选 B。两者不互斥——B 的最小版本（TCP 代理 + 轮询 + 健康检查 + 优雅关停）一个周末能出来，之后再做 A 会更顺。

---

## 五、落地路线：V0 → V3（针对本机环境）

```
V0  第一个周末 · 无框架 · 非流式 · CLI
    单文件 ~300 行：
      messages: Vec<Message>
        → POST /v1/chat/completions
        → 若 tool_calls 非空：逐个执行 → 结果 push 回 messages → 继续循环
        → 若为空：返回文本，结束
    4 个工具：list_files / read_file / write_file / run_command
    目标：跑通 "读文件 → 改 → cargo check → 读报错 → 再改"
    练到：Tool trait 动态分发、serde tagged enum、错误类型
    安全：scratch 目录 + 写/shell 操作需确认

V1  练真本事
    · 手写 SSE 流式解析（含 tool_calls 跨 chunk 累积）
    · CancellationToken 取消 + tokio::select! 超时（Ctrl-C 可中断）
    · 上下文管理：历史裁剪 / 消息配对协议

V2  接回现有项目（复用最大化）
    · 会话与消息落 sqlite：复用 sqlx 层 + 迁移工作流
    · Axum 包一层：POST /chat + SSE / WebSocket 推流
      （WebSocket/SSE 是 axum 版图里唯一还没碰的概念）
    · 鉴权直接复用 CurrentUser → /conversations 天然按用户隔离

V3  继续长
    · MCP
    · 多后端抽象（云端 + 本地 llama.cpp 切换）——到这一步才真正知道抽象该划在哪
```

---

## 六、更远期（原 plan.md 对应，已按 2026-10-07 判断调整）

- ~~阶段 10 分层重构~~ → 降级：思想已用上，收益低于成本
- ~~阶段 13 Docker/CI~~ → 跳过：运维知识，非 Rust 学习目标
- Refresh token（含"注销后 token 失效"的系统化解法：token 版本号 / 黑名单）
- 集成测试（`#[tokio::test]` + 内存 sqlite）——已提到第三节，建议尽快做
- 可选深水区：`QueryBuilder` 动态拼 SQL（关键词搜索 + 分页场景）

---

## 七、过程中沉淀的通用经验（Rust / Web 通用）

- **`?` + `Ok(...)` 是冗余写法**；直接返回表达式
- **引用背后的字段不能 move**：service 要消费字段就收 owned 值，`clone` 出现时先问"我在防什么"
- **小值纯数据类型派生 `Copy`**（`TimeDelta`、`CurrentUser { id }`），借用/clone 纠结自动消失
- **chrono `DateTime` 只能加 `TimeDelta`**，不能加 std `Duration`；`Duration::seconds` 已弃用，用 `try_*` 系列
- **check-then-act 竞态**：有 UNIQUE/FK 约束的地方让数据库说话（直接 INSERT，靠约束兜底映射错误码）
- **明文密码的死亡地 = 哈希的发生地**：请求体传明文，service 内消费成 hash，永不序列化/日志/存储明文
- **Result 优于 bool**：bool 抹掉失败原因；`Err` 自带原因 + `?` 即"失败到此为止"
- **对"新版库 API"的断言不可靠时**：直接 grep `~/.cargo/registry` 里的依赖源码（本机 `D:\Rust\.cargo\registry\src\`），编译器和源码不会说谎
- **HTTP 动词三分**：POST 创建/触发动作（集合 URI）、PATCH 部分更新（Option 字段）、PUT 整体替换（单值凭据、upsert）
- **axum 参数提取顺序**：消费 body 的提取器（`Json`）必须放参数最后
- **凡 "换传输协议还成立" 的逻辑归 service**，只有 HTTP 才懂的（状态码/信封/header）归 handler
- **项目驱动学习的收手标准**：当"新概念覆盖率"和"新知识密度"开始下降，就该换项目，而不是把同一个项目做到生产级

---

## 八、下一步动作

1. `git checkout -b admin-role`（或直接在 master 上）：users 表加 `role TEXT NOT NULL DEFAULT 'user'` 迁移
2. Claims 加 `role` 字段（权衡：免查库 vs 角色变更不即时生效）
3. `Router::nest("/admin", ...)` + `require_admin` 中间件 → `403` 登场，`get_all_users` 复活（带分页）
4. 顺手修掉第二节那两个遗留点（`get_me` 下沉 service、`if let` 收敛为 `ok_or`）
5. 补一组集成测试（注册 → 登录 → 带 token 访问 → 越权 401/403）
6. 收尾 commit，然后开新仓库做 Agent 的 V0
