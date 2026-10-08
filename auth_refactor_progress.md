# rust_learn_my_api 鉴权改造 —— 进度记录、Review 与后续路线

> 记录时间：2026-09-30　最近更新：2026-10-08
> 项目只读，代码由本人手动修改
> 前置：已完成 rustlings，按 plan.md 学习 axum web api；本次为 JWT 鉴权 + auth 体系改造

---

## 〇、当前状态速览（2026-10-08）

- 鉴权改造已于 09-30 完成并通过 review，已提交：
  - `d90abc2 基本完成鉴权部分的改造`
  - `621604c 增加现阶段AI审计和建议`
  - `d0b2b98 更新后续计划` / `d3c6573 更新后续计划细节：单元测试部分`
- **T0 纯函数单测已完成（2026-10-08）：12 条全绿**
  - `src/jwt.rs` 7 条、`src/auth.rs` 5 条；`cargo test` → `12 passed; 0 failed`（0.84s）
  - 改动集中在 `Cargo.toml`（补 feature）、`src/jwt.rs`、`src/auth.rs`；**尚未 commit**
  - 剩余 1 个 warning：`sql.rs::get_all_users` 死代码（已知欠账，admin 里程碑复活）
- **T0 过程中修掉一个运行时级缺陷**（`jsonwebtoken` 缺密码学后端，导致 login 与所有受保护端点从未真正跑通过）——详见 **4.7①**，这一条比测试本身更重要。
- **结论：核心目标已基本达成**——axum 核心概念已用掉大半，做完"T1 + admin/role"两项即可转场，不必按 plan.md 一路走到 Docker/CI。
- **收尾顺序已定案（2026-10-07，未变）：先写测试（T0/T1），再做 admin + role。** 理由见第四节。

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
| **service 层窄依赖**：收 `pool` / `encoding_key` / `ttl` 等具体参数，不收 `&AppState` | `Zero To Production` 风格；签名即依赖清单；换容器零成本；**测试时只需一个 pool，构造成本近乎为零**。窄依赖只约束 service 层以下，handler 收 `&AppState` 天经地义 |
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

1. **密码策略完全缺失 —— `register` 可以创建空密码账户**（2026-10-08 写 T0 时挖出，非 review 名单内）
   - `auth::register` 从 `user_sign.password` 直接 `hash_password` 入库，中间**零校验**；`update_password_me` 同款，可把密码改成空串
   - 认识根源：**argon2 是密码学原语，只管上界**（`argon2-0.6.0/src/lib.rs:618-627` 的 `verify_inputs` 只拦 `pwd.len() > MAX_PWD_LEN`，防的是资源耗尽），**最小长度是业务策略，库不替你做主**
   - 归属：protocol-independent → service 层；可写成 `auth.rs` 内的自由函数供 `register` / `update_password_me` 共用
   - ⚠️ **不要复用 `ApiError::Password`**（映射 500）——弱密码是客户端问题，必须 `BadRequest`(400)。错的错误类型会把客户端的错报成服务端故障
   - 惯例参照：NIST SP 800-63B（min 8、允许 ≥64、**不强制**字符组合规则，强制反会催生 `Passw0rd!` 类可预测密码）
   - **决定：归入 T1**（先写红的断言，再补实现）
2. **`message` 永远无法被清空**（语义缺口，当前可接受）
   - 三态模型缺一态：`Some(x)`=设值、`None`=不动，没有"清空回 NULL"的表达
   - 将来解法引子：JSON Merge Patch 三态（缺席/null/值），Rust 里需 `Option<Option<T>>` 或手工解析——留作练习
3. **`get_me` 绕过 service 层**（对称性）
   - handler 里直接查库 + `if let`；建议下沉为 `auth::get_me(pool, id)`
   - 顺带：`get_user_id` 的 `if let Some/else` 可收敛成 `.ok_or(ApiError::NotFound(..))?`
   - **写 T1 测试时会自然被逼着修**（handler 里的逻辑没有 service 入口可测）
4. **`sql::get_all_users` 死代码**
   - 无调用者，binary crate 会报 dead_code warning；建议先删，admin 里程碑时复活
5. **`DELETE /me` 返回 200 + `data: null`**
   - 可选练习：改成 **204 No Content**，体会"统一信封的合法例外"（返回类型换 `StatusCode`）
