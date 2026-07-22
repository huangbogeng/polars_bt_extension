# CI、Release 与 PyPI 发布 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `polars-bt` 整理成构建一次、多 Python 版本复用测试，并由 GitHub Release 通过 OIDC 自动发布 PyPI 的项目。

**Architecture:** 日常 `ci.yml` 分离质量检查、wheel 构建和 Python 兼容性测试；`release.yml` 独立校验版本、重建并测试发布制品，最后通过受保护的 `pypi` environment 发布。`Cargo.toml` 保持唯一版本源，提交 `Cargo.lock` 保证 Rust 依赖可复现。

**Tech Stack:** GitHub Actions、Rust stable、Maturin、PyO3 abi3-py310、uv、PyPI Trusted Publishing

---

### Task 1: 固化版本与可复现依赖

**Files:**
- Modify: `.gitignore`
- Add: `Cargo.lock`
- Modify: `CHANGELOG.md`
- Modify: `rust-toolchain.toml`
- Modify: `Makefile`

- [ ] **Step 1: 从 `.gitignore` 删除 `Cargo.lock` 规则**

保留 `/target/` 等 Rust 构建产物规则，但让现有 `Cargo.lock` 进入版本控制。

- [ ] **Step 2: 生成并检查锁文件**

Run: `cargo generate-lockfile --locked`

Expected: 退出码 0，且 `git status --short Cargo.lock` 显示新文件。

- [ ] **Step 3: 落定 0.2.0 Changelog**

把当前 Added、Changed、Fixed 内容移动到 `## [0.2.0] - 2026-07-22`；保留空的 `## [Unreleased]`，并将底部链接改为：

```markdown
[Unreleased]: https://github.com/huangbogeng/polars_bt_extension/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/huangbogeng/polars_bt_extension/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/huangbogeng/polars_bt_extension/releases/tag/v0.1.2
```

- [ ] **Step 4: 验证元数据**

Run: `cargo metadata --locked --no-deps --format-version 1`

Expected: JSON 中 `polars_bt` 版本为 `0.2.0`，命令退出码 0。

- [ ] **Step 5: 统一使用 Rust stable**

将 `rust-toolchain.toml` 的 `channel` 改为 `stable`，并把 `Makefile` 中三处 `cargo +nightly` 改为普通 `cargo`。本地命令与 CI 使用同一工具链，避免同一提交因工具链漂移产生不同结果。

### Task 2: 重写日常 CI

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: 建立最小权限与并发策略**

触发器仅包含 `main` push、面向 `main` 的 pull request 和 `workflow_dispatch`；顶层设置 `permissions: contents: read`，并以 workflow + ref 为 concurrency group，取消同分支旧运行。

- [ ] **Step 2: 实现 `quality` job**

在 Ubuntu runner 上检出代码，安装 stable Rust 的 rustfmt/clippy，启用 Rust cache，安装 uv，然后执行：

```bash
cargo fmt --all -- --check
cargo clippy --locked --package polars_bt --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
uvx ruff check polars_bt tests benchmarks
```

- [ ] **Step 3: 实现单次 wheel 构建**

`build-wheel` 使用固定 SHA 的 `PyO3/maturin-action`，参数为：

```yaml
with:
  target: x86_64
  manylinux: "2014"
  command: build
  args: --release --locked --out dist
```

随后上传 `dist/*.whl`，artifact 名称为 `linux-wheel`，找不到文件时失败。

- [ ] **Step 4: 复用 wheel 测试 Python 3.10–3.12**

`python-test` 依赖 `build-wheel`，矩阵只包含 `"3.10"`、`"3.11"`、`"3.12"`。每个 job 下载 `linux-wheel` 到 `dist`，执行：

```bash
uv pip install --system --force-reinstall dist/*.whl pytest
python -m pytest tests -q
```

所有 action 均固定到已解析的 commit SHA，并在注释中保留版本标签。

### Task 3: 新增 GitHub Release 发布工作流

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: 限定发布入口与权限**

工作流只监听：

```yaml
on:
  release:
    types: [published]
```

顶层权限为 `contents: read`；只有 `publish` job 增加 `id-token: write` 并声明 `environment: pypi`。

- [ ] **Step 2: 校验 Release tag 与 Cargo 版本**

`verify` job 检出 `${{ github.event.release.tag_name }}`，使用 Python 3.11 `tomllib` 读取 `Cargo.toml`。脚本要求 tag 匹配 `v[0-9]+.[0-9]+.[0-9]+`，并要求去掉 `v` 后等于 Cargo 版本；否则以非零状态退出。

- [ ] **Step 3: 在 verify job 执行质量门禁**

复用 Task 2 的 stable Rust、cache、uv 配置和四条质量命令，确保 Release 不依赖历史 CI 状态。

- [ ] **Step 4: 构建唯一发布 artifact**

`build` 依赖 `verify`。先用 Maturin action 构建 manylinux2014 x86_64 wheel：

