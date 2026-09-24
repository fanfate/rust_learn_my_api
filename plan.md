<!-- page 1 -->
### Axum + SQLx 实战学习清单

按先跑通最小项目，再逐步加复杂度的顺序来。每一步都建议用你现有的 FastAPI + React 项目做对 照重写，这样学起来最快。

### 阶段 1：环境搭建与最小服务

目标：跑通一个最简单的 Axum HTTP 服务，不急着接数据库。

| 任务 | 重点 |
|---|---|
| 新建项目 | cargo new my-api |
| 引入依赖 | axum 、 to 、 serde 、 se |
| 写第一个路由 | GET /health  返回  {"status": "ok"} |
| 启动服务 | 使用  to  启动  Axum |
| 测试接口 | 用浏览器、 curl  或 Post man 请求 |

这一步完成后，你应该能理解：

- Axum 的路由是怎么写的
- Tokio 异步运行时是怎么启动服务的
- serde 是怎么做 JSON 序列化反序列化的

### 阶段 2：路由、路径参数、查询参数

目标：学会从请求中拿参数。

练习接口示例：

http

```
GET /users/:id
GET /users?keyword=xxx&page=1&size=10
```

kio rde_json kio::main重点掌握：

<!-- page 2 -->
| 知识点 | 作用 |
|---|---|
| Path | 获取路径参数，比如  /users/:id |
| Query | 获取查询参数，比如  ?keyword=xxx |
| Json | 解析 JSON 请求体 |
| serde  派生 | 自动把 JSON 转成 Rust 结 构体 |

建议做一个用户查询接口：

- 按 ID 查询用户
- 按关键词搜索用户
- 支持分页参数

### 阶段 3：请求体、响应结构、统一错误格式

目标：写出更像真实项目的 API

练习内容：

http

```
POST /users
PUT /users/:id
DELETE /users/:id
```

重点掌握：

- 请求体用 Json<UserCreate> 接收
- 响应统一成类似结构：

JSON

```
"code" 0
"message" "ok"
"data"  ...
```

- 错误响应也统一格式：

<!-- page 3 -->
JSON

```
"code" 40001
"message" "user not found"
"data" null
```

这一步对应 FastAPI 里的：

- Pydantic 模型
- 请求校验
- 统一异常处理

在 Rust 里，你会用结构体、 serde IntoResponse 来实现。

### 阶段 4：接入 SQLx 和 PostgreSQL

目标：让项目真正连数据库。

推荐依赖：

| 依赖 | 作用 |
|---|---|
| sqlx | 异步数据库访问 |
| sqlx  的  postgres  特 性 | 连接 PostgreSQL |
| sqlx  的  runtime-tokio  特性 | 和 T 配合 |
| sqlx  的  mi  特性 | 管理数据库迁移 |
| dotenvy  或类似工具 | 读取  .e  配置 |

练习任务：

1. 创建 users 表
2. 写数据库迁移文件
3. 启动时连接数据库
4. 实现 CRUD

http

gratePOST /users

<!-- page 4 -->
```
GET /users
GET /users/:id
PUT /users/:id
DELETE /users/:id
```

### 重点理解：

- SQLx 是异步的
- 数据库连接池怎么共享
- 查询结果怎么映射成 Rust 结构体
- 编译期检查 SQL 的优势

### 阶段 5：状态管理：共享数据库连接池

### 目标：学会在多个 handler 之间共享数据。

### 重点掌握：

- State 提取器
- Arc 共享状态
- 把数据库连接池放进 AppState

### 示例结构：

Rust

```
struct AppState
db PgPool
```

### 然后路由里这样取：

Rust

```
async fn get_user
State state State Arc AppState
Path id Path i64
```

### 这一步非常重要，因为真实项目里不只有数据库连接池，后面还会有：

- 配置
- Redis 客户端
- JWT 密钥
- 缓存

<!-- page 5 -->
- 外部服务客户端

都会放进 AppState

### 阶段 6：错误处理： anyhow 和 thiserror

目标：学会区分应用错误和库领域错误

推荐规则：

| 场景 | 推荐 |
|---|---|
| 应用层、CLI、服务入口 | anyhow |
| 库、领域错误、API 错误 码 | thiserror |
| HTTP 响应错误 | 自定义  Ap  并实 现  In |

练习目标：

- 数据库找不到记录返回 404
- 参数校验失败返回 400
- 未授权返回 401
- 内部错误返回 500
- 所有错误都统一成 JSON 格式

这一步会让你的项目从能跑变成像生产代码

### 阶段 7：中间件：CORS、日志、超时、鉴权

目标：学会用 Tower 中间件组织通用逻辑。

建议顺序：

1. **CORS**
- 让 React 前端能正常跨域调用
2. 日志请求追踪
- 打印请求方法、路径、耗时

pError toResponse  - 后面可以接 tracing

<!-- page 6 -->
3. 超时
- 防止某个接口卡死整个请求
4. 鉴权中间件
- 从 Authorization: Bearer xxx
- 把用户信息注入请求上下文
5. 权限控制
- 比如某些接口只有管理员能访问

这一步对应 FastAPI 里的：

- middleware
- dependency injection
- auth 依赖

### 阶段 8：认证与授权

目标：实现一个完整登录流程。

建议实现：

http

```
POST /auth/register
POST /auth/login
GET /me
```

重点掌握：

| 功能 | 实现方式 |
|---|---|
| 密码存储 | argon2  或  bc |
| JWT 签发 | jsonwebtoken |
| 请求鉴权 | Axum 中间件或提取器 |
| 用户上下文 | 把当前用户放进请求状态 |