6. 小项：`old_password` 可改名 `current_password`；config 的 ttl `unwrap_or` 静默吞错（已知，暂不管）；DTO 分居 auth.rs / models.rs 两处（有意为之，可接受）

### 验收清单（改动后手动跑一遍）

> ⚠️ **状态：尚未真正跑通过。** 09-30 改动后 `jsonwebtoken` 缺少密码学后端（见 4.7①），`POST /auth/login` 及第 5-9 条全部会连接中断；`data/users.db` 的 `users` 表 **0 行**即佐证。feature 已于 10-08 修复，**清单待补跑**——这将是 login 与受保护端点第一次被真正执行。

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

## 三、当前项目的收手标准（2026-10-07）

### 3.1 判断标准

这个项目的使命不是"做完一个生产级后端"，而是：**把 axum 的核心概念各用一遍 + 用 Rust 写一遍类型/所有权密集的代码**。

按这个标准盘点，axum 版图里只剩**两块没碰到**，做完就可以收手：

| 收尾项 | 为什么值得做 | 会练到 |
|---|---|---|
| **测试（T0/T1）**（先做） | 为接下来的 admin 改造提供安全网；逼出 `get_me` 的分层问题；兑现窄依赖红利 | `#[sqlx::test]`、真实库测试、断言行为而非实现 |
| **admin + role**（后做） | 中间件 / `tower::Layer` 是唯一还没碰的 axum 核心概念 | `Router::nest` + `layer`、`403`、增量迁移、role 写进 Claims 的权衡 |
| refresh token（可选） | 业务设计题而非语言题，但能练状态设计与迁移 | token 版本号 / 黑名单、token 轮换 |

### 3.2 建议降级或跳过

| 项目 | 判断 |
|---|---|
| CORS | 一个 `CorsLayer` 的事，真接前端时顺手加，不值得单独立项 |
| tracing | 学的是工程化不是 Rust，下个项目（Agent）里自然会用到 |
| 阶段 10 目录化分层（routes/services/repositories） | 纯体力活，思想已在 auth.rs / sql.rs 用上，收益低于成本 |
| Docker / CI / 部署 / HTTPS | 运维知识，与 Rust 无关，**学习期性价比最低——plan.md 后半段这些直接跳** |
| gRPC / 微服务 / 消息队列 | 同上，等有真实需求再说 |

> 一句话：**补上测试 + 做完 admin/role，这个项目就够本了**；之后它作为"资产"保留，下一个项目会直接复用它。

---

## 四、测试计划（2026-10-07 定案）

### 4.1 顺序：先测试，再 admin

接下来要做的 admin + role 是目前为止**风险最高的一次改动**：要动迁移、动 Claims 结构（加 `role`）、动 models 和整条鉴权路径。先有测试网再改，四条理由：

1. **安全网用在刀刃上**：admin 改造会经过 login/register 的既有路径，改完 `cargo test` 一绿即知没破坏旧行为；没有测试就只能靠手点 curl 抽查。
2. **写测试会逼你先修遗留点 3**：`get_me` 的逻辑写在 handler 里，动手写测试时你会发现它没有 service 入口可测——这份别扭感就是"逻辑住错层"的体感证据。
3. **窄依赖红利在此兑现**：service 收 `pool` 而非 `&AppState`，测试只要一个连接池就能构造。写不写得出测试，正好检验当初的设计决策。
4. **测试脚手架是跨项目资产**：Agent 项目 V2 要接回 sqlite + axum，这套写法直接复用。

### 4.2 三层测试

| 层 | 测什么 | 成本 | 状态 |
|---|---|---|---|
| **T0 纯函数单测** | `hash_password` / `verify_password`、`jwt::sign` / `validate` | 半小时 | ✅ **完成 2026-10-08（12 条全绿）** |
| **T1 服务层测试**（连真实库） | `auth::register` / `login` / `update_me` / `delete_me` / `update_password_me` 的业务规则 | 半天 | **下一步** |
| admin + role | 角色迁移、中间件、403 | 1-2 天 | T1 之后 |
| **T2 HTTP 层测试**（Router + 请求） | 状态码映射、`CurrentUser` 提取、401/403、`From` 裁剪 | 半天 | 与 admin 一起 |

T2 放后面的实际原因：**403 需要 admin 路由存在才测得了**，而 T1 已能覆盖绝大部分业务逻辑。

