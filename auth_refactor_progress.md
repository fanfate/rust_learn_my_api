# rust_learn_my_api 鉴权改造 —— 进度记录、Review 与后续路线

> 记录时间：2026-09-30　最近更新：2026-10-10
> 项目只读，代码由本人手动修改
> 前置：已完成 rustlings，按 plan.md 学习 axum web api；本次为 JWT 鉴权 + auth 体系改造

---

## 〇、当前状态速览（2026-10-10）

- 鉴权改造已于 09-30 完成并通过 review，已提交：
  - `d90abc2 基本完成鉴权部分的改造`
  - `621604c 增加现阶段AI审计和建议`
  - `d0b2b98 更新后续计划` / `d3c6573 更新后续计划细节：单元测试部分`
- **T0 纯函数单测已完成（2026-10-08）：12 条全绿**
  - `src/jwt.rs` 7 条、`src/auth.rs` 5 条；`cargo test` → `12 passed; 0 failed`（0.84s）
  - 改动集中在 `Cargo.toml`（补 feature）、`src/jwt.rs`、`src/auth.rs`；已于 `e54aff8` **提交**
  - 剩余 1 个 warning：`sql.rs::get_all_users` 死代码（已知欠账，admin 里程碑复活）
- **T0 过程中修掉一个运行时级缺陷**（`jsonwebtoken` 缺密码学后端，导致 login 与所有受保护端点从未真正跑通过）——详见 **4.7①**，这一条比测试本身更重要。
- **视图分叉已完成并提交（2026-10-09 / commit `0cee939 更新get_me访问权限问题`）**：`/users/{id}`（匿名观看者）返回
  `PublicUserResponse`（无 email），`/me`（本人）返回 `UserResponse`（含 email）；两个读端点共用 `auth::get_user`。
  已用临时库端到端验证（`POST /auth/login` 首次真正跑通）。**遗留点 3 正式收尾。**
- **T1 服务层测试已完成（2026-10-10）：`cargo test` → 21 条全绿**
  - `#[sqlx::test]` 8 条（register 成功 / 同邮箱冲突 / 短密码、login 成功 / 失败、update、update_password、**delete_me**）
    + 纯函数 `is_valid_password` 1 条；**12 → 21**（jwt.rs 7 + auth.rs 14）。
  - **遗留点 1（密码策略缺失）已解决**：`register` / `update_password_me` 均过 `is_valid_password`，弱密码 → `BadRequest`(400)；
    长度按 **`chars().count()`**（字符数）计，不再是 `.len()`（字节数）。
  - **4.8 的 ①~⑤ 全部已处理**：① `test_update` 强断言；② `.len()` → `chars().count()`；③ `update_password_me` 改为**先验旧密码（401 优先）**并补上**顺序哨兵**；
    ④ `test_update` 末尾加**独立重读**验证落库；⑤ `delete_me` 补 `get_user → NotFound` + 重复注销 → `NotFound`。
  - ✅ **已提交（2026-10-10 / commit `f201924 完成遗漏的测试`，其前 `f35288d`）**，工作树干净 —— T1 基线可回退。
- **admin + role 的设计已定案（2026-10-10）**：**不做"两个注册端点"**。公开注册永远只产 `user`
  （现状已正确：`UserRegister` 无 `role` 字段、`create_user` 的 INSERT 不带 role），admin 由
  「**自举 seed（首个）+ `require_admin` 保护的提权端点（后续）**」产生。详见 **3.3**。
- **role 的类型已定案（2026-10-10）**：DB/JSON 两端是字符串，Rust 内部用 **`enum Role { User, Admin }`**（不用 `String`）；
  落地时 **`query_as!` 必须写列类型注解 `role as "role: Role"`**，否则报 `Role: From<String>`。详见 **3.3⑦⑧**。
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
| **视图按"观看者"再分一层**（`UserResponse` 本人 / `PublicUserResponse` 公开） | 端点返回类型 = 该端点观看者有权看到的字段集；观看者不同就不该共享 DTO。字段显式枚举，将来给 `UserEntity` 加列不会自动泄漏（fail-closed） |
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
             （Json 永远放参数最后；两个读端点共用 auth::get_user，各自 .into() 到不同视图）
sql.rs     → 只按"查找方式"提供函数，全部返回 UserEntity 或操作结果
             （UPDATE 部分更新用 COALESCE(?, message)）
