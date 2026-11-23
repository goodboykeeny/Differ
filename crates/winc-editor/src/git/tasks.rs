//! 任务：多个仓库、工作树里签出的同名分支合在一起看。
//!
//! 一个任务（例如 `task-14514`）往往同时改动几个仓库，每个仓库各有一个签出该分支的工作树。
//! 这里在给定的目录下找出各个工作树，按分支名归并成任务，并列出每个工作树相对基线的改动。

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use super::*;

/// 最多拿多少个基线分支与任务分支求合并基（个）：每个候选要运行两次 `git`。
const BASE_CANDIDATES: usize = 24;

/// `git checkout -b` 在分支的日志和 `HEAD` 的日志里各记一条，两条的时刻最多相差多少（秒）。
const CREATED_WITHIN: u64 = 2;

/// 归并任务时用到的规则。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRules {
    /// 基线分支的写法：`*`（或 `.*`）匹配任意一段文字，其余按字面比较。
    ///
    /// 签出基线分支的工作树不算任务；查不到任务分支是从哪个分支建的时，任务的改动相对最近的
    /// 基线分支计算。
    pub baselines: Vec<String>,
}

impl Default for TaskRules {
    fn default() -> TaskRules {
        let names = ["master", "main", "develop", "dev", "baseline-*"];
        TaskRules {
            baselines: names.iter().map(|s| (*s).to_owned()).collect(),
        }
    }
}

impl TaskRules {
    /// `branch` 是否是基线分支；远程分支先去掉远程名再比（`origin/master` 按 `master` 比）。
    pub fn is_baseline(&self, branch: &str) -> bool {
        self.baselines.iter().any(|p| wildcard(p, branch))
    }
}

/// `pattern` 是否匹配整个 `text`；`*` 和 `.*` 匹配任意一段文字。
fn wildcard(pattern: &str, text: &str) -> bool {
    let pattern = pattern.replace(".*", "*");
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}

/// 任务里的一个工作树。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskProject {
    /// 项目名：所属仓库主工作树的目录名。
    pub name: String,
    /// 工作树的目录。
    pub root: PathBuf,
}

/// 一个任务：各仓库里签出同名分支的工作树。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// 任务名，即分支名。
    pub name: String,
    /// 签出该分支的工作树，按项目名排序。
    pub projects: Vec<TaskProject>,
}

/// 查看任务改动的范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum TaskRange {
    /// 任务分支相对基线的全部改动：与基线分支的合并基到工作区，含已提交和未提交的改动。
    #[default]
    Branch,
    /// 只看未提交的改动：`HEAD` 到工作区。
    Uncommitted,
}

/// 一个工作树在某个范围内的改动。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskChanges {
    /// 左侧的版本（提交号）；仓库还没有提交时为 `None`，左侧为空。
    pub base: Option<String>,
    /// 左侧版本的来历：基线分支的名字，只看未提交的改动时为 `HEAD`。
    pub base_name: String,
    /// 有改动的文件，按路径排序；右侧是工作区里的文件。
    pub files: Vec<CommitFile>,
}