### 4.3 已核实的实现事实（本机 registry 源码确认，非记忆）

- 本项目 sqlx 为 **0.9.0**，`sqlx::test` 属性宏存在；`macros` / `migrate` 都在 sqlx 的**默认 feature** 里 —— 现有 `features = ["sqlite", "runtime-tokio", "macros"]` 并未关闭默认 feature，所以 **`Cargo.toml` 无需改动**即可使用。
- **`#[sqlx::test]` 自带异步运行时**（宏展开为 `#[test]` + `sqlx::testing::TestFn::run_test`），**不要再叠 `#[tokio::test]`**。
- 它给**每个测试建独立库并自动跑迁移**（sqlite 走真实临时文件，跑完删除）。顺带避掉经典坑：手写 `sqlite::memory:` 连接池时内存库按连接隔离，池内多连接 = 多个空库。
- 读响应体：`axum::body::to_bytes(resp.into_body(), usize::MAX)`（axum 0.8.9 已确认存在）。
- HTTP 层测试需 **`tower` 作 dev-dependency**（`oneshot` 在 `tower::util` 下，须显式声明 `features = ["util"]`，不能蹭 axum 的间接依赖）。
- **结构约束**：当前 crate 只有 `src/main.rs`、**没有 lib target**，所以 `tests/` 下的集成测试**无法 `use my_api::...`**。要么加 `src/lib.rs`（`pub mod ...`，main.rs 变薄壳，Zero To Production 的标准结构），要么把测试写成 crate 内 `#[cfg(test)] mod tests`。**现阶段先不动目录结构，T1 写在 crate 内即可。**
- **T0/T1 都不需要新增依赖**：标准 `#[test]` 与 `#[sqlx::test]` 已够用（`[dev-dependencies]` 目前仍为空）。

### 4.4 一个结构约束：`password` 字段是私有的

`UserRegister` / `UserLogin` 里的 `password` 没有 `pub`。这是有意为之的好设计，但意味着：

- **T1 服务层测试必须写在 crate 内**（`auth.rs` 里的 `#[cfg(test)] mod tests` 可访问同模块私有字段），外部 `tests/` 构造不出这两个结构体。
- **T2 HTTP 层测试不受影响**——它走 JSON 反序列化（`Deserialize` 已有），字段私有不妨碍。

> 已用编译器实测的两个对照（探针 `/tmp/rust_test_layout`）：
> ① `auth.rs` 内 `mod tests` → 能读私有字段 ✅；
> ② crate 根 `src/tests.rs` 同级模块 → `error[E0616]: field 'password' ... is private` ❌。
> 原因：Rust 隐私规则是"定义模块**及其后代**可见"，crate 根的同级模块不是 `auth` 的后代。
> **推论：单一 `test.rs` 方案会直接破坏 T1，且逼迫给 `password` 加 `pub` —— 为了测试放宽生产代码可见性是净亏，不要做。**
> 单模块测试变长时的升级路径：`mod tests { ... }` → `#[cfg(test)] mod tests;` + 同目录 `auth/tests.rs`（实测仍保留私有访问）。

### 4.5 断言清单

**T0（`src/jwt.rs`、`src/auth.rs` 内）—— ✅ 已完成 2026-10-08，12 条全绿**

| 文件 | 测试 | 断言要点 |
|---|---|---|
| jwt.rs | `test_sign_success` | 签名产出 `Ok` 字符串 |
| | `test_validate_success` | 签完再验：`sub` 相同 **+ `exp - iat == 10`**（把"ttl 真的被用上"也锁住了，见 4.7④） |
| | `test_validate_wrong_format_token` | `"wrong_token"` → `ErrorKind::InvalidToken` |
| | `test_validate_none_alg_token` | `alg:none` + 尾点 → **`assert!(is_err())`**（不钉具体变体，理由见 4.7③） |
| | `test_validate_other_algorithm_token` | HS512 签名 → HS256 校验 → `ErrorKind::InvalidAlgorithm`（**算法白名单哨兵**） |
| | `test_validate_wrong_secret` | 换 key 验 → `ErrorKind::InvalidSignature` |
| | `test_validate_time_failed` | `ttl = -120s` → `ErrorKind::ExpiredSignature`（**不用 sleep**） |
| auth.rs | `test_hash_success` | `argon2::PasswordHash::new(&h).is_ok()` —— 证明是**合法 PHC 串**，而非只验 `$argon2` 前缀 |
| | `test_hash_salt` | 同明文两次 → 结果不同（随机盐生效） |
| | `test_verify_success` / `test_verify_failed` | 正确→true；错误→false |
| | `test_verify_failed_error` | 损坏 hash → false（**不 panic**，错误路径有人测） |

