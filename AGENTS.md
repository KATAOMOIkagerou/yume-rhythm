# AGENTS.md

给在本仓库工作的 AI 编程助手和人类开发者。**动手前先读 `docs/架构.md`。**

## 项目

桌宠节奏师（yume-rhythm）：Windows 透明桌面宠物。角色会跟随系统音乐的节拍点头，另有学习计时、表情和待办功能。Rust 编写，控制台界面用 Slint。

- 功能范围：`docs/功能清单.md`
- 架构与分层：`docs/架构.md`
- 角色包格式：`docs/角色包格式.md`

## 硬性规则

1. **提交前必须通过 `cargo xtask check`。** 不通过不要提交，也不要为了通过而删测试、加 `#[allow]` 或放宽检查。
2. **业务规则只写在 `crates/core`。** 包括计时、表情、随机事件、待办、设置的取值范围和默认值。界面（`crates/console`）和组装层（`crates/app`）里不写 `if` 业务判断。
3. **状态只能通过 `Command` 修改。** 要加新操作，就在 `yume-core` 里加 `Command` 变体，在 `update` 里处理，并写测试。
4. **依赖方向只能朝下**（见 `docs/架构.md` 第 2 节）。`yume-win`、`yume-audio` 不能依赖任何 `yume-*` crate。
5. **L0 crate（core、beat、character）里**不调用 `Instant::now`、`SystemTime::now`、全局随机数，不开线程，不写 `unsafe`。时间和随机数从参数传入。
6. **`unsafe` 只允许出现在 `crates/win`**，每处都要写 `// SAFETY:` 注释。
7. 单个 `.rs` 文件不超过 500 行，`.slint` 文件不超过 400 行。快超了就按职责拆分模块，不要硬塞。
8. 角色的位置、动作幅度、特效参数写进角色包（`characters/<id>/character.json`），不要写死在代码里。
9. 节拍算法（`crates/beat`）改动后，必须跑通与 JS 基准引擎的回归测试（见 `tools/beat-baseline/README.md`）。

## 改什么去哪里

见 `docs/架构.md` 第 9 节。表里找不到对应行的改动，先更新那张表，再写代码。

## 常用命令

```
cargo xtask check              # 格式、lint、测试、依赖方向、文件长度，提交前必跑
cargo test -p yume-core        # 只测业务逻辑
UPDATE_SNAPSHOTS=1 cargo test -p yume-character   # 有意修改画面后更新基准图
cargo run -p yume-app --release                   # 运行（仅 Windows）
```

## CI 与发布

- 推送到 `main` 和每个 PR 都会在 Linux 和 Windows 上跑 `cargo xtask check`（`.github/workflows/ci.yml`）。
- 发布新版本：
  1. 改根目录 `Cargo.toml` 的 `workspace.package.version`；
  2. 在 `CHANGELOG.md` 顶部加 `## X.Y.Z - 日期` 一节；
  3. 提交并推送，CI 通过后打标签并推送：`git tag vX.Y.Z && git push origin vX.Y.Z`。
- 标签会触发 `.github/workflows/release.yml`：核对标签与版本号一致，跑检查，编译，
  打包成 `yume-rhythm-X.Y.Z-windows-x64.zip`（程序、`characters/`、`LICENSE`），
  以 CHANGELOG 对应小节为说明发布到 GitHub Releases。
