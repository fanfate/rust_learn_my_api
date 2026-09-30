# rust_learn_my_api 鉴权改造 —— 进度记录与后续计划

> 记录时间：2026-09-30
> 项目只读，代码由本人手动修改
> 前置：已完成 rustlings，按 plan.md 学习 axum web api；本次为 JWT 鉴权 + auth 体系改造

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

### 验收清单（commit 前手动跑一遍）

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

## 三、后续计划（里程碑三选一，可并行推进）

1. **CORS** —— 前端（React）连上的第一步，`tower-http` 的 `CorsLayer`，改动小见效快
2. **tracing** —— 把 println 换成结构化日志；顺手给 token 验证失败加 `warn!`（生产形态：记日志但对外仍 401）；请求日志中间件 `TraceLayer`
3. **admin + role**（推荐作为下一个大块）
   - users 表增量迁移加 `role TEXT NOT NULL DEFAULT 'user'`
   - role 写进 JWT Claims（权衡：放 token 里免查库但角色变更不即时生效）
   - `Router::nest("/admin", ...) + layer(require_admin)` 中间件方案正式登场
   - `403 Forbidden` 与"操作者 vs 被操作者"命名（`current_user` vs `target_user_id`）
   - `get_all_users` 复活（加分页 `LIMIT ? OFFSET ?`）

### 更远期（plan.md 对应）

- Refresh token（含"注销后 token 失效"的系统化解法：token 版本号 / 黑名单）
- 集成测试（`#[tokio::test]` + 内存 sqlite，service 层窄依赖在此兑现红利）
- 阶段 10 分层重构（routes/handlers/services/repositories；届时 auth.rs 拆 auth.rs + users.rs，sql.rs 长成 repositories）
- 阶段 13 Docker/CI（引入 `cargo sqlx prepare` 离线模式）
- 可选深水区：`QueryBuilder` 动态拼 SQL（关键词搜索 + 分页场景）

---

## 四、过程中沉淀的通用经验（Rust / Web 通用）

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