> 对照原清单：原计划 `jwt` 四条（`sub` 相同 / 换 key → Err / 垃圾串 → Err / ttl≤0 立即过期）全部落地；`hash` 两条、`verify` 三条全部落地。

**T1（`#[sqlx::test]`，一条一测试）**

- 注册成功 → 拿到实体、库里有 hash、hash ≠ 明文
- 同邮箱再注册 → `ApiError::Conflict`
- **注册拒绝空密码 / 过短密码 → `BadRequest`(400)**（2026-10-08 挖出的缺口，见遗留点 1；**先写红的**）
- 登录成功 → token 能验出正确的 `sub`
- 邮箱不存在 / 密码错 → **都是 `Unauthorized`**（守住"防邮箱枚举"的设计）
- 改资料只传 `name` → `message` 不变；两者都不传 → `BadRequest`
- 改密码：旧密码错 → 401；旧密码对 → 新密码可登录、旧密码登不上
- **改密码设成空串 → 同样被拒（400）**（与注册共用同一个策略函数）
- 注销后 → 按 id 查为 `None`（"token 靠查库兜底"的单元版验证）

### 4.6 两个别做的事

- **别追求覆盖率数字**：学习阶段 T0 + T1 覆盖到上面这些就够。
- **别 mock 数据库**：用真实 sqlite 才有意义；mock 只会让你测出"我的 mock 是对的"。

### 4.7 T0 实施记录与四个真实发现（2026-10-08）

**产出**：`cargo test` → `12 passed; 0 failed`（0.84s），剩余 1 warning（`get_all_users`）。
改动仅三处：`Cargo.toml`（补 feature）、`src/jwt.rs`、`src/auth.rs`。**未新增依赖、未动数据库结构。**

#### ① `jsonwebtoken` 11 缺密码学后端 —— 登录链路从未真正跑通过（最重要）

`jwt::sign` 一调用就 panic：

```
Could not automatically determine the process-level CryptoProvider
from jsonwebtoken crate features.
```

**根因**：`jsonwebtoken` 自 **10.0.0（2025-09-29）** 起把加密实现改成可插拔 trait，CHANGELOG 写明
`BREAKING: now using traits for crypto backends, you have to choose between aws_lc_rs and rust_crypto`。
而它的 `default = ["use_pem"]` —— **只有 PEM 解析，不含任何密码学后端**（`cargo tree -e features -i jsonwebtoken` 可验证）。
`src/crypto/mod.rs:105-137` 的 `from_crate_features()` 在两个 `#[cfg]` 分支都不满足时，返回一个
`signer_factory` / `verifier_factory` 全是 `panic!` 的假 provider。

**修复**：

```toml
jsonwebtoken = { version = "11.1.0", features = ["rust_crypto"] }
```

选 `rust_crypto` 而非 `aws_lc_rs`：前者纯 Rust、无 C 工具链，Windows 上不需要 cmake/NASM。

**影响面（为什么它比测试重要）**：panic 是**运行时行为**，不止测试——

| 调用点 | 结果 |
|---|---|
| `encode()`（`jwt::sign` ← `login`） | **panic** |
| `decode()` 格式合法的 token（extractor） | **panic** |
| `decode()` 格式非法的 token | 先返回 `Err`，不 panic |
| register（只走 argon2）/ `GET /users/{id}` / `/health` | 正常 |

即 **`POST /auth/login` 与所有受保护端点（`/me`、`PATCH /me`、`DELETE /me`、`PUT /me/password`）全部连接中断**——
curl 表现为 `(52) Empty reply from server`（panic 在 tokio task 里展开，task 死掉但**主进程不崩**，默认 `panic = "unwind"`）。
`git log -- Cargo.toml` 显示该版本自 `0c6d981`（"增加依赖用于后续登录鉴权部分"）起从未改过，
**所以登录链路自 JWT 改造以来从未被真正执行过**——`data/users.db` 的 `users` 表 0 行可佐证。

**两个副产品认知**：

