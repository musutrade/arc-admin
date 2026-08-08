# 日志与故障定位

后端使用 `tracing` 输出运行日志。开发环境默认使用紧凑文本，生产环境默认使用 JSON Lines；每一行都是独立 JSON 对象，可直接由容器平台或日志采集器处理。

## 运行配置

```env
LOG_FORMAT=json
RUST_LOG=info,tower_http=info,sqlx=warn
SERVICE_NAME=arc-admin-backend
```

`LOG_FORMAT` 只允许 `pretty` 或 `json`。未设置时，`APP_ENV=production` 默认使用 `json`，其他环境默认使用 `pretty`。项目初始化命令会按项目标识生成独立的 `SERVICE_NAME`。

## 请求关联

后端为每个请求生成 `x-request-id`，并同时作为日志中的 `trace_id`。合法的上游 ID 会被保留；空值、超过 64 字符或包含非字母数字及 `.`、`_`、`-` 的值会被替换。

所有响应都会返回 `X-Request-ID`。HTTP 500 错误响应示例：

```json
{
  "error": {
    "code": "INTERNAL_ERROR",
    "message": "服务器内部错误",
    "traceId": "e2f39c0d-ef21-4bed-9c8b-b67e8f71a743"
  }
}
```

前端会显示“问题编号”，管理员可使用该编号查询同一次请求的访问日志和错误根因。panic 也会转换为相同的 JSON 500 契约。

## 字段规范

请求日志固定记录 `service`、`environment`、`version`、`trace_id`、`method`、匹配后的 `route`、`status_code`、`latency_ms` 和认证后的 `user_id`。错误事件额外记录 `event`、`error_code` 与服务端根因。

访问日志不记录原始 URI 查询串、完整 Header、请求体或响应体。禁止写入 `Authorization`、Cookie、JWT、密码、验证码、Token、数据库连接串和 SQL 参数值。auditor 会拦截完整 Header 和显式敏感日志字段。

数据库 `audit_logs` 负责“谁修改了什么”，JSON 运行日志负责“请求为什么失败”，二者用途不同，不应互相替代。

## 本地排查

捕获 JSON 日志后，可按问题编号筛选：

```bash
rg 'e2f39c0d-ef21-4bed-9c8b-b67e8f71a743' application.jsonl
```

也可以生成同一请求的紧凑错误上下文：

```bash
cargo flow parse-logs \
  --input application.jsonl \
  --output /tmp/error-context.json
```

解析器优先选择第一条 ERROR 日志，支持从事件字段、`span`、`spans` 中读取 `trace_id`，并保留同一请求最多 30 条上下文。