models.rs  → UserEntity / UserResponse（本人）/ PublicUserResponse（公开）/ 请求体 DTO / LoginResponse
error.rs   → ApiError（含 Conflict/Password 等）+ IntoResponse 统一映射
```

---

## 二、Review 结论与遗留点（2026-09-30，遗留点持续追记）

**整体通过，无编译级问题，架构原则基本全部落地。** 亮点：分层闭环完整、`UserEntity` 无 `Serialize` 的类型级防线、错误翻译位置正确、`jwt.rs` 修掉了 `Ok(?)` 冗余。

### 遗留点（按重要性）

1. **密码策略完全缺失 —— `register` 可以创建空密码账户**（2026-10-08 挖出；**✅ 2026-10-09 已解决**）
   - `auth::register` 从 `user_sign.password` 直接 `hash_password` 入库，中间**零校验**；`update_password_me` 同款，可把密码改成空串
   - 认识根源：**argon2 是密码学原语，只管上界**（`argon2-0.6.0/src/lib.rs:618-627` 的 `verify_inputs` 只拦 `pwd.len() > MAX_PWD_LEN`，防的是资源耗尽），**最小长度是业务策略，库不替你做主**
   - 归属：protocol-independent → service 层；可写成 `auth.rs` 内的自由函数供 `register` / `update_password_me` 共用
   - ⚠️ **不要复用 `ApiError::Password`**（映射 500）——弱密码是客户端问题，必须 `BadRequest`(400)。错的错误类型会把客户端的错报成服务端故障
   - 惯例参照：NIST SP 800-63B（min 8、允许 ≥64、**不强制**字符组合规则，强制反会催生 `Passw0rd!` 类可预测密码）
   - **决定：归入 T1**（先写红的断言，再补实现）
   - **✅ 已落地（2026-10-09 ~ 10-10）**：新增自由函数 `is_valid_password`（`auth.rs:72`，现为 `origin_password.chars().count() >= 8`），
     `register`（`auth.rs:79`）与 `update_password_me`（`auth.rs:175`）均拦一道，弱 / 空密码 → `BadRequest`(400)
     （**未复用** `ApiError::Password`）。T1 用例 `test_register_short_password` 覆盖空串 + 过短。
     **长度口径已从 `.len()`（字节）改为 `chars().count()`（字符）——详见 4.8②。**
2. **`message` 永远无法被清空**（语义缺口，当前可接受）
   - 三态模型缺一态：`Some(x)`=设值、`None`=不动，没有"清空回 NULL"的表达
   - 将来解法引子：JSON Merge Patch 三态（缺席/null/值），Rust 里需 `Option<Option<T>>` 或手工解析——留作练习
3. **`get_me` 绕过 service 层**（对称性）—— **✅ 2026-10-09 已解决（commit `0cee939`）**
   - 已下沉为通用的 `auth::get_user`（`auth.rs:194`，内含查库 + `.ok_or(NotFound)`）；
     两个读端点（`/me`、`/users/{id}`）共用它，各自 `.into()` 到不同视图；`get_user_id` 的 `if let` 已收敛为 `.ok_or(..)?`
   - 命名从 `get_me` 改为 `get_user`：按**操作**命名（"取一个用户"），可被多处复用，与"我"无关
4. **`sql::get_all_users` 死代码**
   - 无调用者，binary crate 会报 dead_code warning；建议先删，admin 里程碑时复活
5. **`DELETE /me` 返回 200 + `data: null`**
   - 可选练习：改成 **204 No Content**，体会"统一信封的合法例外"（返回类型换 `StatusCode`）
6. 小项：`old_password` 可改名 `current_password`；config 的 ttl `unwrap_or` 静默吞错（已知，暂不管）；DTO 分居 auth.rs / models.rs 两处（有意为之，可接受）
7. **id 可枚举（`/users/{id}` 公开读）—— 已知遗留，学习阶段决定不动**（2026-10-09 追记）
   - 事实：`users.id` 是 `AUTOINCREMENT` 且 `/users/{id}` 无鉴权 → 理论上 `for` 循环即可遍历出
     "有多少用户 / 按注册顺序的用户目录"（email harvesting 的入口）
   - **为什么现在可接受**：`data/users.db` 的 users 表 0 行、服务仅本地访问 → 当前**没有可枚举的对象**；
     且该端点返回的 `PublicUserResponse` 已不含 email（字段侧已守住，见下方"两个正交的安全维度"）
   - **升级触发条件（三条同时满足才值得动）**：端点真正对外 + 有真实用户数据 + 会返回他人信息
   - **升级路径**：加 `public_id` 列（UUID/随机 token + 唯一索引，API 层只用它定位资源），**不替换自增主键**
     —— 换主键要付索引碎片 / 存储翻倍 / 可读性下降三重代价，而且**并不解决授权**
   - ⚠️ **概念别记错**：UUID/`public_id` 是**纵深防御，不是访问控制**。"改 id 拿到别人数据"这个伤害靠**授权检查**兜底，
     与"ID 长什么样"正交 —— 别把"上了 UUID"当成"授权做完了"

### 两个正交的安全维度：字段可见性 vs 资源可枚举性（2026-10-09）

排 T1 时确认：这两个问题常被混为一谈，但它们**正交、可独立升级**。

| 维度 | 问的问题 | 现状 | 升级项（可延后） |
|---|---|---|---|
| **A 字段可见性** | 谁会看到哪些列 | ✅ 已处理：DTO 按观看者分叉（`PublicUserResponse` 无 email） | 窄查询：SQL 只 `SELECT` 需要的列 |
| **B 资源可枚举性** | id 能不能被遍历 | ⚠️ 遗留：自增 id 可枚举（遗留点 7） | 加 `public_id` 列 |

**"分叉"要分两层看（最容易混的地方）**：

| 层 | 管什么 | 本项目 |
|---|---|---|
| **L1 返回类型 / DTO 分叉** | 哪些字段**能出服务边界**（序列化那一步） | ✅ 已做：两个读端点各自的视图类型 |
| **L2 查询形状（窄查询）** | 敏感列**根本不进内存 / 不出 SQL** | ❌ 未做：两个读端点仍共用 `auth::get_user` → 取整行 `UserEntity` |

- **为什么 L1 才是关键那一层**：它就是本项目一贯的"让类型替我守"（同 `UserEntity` 无 `Serialize`、`password` 私有）。
  `UserEntity` 没有出口，所以"整行进了内存"本身不构成泄露。L2 只是把同一条边界**再往数据库推一步**，
  多出的收益是"性能/内存"+"万一将来有人给 entity 加了 `Serialize` 或新写出口"。0 用户 + 本地环境下收益 ≈ 0 → **延后**。
- **将来做 L2 的形状**（照抄即可）：每个视图一条 `SELECT`，service 直接返视图类型、而不是返 `UserEntity`。
- **与 UUID 的关系**：L1 已把 `email` 挡在边界外；维度 B（UUID/`public_id`）与维度 A（L1/L2）是**两条独立的轴**。
  两者都属**纵深防御**，都不能替代**服务端授权检查** —— 别把"上了 UUID"误当成"授权做完了"。

### 验收清单（改动后手动跑一遍）

> **状态：10-09 已部分跑通（用临时库 / 端口 3011）。** 4.7① 的 feature 缺陷已于 10-08 修复，
> 第 **1-6 条**已实测通过（`register → login → /me`，**`login` 首次真正执行**；`/users/{id}` 匿名返回 3 字段、`/me` 带 token 返回 4 字段）。
> **第 7-9 条**（`PATCH /me`、`PUT /me/password`、`DELETE /me` 的 HTTP 层行为）**尚未手动跑**——
> 其服务层逻辑已被 T1 覆盖（见 4.5），HTTP 层留到 T2 一起测。

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
| **admin + role**（后做） | 中间件 / `tower::Layer` 是唯一还没碰的 axum 核心概念（**设计决策见 3.3，2026-10-10 定案**） | `Router::nest` + `layer`、`403`、增量迁移、role 写进 Claims 的权衡 |
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

### 3.3 admin + role 的设计决策（2026-10-10 定案）

**问题：admin 的注册要不要和普通用户分开？**

**结论：不做"两个注册端点"。公开注册永远只产 `user`；admin 由「自举 + 提权」两条独立路径产生，两条都不是注册。**

#### ① 现状已是正确形状，别动

`UserRegister`（`auth.rs:16-21`）只有 `name / email / password`，**无 `role` 字段** → 保持。
`sql::create_user`（`sql.rs:46`）的 INSERT 也不带 role → 新用户自动落 DB 默认值 `'user'`。
**注册路径在字面上就无法表达一个角色**，这是最安全的形状。

#### ② 为什么"两个注册端点"是错的方向

若做 `POST /auth/register`（产 user）+ `POST /auth/admin/register`（产 admin），第二个端点存在本身就等于
"有一条 HTTP 路径能产出管理员"，只有两种命运：

- **公开** → 任何人调它自封 admin（灾难）
- **受保护** → 那它就不是"注册"，是"管理员创建用户"，属下面的提权路径

**词汇层面**："注册"天然含"匿名可调"，而 admin 的产生天生不该匿名可调。所以不是"注册分两套"，
而是"**注册只有一套（永远产 user），admin 的产生根本不算注册**"。

#### ③ 反例：注册端点接受 `role` 字段 = mass assignment 越权

给 `UserRegister` 加 `role: String` 并透传给 INSERT，任何人 `POST {"role":"admin"}` 即自封管理员。
"客户端能设置它不该设置的字段"这类漏洞统称 **mass assignment**（Rails 的 strong parameters、Django 的表单白名单都是为防它而生）。

#### ④ 行业惯例：没有一家提供"公开注册成管理员"

| 系统 | 首个 admin | 后续 admin |
|---|---|---|
| Django | `createsuperuser` 命令 | 后台 / shell 改 `is_staff` |
| Laravel | seeder | tinker / 后台 |
| GitHub | 组织创建者 | owner 邀请 / 提权 |
| Supabase | Dashboard / `service_role` key | Dashboard |
| Rails + Devise | seed / 手动 | 手动改 flag |

共同形状 = **自举 bootstrap（绕开公开 HTTP）+ 提权 promotion（受保护端点）**。

#### ⑤ 落到本项目：三条路，选最小

| 路径 | 内容 | 判断 |
|---|---|---|
| **A 自举首个 admin** | seed 脚本 `INSERT ... role='admin'`，手动跑一次（`sqlite3 data/users.db < seeds/00_admin.sql`） | **必做** —— 清楚表达"bootstrap 是运维动作，不是 API" |
| **B 提权** | `PATCH /admin/users/{id}/role` + `require_admin` 中间件 | **可选但值得** —— admin 里程碑里唯一能用上中间件的**写**路径，比只做 `GET /admin/users` 更能练到 `Layer` |
| **C `POST /admin/users`**（admin 代建账号） | admin 创建用户 | **跳过** —— 价值与 B 重叠，还会引入"初始密码怎么给"等额外问题，超出收手标准 |

> env 驱动启动自举（`ADMIN_EMAIL` 存在且无 admin 则创建）更"生产"，但对学习项目过度设计 —— 用 seed 文件即可。

#### ⑥ ⚠️ 必须提前知道的坑：role 放进 Claims 之后

计划让 `JwtClaims` 带 `role`（省一次查库），代价必须知道：

- **提权不即时生效**：X 被提权为 admin，他手里旧 token 的 claims 仍是 `role="user"`，**需重新登录**才拿到新角色。
- **降权更危险**：被降权者的旧 token **仍然能当 admin 用**，直到 token 过期
  （受 `access_ttl` 限制；叠加 4.7② 的 `leeway=60`，实际窗口比 ttl 更长）。
- **严格做法** = `require_admin` 里拿 `claims.sub` **回库查一次 role** —— 那就等于放弃了"免查库"这个收益。
- 这就是此前已记过的那条权衡（免查库 vs 角色变更不即时生效）。**学习阶段接受它没问题，但要明确自己接受了什么。**

**决定：按 A + B 实施；role 写进 Claims（接受"不即时生效"）；`require_admin` 用中间件（`Router::nest("/admin") + layer`）。**

#### ⑦ role 用什么类型：边界是字符串，内部是 enum

**三层分工 —— DB 与 JSON 是"边界"（字符串是它们的母语），Rust 内部保持类型安全。**

| 层 | 类型 | 说明 |
|---|---|---|
| DB 列 | `TEXT` | SQLite 没有原生 enum 类型，只能存字符串（或整数） |
| Rust（entity / claims / service） | `enum Role { User, Admin }` | 编译期穷尽匹配，非法状态不可表示 |
| JSON / JWT | `"user"` / `"admin"` | 由 serde `rename_all` 生成 |

`Role` 的落地形状（`models.rs`）：

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]   // DB  TEXT
#[serde(rename_all = "lowercase")]  // JSON / JWT
pub enum Role { User, Admin }
```