- `EncodingKey::from_secret` / `DecodingKey::from_secret`（`encoding.rs:34` / `decoding.rs:95`）是**纯数据构造**
  （只存 bytes + 打 `family = Hmac` 标记），**不碰 provider、不会 panic**。所以服务能正常启动，炸在**真正使用它的那一刻**。
- `auth.rs` 里的 `.map_err(|_| ApiError::Unauthorized / Internal)` **抓不住它**——
  `map_err` 只处理 `Err`，而 **panic 不走 `Result`**，直接展开栈。这解释了它为什么藏了近两周。

#### ② `Validation::default()` 的 `leeway = 60` —— 线上令牌过期后还有 60 秒可用

`validation.rs:126` 默认 `leeway: 60`（文档注释 "Defaults to 60"），判定式在 `validation.rs:283-286`：

```rust
if options.validate_exp
   && exp - options.reject_tokens_expiring_in_less_than < now - options.leeway
{ return Err(ExpiredSignature); }
```

即 **`exp` 必须比 `now` 早 60 秒以上才算过期**。实测临界点正好落在 −61s（`leeway=60` 接受 `-60s`，拒绝 `-61s`）。

不是 bug（RFC 7519 允许 leeway，用于抗分布式时钟漂移），但要知道：`access_ttl` 若为 900s，
**线上实际有效期约 960s**。想收紧设 `validation.leeway = 0`（多机部署需留余量）；
另有 `reject_tokens_expiring_in_less_than`（默认 0）可提前拒绝"即将过期"的令牌。

**顺带**：`JwtClaims::new` 用 `exp.timestamp()` / `iat.timestamp()`（i64 **秒**），所以 **1ms 级 ttl 会被截断归零**
（JWT 的 `exp` 本就是 NumericDate 秒级，最小粒度 1 秒）——这也是最初那条 `try_milliseconds(1) + sleep(2ms)` 测试
必然失败的原因之一。

#### ③ `alg=none` 死在第 1 关，算法白名单是第 2 关（两关别混）

`decoding.rs:270-291` 的 `decode()` 是**五关依次过**：

```
1. decode_header(token)?                                  // :332 解析 header（含段数检查）
2. !validation.algorithms.contains(&header.alg)
       → Err(InvalidAlgorithm)                            // :278 算法白名单
3. verifier_factory(&header.alg, key)?
4. verify_signature(token, validation, provider)?
5. validate(claims, validation)?                          // exp / leeway
```

`{"alg":"none"}` 的 token **死在 1、走不到 2**：`algorithms.rs` 的 `Algorithm` 枚举里**根本没有 `none` 变体**
（只有 HS256/384/512、ES…、RS…、PS…、EdDSA），所以是 **serde 反序列化 header 失败**（`ErrorKind::Json`）。
报错里的 `line: 1, column: 13` 正是 `"none"` 结尾引号的位置——铁证。

→ **推论：`test_validate_none_alg_token` 只证明"被拒绝"，完全没有覆盖算法白名单。**
真正测白名单要用"能正常签名但算法不对"的 token（`Header::new(Algorithm::HS512)`）→ `InvalidAlgorithm`，
即新增的 `test_validate_other_algorithm_token`。它的价值在于：**证明保护完全来自 `validation.algorithms` 白名单**
——哪天为了支持别的算法放开它，只有这条测试会亮红灯。

（构造方式备查：`base64url(header) + "." + base64url(payload) + "." + base64url(sig)`，
base64url = 标准 base64 把 `+`→`-`、`/`→`_`、去掉尾部 `=`；`alg=none` 的恶意输入在测试里直接硬编码常量即可，不必引 `base64` 依赖。）

#### ④ 断言强弱的判断标准：锁**契约**还是锁**实现**

钉不钉具体的错误变体，取决于你锁的是自己的契约还是上游的实现细节：

| 测试 | 锁的是什么 | 该不该钉具体变体 |
|---|---|---|
| `wrong_secret` → `InvalidSignature` | **自己的契约**：签名校验真的在跑 | ✅ 钉 |
| `time_failed` → `ExpiredSignature` | **自己的契约**：过期真的生效 | ✅ 钉 |
| `other_algorithm` → `InvalidAlgorithm` | **自己的契约**：算法白名单真的在跑 | ✅ 钉 |
| `none_alg` → `Json(..)` | 上游实现细节（换库/换版本就变） | ❌ 只 `assert!(is_err())` |

