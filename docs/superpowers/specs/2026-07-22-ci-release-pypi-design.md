# CI、Release 与 PyPI 发布设计

## 目标

为 `polars-bt` 建立清晰、可复现、低重复构建的自动化流程：日常 CI 只负责质量检查、构建一次 wheel，并在 CPython 3.10、3.11、3.12 上复用该 wheel 测试；GitHub Release 独立触发 PyPI Trusted Publishing。首个新流程发布版本为 `0.2.0`。

## 范围

- 支持 CPython 3.10、3.11、3.12，其他 Python 版本不进入 CI 或兼容性承诺。
- 首期只构建 Linux x86_64、manylinux2014、CPython abi3-py310 wheel 和源码包。
- `Cargo.toml` 是唯一版本源，`pyproject.toml` 继续使用动态版本。
- `alpha-lab` 后续从 PyPI 使用 `polars-bt>=0.2.0,<0.3.0`，`uv.lock` 锁定具体版本；不使用 Git 或本地路径依赖。
- 不在常规 CI 中运行 1250 万行性能基准；性能基准保留为发布前的人工或本地验证，避免把共享 runner 波动误判为回归。

## 日常 CI

`.github/workflows/ci.yml` 在推送到 `main`、面向 `main` 的 pull request 和人工触发时运行，并为同一分支启用并发取消。

工作流分为三个职责明确的 job：

1. `quality` 在一个 runner 上执行 Rust 格式检查、Clippy、Rust 测试和 Ruff。Rust 使用稳定工具链；依赖缓存以已提交的 `Cargo.lock` 为基础。
2. `build-wheel` 只构建一次 manylinux2014 x86_64 abi3 wheel，并上传为 GitHub Actions artifact。
3. `python-test` 使用 3.10、3.11、3.12 矩阵。每个矩阵 job 下载同一个 wheel，安装后运行完整 Python 测试。矩阵不重新编译 Rust。

工作流默认只授予 `contents: read`。第三方 action 固定到不可变 commit SHA，并在行尾注明对应版本，减少供应链漂移。

## Release 与 PyPI

`.github/workflows/release.yml` 仅响应 GitHub Release 的 `published` 事件，不响应普通 push 或 tag push。发布顺序如下：

1. 从 Release tag 检出代码，要求 tag 格式为 `vX.Y.Z`，并验证其版本与 `Cargo.toml` 完全一致。
2. 重新执行 Rust 和 Python 静态检查及 Rust 测试，防止从未经验证的提交发布。
3. 构建一个 manylinux2014 x86_64 abi3-py310 wheel 和一个 sdist，并作为单一 artifact 上传。
4. 下载 wheel，在 CPython 3.10、3.11、3.12 上分别安装并运行 Python 测试。
5. 只有所有检查通过后，`publish` job 才进入受保护的 `pypi` environment；该 job 单独获得 `id-token: write`，通过 PyPI Trusted Publishing 上传之前测试过的同一组文件。

工作流以 Trusted Publishing 上传成功作为终点，不立即查询或安装公共 PyPI。PyPI API、索引和 resolver 的传播并非原子操作，紧随上传的公共安装检查会产生假失败。

PyPI Trusted Publisher 的一次性配置为：GitHub owner `huangbogeng`、repository `polars_bt_extension`、workflow `release.yml`、environment `pypi`。流程不保存长期 PyPI token。

## 版本与变更记录

- 提交 `Cargo.lock`，并从 `.gitignore` 中移除对应规则。
- 将 `CHANGELOG.md` 当前 `Unreleased` 内容落为 `0.2.0`，日期为 `2026-07-22`，修正仓库 Release 链接。
- PyPI 版本不可覆盖。如果 `0.2.0` 已存在，发布明确失败；修复后使用新的补丁版本，不跳过已有文件。
- 先推送 `main` 并确认 CI 通过，再人工创建 `v0.2.0` GitHub Release。Release 发布是唯一的生产发布授权点。

## 失败边界

- tag 与包版本不一致时，在构建和上传前失败。
- 任一质量检查、构建或 Python 版本测试失败时，不进入 `publish`。
- PyPI 上传失败时保留 GitHub Actions 日志和构建 artifact，不修改或重写 GitHub tag。
- 上传后的索引传播不属于 Release 工作流失败边界，消费方按自身依赖升级节奏解析新版本。

## 验收标准

- 日常 CI 中 Rust wheel 只构建一次，且同一 wheel 在 Python 3.10、3.11、3.12 上通过测试。
- Release 工作流只由 GitHub Release `published` 事件触发，并在发布前验证 tag、质量和三版本兼容性。
- 发布 job 只使用 OIDC Trusted Publishing，不引用 PyPI API token secret。
- wheel 标签体现 abi3-py310 和 manylinux2014 x86_64；同时产出 sdist。
- `Cargo.lock` 被版本控制，Changelog 和版本链接准确。
- Release 工作流在 Trusted Publishing 上传成功后结束，不受 PyPI 索引传播延迟影响。
