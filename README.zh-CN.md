# Differ

[English](README.md) · 简体中文

**一个任务，集中查看多个 Git 仓库的改动。**

一个需求可能同时改到后端、前端和部署配置。Differ 是一款桌面应用，按当前检出的分支名将本地 Git
工作树归到同一个任务，让你不用来回切换仓库，就能审阅各个项目的改动。

## 审阅任务

1. 打开存放各项目的目录，在侧栏选择「任务」。Differ 会扫描该目录、它的直属子目录及其中的
   `.worktrees/` 目录，也会查找已发现仓库中登记的其他工作树。
2. 选择任务分支，例如 `task-1042`。多个项目检出同名分支时，改动文件会集中列在同一任务下，按项目分组。
   可用关键词筛选任务。
3. 点击文件查看左右并排的差异。在同一个差异标签页里切换上一个或下一个文件，即可跨项目审阅。

| 范围 | 查看内容 |
| --- | --- |
| **全部改动** | 比较工作区与最近基线分支的合并基点，包含已提交和未提交的改动。没有可用的基线时，先尝试上游分支，再回退到 `HEAD`。 |
| **未提交** | 比较工作区与 `HEAD`，包含已暂存、未暂存和未跟踪的文件。 |

## 在 macOS 上从源码运行

安装 Git、Xcode 命令行工具，并通过 [rustup](https://rustup.rs/) 安装 Rust。Rust 工具链版本由
[`rust-toolchain.toml`](rust-toolchain.toml) 指定。

```sh
git clone --branch dev https://github.com/goodboykeeny/Differ.git
cd Differ
cargo run --locked --release -- "$HOME/Projects"
```

将 `"$HOME/Projects"` 换成存放你各个仓库的目录。编译后的可执行文件为 `target/release/differ`。

## 配置任务扫描

默认扫描当前打开的目录。如果希望每次都扫描固定目录，可在 macOS 的
`~/Library/Application Support/differ/config.json` 中加入 `tasks` 配置：

```json
{
  "tasks": {
    "roots": ["~/Projects"],
    "keywords": ["bugfix", "task"],
    "baselines": ["master", "main", "develop", "dev", "baseline-*"]
  }
}
```

- `roots`：指定扫描目录。不设置时，扫描当前打开的目录。
- `keywords`：设置任务筛选按钮。默认为 `bugfix` 和 `task`。
- `baselines`：设置用于比较的基线分支。这些分支不出现在任务列表中。示例为默认配置，`*` 可匹配任意字符序列。

## 致谢与许可证

Differ 基于 [Winc](https://github.com/DEEIX-AI/Winc) 编辑器开发，采用 [Apache License 2.0](LICENSE)。
版权和第三方声明见 [NOTICE](NOTICE)。
