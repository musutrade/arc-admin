# 后端原生质量采集试运行（GH-215）

这条链路把真实 Rust 测试和 LLVM 输出交给 Harness-Gate 的签名适配器及
质量评估器。它是后端接入试运行，不是完整质量验收，不替代原有 full/hook
门禁，也不生成已接受的基线。

## 当前能力和明确限制

- 对固定 Git 提交的独立快照运行原有后端测试，使用一次性 PostgreSQL，
  不读取生产数据库环境变量。覆盖率使用 cargo-llvm-cov 0.9.0 和与项目
  Rust 1.97.1 匹配的 LLVM 工具。
- 清点全部 `backend/src/**/*.rs`，保留原始 LLVM 输出、测试日志、退出码、
  源码哈希和工具身份。没有 LLVM 行记录的文件明确报告测量错误。
- 当前投影的是 **LLVM 文件汇总值，包含文件内的测试代码**。它不是原来的
  已认证生产函数测量系列，不能用于生产覆盖率等价声明、历史比较或接管。
  新系列明确使用 `llvm-file-summary-unfiltered/1`，不复制参考系列哈希。
- 原有复杂度分析器不支持部分日志宏、`tokio::select!`、`task_local!`
  和宏生成代码。整个后端的函数/LLVM 映射尚未认证，因此所有 CRAP 能力
  都报告 `measurement_error`，没有 CRAP 数值，也不会将遗漏当作零风险。
- 影子配置仅在独立输出快照内生成，标记为 partial。后端规则文件逐字复制
  自固定 Harness-Gate 提交，仍要求 80% 行覆盖率、80% 区域覆盖率和 CRAP
  不超过 30；本试运行的签名传输成功不能放行这些失败。
- 保留 required Git baseline 配置，但本轮没有基线测量和认证。
  直接策略诊断不执行基线比较；即使诊断结果将来通过，也不代表完整验收。

## 本地运行

先在仓库外准备固定 Harness-Gate 提交
`44689dd839852608b438f010694c193105ce6b55`，构建其 `harness-gate`
与 `harness-gate-rust-measure`。不要使用可能仍是旧版本的 PATH 安装。
主机需要 Docker、项目 Rust 工具链、cargo-llvm-cov，以及匹配的 llvm-cov
和 llvm-profdata。签名主机还需要 Python 的 `cryptography==46.0.5`。

从 Arc-Admin 仓库根运行，以下变量均为操作者选定的绝对路径：

```bash
python3 scripts/quality/rust_native.py \
  --repository "$PWD" --output "$SAMPLE_DIR" \
  --analyzer "$RUST_MEASURE_BIN" --llvm-tools "$LLVM_TOOLS_DIR"

python3 scripts/quality/rust_host.py \
  --repository "$PWD" --sample "$SAMPLE_DIR" \
  --harness-source "$HARNESS_SOURCE" --harness-bin "$HARNESS_BIN" \
  --private-key "$HOST_PRIVATE_KEY" --output "$HOST_RESULT_DIR"

python3 scripts/quality/verify_native_receipt.py \
  --host-result "$HOST_RESULT_DIR" --harness-bin "$HARNESS_BIN" \
  --private-key "$HOST_PRIVATE_KEY" --output "$NEGATIVE_RESULT_DIR"
```

每个输出目录必须全新且位于仓库外，建议放在 SSD。后端或工具链存在未提交
修改时采样会拒绝执行，防止把工作区变化错误绑定到 Git 提交。

主机从自己的 Git checkout 独立核对提交、完整 Rust 文件清单和每个 blob，
再构造可信状态及签名请求。首次使用时，主机可以在仓库和证据目录之外生成
权限为 0600 的 Ed25519 私钥；公钥只作为本地影子信任锚。采集器不能获取该
私钥、生成自己的可信状态或接受自己的基线。

`rust_host.py` 调用真正的 `quality compile`、`quality collect` 和
`quality evaluate`。当前预期为采集退出 0、策略退出 1。读取
`host-receipt.json` 和 `evaluation-receipt.json` 区分传输成功与质量失败。

真实反例验证使用产物和源码的副本，保留原件。它分别验证篡改/缺失产物、
修改源码、旧运行上下文、无效签名和过期签名请求。必须观察到各自的完整性
错误，不能仅因原有 CRAP 失败导致非零退出，就声称反例通过。

## GitHub 托管运行

手动运行 `Backend native quality shadow` 工作流。它使用 GitHub 托管
Ubuntu、只读仓库权限和固定 Harness-Gate 提交，安装测量工具并执行上述链路。
每次 job 生成独立的临时影子私钥，私钥目录不在 artifact 上传清单中，不需要
配置生产 secrets。缺失能力和不通过的阈值保持失败，不使用 continue-on-error。
普通 CI 只新增快速 Python 契约测试，不为每个 PR 重跑原生覆盖率试验。

工作流保留原始测量、源代码归档、执行凭据和反例结果 30 天。本地应先保留
这些证据，再用 `cargo clean --target-dir "$SAMPLE_DIR/build/llvm-cov-target"` 清理本次构建。
不要清理共享 Cargo target、其他任务目录或仍在执行的测量。

## 首次真实试运行

测量源码为 Arc-Admin 提交 `e5a1ee5f7ec6dbae461355106d469384581d4607`。
原生测试通过，47 个源码文件全部进入证据清单，其中 42 个有真实 LLVM
覆盖率记录，5 个没有记录。`quality collect` 成功验证全部 47 个主体；
质量评估失败，未接受基线，也未授予接管权限。

具体退出码、来源哈希和真实反例结果见
[试运行凭据](backend-native-pilot/receipt.json)。原始 LLVM、源码归档和测试
日志保留在该凭据标明的主机目录中；它们没有被测试夹具替代。
首次凭据来自本地执行，不声称已经运行新的手动 GitHub 工作流。

下一阶段需要认证生产函数清单及宏/异步代码的函数映射，再接入真实 CRAP、
独立基线、前端与 API 证据。GH-215 的完整验收及 GH-208 的接管结论仍待完成。