```yaml
command: build
args: --release --locked --out dist
```

再用同一 action 在 runner 上构建源码包：

```yaml
command: sdist
args: --out dist
```

上传 `dist/*` 为 `release-distributions`。

- [ ] **Step 5: 在三个 Python 版本测试发布 wheel**

`python-test` 依赖 `build`，下载发布 artifact，按 Task 2 的矩阵安装 wheel 并运行 `python -m pytest tests -q`。

- [ ] **Step 6: 使用 OIDC 发布同一 artifact**

`publish` 依赖 `python-test`，下载 `release-distributions` 到 `dist`，使用固定 SHA 的 `pypa/gh-action-pypi-publish`：

```yaml
with:
  packages-dir: dist/
  skip-existing: false
  verbose: true
```

不设置用户名、密码或 token secret。

- [ ] **Step 7: 验证公共 PyPI**

`verify-pypi` 依赖 `publish`，从 tag 得到版本，最多重试 12 次查询 `https://pypi.org/pypi/polars-bt/<version>/json`；可见后使用 `uv pip install --system --no-cache polars-bt==<version>`，并断言 `polars_bt.__version__` 等于该版本。

### Task 4: 本地验证与提交

**Files:**
- Verify: `.github/workflows/ci.yml`
- Verify: `.github/workflows/release.yml`
- Verify: `.gitignore`
- Verify: `Cargo.lock`
- Verify: `CHANGELOG.md`

- [ ] **Step 1: 验证 YAML 和 workflow 静态结构**

Run: `uv run --with pyyaml python -c 'import pathlib,yaml; [yaml.safe_load(p.read_text()) for p in pathlib.Path(".github/workflows").glob("*.yml")]'`

Expected: 退出码 0。

Run: `uvx zizmor .github/workflows/ci.yml .github/workflows/release.yml`

Expected: 无高风险或可利用问题；对 action 固定 SHA 的检查通过。

- [ ] **Step 2: 执行本地质量门禁**

Run: `cargo fmt --all -- --check`

Run: `cargo clippy --locked --package polars_bt --all-targets --all-features -- -D warnings`

Run: `cargo test --locked --all-targets`

Run: `.venv/bin/python -m ruff check polars_bt tests benchmarks`

Expected: 所有命令退出码 0。

- [ ] **Step 3: 构建并测试 wheel 与 sdist**

Run: `.venv/bin/maturin build --release --locked --out dist/verify`

Run: `.venv/bin/maturin sdist --out dist/verify`

Run: `uv venv --python 3.10 .venv-release-test`

Run: `uv pip install --python .venv-release-test/bin/python --force-reinstall dist/verify/*.whl pytest`

Run: `.venv-release-test/bin/python -m pytest tests -q`

Expected: 构建产生一个 abi3-py310 wheel 和一个 sdist，Python 测试全部通过。

- [ ] **Step 4: 检查差异并提交**

Run: `git diff --check`

Run: `git status --short`

Expected: 仅包含本计划文件、两份 workflow、`.gitignore`、`Cargo.lock`、`CHANGELOG.md`、`rust-toolchain.toml` 和 `Makefile`。

Commit: `ci: automate tested PyPI releases`

### Task 5: 启用远端发布链路

**Files:**
- Remote GitHub environment: `pypi`
- PyPI project setting: `polars-bt` Trusted Publisher

- [ ] **Step 1: 推送 main 并确认 CI**

Run: `git push origin main`

Run: `gh run watch --exit-status`

Expected: 新 CI 的 quality、build-wheel 和三个 python-test job 全部成功。

- [ ] **Step 2: 创建 GitHub environment**

使用 GitHub API 创建 `pypi` environment；该环境不保存 PyPI secret。若仓库权限支持，可配置 required reviewers，否则保留为发布 job 的 OIDC environment 绑定。

- [ ] **Step 3: 确认 PyPI Trusted Publisher**

在 `polars-bt` 项目中配置 owner `huangbogeng`、repository `polars_bt_extension`、workflow `release.yml`、environment `pypi`。这是发布前唯一需要账户所有者完成的一次性外部配置。

- [ ] **Step 4: 创建 GitHub Release 并观察发布**

Run: `gh release create v0.2.0 --target main --generate-notes --title "polars-bt 0.2.0"`

Run: `gh run watch --exit-status`

Expected: Release 工作流完成验证、构建、三版本测试、OIDC 上传和公共 PyPI 冒烟测试。

- [ ] **Step 5: 更新 alpha-lab 的 PyPI 依赖**

仅在公共验证通过后，在 `/home/huangbogeng/github.com/Aequiludium/alpha-lab/pyproject.toml` 添加 `polars-bt>=0.2.0,<0.3.0`，运行 `uv lock`，执行 `LofiEx` 相关测试，并仅提交该依赖及锁文件的精确 hunks，保留 alpha-lab 其他未提交修改。