- **两个 `rename_all` 必须一致**：只写一个会让 DB 存 `"user"`、JWT 里却是 `"User"` —— 同一概念在两个边界长得不同。
- **`#[derive(sqlx::Type)]` 不用改 `Cargo.toml`**：`sqlx` 的 `macros` feature 定义里已含 `derive`（`macros = ["derive", ...]`），项目已开 `macros`。
- **强枚举 → TEXT，弱枚举 → INTEGER**：无 `#[repr]` 的强枚举，`type_info()` 是 `<str>::type_info()` = TEXT
  （`sqlx-macros-core-0.9.0/src/derives/type.rs:224-233`）；加 `#[repr(i32)]` 则编码为整数。**选强枚举**：SQL 里可读、可手查。
- **未知值 fail-closed**：decode 是严格 match，未匹配 → `Err("invalid value .. for enum ..")`（`derives/decode.rs:176`）。
  DB 里被手改成非法值 → **读取直接报错**，不会静默当普通用户。代价：加新角色时旧服务读新值会炸 → **先升代码、再写数据**。
- **为什么不选 `String`**：拼写错编译器不查（`"admni"` 照样编译）；`match` 必须写 `_ =>` default（新角色静默落 default）；
  大小写/空白不一致静默失效。enum 把这三类错误全部变成编译错误。
