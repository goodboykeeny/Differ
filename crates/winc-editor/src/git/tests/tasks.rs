//! 任务的测试：在真实的仓库和工作树里按分支归并任务、列出两种范围的改动。

use super::*;

fn git_at(dir: &Path, args: &[&str]) {
    let done = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(done.status.success(), "git {args:?}");
}

/// 在 `root` 下建一个仓库：`master` 上有一个提交，`.worktrees/` 下另有签出 `branch` 的工作树，
/// 其中提交了一处修改。返回该工作树的目录。
fn project(root: &Path, name: &str, branch: &str) -> PathBuf {
    let main = root.join(name);
    std::fs::create_dir_all(&main).unwrap();
    git_at(&main, &["init", "-q", "-b", "master"]);
    std::fs::write(main.join("a.txt"), "one\n").unwrap();
    git_at(&main, &["add", "."]);
    git_at(&main, &["commit", "-q", "-m", "base"]);
    let tree = root.join(".worktrees").join(format!("{branch}-{name}"));
    let path = tree.to_string_lossy().into_owned();
    git_at(&main, &["worktree", "add", "-q", "-b", branch, &path]);
    std::fs::write(tree.join("a.txt"), "one\ntwo\n").unwrap();
    git_at(&tree, &["commit", "-q", "-am", "change"]);
    tree
}

#[test]
fn baseline_patterns_match_whole_branch_names() {
    let rules = TaskRules::default();
    for name in ["master", "develop", "baseline-4.2.0", "baseline-4.2.1-core"] {
        assert!(rules.is_baseline(name), "{name}");
    }
    for name in ["task-14514", "bugfix/master-crash", "baseline", "mastery"] {
        assert!(!rules.is_baseline(name), "{name}");
    }
}

