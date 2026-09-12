# Polars 内核升级与项目解析

本项目已将支持范围从 Python Polars 1.43.x 升级到 `>=1.44.2,<1.45.0`，
并同步升级 Rust Polars、pyo3-polars 和 PyO3。三个公开函数仍然是
`pulse`、`mosaic`、`tempo`，交易状态机和结果字段保持原样。
升级内容包括依赖锁定、原生扩展重建、安装与 CI 配置、边界测试及架构文档。

核对截至 2026-09-12 的官方发行记录，最新 Python 稳定版为 1.44.2；
2.0.0rc1 属于预发布版本，不在此次支持范围内。1.44.0 已被撤回，
因此此次直接以 1.44.2 为最低支持版本。[^1]

## 版本组合与选择依据

| 层次 | 升级前 | 升级后 |
| --- | --- | --- |
| Python Polars 依赖范围 | `>=1.43.0,<1.44.0` | `>=1.44.2,<1.45.0` |
| 本地实际验证的 Python Polars | 1.43.0 | 1.44.2，标准 rt32 运行时 |
| Rust Polars / polars-config | 0.54.4 | 0.55.2 |
| pyo3-polars | 0.27.0 | 0.28.0 |
| pyo3-polars-derive | 0.21.0 | 0.22.0 |
| PyO3 锁定版本 | 0.28.3 | 0.29.2 |
| Rust 工具链 | 1.95 | 1.95 |
| 支持并测试的 CPython | 3.10–3.12 | 3.10–3.12 |

Python Polars 和 Rust Polars 使用不同版本序列，不能按数字直接对应。
Python `py-1.44.2` 标签的 workspace 声明 Rust Polars 0.55.1、PyO3 0.29
和 pyo3-polars 0.28.0；发布到 crates.io 的 pyo3-polars 0.28.0 接受
Polars `^0.55.1`。因此本项目选择 Rust 补丁版 0.55.2，并通过实际 wheel
运行验证该组合。0.55.2 的官方说明包含 rayon soundness 等修复。[^2][^3][^4]

此次没有发现需要本项目处理的 FFI 结构破坏。对比官方 `py-1.44.2`、
`rs-0.55.1` 和 `rs-0.55.2` 标签，插件协议均为 0.1，`SeriesExport`、
`CallerContext` 的定义一致。表达式 derive 和分配器相关接口也保持一致；
与旧 `rs-0.54.4` 对比，这些边界结构同样没有变化。这支持同步依赖并重建
扩展的方案，但单独比较协议编号或结构不能替代运行测试。[^5][^6]

CPython 的 `abi3-py310` 只描述 Python 扩展 ABI，不能据此承诺任意
Polars 版本兼容。Python 依赖保留下一 minor 的上界；未来增加新 minor
支持时，应重新核对 Rust 依赖和运行结果，而不能只放宽 Python 元数据。

## 项目职责与调用路径

项目提供的是嵌入 Polars 表达式执行过程的有状态回测计算内核。
它不负责加载行情、构建完整资产面板、排序、策略研究编排或实盘下单。
Python wrapper 将列名或表达式转换成适合 Rust 的输入，Polars 再调用原生插件。

```mermaid
flowchart LR
    A[调用者准备并排序完整面板] --> B[Python wrapper: Expr 与类型转换]
    B --> C[Polars 插件注册与 schema 推断]
    C --> D[FFI Series 输入]
    D --> E[Pulse / Mosaic / Tempo 状态机]
    E --> F[Struct Series 输出]
    F --> G[Polars select / unnest]
```

相关入口是 [`polars_bt/`](../polars_bt/)、[`src/lib.rs`](../src/lib.rs) 和
各引擎的 `mod.rs`。插件通过 `register_plugin_function` 注册，通过
`#[polars_expr]` 生成原生入口，使用 `PolarsAllocator` 分配内存。
执行发生在同一进程内，不经过 DataFrame→Arrow IPC 字节流→Rust 的序列化往返。
这不等于整条路径零拷贝：类型转换、null 填充、rechunk、Boolean 解码和输出构造
仍可能分配内存。

三个引擎分开维护交易状态。Mosaic 与 Tempo 共用诊断格式和 Polars verbose
开关，但不共用持仓状态机。更完整的模块关系见
[`ARCHITECTURE.md`](../ARCHITECTURE.md)。