- **何时该改用 String / 表**：角色集合**运行时可变**（自定义 RBAC）→ 走 `roles` 表 + String；固定 `user/admin` → enum。
  DB 列本来就是 TEXT，将来切换**不需要迁移数据**，只换 Rust 侧类型。

#### ⑧ ⚠️ 落地坑：`query_as!` 遇自定义类型必须写**列类型注解**

给 `UserEntity` 加 `role: Role` 后，`cargo check` 报：

```
error[E0277]: the trait bound `Role: From<std::string::String>` is not satisfied
```

**根因**：`query_as!` 宏**看不到目标类型的字段定义**（宏里只给了 `UserEntity` 这个**名字**，Rust 类型信息在宏展开时不可见），
所以它只能从 **SQL 列的声明类型**推断出一个 Rust 类型，再用 `.into()` 桥接到结构体字段
（`sqlx-macros-core-0.9.0/src/query/output.rs:153-160`）：

```rust
// 宏生成的代码（简化）：role 列被推断为 String，再 .into() 到 Role
let v = row.try_get_unchecked::<String, _>(i)?.into();
```

而 SQLite 把 TEXT 映射为 `String`（`sqlx-sqlite-0.9.0/src/type_checking.rs:8-24` 的 `impl_type_checking!(Sqlite { .., String, .. })`），
于是编译器要求 `String: Into<Role>`，即 **`Role: From<String>`** —— 这是 `query_as!` 宏对 SQLite 自定义类型的**固有限制**，不是你写错。

