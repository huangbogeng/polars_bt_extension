# Tempo 日内/跨日截面回测设计

## 目标

新增独立的 `tempo` Polars 表达式插件，用于普通多头 Alpha 策略在任意
`datetime` 上推进持仓，并在一天内执行零次、一次或多次截面换仓。

现有 `pulse` 和 `mosaic` 的公开接口、结果与热路径保持不变。`tempo` 不通过
IPC，不做 join，也不在插件内部排序。

## 公开接口

```python
tempo(
    datetime,
    asset,
    weight,
    period_ret,
    buyable,
    sellable,
    is_rebalance,
    *,
    t1=True,
    st_fee=6e-4,
    lg_fee=1e-4,
) -> pl.Expr
```

输入必须预先按 `(datetime, asset)` 排列。相同 `datetime` 构成一个完整资产
截面。第一组截面确定标准资产数量和资产顺序，后续截面必须完全一致，因此
公开接口不再要求 `asset_num`。

`period_ret` 表示上一个 `datetime` 到当前 `datetime` 的资产收益。引擎先把
收益应用到旧持仓，再根据当前截面的 `is_rebalance` 决定是否换仓。为了计入
最后一次换仓到收盘的收益，调用方应增加一组收盘数据并设置
`is_rebalance=False`。

Python 包装层统一转换输入：

- `datetime` 转换为 Polars `Datetime`；同时从该表达式派生内部使用的交易日。
- `asset` 转换为 `String`。
- `weight` 和 `period_ret` 转换为 `Float64`，null 转换为 NaN。
- 三个布尔输入转换为 `Boolean`，不接受 null。

## 前置校验

Rust 插件在创建组合状态、执行任何交易前完成一次线性扫描：

- 所有输入列长度一致，关键列没有残余 null。
- `datetime` 单调递增，相同时点的行连续存放。
- 第一组截面没有重复资产。
- 后续每组截面的资产数量、资产内容和资产顺序与第一组逐项一致。
- 同一时点所有行的 `is_rebalance` 值完全一致。
- 有限权重不能为负；第一版只支持多头组合。
- 交易费必须有限且非负。
- `period_ret` 或 `weight` 的正负无穷属于错误。

校验失败时返回包含列名、行号、`datetime` 和 `asset` 的错误。插件不自动
排序、补齐或修复面板。上层框架可以额外使用 `group_by().agg()` 提前统计
截面数量、资产唯一数和换仓标志唯一数，但 Rust 校验仍是不能绕过的防线。

## 缺失值与诊断

沿用 `mosaic` 的数值缺失契约：

- `weight` NaN 按目标权重 0 处理。
- `period_ret` NaN 按收益 0 处理。
- NaN 不终止回测；开启 Polars verbose 日志时分别记录 `NAN_WEIGHT` 和
  `NAN_PERIOD_RET`，并带上时点、资产和行号。
- 正负无穷不作缺失值处理，直接返回准确错误。

权重和小于 1 时剩余资金保留为现金。权重和大于 1 时记录诊断，实际买入仍
受可用现金和费用约束。

## 状态推进与交易

每个 `datetime` 按以下顺序处理：

1. 如果日期发生变化，释放上一交易日冻结的买入持仓。
2. 将 `period_ret` 应用到总持仓和可卖持仓。
3. 如果 `is_rebalance=False`，不交易。
4. 如果需要换仓，以当前净值和目标权重计算目标金额。
5. 先卖后买；卖出受 `sellable` 和可卖持仓数量约束。
6. 买入受 `buyable`、现金和费用约束，并保持稳定资产行顺序。
7. `t1=True` 时新买持仓当天冻结；`t1=False` 时立即成为可卖持仓。
8. 生成当前时点快照。

组合状态至少包含总持仓、可卖持仓、现金和当前交易日。布尔 `sellable`
只表示停牌、涨跌停等当前市场约束，T+1 的部分可卖数量由 Rust 状态独立
维护。

## 输出

每个输入 `datetime` 输出一行 Struct：

```text
datetime | cash | nav | turnover | holding_count
```

`turnover` 是当前时点实际成交金额绝对值除以当前净值。日频结果由调用方按
交易日取最后一条快照，并汇总当日各时点 turnover；插件不丢弃日内路径。

## 代码边界

- 新增 `polars_bt/tempo.py`，负责公开表达式包装和类型标准化。
- 新增 `src/tempo/`，独立容纳输入校验、状态机、诊断和输出构造。
- 只复用已经稳定且语义一致的底层小组件；不向现有 `mosaic` 热循环增加
  日内分支。
- 从 `polars_bt.__init__` 导出 `tempo`，并在 README 增加最小示例。

## 验证

Rust 单元测试覆盖收益先于换仓、同日多次换仓、跨日状态、T+1 部分可卖、
T+0 配置、先卖后买、现金约束和逐时点换手。

Python 集成测试覆盖表达式注册、输出 schema、null 到 NaN 的等价语义、
完整截面校验、资产错序、重复资产、混合换仓标志、负权重和 verbose 诊断。
另外增加一个多日多时点的纯 Python 参考实现做结果一致性测试，并保留大
截面基准以避免引入 IPC、join 或意外的超线性扫描。