| 引擎 | 状态沿什么维度推进 | 输入责任 | 结果 |
| --- | --- | --- | --- |
| Pulse | 报价时间序列 | 调用者提供时间有序的报价、信号和有效时间/价格限制 | 一行成交及持仓汇总 Struct |
| Mosaic | 固定宽度的每日资产截面 | 调用者保证每日同样的资产数量与资产顺序 | 每个已完成日期一行净值、现金、换手和持仓数 |
| Tempo | 固定资产集合的时间截面 | 调用者准备完整面板；Rust 校验时间块及资产顺序 | 每个时间截面一行组合快照 |

## 三套引擎的关键语义

### Pulse

[`src/pulse/`](../src/pulse/) 保留原有报价撮合逻辑，包含交易所、matcher、
订单及持仓对象。每个报价先进入交易所处理，再由信号决定下单。
输出是整个输入的单行汇总，包含多空持仓、胜利计数、信号数、浮动盈亏和已实现盈亏。
Python 注册时使用 `returns_scalar=True`、`is_elementwise=False`。

Pulse 当前假设输入非空，报价、时间以及首行价格限制有效；部分入口仍使用
`unwrap()`。信号 null 会作为未触发处理，但其他 null、空表和异常形状并未
得到同等程度的输入校验。这是已有边界，不是本次升级引入的行为。

### Mosaic

[`src/mosaic/mod.rs`](../src/mosaic/mod.rs) 按 `asset_num` 将输入划成每日截面，
[`engine.rs`](../src/mosaic/engine.rs) 执行隔夜收益、换手重置、卖出、买入、
日内收益、快照及负现金停止判断。买入按稳定的资产行顺序推进，使用捕获的现金
计算累计支出，保持严格 `<` 的资金门槛。因此改变行序可能改变最终买入集合。

Mosaic 不接收资产标识，无法检查跨日资产顺序是否改变。
Python wrapper 也不会补齐资产、做 join 或排序。
完整的 `(date, asset)` 面板必须由调用者准备；Rust 主要检查长度、
`asset_num`、费用和输入类型。verbose 可以报告日期块或调仓标志异常，但不会修复输入。

输出日期保留输入 dtype。每个已完成日期输出一行；如果出现负现金停止，
保留停止当天的快照并截断后续日期。输出长度与资产行数不同，注册时使用
`changes_length=True`、`is_elementwise=False`。

### Tempo

[`src/tempo/validation.rs`](../src/tempo/validation.rs) 从第一个时间截面建立
资产顺序，检查后续截面的完整性、重复资产、资产顺序、时间递增和截面内一致的调仓标志。
Python 将时间转换为微秒 Datetime，并额外传入派生日期用于 T+1；资产转换为 String。

每个时间截面先处理跨日可卖持仓释放及 `period_ret`，再按需调仓。
不调仓的时间点仍然更新估值并输出快照。卖出先于买入，买入同样依赖资产行顺序和
严格现金门槛。`t1=True` 时当日买入不能当日卖出，进入后续日期后释放；
`t1=False` 时允许日内卖出。收益必须表示从上一时间点到当前时间点的收益。

Tempo 为 long-only，拒绝负权重和无穷权重/收益。
输出每个时间截面一行，换手是当前时间点的换手；注册 flags 与 Mosaic 一致。

## Polars 边界与升级风险

数值输入处理是此次必须保留的路径。Mosaic、Tempo wrapper 对数值做
`cast(Float64).fill_null(NaN)`，Rust 随后 rechunk 并读取连续切片。
rechunk 只能合并块，不能消除 null validity bitmap；删掉 null→NaN 处理会重新
暴露连续切片读取错误。现有语义是 NaN 权重作为零目标、NaN 收益作为零收益；
Mosaic 的 NaN 或非正价格阻止相应交易，诊断仍保留异常记录。

Polars 1.44 的 `Expr.rechunk()` 弃用与 Rust `Series::rechunk()` 并非同一 API。
本项目所需的 Rust 连续化操作不应因该弃用被删除。PyO3 0.29 的迁移项也没有
要求改写本项目简单的 `#[pymodule]` 初始化；新版编译已经验证这一点。[^7][^8]

