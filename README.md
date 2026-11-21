# Differ

English · [简体中文](README.zh-CN.md)

**Review one task across multiple Git repositories.**

A single task can touch an API, a frontend, and a deployment repo. Differ is a desktop app that
groups local Git worktrees by their checked-out branch, so you can review all three projects
without switching repositories.

## Review a task

1. Open the folder that contains your projects, then select **Tasks** in the sidebar. Differ scans
   the folder, its immediate subfolders, and their `.worktrees/` directories. It also finds the
   linked worktrees of any repository it discovers.
2. Select a task branch such as `task-1042`. If several projects have that branch checked out,
   Differ groups their changed files under the same task. Use the keyword filters to narrow the list.
3. Select a file to see its side-by-side diff. Next and previous file navigation stays in the same
   diff tab, even when you move to a different project.

| Range | What you see |
| --- | --- |
| **All changes** | Committed and uncommitted changes since the merge base with the nearest baseline branch. If no baseline can be used, Differ tries the upstream branch, then `HEAD`. |
| **Uncommitted** | Working-tree changes against `HEAD`, including staged, unstaged, and untracked files. |

## Run from source on macOS

Install Git, the Xcode Command Line Tools, and [Rust via rustup](https://rustup.rs/). The repository
pins the Rust toolchain in [`rust-toolchain.toml`](rust-toolchain.toml).

```sh
git clone --branch dev https://github.com/goodboykeeny/Differ.git
cd Differ
cargo run --locked --release -- "$HOME/Projects"
```

Replace `"$HOME/Projects"` with the folder containing your repositories. The built executable is
`target/release/differ`.

## Configure task discovery

By default, Differ scans the open folders. To reuse the same set of scan roots across sessions,
add a `tasks` section to `~/Library/Application Support/differ/config.json` on macOS:

```json
{
  "tasks": {
    "roots": ["~/Projects"],
    "keywords": ["bugfix", "task"],
    "baselines": ["master", "main", "develop", "dev", "baseline-*"]
  }
}
```

- `roots` sets the folders to scan. Omit it to scan the open folders instead.
- `keywords` sets the filter buttons. The defaults are `bugfix` and `task`.
- `baselines` sets the branches used for comparison and excluded from the task list. The example
  shows the defaults; use `*` to match any sequence of characters.

## Credits and license

Differ is built on the [Winc](https://github.com/DEEIX-AI/Winc) editor and uses the
[Apache License 2.0](LICENSE). See [NOTICE](NOTICE) for attribution.