**解法 1（推荐）：给该列写类型注解**

```rust
sqlx::query_as!(
    UserEntity,
    r#"SELECT id, role as "role: Role", name, email, password_hash, message FROM users"#
)
```

宏改用 `try_get_unchecked::<Role>(i)` → 调你 derive 出来的 `Role::decode`（**fail-closed 保住**），
并**保留 `query_as!` 的编译期检查**（列名/类型错在编译期就炸）。代价：每处 SELECT 都要写注解。

**解法 2：改用 `FromRow` + `query_as` 函数**

```rust
#[derive(Clone, sqlx::FromRow)]
pub struct UserEntity { /* 字段同前，多一个 role: Role */ }

sqlx::query_as::<_, UserEntity>("SELECT id, role, name, email, password_hash, message FROM users")
```

走 `row.try_get::<Role>(i)`（`derives/row.rs:100-108` 的 `(false, None, None)` 分支），同样 fail-closed、SQL 干净；
**但失去编译期检查**（列名写错要运行时才发现）。

**❌ 不要做**：给 `Role` 实现 `From<String>` —— `From` 不能失败，遇到未知值只能 `panic!`（崩服务）或静默降级成 `User`（**破坏 fail-closed**）。

> 实测（2026-10-10）：临时 crate 指向本项目库副本 —— 无注解精确复现 `Role: From<String>`；上述两条解法均编译通过。
> 顺带：`get_user_by_id` / `get_user_by_email` 的 SELECT **原本漏了 `role` 列**，会报 `missing field role`（E0063），加注解前先补列。

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
| **T1 服务层测试**（连真实库） | `auth::register` / `login` / `update_me` / `delete_me` / `update_password_me` 的业务规则 | 半天 | ✅ **完成 2026-10-10（8 条，共 21 全绿）** |
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

**T1（`#[sqlx::test]`，一条一测试）—— ✅ 完成 2026-10-10，`cargo test` → 21 passed**