所以"`assert!(is_err())` 太弱"**不是普适结论**——它针对的是"该钉契约却只写了存在性"的情况，
不是对写法本身的批评。`none_alg` 这条用 `is_err()` 反而是正确的。

**遗留的方法论盲区（T1 要避免）**：写断言时问一句"**这个 bug 出现时，我会不会恰好也通过？**"
T0 第一轮就踩过这个坑（`assert!(!validate_err.is_ok())` 换错密钥也通过）。

---

## 五、下一个项目：Rust Coding Agent（2026-10-07 定案）

### 5.1 结论

**采纳"做 Agent"这条路线，但附加三个限定条件**：不用现成 Agent 框架、先做非流式 + CLI、在隔离目录里跑。

理由：
- 和当前项目**天然衔接**——Axum/SQLx/鉴权/分层这些已攒下的能力，在 V2 阶段直接复用（agent 的 HTTP 层 + 会话持久化 + 多用户隔离）。
- 会真正逼你练到 Rust 特有的东西：**trait object 动态分发**、**serde tagged enum 建模工具参数**、**async 取消与超时**、错误类型设计。
- 产出是**可用的东西**（一个能帮你读代码、跑 cargo 的 CLI），比再写一个 Todo List 有意思。
- 本地模型条件已具备：llama.cpp / CUDA 环境在手，可直接打本地 OpenAI 兼容端点，无需云端 key。

### 5.2 它真在练什么（分两栏看，避免预期错位）

| 真在练 Rust | 换 Python/TS 写是同一件事 |
|---|---|
| `Tool` trait 动态分发（`Box<dyn Tool>` + 注册表） | 调 LLM 的 HTTP 请求 |
| serde tagged enum 表达工具参数（**最能体现 Rust 表达力**） | prompt 拼装 |
| 错误类型设计（工具失败也要回灌给模型） | JSON Schema 描述 |
| `Arc` / 共享状态（工具注册表、会话状态） | 流式 chunk 的字符串处理 |
| 取消与超时（`CancellationToken` + `tokio::select!`） | |
| async 生命周期、跨 await 的借用 | |

### 5.3 三处要打折的地方（避免踩坑）

1. **"核心代码也就这么一个循环"是极度乐观的说法。** 那 20 行是骨架，真正的活在外面：
   - 流式：手写 SSE 解析，`tool_calls` 分片跨 chunk 累积（最麻烦的一块）
   - 可靠性：重试 / 限流 / 超时 / 可中断（Ctrl-C 要能停）
   - 上下文：历史裁剪、token 计数、assistant/tool 消息配对协议
   - 时间预期按 **两周以上** 算，不是"一个周末"
2. **本地小模型的 tool calling 可靠性差。** 7B 级模型多轮 tool call 容易不听话，会分不清"我循环写错了"还是"模型不肯调工具"。对策：先用云端强模型验证循环正确性，再切本地；或不依赖原生 tool call，让模型输出 JSON 代码块自己解析（早期 agent 的做法，本身是很好的练习）。
3. **框架生态的名字不重要，别照抄教程。** 各家 agent crate 更新极快，写之前先 `cargo search` / docs.rs 核对当前版本 API，或干脆不用。

### 5.4 必须自己立起来的安全边界（最容易被忽略）

Agent 能 `write_file` + `run_command`，等于把"任意文件写入 + 任意命令执行"交给一个概率模型。学习版必须：

- **只在专门的 scratch 目录里跑**，不指向真实项目（练习项目本身是只读的，另开一个玩具目录给它折腾）
- **写操作与 shell 走人工确认**：先打印 diff / 待执行命令，回车才执行——这个"人机确认"也是真实 agent 的标准设计
- **工具白名单**，不做万能 shell；命令限定在 `cargo check` / `cargo test` 这类
- 结果输出要**截断**，别把整个文件塞回上下文

### 5.5 备选方向 B：反向代理 / 负载均衡器

| | 方向 A：Agent | 方向 B：负载均衡器 |
|---|---|---|
| Rust 浓度 | 中（一半时间在 LLM 管道） | 高，纯 Rust |
| 与当前项目关系 | 直接复用（同一项目长出来） | 同一技术栈的下一层（tokio/hyper/tower） |
| 最硬的知识点 | trait object、流式并发、状态机 | tokio 并发、`Arc`/`RwLock`、手写 `tower::Service`、故障转移 |
| 反馈速度 | 慢（外部依赖） | 快（改一个文件就能看到效果） |
| 产出价值 | 有，可自用 | 无，纯练习 |

