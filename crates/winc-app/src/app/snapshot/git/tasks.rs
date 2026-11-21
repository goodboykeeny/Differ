//! 任务视图的测试：按分支归并任务、关键字筛选、跨项目切换文件、两种改动范围。

use super::*;
use crate::app::git::tasks::{TaskRow, TaskTarget};
use winc_editor::git::TaskRange;

/// 在 `root` 下建两个仓库 `core`、`ui`，各有签出 `task-7` 的工作树，`core` 另有 `bugfix-9`。
fn task_folders(root: &Path) {
    let identity = ["-c", "user.name=t", "-c", "user.email=t@t"];
    for name in ["core", "ui"] {
        let repo = root.join(name);
        std::fs::create_dir_all(&repo).unwrap();
        git_in(&repo, &["init", "-q", "-b", "master"]);
        std::fs::write(repo.join("a.txt"), "one\n").unwrap();
        git_in(&repo, &["add", "-A"]);
        git_in(&repo, &[&identity[..], &["commit", "-qm", "init"]].concat());
        let tree = root.join(".worktrees").join(format!("{name}-task-7"));
        let tree = tree.to_string_lossy().into_owned();
        git_in(&repo, &["worktree", "add", "-q", "-b", "task-7", &tree]);
        let tree = Path::new(&tree);
        std::fs::write(tree.join("a.txt"), "two\n").unwrap();
        git_in(tree, &["add", "-A"]);
        git_in(
            tree,
            &[&identity[..], &["commit", "-qm", "change"]].concat(),
        );
        std::fs::write(tree.join("new.txt"), "new\n").unwrap();
    }
    let tree = root.join(".worktrees/core-bugfix-9");
    let tree = tree.to_string_lossy().into_owned();
    git_in(
        &root.join("core"),
        &["worktree", "add", "-q", "-b", "bugfix-9", &tree],
    );
}

#[test]
fn a_task_lists_every_project_and_steps_across_them() {
    let dir = search_folder("git-tasks");
    task_folders(&dir);
    let mut app = app_with(&dir, false);
    app.settings_toggle(Setting::GitModule);
    if git_unusable(&app) {
        return;
    }
    app.show_tasks();
    paint(&mut app);
    let names = |app: &App| -> Vec<String> {
        (app.shown_tasks().into_iter())
            .map(|i| app.tasks.tasks[i].name.clone())
            .collect()
    };
    assert_eq!(names(&app), ["bugfix-9", "task-7"]);

    // 关键字筛选：只留下名字里有「task」的。
    let keyword = app.task_keywords().iter().position(|k| k == "task");
    app.press_task(TaskTarget::Keyword(keyword));
    assert_eq!(names(&app), ["task-7"]);
    paint(&mut app);
    let task = (app.hits.tasks.iter())
        .find_map(|(_, t)| matches!(t, TaskTarget::Task(_)).then_some(*t))
        .unwrap();

    // 选中任务：两个项目各两个文件，第一个文件的差异已打开。
    app.press_task(task);
    paint(&mut app);
    let files = |app: &App| -> Vec<(String, Vec<String>)> {
        (app.tasks.changes.iter())
            .map(|p| {
                let files = p
                    .files
                    .iter()
                    .map(|f| winc_editor::display_name(&f.path).into_owned());
                (p.name.clone(), files.collect())
            })
            .collect()
    };
    let both = vec!["a.txt".to_owned(), "new.txt".to_owned()];
    assert_eq!(
        files(&app),
        [("core".to_owned(), both.clone()), ("ui".to_owned(), both)]
    );
    assert_eq!(app.task_rows().len(), 6);
    assert_eq!(app.tasks.current, Some((0, 0)));
    assert!(app.editor.active().unwrap().diff.is_some());
    assert!(app.diff_steps_files());

    // 逐行追溯两侧都能开：左侧是基线上的版本，右侧是工作区里的文件。
    app.toggle_diff_blame();
    let blamed = |app: &App| app.editor.active().unwrap().diff.as_deref().unwrap().blamed;
    assert_eq!(blamed(&app), [true, true]);
    paint(&mut app);

    // 下一个文件一直走到另一个项目；到头后不再前进。未跟踪的文件不能追溯，走过它之后追溯还开着。
    app.step_diff_file(true);
    assert_eq!(app.tasks.current, Some((0, 1)));
    assert_eq!(blamed(&app), [false, false]);
    paint(&mut app);
    app.step_diff_file(true);
    assert_eq!(app.tasks.current, Some((1, 0)));
    assert_eq!(blamed(&app), [true, true]);
    paint(&mut app);

    // 点追溯栏里的一行：项目的工作树不在打开的目录找到的仓库里，也切换过去并在日志里选中该提交。
    let tree = dir.join(".worktrees/ui-task-7");
    assert!(!app.git.repositories.contains(&tree));
    let preview = app.editor.active().unwrap().id;
    app.click_diff_blame(1, 0);
    assert_eq!(app.git.root.as_deref(), Some(tree.as_path()));
    let log = app.editor.active().unwrap().log.as_deref().unwrap();
    assert_eq!(log.root.as_deref(), Some(tree.as_path()));
    app.close_tab(app.editor.active.unwrap());
    let index = app.editor.documents.iter().position(|d| d.id == preview);
    app.activate_tab(index.unwrap());

    app.toggle_diff_blame();
    assert_eq!(blamed(&app), [false, false]);
    app.step_diff_file(true);
    app.step_diff_file(true);
    assert_eq!(app.tasks.current, Some((1, 1)));
    app.step_diff_file(false);
    assert_eq!(app.tasks.current, Some((1, 0)));
    assert_eq!(app.editor.documents.len(), 1);

    // 只看未提交的改动：每个项目只剩没有提交的新文件，仍在同一个标签里。
    app.press_task(TaskTarget::Range(TaskRange::Uncommitted));
    let new = vec!["new.txt".to_owned()];
    assert_eq!(
        files(&app),
        [("core".to_owned(), new.clone()), ("ui".to_owned(), new)]
    );
    assert_eq!(app.editor.documents.len(), 1);

    // 收起一个项目；返回任务列表。
    app.press_task(TaskTarget::Project(0));
    assert_eq!(
        app.task_rows(),
        [
            TaskRow::Project(0),
            TaskRow::Project(1),
            TaskRow::File(1, 0)
        ]
    );
    app.press_task(TaskTarget::Back);
    assert!(app.tasks.selected.is_none());
    paint(&mut app);

    let _ = std::fs::remove_dir_all(&dir);
}