| 测试 | 覆盖 | 状态 |
|---|---|---|
| `test_valid_password`（纯函数） | `is_valid_password` 合法 / 空 / 过短（按**字符数**） | ✅ |
| `test_register_success` | 拿到实体 + **round-trip `verify_password`** + hash ≠ 明文 + name/email 落库 | ✅ |
| `test_register_same_email` | 第二次注册 → `Conflict` | ✅ |
| `test_register_short_password` | 空串 + `"123"` → `BadRequest`(400) | ✅ |
| `test_login_success` | 返回体字段一致 + token 的 `sub == user.id`（锁契约） | ✅ |
| `test_login_failed` | 邮箱不存在 / 密码错 → **都是 `Unauthorized`** | ✅ |
| `test_update` | 先置 message → 只改 name → message 不变；只改 message；都传；都不传 → `BadRequest`；末尾**重读库**验证落库 | ✅（① 已修 / ④ 已补） |
| `test_update_password` | 旧密码错 → 401；**旧密码错 + 新密码短 → 401（顺序哨兵）**；旧密码对+新密码短 → 400；改完能登、旧密码登不上 | ✅（③ 已改旧密码先 + **哨兵已补**） |
| `test_delete_me` | 注册 → 登录 → 注销 → **再登录 → `Unauthorized`** + `get_user → NotFound` + 重复注销 → `NotFound` | ✅（⑤ 已补全） |

- 通用断言约束：`UserEntity` / `LoginResponse` **无 `Debug`** → 用 `matches!(res, Err(ApiError::X(_)))`，**不要 `unwrap_err()`**。
- 三条 review 打磨项（断言强度 / 字节 vs 字符 / 校验顺序）详见 **4.8**。
- ✅ **4.8 的 ①~⑤ 全部补齐**（21 条全绿）。唯一可挑的只剩命名：哨兵那两条复用了 `update_pwd_1` / `pwd_res1` 的名字（Rust 允许遮蔽，但读起来费劲）。

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

### 4.8 T1 实施记录与三条 review 结论（2026-10-09 记录，2026-10-10 更新）

**产出**：`cargo test` → `21 passed; 0 failed`（6.26s）。构成 = jwt.rs 7 条 + auth.rs 内 T0 纯函数 5 条
+ `test_valid_password` 1 条 + `#[sqlx::test]` 8 条。改动集中在 `src/auth.rs`（新增 `is_valid_password` + 测试模块）。**待 commit。**

**做对的**：`matches!` 全部用对（Conflict / BadRequest / Unauthorized）；`test_login_success` 验了 token 的 `sub`（锁契约而非实现）；
`test_login_failed` 两个分支都覆盖（守住防邮箱枚举）；`test_register_success` 采纳了 round-trip `verify_password`
（锁住"存进去能验回来"这个真契约，而非只 `assert_ne!` 明文）；密码策略用 `BadRequest`(400) 而非 `ApiError::Password`(500)——**对了**。

#### ① `test_update` 的"只传 name → message 不变"是**弱断言** → ✅ 2026-10-10 已修

原来 `assert_eq!(update_res1.message, user.message)` 的两边**一开始都是 `None`**——即使实现把 `message` 误清成 `NULL`，这条**照样通过**。
**这正是 4.7④ 那句"这个 bug 出现时，我会不会恰好也通过？"的实战翻版。**

**修法（已做）**：加一段 `update_request_0`（只传 `message: Some("message_0")`）**先落一个值**，再发 `update_request_1`（只传 name），
断言 `message` 仍是那个**字面量** `Some("message_0")`——不再是 `user.message` 这个"也等于 None"的参照。

#### ② `is_valid_password` 的 `.len()` 是**字节数**，不是**字符数** → ✅ 2026-10-10 已改

原来 3 个汉字的密码 = 9 字节 ≥ 8 → 判"合法"，实际只有 3 个字符。NIST SP 800-63B 原文是 min 8 **characters**。
**已改为 `origin_password.chars().count() >= 8`**（`auth.rs:72`）。

#### ③ `update_password_me` 的校验顺序 → ✅ 2026-10-10 已改为**旧密码先**，**顺序哨兵已补**

**决策（2026-10-10 定案）：先验旧密码（401 优先）。** 理由，从强到弱：

- **与项目自身分层同构（最硬）**：`handler.rs` 里 `CurrentUser`（认证）**永远先于** `Json`（数据）——不通过认证就不解析 body。
  旧密码 = 这次敏感操作的"再认证"，service 层理应一致。
