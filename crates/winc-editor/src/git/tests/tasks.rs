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
