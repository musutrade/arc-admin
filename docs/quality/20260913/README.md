# Arc Admin 原生质量接入验收 — 2026-09-13

Core **0.4.2** 的正式构建已完成本机 full 和实际 `harness-gate hook` 验收，并通过公开下载的 SHA-256／Sigstore 安装验证。验收构建与公开安装文件的摘要一致：`eda5179d96b32269124150980ffcaa7e57878605133bc6ead51e5aeb8a6ad0a0`。接入验收通过，项目质量仍为 **FAIL**。

| 验收项 | 实际结果与回执 |
| --- | --- |
| 完整执行与质量 | [正式 Core full](core042-full-acceptance.json)：原有 25 个执行步骤及 2 个前置检查全部通过；3 个认证 producer 提交 1,921 条记录，质量阶段完整结束 |
| Git 基线 | HEAD `859daa95b180c88f86fb817418e6b6ff5a5b1123`，真实 merge base `d5e113ec2d49cd7dc30aaf2a14db016ba9d7ca94`；[正确基线、篡改、缺失、错误 Git base 控制](baseline-controls.json) |
| 完整报告 | [构建验收](core042-artifact-acceptance.json)：5,760 个质量判定与原评估逐项相同，33 个 manifest 产物哈希全部核验通过 |
| 暂存区 | 同一回执：实际调用 `harness-gate hook`，暂存区与工作目录内容不同，执行通过；完整质量明确 `not_collected` |
| 环境／服务 | 真实 api_flow 测试进程无 DATABASE_URL、有 TEST_DATABASE_URL；正常测试服务全部 CLEANED；[无效测试数据库名失败清理](service-failure-control.json)单独记录执行隔离范围 |
| 证据反例 | [集成传输控制](transport-controls.json)：篡改、缺失、过期、旧上下文、测试输入变化和缺少必需能力均拒绝；保留其原源码和测量包身份 |
| 路由 | [路由控制](routing-controls.json)：backend／frontend 各自命中，质量配置命中三组件；Core 与原 staged scope 一致 |
| 公开发行 | [Core 公开安装](core-public-install.json)：[GitHub 0.4.2](https://github.com/musutrade/Harness-Gate/releases/tag/v0.4.2) 与 [crates.io 0.4.2](https://crates.io/crates/harness-gate/0.4.2) 已发布；旧版本保留 |

## 原生数据和实际质量

当前 main 的 chacha20 lockfile 更新后，旧输入正确拒绝。只补变化的后端原生采集，1,778 个生产函数映射完整，保留二进制两次重新导出一致；[新系列记录](backend-series-transition.json)保留真实来源。前端与 API 输入未变，继续复用已有原生数据。本次新 Core 验收没有重跑原生测量工具链的编译和采样。

前端 24 个测试文件、85 个测试通过，全部 141 个非测试 TS 文件进入测量。行覆盖率 877/1986（44.15%）、函数覆盖率 239/578（41.34%），低于原有 80% 门槛。API 破坏性变化、生成漂移和兼容性三项通过；部分后端覆盖率和 CRAP 仍失败。全部 5,760 项判定为 2,110 fail、3,391 pass、118 not_applicable、141 unsupported。Angular CRAP 保持 unsupported，不合成有利数值。

测量包 SHA-256：`c3e17d97984ab8832a736b3ea6d475f8f6e0f38a88f20f2d63c49561748189b4`。已接受的项目数据保留 RC2 来源及原 measurement series；后续插件分发版本不重标旧证据、不自动建立跨系列等价。安装版本与采集来源不是同一件事。[最新插件安装与兼容性](https://github.com/musutrade/Harness-Gate/blob/main/docs/quality/rust-collector-installation.md)由交付记录独立说明。

## 成本记录与边界

[完整成本观察](complete-quality-cost.json)保留原三 job 并发执行 345.851 秒，加同输入质量聚合 38.485 秒，阶段合计 384.336 秒；修复候选 full 为 308.396 秒。本次正式 Core 主机准备加 full 为 305.652 秒，精确值见正式回执。

两侧 300 项源码／测试／工具输入和 5 项质量配置字节相同，复用同一原生测量；原拓扑重复前置检查，总计 31 步，统一 full 为 27 步，业务步骤均为 25。共享主机缓存和 Cargo 锁，后续执行缓存更热；阶段合计不含修复间隔、首次原生编译采样、发行下载或安装。原拓扑质量聚合和统一报告输出布局不同。这是单组有边界的本机观察，不能据此声称统计性加速、冷缓存收益或批准替换正式 CI。

## 保留的失败与后续边界

Core 0.4.1 的真实暂存区主机输入问题和大型报告发布失败由 0.4.2 修复；外部证据的 16 MiB 限制没有扩大。旧 [Core 失败与候选凭据](https://github.com/musutrade/Harness-Gate/blob/718bc5a1ea82f8364f91fc72f179ea0db689f6b5/docs/quality/arc-native-20260913/released-core-large-report-failure.json)原样保留。正式文件首次本机尝试因后台 PATH 缺 Cargo 而执行失败；补齐主机启动环境后用同一二进制通过，上述正式回执绑定成功重跑。

本目录的 `integration-controls.json` 是 full 基线验收前的历史阶段回执，其 `full_git_baseline_accepted=false` 不改写；后续成功结果以本页新回执为准。

正式提交和 CI 继续使用 `cargo flow`；原 hook、全部执行步骤、覆盖率／CRAP 门槛、基线与失败阻断语义保持不变。没有提升插件稳定性级别或移除旧门禁。完整大报告、原始采集和私钥保留在验收主机，本目录只提交小型回执和摘要。