#[test]
fn worktrees_on_the_same_branch_form_one_task_with_both_ranges() {
    let shell = Shell::with_vars(std::env::vars_os().collect());
    let Ok(git) = Git::find(&shell) else {
        return;
    };
    let root = crate::test_util::scratch("tasks");
    let core = project(&root, "core", "task-1");
    let ui = project(&root, "ui", "task-1");
    project(&root, "scripts", "bugfix-2");
    let (rules, cancel) = (TaskRules::default(), Cancel::default());

    let tasks = git.scan_tasks(std::slice::from_ref(&root), &rules, &cancel);
    let names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
    // 主工作树都在 `master` 上，不算任务。
    assert_eq!(names, ["bugfix-2", "task-1"]);
    let projects: Vec<&str> = tasks[1].projects.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(projects, ["core", "ui"]);

    // 未提交的修改和未跟踪的文件。
    std::fs::write(core.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    std::fs::write(core.join("new.txt"), "new\n").unwrap();
    let all = git
        .task_changes(&core, TaskRange::Branch, &rules, &cancel)
        .unwrap();
    assert_eq!(all.base_name, "master");
    let seen: Vec<_> = all.files.iter().map(|f| (&f.path, f.change)).collect();
    assert_eq!(
        seen,
        [
            (&core.join("a.txt"), Change::Modified),
            (&core.join("new.txt"), Change::Untracked)
        ]
    );
    assert_eq!(all.files[0].lines, Some((2, 0)));
    let dirty = git
        .task_changes(&core, TaskRange::Uncommitted, &rules, &cancel)
        .unwrap();
    assert_eq!(dirty.files.len(), 2);
    assert_eq!(dirty.files[0].lines, Some((1, 0)));

    // 没有未提交的改动：相对基线仍有已提交的那处修改。
    let all = git
        .task_changes(&ui, TaskRange::Branch, &rules, &cancel)
        .unwrap();
    assert_eq!(all.files.len(), 1);
    let dirty = git
        .task_changes(&ui, TaskRange::Uncommitted, &rules, &cancel)
        .unwrap();
    assert!(dirty.files.is_empty());
}

/// 在 `root` 下建一个仓库：`master` 上一个提交，从它分出的 `4.2.0` 上再一个提交，然后切回
/// `master`。返回仓库的目录。
fn release_repo(root: &Path, name: &str) -> PathBuf {
    let main = root.join(name);
    std::fs::create_dir_all(&main).unwrap();
    git_at(&main, &["init", "-q", "-b", "master"]);
    std::fs::write(main.join("a.txt"), "one\n").unwrap();
    git_at(&main, &["add", "."]);
    git_at(&main, &["commit", "-q", "-m", "base"]);
    git_at(&main, &["checkout", "-q", "-b", "4.2.0"]);
    std::fs::write(main.join("release.txt"), "4.2.0\n").unwrap();
    git_at(&main, &["add", "."]);
    git_at(&main, &["commit", "-q", "-m", "release"]);
    git_at(&main, &["checkout", "-q", "master"]);
    main
}

/// 在工作树 `tree` 里提交一处修改，返回「全部改动」的基线名字和有改动的文件。
fn fixed(git: &Git, tree: &Path) -> (String, Vec<String>) {
    std::fs::write(tree.join("a.txt"), "one\nfix\n").unwrap();
    git_at(tree, &["commit", "-q", "-am", "fix"]);
    let (rules, cancel) = (TaskRules::default(), Cancel::default());
    let changes = git
        .task_changes(tree, TaskRange::Branch, &rules, &cancel)
        .unwrap();
    let paths = changes
        .files
        .iter()
        .filter_map(|f| Some(f.path.file_name()?.to_string_lossy().into_owned()))
        .collect();
    (changes.base_name, paths)
}

#[test]
fn branch_changes_compare_with_the_branch_a_task_was_created_from() {
    let shell = Shell::with_vars(std::env::vars_os().collect());
    let Ok(git) = Git::find(&shell) else {
        return;
    };
    let root = crate::test_util::scratch("task-source");
    let on_release = ("4.2.0".to_owned(), vec!["a.txt".to_owned()]);
    let tree = |name: &str| root.join(name).to_string_lossy().into_owned();

    // 建工作树时写明起点。`4.2.0` 不在基线名单里，按名单会跟 `master` 比，多出 `release.txt`。
    let main = release_repo(&root, "explicit");
    let path = tree("explicit-fix");
    git_at(
        &main,
        &["worktree", "add", "-q", "-b", "bugfix/a", &path, "4.2.0"],
    );
    assert_eq!(fixed(&git, Path::new(&path)), on_release);

    // 在 `4.2.0` 上 `git checkout -b`：分支的日志里只记了 `HEAD`。
    let main = release_repo(&root, "switched");
    git_at(&main, &["checkout", "-q", "4.2.0"]);
    git_at(&main, &["checkout", "-q", "-b", "bugfix/b"]);
    assert_eq!(fixed(&git, &main), on_release);

    // 主工作树签出 `4.2.0` 时建工作树，没写起点。
    let main = release_repo(&root, "implicit");
    git_at(&main, &["checkout", "-q", "4.2.0"]);
    let path = tree("implicit-fix");
    git_at(&main, &["worktree", "add", "-q", "-b", "bugfix/c", &path]);
    git_at(&main, &["checkout", "-q", "master"]);
    assert_eq!(fixed(&git, Path::new(&path)), on_release);

    // 在主工作树里 `git checkout -b`，后来才把分支签出到另一个工作树。
    let main = release_repo(&root, "moved");
    git_at(&main, &["checkout", "-q", "4.2.0"]);
    git_at(&main, &["checkout", "-q", "-b", "bugfix/e"]);
    git_at(&main, &["checkout", "-q", "master"]);
    let path = tree("moved-fix");
    git_at(&main, &["worktree", "add", "-q", &path, "bugfix/e"]);
    assert_eq!(fixed(&git, Path::new(&path)), on_release);

    // 克隆时直接签出 `4.2.0`，主工作树从没切换过分支；建工作树时没写起点。
    let origin = release_repo(&root, "cloned-origin");
    let clone = PathBuf::from(tree("cloned"));
    let from = origin.to_string_lossy().into_owned();
    let cloned = [
        "clone",
        "-q",
        "-b",
        "4.2.0",
        &from,
        &clone.to_string_lossy(),
    ];
    git_at(&root, &cloned);
    let path = tree("cloned-fix");
    git_at(&clone, &["worktree", "add", "-q", "-b", "bugfix/f", &path]);
    assert_eq!(fixed(&git, Path::new(&path)), on_release);

    // 主工作树一直签出 `4.2.0`，仓库里还有同名的标签。
    let main = release_repo(&root, "tagged");
    git_at(&main, &["checkout", "-q", "4.2.0"]);
    git_at(&main, &["tag", "4.2.0"]);
    git_at(&main, &["reflog", "expire", "--expire=all", "HEAD"]);
    let path = tree("tagged-fix");
    git_at(&main, &["worktree", "add", "-q", "-b", "bugfix/g", &path]);
    assert_eq!(fixed(&git, Path::new(&path)), on_release);

    // 从远程分支建的分支：起点记成 `origin/4.2.0`，本地没有 `4.2.0`。
    let origin = release_repo(&root, "origin");
    let clone = tree("clone");
    git_at(&root, &["clone", "-q", &origin.to_string_lossy(), &clone]);
    let clone = PathBuf::from(clone);
    git_at(
        &clone,
        &["checkout", "-q", "-b", "bugfix/d", "origin/4.2.0"],
    );
    let remote = ("origin/4.2.0".to_owned(), vec!["a.txt".to_owned()]);
    assert_eq!(fixed(&git, &clone), remote);
}

#[test]
fn branch_changes_fall_back_when_the_source_branch_is_unknown() {
    let shell = Shell::with_vars(std::env::vars_os().collect());
    let Ok(git) = Git::find(&shell) else {
        return;
    };
    let root = crate::test_util::scratch("task-source-unknown");
    let tree = |name: &str| root.join(name).to_string_lossy().into_owned();
    let on = |name: &str, paths: &[&str]| {
        let paths = paths.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>();
        (name.to_owned(), paths)
    };

    // 分支的日志没了：跟最近的基线分支比。
    let main = release_repo(&root, "expired");
    let path = tree("expired-fix");
    git_at(
        &main,
        &["worktree", "add", "-q", "-b", "bugfix/a", &path, "4.2.0"],
    );
    let expire = ["reflog", "expire", "--expire=all", "refs/heads/bugfix/a"];
    git_at(&main, &expire);
    let found = fixed(&git, Path::new(&path));
    assert_eq!(found, on("master", &["a.txt", "release.txt"]));

    // 来源分支已经删掉：同样跟最近的基线分支比。
    let main = release_repo(&root, "deleted");
    let path = tree("deleted-fix");
    git_at(
        &main,
        &["worktree", "add", "-q", "-b", "bugfix/b", &path, "4.2.0"],
    );
    git_at(&main, &["branch", "-q", "-D", "4.2.0"]);
    let found = fixed(&git, Path::new(&path));
    assert_eq!(found, on("master", &["a.txt", "release.txt"]));

    // 克隆下来的任务分支：起点是它自己的远程分支，不算来源；上游与它同名，也不算。
    let origin = release_repo(&root, "origin");
    git_at(&origin, &["branch", "-q", "bugfix/c", "4.2.0"]);
    let clone = PathBuf::from(tree("clone"));
    let from = origin.to_string_lossy().into_owned();
    git_at(&root, &["clone", "-q", &from, &clone.to_string_lossy()]);
    git_at(&clone, &["checkout", "-q", "bugfix/c"]);
    let found = fixed(&git, &clone);
    assert_eq!(found, on("master", &["a.txt", "release.txt"]));

    // 远程名里带 `/`：任务分支自己的远程分支同样不算来源。
    let clone = PathBuf::from(tree("slash"));
    let slash = [
        "clone",
        "-q",
        "-o",
        "forks/origin",
        &from,
        &clone.to_string_lossy(),
    ];
    git_at(&root, &slash);
    git_at(&clone, &["checkout", "-q", "bugfix/c"]);
    let found = fixed(&git, &clone);
    assert_eq!(found, on("master", &["a.txt", "release.txt"]));

    // 来源是另一个名字空间下的同名远程分支：它不是任务分支自己的远程分支，照常算来源。
    git_at(&origin, &["branch", "-q", "team/bugfix/h", "4.2.0"]);
    let clone = PathBuf::from(tree("namespaced"));
    git_at(&root, &["clone", "-q", &from, &clone.to_string_lossy()]);
    let start = ["checkout", "-q", "-b", "bugfix/h", "origin/team/bugfix/h"];
    git_at(&clone, &start);
    let found = fixed(&git, &clone);
    assert_eq!(found, on("origin/team/bugfix/h", &["a.txt"]));

    // 在另一个工作树里建的分支，没写起点：主工作树没签出过那个分支，猜不出来源，按基线名单比。
    let main = release_repo(&root, "elsewhere");
    let other = PathBuf::from(tree("elsewhere-next"));
    let next = [
        "worktree",
        "add",
        "-q",
        "-b",
        "5.0.0",
        &other.to_string_lossy(),
    ];
    git_at(&main, &next);
    std::fs::write(other.join("next.txt"), "5.0.0\n").unwrap();
    git_at(&other, &["add", "."]);
    git_at(&other, &["commit", "-q", "-m", "next"]);
    let path = tree("elsewhere-fix");
    git_at(&other, &["worktree", "add", "-q", "-b", "bugfix/d", &path]);
    let found = fixed(&git, Path::new(&path));
    assert_eq!(found, on("master", &["a.txt", "next.txt"]));
}