注意：

- 密码不要明文存储
- JWT 密钥不要写死在代码里

rypt里解析 token

<!-- page 7 -->
- 登录失败不要暴露用户不存在或密码错误的过细信息
- 敏感接口必须校验权限

### 阶段 9：流式传输：SSE WebSocket、文件流

目标：解决你之前担心的流式场景。

建议练习三个场景：

### 1. SSE：服务端推送

适合：

- AI 流式输出
- 通知推送
- 实时状态更新

练习接口：

http

```
GET /stream/chat
```

重点：

- Axum 可以返回流式响应
- 配合 tokio-stream 或 futures::stream
- 前端用 EventSource 接收

### 2. WebSocket

适合：

- 聊天
- 实时协作
- 双向通信

练习接口：

http

```
WS /ws
```

重点：

<!-- page 8 -->
- 理解 WebSocket 握手
- 理解消息收发
- 理解连接生命周期
- 处理断线重连

### 3. 文件流式下载

适合：

- 大文件下载
- 报表导出
- 视频音频流

练习接口：

http

```
GET /files/:id/download
```

重点：

- 不要一次性把大文件读进内存
- 使用流式响应
- 正确设置 Content-Type 和 Content-Disposition

### 阶段 10：项目分层结构

目标：把项目从一个 main.rs 变成可维护工程。

推荐目录结构：

text

```
src/
main.rs
app.rs
config.rs
error.rs
state.rs
routes/
mod.rs
users.rs
auth.rs
files.rs
```

<!-- page 9 -->
```
handlers/
mod.rs
users.rs
auth.rs
services/
mod.rs
user_service.rs
auth_service.rs
repositories/
mod.rs
user_repo.rs
models/
mod.rs
user.rs
dtos/
mod.rs
user_dto.rs
middleware/
mod.rs
auth.rs
```

分层建议：

| 层 | 职责 |
|---|---|
| routes | 定义路由 |
| handlers | 接收请求、调用 service、 返回响应 |
| services | 业务逻辑 |
| repositories | 数据库访问 |
| models | 数据库实体 |
| dtos | 请求/响应数据结构 |
| middleware | 通用横切逻辑 |

### 这一步会让你的项目更接近真实后端工程。

### 阶段 11：测试

<!-- page 10 -->
目标：不要只会

建议覆盖三类测试：

### 1. 单元测试

测试 service 层逻辑，比如：

- 密码校验
- 分页参数处理
- 业务规则判断

### 2. 集成测试

测试完整 HTTP

http

```
POST /users
```

启动服务手动测

请求：

```
GET /users/:id
```

重点：

- 使用测试数据库
- 每个测试前清理数据
- 避免测试之间互相污染

### 3. 请求响应测试

可以用 Axum 的测试方式直接构造请求，不需要真正启动端口。

### 阶段 12：配置、日志、健康检查

目标：让项目更像生产服务。

建议实现：

http

```
GET /health
GET /ready
```

配置项建议放进 .env

env

```
DATABASE_URL=postgres://...
SERVER_PORT=3000
```

<!-- page 11 -->
```
JWT_SECRET=...
LOG_LEVEL=info
```

### 日志建议用 tracing ，可以打印：

- 请求路径
- 请求耗时
- 错误信息
- trace id
- 用户 ID

### 阶段 13 Docker 与部署

### 目标：把项目打包成可部署服务。

### 建议做两件事：

### 1. Dockerfile

### 重点：

- 使用 Rust 官方镜像构建
- 构建完成后只保留二进制文件
- 最终镜像尽量小

### 2. docker-compose

### 至少包含：

YAML

```
services
api
build
ports
"3000:3000"
depends_on
postgres
postgres
image postgres
environment
POSTGRES_DB myapi
POSTGRES_USER myapi
POSTGRES_PASSWORD myapi
```

### 后续还可以加：

<!-- page 12 -->
- Nginx 反向代理
- Redis
- 数据库迁移容器
- 日志收集

### 阶段 14：对照 FastAPI 项目重写

目标：把已有项目迁移成 Axum 版本。

建议按模块迁移：

1. 用户注册登录
2. 用户 CRUD
3. 权限控制
4. 文件上传下载
5. 流式接口
6. WebSocket
7. 数据库迁移
8. 错误处理
9. 日志追踪
10. Docker 部署

每迁移一个模块，就对比 FastAPI 版本：

- 开发体验差在哪
- 类型安全强在哪
- 错误处理是否更清晰
- 性能是否有提升
- 哪些地方 Rust 更麻烦

这种对比学习比单纯看教程有效得多。

### 阶段 15：性能与生产化优化

目标：从能运行升级到可上线

建议关注：

<!-- page 13 -->
| 方向 | 内容 |
|---|---|
| 数据库 | 连接池、索引、慢查询 |
| 缓存 | Redis 缓存热点数据 |
| 日志 | 结构化日志、请求追踪 |
| 监控 | 健康检查、指标接口 |
| 安全 | 输入校验、SQL 注入防 护、CORS、限流 |
| 部署 | Docker、CI/CD、环境变 量管理 |

### 最小学习顺序

如果你时间有限，先按这个顺序来：

1. Axum 基础路由
2. JSON 请求响应
3. SQLx + PostgreSQL
4. State 共享数据库连接池
5. 统一错误处理
6. JWT 登录
7. CORS 和中间件
8. SSE / WebSocket / 文件流
9. 项目分层
10. Docker 部署

先把这 10 步做完，你就已经具备用 Rust 写真实 Web 后端的能力了。

AI生成）
