# Harness-Gate 完整质量验收

项目质量配置已通过 PR #40 纳入主分支。正式门禁仍是 `cargo flow`；本流程保留原有 25 个执行步骤，并由 Core 对真实原生数据执行完整质量判断。

## 基线与采集

在包含质量配置的主分支提交之后创建验收提交，保持被测生产源码和测试输入不变。主机必须审阅测量文件的来源及 SHA-256；不得从测量文件自身取得可信摘要。

```bash
python3 -B .harness-gate/host.py \
  --measurements "$ARC_QUALITY_BUNDLE" \
  --sha256 "$ARC_QUALITY_BUNDLE_SHA256" \
  --core "$ARC_QUALITY_CORE" \
  --baseline
"$ARC_QUALITY_CORE" verify --all
```

基线取 `origin/main` 与 HEAD 的唯一 merge base，且不能等于 HEAD。主机校验基线的真实 Git 配置、源码、测试输入和 API 父版本，再生成受外部摘要约束的 manifest。首次配置提交之前不存在这些 Git 文件，必须阻断，不能自动补写历史基线。

原始后端 capture、工具路径、锚和重新导出的二进制均保留。相同源码可通过已审阅的原生数据重放；改变生产源码、测试、模板或工具输入后，必须提供新测量。签名请求、主机私钥、公钥、state、基线目录及原始报告不作为普通源码提交。

## 判定与反例

安装成功、传输成功、执行步骤通过和质量达标是不同结果。保持覆盖率 80%、CRAP 30、API 零破坏性变化/无客户端漂移/兼容性要求；Angular CRAP 为 unsupported，零分母不转换成有利数值。

验收保留真实数值失败，并检查以下输入正确阻断：错误测量摘要、缺失原始文件、修改测试输入、过期签名请求、原始产物篡改、旧上下文、缺少必需能力，以及无效 Git 基线。相同 run 的原始认证响应可以显式保留复用，不能在缺少证据时自动重新采集。

成本记录同时保留执行、主机准备、基线解析和质量采集/评估时间。比较原多 job 拓扑时记录 checkout、依赖安装、缓存、服务和作业排队；单次暖缓存样本不构成普遍提速承诺。

Core 的暂存区 partial profile 修复见 Harness-Gate PR #251。partial hook 不采集完整覆盖率，完整质量必须显示 not_collected；不能用它批准 full 质量。