- **HTTP 语义不该颠倒**：401 = "未通过身份验证"，400 = "通过了但请求数据不合法"；先报 400 等于在未认证请求上评点数据。
- **业界惯例**：Django `PasswordChangeForm` 显式声明 `field_order = ["old_password", "new_password1", "new_password2"]`（把旧密码硬排第一）；
  Laravel 的 `current_password` 规则同理。
- **错误指向正确方向**：policy-first 下"旧密码错 + 新密码短"报"新密码太短"，用户照着改完再提交才发现旧密码也错——白改一轮。
- **诚实的边界**：这**不是安全漏洞**（规则公开 + 调用方已持合法 session，泄露 ≈ 0），是**一致性与响应语义**问题。
- **反方（fail-fast）很弱**：省一次 argon2，但攻击者持合法 token 可发合规长度密码，成本照付；真要防 argon2 被刷，正确工具是限流。

**✅ 顺序哨兵已补（2026-10-10，`auth.rs:468-475`）**：新增一条 `old 错 + new 短 → Unauthorized` 的用例——
这是**唯一能区分两种顺序**的组合（旧密码先 → 401；policy 先 → 400）。
**从此把实现翻回 policy-first，这条会立刻亮红。**

> 中途的教训值得留档：改顺序时，原来的第 2 例（旧密码**错** + 新密码短 → 期望 `BadRequest`）被改成"旧密码**对** + 新密码短"，
> **顺手就把唯一勉强锁住顺序的约束撤掉了**——而那两条 `old 错 + new 合法` / `old 对 + new 短` 在两种顺序下**结果完全相同**，
> 谁也区分不了。**"断言在，顺序没在"**：没有一条用例能区分两种实现时，这个顺序等于没被保护。

#### ④ `test_update` 的断言全基于返回值、证明不了落库 → ✅ 2026-10-10 已补独立重读

`update_me`（`auth.rs:124`）开头 `get_user` 读库 → 在内存里改 → `sql::update_user` 落库 → **直接 `Ok(user)` 返回内存对象**（没有再读一次库）。
所以对返回值的断言证明的是"函数内的内存状态"，**不是"数据库真的写了"**。

- 原状：`update_0` 的 message 落库**被间接验证**（`update_1` 调用**开头会重读库**）；但 `update_1` / `update_2` 自身的落库**无人验证**。
- **✅ 已补**：`test_update` 末尾加了一次**独立重读**——
```rust
let persist = get_user(&pool, user.id).await.expect("期望重查成功");
assert_eq!(persist.name, "name_update_2");
assert_eq!(persist.message, Some("message_2".to_string()));
```
- **对照**：`test_update_password` **从没**这个问题——它靠"用新密码登录成功"验证落库，是端到端的真验证。

#### ⑤ `delete_me` 的测试 → ✅ 2026-10-10 已补全（含文档原意那条）

用户此前以为"注销函数不存在"——其实存在，只是**命名不对称**：auth 层 `delete_me`（`auth.rs:155`）× sql 层 `delete_user`（`sql.rs:57`），
在 `auth.rs` 里搜 `delete_user` 自然搜不到。

`test_delete_me` 现覆盖三件事：
1. 注册 → 登录 → `delete_me` → **再登录 → `Unauthorized`**（能抓到"delete 是空操作"）
2. **`get_user(&pool, user.id)` → `Err(NotFound)`** ← **这条才对上 §4.5 原本写的"持旧 token 查库兜底"**（token 仍解得出 id，但按 id 取不到 → 404）
3. **重复注销 → `Err(NotFound)`** ← 覆盖 `delete_me` 的 `!res` 分支