**取舍**：要"产出 + 覆盖面"选 A；要"把并发练到手"选 B。两者不互斥——B 的最小版本（TCP 代理 + 轮询 + 健康检查 + 优雅关停）一个周末能出来，之后再做 A 会更顺。

---

## 六、落地路线：V0 → V3（针对本机环境）

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

## 七、更远期（原 plan.md 对应，已按 2026-10-07 判断调整）

- ~~阶段 10 分层重构~~ → 降级：思想已用上，收益低于成本
- ~~阶段 13 Docker/CI~~ → 跳过：运维知识，非 Rust 学习目标
- Refresh token（含"注销后 token 失效"的系统化解法：token 版本号 / 黑名单）
- 可选深水区：`QueryBuilder` 动态拼 SQL（关键词搜索 + 分页场景）

---

## 八、过程中沉淀的通用经验（Rust / Web 通用）

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
- **依赖的 `default` feature 可能"什么都不带"**：`jsonwebtoken >= 10` 必须显式选 `rust_crypto` / `aws_lc_rs`。加依赖后先看 `default` 里有什么（`cargo tree -e features -i <crate>` + 读 registry 源码），别假设"默认能用"
- **密码学原语只管上界（防资源耗尽），下界是业务策略**：argon2 会拒绝超长密码，但坦然接受空字符串
- **panic 不是 `Err`**：`?` / `map_err` / 任何 `Result` 组合都抓不住它；它直接展开栈、断掉 tokio task，表现为"连接被掐断"，比 500 更难查
- **`timestamp()` 是秒**：JWT 的 `exp` / `iat` 是 NumericDate，最小粒度 1 秒，毫秒级 ttl 恒被截断
- **别用 `thread::sleep` 测时间**：直接构造已过期的输入（`ttl = -120s`），又快又稳，还不受调度抖动影响
- **断言要区分"锁契约"还是"锁实现"**：自己的行为 → 钉死具体错误变体；依赖的实现细节 → 只断言"失败"
- **写断言时问："这个 bug 出现时，我会不会恰好也通过？"** —— 断言强度比断言数量重要
- **Rust 隐私规则是"定义模块及其后代可见"**：单元测试必须住在被测模块内（或其后代），crate 根放一个 `test.rs` 收集所有测试会看不到私有字段。别为了让测试通过而放宽生产代码的可见性

---

## 九、下一步动作（按顺序执行）

1. **T0 ✅ 已完成（2026-10-08）**：`src/jwt.rs` 7 条 + `src/auth.rs` 5 条，`cargo test` → 12 passed。
   改动（`Cargo.toml` 补 `rust_crypto` feature、两个 `#[cfg(test)] mod tests`）**尚未 commit**。
2. **补跑第二节的验收清单**（此前从未跑通，因 4.7① 的 feature 缺陷）——这将是 `login` 与
   所有受保护端点第一次被真正执行；重点看第 3、5-9 条。
3. **T1**（约半天）：`src/auth.rs` 内用 `#[sqlx::test]` 覆盖 4.5 的服务层断言；`cargo test` 全绿。
   其中**密码策略**（遗留点 1）先写红断言、再补实现，校验走 `BadRequest`(400) 而非 `ApiError::Password`(500)。
4. **顺手修遗留点 3**：`get_me` 下沉为 `auth::get_me`；`get_user_id` / `get_me` 的 `if let` 收敛为 `.ok_or(..)?`
5. **T2 前置**：加 `src/lib.rs`（模块转 `pub`，`main.rs` 变薄壳）；`Cargo.toml` 加
   `[dev-dependencies] tower = { version = "0.5", features = ["util"] }`
6. **admin + role**：迁移加 `role TEXT NOT NULL DEFAULT 'user'` → Claims 加 `role`（权衡：免查库 vs 角色变更不即时生效）
   → `Router::nest("/admin", ...)` + `require_admin` 中间件 → `403` 登场，`get_all_users` 复活（带分页）
7. **T2**：HTTP 层测试补 401 / 403 / 201 / 409 的状态码断言与 `CurrentUser` 失败路径
8. `git commit` 收尾 → 开新仓库做 Agent 的 V0