impl Git {
    /// 在 `roots` 下找出各个工作树，按分支名归并成任务，按任务名排序。
    ///
    /// 每个目录本身、它的直接子目录、它和子目录里 `.worktrees/` 下的各目录都会查看；找到的
    /// 仓库用 `git worktree list` 登记的其他工作树也算在内。签出基线分支、游离 `HEAD` 的
    /// 工作树不算任务。
    pub fn scan_tasks(&self, roots: &[PathBuf], rules: &TaskRules, cancel: &Cancel) -> Vec<Task> {
        let mut seen = HashSet::new();
        let mut trees = Vec::new();
        for root in roots {
            for dir in candidates(root) {
                if git_dir(&dir).is_some() && seen.insert(dir.clone()) {
                    trees.push(dir);
                }
            }
        }
        let mut listed = HashSet::new();
        for i in 0..trees.len() {
            let Some(common) = git_common_dir(&trees[i]) else {
                continue;
            };
            if !listed.insert(clean(&common)) {
                continue;
            }
            for tree in self.worktrees(&trees[i], cancel).unwrap_or_default() {
                if !tree.prunable && tree.head.is_some() && seen.insert(tree.path.clone()) {
                    trees.push(tree.path);
                }
            }
        }
        let mut tasks: BTreeMap<String, Vec<TaskProject>> = BTreeMap::new();
        let mut placed = HashSet::new();
        for root in trees {
            let Some(branch) = head_branch(&root) else {
                continue;
            };
            // `.worktrees/baseline-…` 这样的目录是基线的工作树；仓库自己的目录名不算数。
            let parked =
                (root.parent().and_then(Path::file_name)).is_some_and(|n| n == ".worktrees");
            let folder = root.file_name().map(|n| n.to_string_lossy().into_owned());
            if rules.is_baseline(&branch)
                || (parked && folder.is_some_and(|f| rules.is_baseline(&f)))
            {
                continue;
            }
            // 同一个工作树经符号链接、不同写法的路径可能出现两次。
            let real = winc_config::files::canonicalize(&root).unwrap_or_else(|_| root.clone());
            if !placed.insert(real) {
                continue;
            }
            let name = project_name(&root);
            tasks
                .entry(branch)
                .or_default()
                .push(TaskProject { name, root });
        }
        tasks
            .into_iter()
            .map(|(name, mut projects)| {
                projects.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.root.cmp(&b.root)));
                Task { name, projects }
            })
            .collect()
    }

    /// 列出工作树 `root` 在 `range` 内有改动的文件。
    ///
    /// 相对基线时左侧是 `HEAD` 与任务分支的来源的合并基：来源是建分支时的起点分支，查不到时是
    /// 最近的基线分支，再退回上游分支、`HEAD`。未跟踪的文件算新增。
    ///
    /// # Errors
    ///
    /// `git` 运行失败时返回原因。
    pub fn task_changes(
        &self,
        root: &Path,
        range: TaskRange,
        rules: &TaskRules,
        cancel: &Cancel,
    ) -> Result<TaskChanges, String> {
        let head = self.rev(root, "HEAD", cancel);
        let (base, base_name) = match (range, head) {
            (_, None) => (None, String::new()),
            (TaskRange::Uncommitted, Some(head)) => (Some(head), "HEAD".to_owned()),
            (TaskRange::Branch, Some(head)) => match self.task_base(root, rules, cancel) {
                Some((name, oid)) => (Some(oid), name),
                None => (Some(head), "HEAD".to_owned()),
            },
        };
        let mut files = Vec::new();
        if let Some(base) = &base {
            let mut command = self.command(root);
            command.args(["diff", "-M", "-z", "--raw", "--numstat", "--end-of-options"]);
            command.arg(base);
            let stdout = self.run(&mut command, STATUS_TIMEOUT, cancel)?;
            files = parse_diff_tree(&stdout, root);
        }
        let mut command = self.command(root);
        command.args(["ls-files", "-z", "--others", "--exclude-standard"]);
        // 还没有提交过时没有可比的提交：暂存了的文件也都是新增的。
        if base.is_none() {
            command.arg("--cached");
        }
        let stdout = self.run(&mut command, STATUS_TIMEOUT, cancel)?;
        let known: HashSet<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
        for name in stdout.split(|&b| b == 0).filter(|e| !e.is_empty()) {
            // 以 `/` 结尾的是嵌在里面的另一个仓库（例如 `.worktrees/` 下的工作树），不是文件。
            if name.ends_with(b"/") {
                continue;
            }
            let path = root.join(String::from_utf8_lossy(name).as_ref());
            if !known.contains(&path) {
                files.push(CommitFile {
                    path,
                    from: None,
                    change: Change::Untracked,
                    lines: None,
                });
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(TaskChanges {
            base,
            base_name,
            files,
        })
    }

    /// 返回 `HEAD` 与任务分支的来源的合并基：来源分支的名字和合并基的提交号。
    ///
    /// 来源依次是：建任务分支时的起点分支、与任务分支不同名的上游分支、最近的基线分支。
    /// 「最近」指合并基之后 `HEAD` 上的提交最少。都没有时用上游分支。
    fn task_base(
        &self,
        root: &Path,
        rules: &TaskRules,
        cancel: &Cancel,
    ) -> Option<(String, String)> {
        let mut command = self.command(root);
        command.args([
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads",
            "refs/remotes",
        ]);
        let stdout = self.run(&mut command, STATUS_TIMEOUT, cancel).ok()?;
        let text = String::from_utf8_lossy(&stdout);
        let refs: Vec<&str> = text.lines().collect();
        if let Some(branch) = head_branch(root) {
            let source = (self.created_from(root, &branch, cancel))
                .and_then(|start| branch_ref(&start, &branch, &refs))
                .or_else(|| branch_ref(&self.upstream(root, cancel)?, &branch, &refs));
            if let Some((name, full)) = source
                && let Some(oid) = self.merge_base(root, &full, cancel)
            {
                return Some((name, oid));
            }
        }
        let mut candidates: Vec<(String, String)> = Vec::new();
        for full in refs {
            if let Some((short, name)) = ref_names(full)
                && short != "HEAD"
                && rules.is_baseline(short)
            {
                candidates.push((name.to_owned(), full.to_owned()));
            }
        }
        candidates.truncate(BASE_CANDIDATES);
        if candidates.is_empty() {
            candidates.push(("@{upstream}".to_owned(), "@{upstream}".to_owned()));
        }
        let mut best: Option<(usize, String, String)> = None;
        let mut counted: BTreeMap<String, usize> = BTreeMap::new();
        for (name, full) in candidates {
            let Some(oid) = self.merge_base(root, &full, cancel) else {
                continue;
            };
            let ahead = match counted.get(&oid) {
                Some(ahead) => *ahead,
                None => {
                    let mut command = self.command(root);
                    command.args(["rev-list", "--count"]);
                    command.arg(format!("{oid}..HEAD"));
                    let stdout = self.run(&mut command, STATUS_TIMEOUT, cancel).ok()?;
                    let ahead = String::from_utf8_lossy(&stdout).trim().parse().ok()?;
                    counted.insert(oid.clone(), ahead);
                    ahead
                }
            };
            if best.as_ref().is_none_or(|(least, ..)| ahead < *least) {
                best = Some((ahead, name, oid));
            }
        }
        best.map(|(_, name, oid)| (name, oid))
    }

    /// `HEAD` 与 `rev` 的合并基；没有共同的提交时为 `None`。
    fn merge_base(&self, root: &Path, rev: &str, cancel: &Cancel) -> Option<String> {
        let mut command = self.command(root);
        command.args(["merge-base", "HEAD"]).arg(rev);
        let stdout = self.run(&mut command, STATUS_TIMEOUT, cancel).ok()?;
        let oid = String::from_utf8_lossy(&stdout).trim().to_owned();
        (!oid.is_empty()).then_some(oid)
    }

    /// 建分支 `branch` 时的起点，按引用日志里记的写法（`4.2.0`、`origin/4.2.0`）。
    ///
    /// `git branch b 4.2.0`、`git worktree add -b b … 4.2.0` 把起点记在分支的日志里。没写起点
    /// 时记的是 `HEAD`：`git checkout -b b` 在 `HEAD` 的日志里同时记下从哪个分支切过来；
    /// `git worktree add -b b …` 则看主工作树当时签出的分支。日志过期被清掉、分支是在别处建的
    /// 时为 `None`。
    fn created_from(&self, root: &Path, branch: &str, cancel: &Cancel) -> Option<String> {
        let created = self.reflog(root, &format!("refs/heads/{branch}"), cancel);
        let (at, oid, subject) = created.last()?;
        let start = subject.strip_prefix("branch: Created from ")?;
        if start != "HEAD" {
            return Some(start.to_owned());
        }
        let arrived = format!(" to {branch}");
        let switched = (self.reflog(root, "HEAD", cancel).iter())
            .filter(|(moved, ..)| moved.abs_diff(*at) <= CREATED_WITHIN)
            .find_map(|(.., subject)| {
                Some(moved_from(subject)?.strip_suffix(&arrived)?.to_owned())
            });
        if switched.is_some() {
            return switched;
        }
        // 主工作树在那一刻之前最后一次切换分支，切到的就是当时签出的分支。命令也可能是在别的
        // 工作树里运行的，所以只在分支建在它的提交上时才算数。日志的时刻只精确到秒，同一秒里
        // 的几次切换分不出先后，逐个试。
        let log = self.reflog(root, "main-worktree/HEAD", cancel);
        let moves = log.iter().filter_map(|(moved, _, subject)| {
            let to = moved_from(subject)?.rsplit_once(" to ")?.1;
            (moved <= at).then_some((moved, to))
        });
        for (moved, main) in moves {
            let mut command = self.command(root);
            command.args(["merge-base", "--is-ancestor", oid]);
            command.arg(format!("refs/heads/{main}"));
            if self.run(&mut command, STATUS_TIMEOUT, cancel).is_ok() {
                return Some(main.to_owned());
            }
            if moved < at {
                break;
            }
        }
        None
    }

    /// `rev` 的引用日志，新的在前：每条的时刻（Unix 秒）、写入的提交号和说明。读不了时为空。
    fn reflog(&self, root: &Path, rev: &str, cancel: &Cancel) -> Vec<(u64, String, String)> {
        let mut command = self.command(root);
        command.args(["reflog", "show", "--date=unix"]);
        command.args(["--format=%gd%x09%H%x09%gs", rev]);
        let Ok(stdout) = self.run(&mut command, STATUS_TIMEOUT, cancel) else {
            return Vec::new();
        };
        String::from_utf8_lossy(&stdout)
            .lines()
            .filter_map(|line| {
                // 每行形如 `b@{1700000000}<TAB>提交号<TAB>branch: Created from 4.2.0`。
                let mut fields = line.splitn(3, '\t');
                let at = fields.next()?.rsplit_once("@{")?.1.strip_suffix('}')?;
                let oid = fields.next()?.to_owned();
                Some((at.parse().ok()?, oid, fields.next()?.to_owned()))
            })
            .collect()
    }

    /// 当前分支的上游分支，写成 `origin/4.2.0`；没有设置时为 `None`。
    fn upstream(&self, root: &Path, cancel: &Cancel) -> Option<String> {
        let mut command = self.command(root);
        command.args(["rev-parse", "--abbrev-ref", "--symbolic-full-name"]);
        command.arg("@{upstream}");
        let stdout = self.run(&mut command, STATUS_TIMEOUT, cancel).ok()?;
        let name = String::from_utf8_lossy(&stdout).trim().to_owned();
        (!name.is_empty()).then_some(name)
    }

    /// 把 `rev` 解析成提交号；解析不了（还没有提交）时返回 `None`。
    fn rev(&self, root: &Path, rev: &str, cancel: &Cancel) -> Option<String> {
        let mut command = self.command(root);
        command.args(["rev-parse", "--verify", "-q", "--end-of-options"]);
        command.arg(format!("{rev}^{{commit}}"));
        let stdout = self.run(&mut command, STATUS_TIMEOUT, cancel).ok()?;
        let oid = String::from_utf8_lossy(&stdout).trim().to_owned();
        (!oid.is_empty()).then_some(oid)
    }
}

/// 引用 `full` 的两个名字：去掉远程名的分支名（`4.2.0`）和显示用的名字（`origin/4.2.0`）。
/// 不是分支时为 `None`。
fn ref_names(full: &str) -> Option<(&str, &str)> {
    if let Some(name) = full.strip_prefix("refs/heads/") {
        Some((name, name))
    } else {
        let name = full.strip_prefix("refs/remotes/")?;
        Some((name.split_once('/').map_or(name, |(_, n)| n), name))
    }
}

/// 引用日志里一次切换分支的说明（`checkout: moving from 4.2.0 to b`）去掉前缀后的部分。
fn moved_from(subject: &str) -> Option<&str> {
    subject.strip_prefix("checkout: moving from ")
}

/// 在 `refs` 里找 `start` 指的分支，返回显示用的名字和完整的引用名。
///
/// `start` 可以写成 `4.2.0`、`origin/4.2.0` 或完整的引用名；本地分支已经删掉时用远程上的同名
/// 分支。`branch` 自己和它在远程上的同名分支不算来源，找到的是它们时为 `None`。
fn branch_ref(start: &str, branch: &str, refs: &[&str]) -> Option<(String, String)> {
    let find = |local: bool, by_short: bool| {
        refs.iter().copied().find(|full| {
            (!local || full.starts_with("refs/heads/"))
                && ref_names(full).is_some_and(|(short, name)| {
                    *full == start || name == start || (by_short && short == start)
                })
        })
    };
    let full = find(true, false)
        .or_else(|| find(false, false))
        .or_else(|| find(false, true))?;
    let (short, name) = ref_names(full)?;
    (short != branch && short != "HEAD").then(|| (name.to_owned(), full.to_owned()))
}

/// `root` 下可能是工作树的目录：它本身、直接子目录，以及它和子目录里 `.worktrees/` 下的各目录。
fn candidates(root: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![root.to_path_buf()];
    let children = subdirs(root);
    dirs.extend(subdirs(&root.join(".worktrees")));
    for child in children {
        if child.file_name().is_some_and(|n| n == ".worktrees") {
            continue;
        }
        dirs.extend(subdirs(&child.join(".worktrees")));
        dirs.push(child);
    }
    dirs.sort();
    dirs
}

/// `dir` 的直接子目录（跟随符号链接）；读不了时为空。
fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = winc_backend::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| winc_backend::fs::is_dir(p))
        .collect()
}

/// 工作树 `root` 签出的分支名；游离的 `HEAD`、读不出时返回 `None`。
fn head_branch(root: &Path) -> Option<String> {
    let head = winc_backend::fs::read_to_string(git_dir(root)?.join("HEAD")).ok()?;
    let branch = head.trim().strip_prefix("ref:")?.trim();
    Some(branch.strip_prefix("refs/heads/")?.to_owned())
}

/// 工作树 `root` 所属项目的名字：仓库主工作树的目录名，取不到时用工作树自己的目录名。
fn project_name(root: &Path) -> String {
    let own = || winc_config::files::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let main = git_common_dir(root)
        .map(|d| clean(&d))
        .filter(|d| d.file_name().is_some_and(|n| n == ".git"))
        .and_then(|d| d.parent().map(Path::to_path_buf))
        .unwrap_or_else(own);
    match main.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => main.display().to_string(),
    }
}

/// 解析路径里的 `..` 和符号链接；解析不了时原样返回。
fn clean(path: &Path) -> PathBuf {
    winc_config::files::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