另一个边界是 schema 推断。输出类型函数必须和实际 Struct 的字段、日期类型一致。
Pulse 的标量标记与 Mosaic/Tempo 的变长标记会影响 Polars 执行规划，不能改成
elementwise 来规避适配问题。新增测试只对 lazy plan 做 `collect_schema()`，
实际回测继续采用 eager 整表执行，不据此扩展 streaming 或任意分组执行的支持声明。

[`src/diagnostics.rs`](../src/diagnostics.rs) 每次执行重新读取 `POLARS_VERBOSE`。
因此导入之后调用 `pl.Config.set_verbose()` 或使用上下文管理器仍然有效。
原有测试验证了开关切换、恢复、静默和异常记录；此次继续使用同样的语义。

## 实际修改

- `Cargo.toml` 与 `Cargo.lock`：统一到 Rust Polars 0.55.2、pyo3-polars 0.28.0、
  PyO3 0.29.2，相关 Polars 运行时 crates 使用同一版本。
- `pyproject.toml`：运行时范围改为 `>=1.44.2,<1.45.0`；构建环境只保留 maturin，
  删除并不参与本项目编译且过宽的 Python Polars 构建依赖。
- `requirements.txt`、`Makefile` 与安装文档：统一开发依赖，开发编译使用 `--locked`。
- CI 和 release workflow：CPython 3.10、3.11、3.12 分别测试最低支持的 1.44.2
  与最新允许的 `1.44.*`，从源码目录之外运行已安装 wheel 的测试。
- `tests/test_import.py` 与新增的
  [`tests/test_polars_compatibility.py`](../tests/test_polars_compatibility.py)：
  更新版本检查，并增加 15 个输入布局及类型边界场景。
- README、CONTRIBUTING、ARCHITECTURE 和 CHANGELOG：更新支持范围和三个引擎说明。

Rust 引擎源文件和 Python 公开 wrapper 无需修改。依赖适配最初在开发版本
0.2.1 上核验；正式版本定为 0.2.2，同时包含自上次发布后新增的 Tempo 引擎。
发布说明和版本范围见 CHANGELOG 与 README。

## 验证结果

升级前在 CPython 3.12.12 / Polars 1.43.0 上执行原有 Python 测试，49 项通过。
升级后使用 `maturin build --release --locked` 构建一个 abi3 wheel，分别安装到
三个独立环境，将测试复制到独立目录执行，并检查包和原生模块均来自各环境的
`site-packages`。这样排除了源码目录中旧开发扩展被误用的情况。

| 检查 | 环境或范围 | 结果 |
| --- | --- | --- |
| 升级前 Python 回归 | CPython 3.12.12 / Polars 1.43.0 | 49 passed |
| 升级后 wheel 回归 | CPython 3.10.12 / Polars 1.44.2 | 64 passed |
| 升级后 wheel 回归 | CPython 3.11.14 / Polars 1.44.2 | 64 passed |
| 升级后 wheel 回归 | CPython 3.12.12 / Polars 1.44.2 | 64 passed |
| 重新安装后的开发环境回归 | 项目 `.venv`，CPython 3.12.12 / Polars 1.44.2 | 64 passed |
| Rust 单元测试 | `cargo test --offline --locked --all-targets` | 31 passed |
| Rust 静态检查 | Clippy，all-targets / all-features，warnings as errors | 通过 |
| 格式与 Python 静态检查 | cargo fmt / Ruff / git diff --check | 通过 |
| Mosaic 性能验收 | 2,500 日 × 5,000 资产 | 1.738754 秒；2,500 输出行 |
| Tempo 性能验收 | 500 日 × 5 时间点 × 5,000 资产 | 1.949562 秒；2,500 输出行 |

两套 benchmark 都处理 1,250 万行，均通过仓库的 5 秒上限。
计时采用原有脚本的定义：构造输入并预热后，只测表达式调用边界。
这些结果来自本机 Linux x86_64、Polars rt32、release wheel，属于当前环境的验收数据；
没有进行受控的升级前后性能对照，因此不能据此声称性能提升或精确量化性能变化。

新增 15 项覆盖：三个引擎各两种布局（多 chunk、非零偏移切片）共六项；
Mosaic 的 String、Date、Datetime ms/us/ns 共五项；Tempo 的 ms/ns 输入和
String/Categorical 资产组合共四项。原有测试继续覆盖 Mosaic/Tempo 的独立 Python
参考实现对照、费用、T+1、null/NaN、异常输入和诊断日志。Pulse 的参考对照覆盖较弱，
本次新增布局测试验证相同有效输入的结果一致性，不等于完整撮合模型正确性证明。

