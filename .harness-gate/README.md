# Arc Admin 质量接入

这是项目自己的原生数据接入层。正式提交和 CI 继续执行 `cargo flow`；不删除既有 25 个执行步骤，也不改变覆盖率 80% 或 CRAP 30 的门槛。

`quality.toml` 将生产 Rust 函数、全部非测试 TypeScript 文件及 API 提供者/生成客户端分别绑定到原生测量系列。API 关系的方向为 backend → frontend。OpenAPI 文档不属于生产函数覆盖率的计数范围。Angular CRAP 保留 unsupported，零分母文件保留 not_applicable，均不伪造数值。

## 主机准备

先完成并审阅原生采集，保留源文件、工具输入、原始产物和采集上下文。主机为 `arc-native-measurements/v1` 文件提供外部 SHA-256；不要从下载的文件自身读取“可信哈希”。现有固定源码可复用已审阅的数据，源码或工具输入有变化必须重新采集。

```bash
cd frontend
npm run test:coverage
cd ..
python3 -B .harness-gate/host.py \
  --measurements /absolute/path/measurements.json \
  --sha256 REVIEWED_SHA256 \
  --core /absolute/path/harness-gate \
  --baseline
harness-gate verify --all
```

`host.py` 只验证并重放主机审阅的数据，生成短期 Ed25519 请求、可信 state 和公钥；`collect.py` 校验签名请求绑定的文件，再向 Core 提交数据。Core 决定最终 PASS/FAIL。私钥、state、原始数据和报告不提交到 Git；三个空请求文件是 Git 基线的配置占位，主机准备时会在工作目录覆盖。

Git 基线必须来自 `origin/main` 的真实、唯一 merge base，且与 HEAD 不同。首次接入必须先让配置进入主分支，再在后续提交上接受相同源码的审阅数据；不能把当前提交伪装成自己的基线。`--baseline` 会核对基线提交及其 API 父版本的实际 Git 内容。

## Hook

hook 是 partial，不采集完整覆盖率。用 `harness-gate scope --staged --json` 的结果作为 `host.py --profile hook --scope-json ...` 输入。完整质量仍显示 not_collected。

Core 0.4.2 已修复暂存区主机 state 和大型报告发布问题，正式构建及公开签名文件已通过本机 full 和实际 `harness-gate hook` 验收。暂存区与工作目录内容不同的用例通过；hook 的完整质量仍为 not_collected。正式 `cargo flow hook` 保持原有行为。详见[2026-09-13 验收](../docs/quality/20260913/README.md)。

## 当前测量

2026-09-13：真实后端 1,778 个函数；前端 141 个生产 TS 文件，24 个测试文件、85 个测试通过。前端行覆盖率 877/1986（44.15%），函数覆盖率 239/578（41.34%）。API 原生比较无破坏性变化，重新生成的客户端与 Git 内容一致。

三路认证采集和完整数值评估已通过传输验证，但质量总结果为 fail：覆盖率和部分 CRAP 不达标。不得以安装、传输成功或 API 三项通过替代质量达标。