> 区分清楚：**登出 / token 失效**确实不存在（第七节的 refresh token / 黑名单），但这条指的是 `DELETE /me`（注销账号）。

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
- **`.len()` 数的是字节，不是字符**：非 ASCII（中文 / emoji）下会远大于"字符数"。做长度校验前先问"我数的是字节还是字符"
- **测试会把"实现顺序"固化成"契约"**：校验的先后（如"先验密码策略还是先验旧凭据"）一旦被断言写死，改实现就必须同步改测试——要有意识地选择锁哪个顺序；反过来，若**没有任何**用例能区分两种顺序，那这个顺序就等于**没被保护**（"断言在，顺序没在"）
- **命名不对称是真实的排查成本**：`auth::delete_me` vs `sql::delete_user` 会让搜索落空，进而误判"函数不存在"。跨层同名操作尽量对齐词根
- **函数返回内存对象时，断言返回值证明不了落库**：先读库 → 改内存 → 写库 → `Ok(内存对象)` 的模式下，"返回值正确"与"数据库真的写了"是两件事；要验证落库，必须在更新**之后重新读一次库**
- **公开注册端点永远不接受 `role` 之类的授权字段**（mass assignment 越权）。管理员由「自举（绕开公开 HTTP）+ 提权（受保护端点）」产生 —— **"admin 注册"不该是一个端点**；"注册"一词天然含"匿名可调"，而特权产生天生不该匿名可调
- **授权信息塞进凭据会引入不一致窗口**：JWT 带 `role` 后，**提权**要重登才生效（旧 token 仍是旧角色），**降权**则旧 token 仍可用直到过期 —— "免查库"的收益正是冒这个窗口的风险换来的
- **`query_as!` 宏看不到目标类型的字段定义**：它只能从 SQL 列的声明类型推断 Rust 类型、再用 `.into()` 桥接到结构体字段。SQLite 把 TEXT 推断为 `String`，
  所以自定义类型（enum / newtype）**必须写列类型注解** `col as "col: Type"`，否则报 `X: From<String>`。所谓"绕过"（给类型实现 `From<String>`）会牺牲 fail-closed，别用
- **"有限、封闭、语义明确的集合"用 enum，别用 `String`**（角色、状态、来源…）：DB/JSON 是**边界**（字符串是它们的母语），程序**内部**保持类型安全。
  `String` 会把"拼写错 / 非穷尽 match / 大小写不一致"三类错误全部推迟到运行时，而且往往是**静默**的

---

## 九、下一步动作（按顺序执行）

1. **T0 ✅ 已完成并提交（2026-10-08 / commit `e54aff8`）**：`src/jwt.rs` 7 条 + `src/auth.rs` 5 条，`cargo test` → 12 passed。
2. **✅ 遗留点 3 已修并提交（2026-10-09 / commit `0cee939`）**：`get_me` 逻辑下沉为通用的 `auth::get_user`；
   两个读端点共用之，`get_user_id` 改返 `PublicUserResponse`（视图分叉，见第二节末）。
3. **T1 ✅ 完成并提交（2026-10-10 / commit `f201924`）**：`#[sqlx::test]` 8 条 + 纯函数 1 条，`cargo test` → **21 passed**。
   4.8 的 ①~⑤ **全部补齐**（`chars().count()` / 先验旧密码 + **顺序哨兵** / `test_update` 强断言 / **落库重读** / `delete_me` 三条断言）。
   工作树已干净。顺手可把哨兵处重复的 `update_pwd_1` / `pwd_res1` 改成不同名字（非必须）。
4. **补跑第二节验收清单**：10-09 已跑通第 1-6 条（`login` 首次真正执行）；**第 7-9 条**（PATCH / PUT password / DELETE 的 HTTP 层）留到 T2 一起。
5. **T2 前置**：加 `src/lib.rs`（模块转 `pub`，`main.rs` 变薄壳）；`Cargo.toml` 加
   `[dev-dependencies] tower = { version = "0.5", features = ["util"] }`
6. **admin + role**（设计已定案，见 **3.3**）：
   - **a.** 迁移加 `role TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('user','admin'))`（`UserEntity` 同步加 `role: Role`；
     `sql.rs` 的**三处** SELECT 列清单要跟着改，并按 **3.3⑧** 写类型注解 `role as "role: Role"`，否则报 `Role: From<String>`）
   - **b.** **自举首个 admin**：`seeds/00_admin.sql`（`INSERT ... role='admin'`），手动执行一次。
     ⚠️ **注册端点不动，永远产 user**（`UserRegister` 不加 `role` 字段）
   - **c.** Claims 加 `role`（接受"角色变更不即时生效"，见 3.3⑥）→ `Router::nest("/admin", ...)` + `require_admin` 中间件 → `403` 登场
   - **d.** 提权端点 `PATCH /admin/users/{id}/role`（可选但推荐：中间件唯一能用上的**写**路径）
   - **e.** `get_all_users` 复活（带分页）；据此决定 `UserResponse` 是否加 `role`
7. **T2**：HTTP 层测试补 401 / 403 / 201 / 409 的状态码断言与 `CurrentUser` 失败路径
8. `git commit` 收尾 → 开新仓库做 Agent 的 V0