独立 Rust 测试进程未初始化 Python 时，pyo3-polars 的 debug allocator 会输出
`failed to get allocator capsule` 并按上游实现回退到系统分配器，测试仍全部通过。
Python 插件执行边界由另外三套已安装 wheel 的集成测试覆盖。[^6]

本地 wheel 标签为 `cp310-abi3-manylinux_2_34_x86_64`。
本节记录发版前的本地验证；manylinux2014 打包和远程测试结果以对应提交的
GitHub Actions CI 与 Release 记录为准。Windows、macOS、rt64、未来 Polars
版本及下游完整研究仓库未在此次本地验证范围内。
本地日志和 wheel 位于 `target/polars-upgrade-validation/` 与
`target/polars-upgrade-dist/`，属于不纳入 Git 的构建产物。
`target/polars-upgrade-validation/manifest.json` 保存精确依赖版本、wheel 和
Cargo.lock 的 SHA256、各环境测试结果及 benchmark 输出。

## 后续升级方法

先分别核对 Python 的稳定发行标签、对应 workspace Cargo 版本和 pyo3-polars
的发布依赖，再检查插件入口、Series/Arrow FFI、allocator 与本项目使用的
Polars API。不要把 Python minor 与 Rust crate minor 当成同一个版本。

修改依赖和锁文件后重新构建 wheel；运行 Rust 单元测试、三个引擎的完整 Python
测试以及变长结果、null、chunk、时间类型、诊断等边界测试。最后在隔离环境验证
wheel 安装，并同步更新 Python 版本约束、开发说明和 CI 的最低/最新 patch 矩阵。
这一过程同时检验打包、插件执行和既有交易语义。

## 来源

[^1]: Polars maintainers, [Polars 1.44.2 / PyPI 发行记录](https://pypi.org/project/polars/1.44.2/)，2026-09-09；[官方发行页](https://github.com/pola-rs/polars/releases/tag/py-1.44.2)。
[^2]: Polars maintainers, [`py-1.44.2` workspace Cargo.toml](https://github.com/pola-rs/polars/blob/py-1.44.2/Cargo.toml)，用于核对 Rust 与 Python 发布标签的对应关系。
[^3]: Polars maintainers, [pyo3-polars 0.28.0 crate 元数据](https://docs.rs/crate/pyo3-polars/0.28.0)；[标签内的 pyo3-polars manifest](https://github.com/pola-rs/polars/blob/py-1.44.2/pyo3-polars/pyo3-polars/Cargo.toml)。
[^4]: Polars maintainers, [Rust Polars 0.55.2 发行说明](https://github.com/pola-rs/polars/releases/tag/rs-0.55.2)。
[^5]: Polars maintainers, [Python 发布标签的插件 FFI 定义](https://github.com/pola-rs/polars/blob/py-1.44.2/crates/polars-ffi/src/version_0.rs)；[Rust 0.55.2 对应定义](https://github.com/pola-rs/polars/blob/rs-0.55.2/crates/polars-ffi/src/version_0.rs)；[旧 Rust 0.54.4 定义](https://github.com/pola-rs/polars/blob/rs-0.54.4/crates/polars-ffi/src/version_0.rs)。
[^6]: Polars maintainers, [`py-1.44.2` 插件调用实现](https://github.com/pola-rs/polars/blob/py-1.44.2/crates/polars-plan/src/plans/aexpr/function_expr/plugin.rs)；[表达式 derive](https://github.com/pola-rs/polars/blob/py-1.44.2/pyo3-polars/pyo3-polars-derive/src/lib.rs)；[PolarsAllocator](https://github.com/pola-rs/polars/blob/py-1.44.2/pyo3-polars/pyo3-polars/src/alloc.rs)。
[^7]: Polars maintainers, [Python Polars 1.44.0 发行说明](https://github.com/pola-rs/polars/releases/tag/py-1.44.0)，Expr.rechunk 等弃用说明。
[^8]: PyO3 maintainers, [PyO3 0.28→0.29 迁移说明](https://pyo3.rs/v0.29.0/migration.html#from-028-to-029)。
