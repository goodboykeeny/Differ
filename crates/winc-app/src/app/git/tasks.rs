//! 任务视图：把多个仓库、工作树里签出同名分支的改动合在一起看。
//!
//! 在配置的目录（未配置时为打开的目录）下找出各个工作树，按分支名归并成任务；选中一个任务后
//! 列出它在每个项目里的改动，逐个文件在同一个差异标签中打开，上一个、下一个文件跨项目切换。

use winc_editor::git::{CommitFile, Task, TaskRange, TaskRules};

use super::*;
use winc_render::Rect;

/// 任务视图里一行的高度（逻辑像素），与文件树相同。
pub(crate) const TASK_ROW: f32 = crate::theme::TREE_ROW;

/// 任务视图里可以点击的位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskTarget {
    /// 关键字筛选项；`None` 是「全部」。
    Keyword(Option<usize>),
    /// 改动范围的一段。
    Range(TaskRange),
    /// 重新查找任务、重新读取改动。
    Refresh,
    /// 从任务的改动回到任务列表。
    Back,
    /// 任务列表里的一个任务（筛选后的序号）。
    Task(usize),
    /// 一个项目的标题行：展开、收起它的文件。
    Project(usize),
    /// 一个项目里的一个文件。
    File(usize, usize),
}

/// 选中的任务在一个项目里的改动。
#[derive(Debug, Clone)]
pub(crate) struct ProjectChanges {
    /// 项目名。
    pub name: String,
    /// 工作树的目录。
    pub root: PathBuf,
    /// 左侧的版本；仓库还没有提交时为 `None`。
    pub base: Option<String>,
    /// 左侧版本的来历：基线分支的名字或 `HEAD`。
    pub base_name: String,
    /// 有改动的文件。
    pub files: Vec<CommitFile>,
    /// 读取失败的原因。
    pub error: Option<String>,
}

/// 后台任务送回的结果。
enum TaskLoaded {
    /// 找到的任务。
    Tasks { request: u64, tasks: Vec<Task> },
    /// 选中的任务在各项目里的改动。
    Changes {
        request: u64,
        changes: Vec<ProjectChanges>,
    },
    /// 一个文件的差异。
    Diff {
        request: u64,
        at: (usize, usize),
        title: String,
        view: Result<Box<DiffView>, String>,
    },
}

/// 任务视图的状态。
pub(crate) struct TaskState {
    /// 找到的全部任务。
    pub tasks: Vec<Task>,
    /// 是否正在查找任务。
    pub scanning: bool,
    /// 是否已经查找过。
    pub scanned: bool,
    /// 选中的关键字筛选项（[`App::task_keywords`] 的下标）；`None` 是全部。
    pub keyword: Option<usize>,
    /// 改动的范围。
    pub range: TaskRange,
    /// 选中的任务名。
    pub selected: Option<String>,
    /// 选中的任务在各项目里的改动。
    pub changes: Vec<ProjectChanges>,
    /// 是否正在读取改动。
    pub loading: bool,
    /// 收起的项目（工作树的目录）。
    pub collapsed: HashSet<PathBuf>,
    /// 列表的滚动位置（逻辑像素）。
    pub scroll: f32,
    /// 差异标签里显示的文件：（项目，文件）。
    pub current: Option<(usize, usize)>,
    /// 显示差异的标签。
    pub preview: Option<DocumentId>,
    /// 读取改动后要打开的文件：（工作树的目录，文件）；没有时打开第一个文件。
    reopen: Option<(PathBuf, PathBuf)>,
    scan_request: u64,
    changes_request: u64,
    diff_request: u64,
    results: (Sender<TaskLoaded>, Receiver<TaskLoaded>),
}

impl Default for TaskState {
    fn default() -> TaskState {
        TaskState {
            tasks: Vec::new(),
            scanning: false,
            scanned: false,
            keyword: None,
            range: TaskRange::default(),
            selected: None,
            changes: Vec::new(),
            loading: false,
            collapsed: HashSet::new(),
            scroll: 0.0,
            current: None,
            preview: None,
            reopen: None,
            scan_request: 0,
            changes_request: 0,
            diff_request: 0,
            results: channel(),
        }
    }
}

/// 任务视图列表里的一行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskRow {
    /// 任务列表里的一个任务（在 [`TaskState::tasks`] 中的下标）。
    Task(usize),
    /// 项目的标题行。
    Project(usize),
    /// 项目里的一个文件。
    File(usize, usize),
}

impl App {
    /// 筛选任务的关键字：配置里的，没有配置时是默认的。
    pub(crate) fn task_keywords(&self) -> Vec<String> {
        if self.config.task_keywords.is_empty() {
            let defaults = winc_config::Config::TASK_KEYWORDS;
            return defaults.iter().map(|k| (*k).to_owned()).collect();
        }
        self.config.task_keywords.clone()
    }

    /// 归并任务的规则：配置里的基线分支，没有配置时是默认的。
    fn task_rules(&self) -> TaskRules {
        if self.config.task_baselines.is_empty() {
            return TaskRules::default();
        }
        TaskRules {
            baselines: self.config.task_baselines.clone(),
        }
    }

    /// 查找任务的目录：配置里的（开头的 `~/` 展开为主目录），没有配置时是打开的各个目录。
    fn task_roots(&self) -> Vec<PathBuf> {
        if self.config.task_roots.is_empty() {
            return (self.editor.workspace.iter())
                .flat_map(|w| w.folders().map(Path::to_path_buf))
                .collect();
        }
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
        (self.config.task_roots.iter())
            .map(|root| match (root.strip_prefix("~/"), &home) {
                (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
                _ if root == "~" => home.clone().map_or_else(|| root.into(), PathBuf::from),
                _ => PathBuf::from(root),
            })
            .collect()
    }

    /// 把侧栏切换到任务视图；还没有查找过任务时开始查找。
    pub(crate) fn show_tasks(&mut self) {
        self.set_sidebar_view(SidebarView::Tasks);
        if !self.panels.sidebar.visible {
            self.set_sidebar(true);
        }
        if !self.tasks.scanned && !self.tasks.scanning {
            self.scan_tasks();
        }
        self.request_redraw();
    }

    /// 打开的目录换了之后：任务是从打开的目录里找的（没有配置查找目录）时，丢掉旧目录的任务，
    /// 任务视图正显示着就重新查找。
    pub(in crate::app) fn tasks_folder_changed(&mut self) {
        if !self.config.task_roots.is_empty() {
            return;
        }
        let state = &mut self.tasks;
        state.tasks.clear();
        state.changes.clear();
        (state.selected, state.current, state.reopen) = (None, None, None);
        (state.scanned, state.loading, state.scroll) = (false, false, 0.0);
        // 作废还在路上的结果。
        state.changes_request += 1;
        state.diff_request += 1;
        if self.panels.sidebar.view == SidebarView::Tasks {
            self.scan_tasks();
        } else {
            self.tasks.scan_request += 1;
            self.tasks.scanning = false;
        }
    }

    /// 在后台查找任务。
    pub(in crate::app) fn scan_tasks(&mut self) {
        let (roots, rules) = (self.task_roots(), self.task_rules());
        self.tasks.scan_request += 1;
        self.tasks.scanning = true;
        let request = self.tasks.scan_request;
        let sender = self.tasks.results.0.clone();
        self.spawn_git(UserEvent::Tasks, move |tool| {
            let tasks = match tool {
                Ok(git) => git.scan_tasks(&roots, &rules, &Cancel::default()),
                Err(_) => Vec::new(),
            };
            let _ = sender.send(TaskLoaded::Tasks { request, tasks });
        });
    }

    /// 符合当前关键字筛选的任务（在 [`TaskState::tasks`] 中的下标）。
    pub(crate) fn shown_tasks(&self) -> Vec<usize> {
        let keywords = self.task_keywords();
        let keyword = self.tasks.keyword.and_then(|i| keywords.get(i));
        let keyword = keyword.map(|k| k.to_lowercase());
        (self.tasks.tasks.iter().enumerate())
            .filter(|(_, task)| {
                (keyword.as_ref()).is_none_or(|k| task.name.to_lowercase().contains(k.as_str()))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// 列表里的各行：没有选中任务时是任务，选中后是各项目和其中的文件。
    pub(crate) fn task_rows(&self) -> Vec<TaskRow> {
        if self.tasks.selected.is_none() {
            return self.shown_tasks().into_iter().map(TaskRow::Task).collect();
        }
        let mut rows = Vec::new();
        for (p, project) in self.tasks.changes.iter().enumerate() {
            rows.push(TaskRow::Project(p));
            if !self.tasks.collapsed.contains(&project.root) {
                rows.extend((0..project.files.len()).map(|f| TaskRow::File(p, f)));
            }
        }
        rows
    }

    /// 选中的任务改动的文件总数。
    pub(crate) fn task_file_count(&self) -> usize {
        self.tasks.changes.iter().map(|p| p.files.len()).sum()
    }

    /// 列表的位置：侧栏里标题行、筛选行、范围行之下的部分。
    pub(crate) fn task_list(&self) -> Rect {
        let panel = self.panel_rect();
        let top = self.layout.sidebar_header.bottom() + 2.0 * (TASK_ROW + 6.0) + 4.0;
        Rect::new(panel.x, top, panel.w, (panel.bottom() - top).max(0.0))
    }

    /// 列表最多能滚动多少（逻辑像素）。
    pub(crate) fn task_max_scroll(&self) -> f32 {
        let content = (self.task_rows().len() + 1) as f32 * TASK_ROW;
        (content - self.task_list().h).max(0.0)
    }

    /// 任务视图是否正显示在侧栏里。
    pub(crate) fn tasks_shown(&self) -> bool {
        self.panels.sidebar.view == SidebarView::Tasks && self.layout.sidebar.right() > 0.0
    }

    /// 处理任务视图里的点击。
    pub(in crate::app) fn press_task(&mut self, target: TaskTarget) {
        match target {
            TaskTarget::Keyword(keyword) => {
                self.tasks.keyword = keyword;
                self.tasks.scroll = 0.0;
            }
            TaskTarget::Range(range) => {
                if self.tasks.range != range {
                    self.tasks.range = range;
                    self.reload_task_changes();
                }
            }
            // 任务里的项目可能变了：查找完再重新读取改动（见 `poll_tasks`）。
            TaskTarget::Refresh => self.scan_tasks(),
            TaskTarget::Back => {
                self.tasks.selected = None;
                self.tasks.changes.clear();
                self.tasks.current = None;
                self.tasks.scroll = 0.0;
            }
            TaskTarget::Task(i) => {
                if let Some(task) = self.tasks.tasks.get(i) {
                    self.tasks.selected = Some(task.name.clone());
                    self.tasks.changes.clear();
                    self.tasks.current = None;
                    self.tasks.reopen = None;
                    self.tasks.scroll = 0.0;
                    self.load_task_changes();
                }
            }
            TaskTarget::Project(p) => {
                if let Some(project) = self.tasks.changes.get(p) {
                    let root = project.root.clone();
                    if !self.tasks.collapsed.remove(&root) {
                        self.tasks.collapsed.insert(root);
                    }
                }
            }
            TaskTarget::File(p, f) => self.open_task_file(p, f),
        }
        self.request_redraw();
    }

    /// 重新读取选中任务的改动，读完后仍显示原来的文件。
    fn reload_task_changes(&mut self) {
        if self.tasks.selected.is_none() {
            return;
        }
        self.tasks.reopen = self.tasks.current.and_then(|(p, f)| {
            let project = self.tasks.changes.get(p)?;
            Some((project.root.clone(), project.files.get(f)?.path.clone()))
        });
        self.load_task_changes();
    }

    /// 在后台读取选中的任务在各项目里的改动。
    fn load_task_changes(&mut self) {
        let Some(name) = self.tasks.selected.clone() else {
            return;
        };
        let Some(task) = self.tasks.tasks.iter().find(|t| t.name == name).cloned() else {
            return;
        };
        let (range, rules) = (self.tasks.range, self.task_rules());
        self.tasks.changes_request += 1;
        self.tasks.loading = true;
        let request = self.tasks.changes_request;
        let sender = self.tasks.results.0.clone();
        self.spawn_git(UserEvent::Tasks, move |tool| {
            let cancel = Cancel::default();
            let changes = (task.projects.iter())
                .map(|project| {
                    let read = match &tool {
                        Ok(git) => git.task_changes(&project.root, range, &rules, &cancel),
                        Err(e) => Err(unusable(e)),
                    };
                    let (changes, error) = match read {
                        Ok(changes) => (changes, None),
                        Err(e) => (Default::default(), Some(e)),
                    };
                    ProjectChanges {
                        name: project.name.clone(),
                        root: project.root.clone(),
                        base: changes.base,
                        base_name: changes.base_name,
                        files: changes.files,
                        error,
                    }
                })
                .collect();
            let _ = sender.send(TaskLoaded::Changes { request, changes });
        });
    }

    /// 在差异标签里打开项目 `p` 的第 `f` 个文件。
    pub(in crate::app) fn open_task_file(&mut self, p: usize, f: usize) {
        let Some(project) = self.tasks.changes.get(p) else {
            return;
        };
        let Some(file) = project.files.get(f).cloned() else {
            return;
        };
        let (root, base) = (project.root.clone(), project.base.clone());
        let base_name = project.base_name.clone();
        let name = winc_editor::display_name(&file.path);
        let title = format!("{name} · {}", project.name);
        self.tasks.diff_request += 1;
        let request = self.tasks.diff_request;
        let sender = self.tasks.results.0.clone();
        self.spawn_git(UserEvent::Tasks, move |tool| {
            let view = match tool {
                Ok(git) => task_diff(&git, &root, base.as_deref(), &base_name, &file).map(Box::new),
                Err(e) => Err(unusable(&e)),
            };
            let _ = sender.send(TaskLoaded::Diff {
                request,
                at: (p, f),
                title,
                view,
            });
        });
    }

    /// 接收后台任务送回的结果。
    pub(in crate::app) fn poll_tasks(&mut self) {
        while let Ok(loaded) = self.tasks.results.1.try_recv() {
            match loaded {
                TaskLoaded::Tasks { request, tasks } => {
                    if request != self.tasks.scan_request {
                        continue;
                    }
                    self.tasks.scanning = false;
                    self.tasks.scanned = true;
                    self.tasks.tasks = tasks;
                    let gone = (self.tasks.selected.as_ref())
                        .is_some_and(|name| !self.tasks.tasks.iter().any(|t| &t.name == name));
                    if gone {
                        self.tasks.selected = None;
                        self.tasks.changes.clear();
                        self.tasks.current = None;
                    }
                    self.reload_task_changes();
                }
                TaskLoaded::Changes { request, changes } => {
                    if request != self.tasks.changes_request {
                        continue;
                    }
                    self.tasks.loading = false;
                    self.tasks.changes = changes;
                    let reopen = self.tasks.reopen.take().and_then(|(root, path)| {
                        let p = self.tasks.changes.iter().position(|c| c.root == root)?;
                        let f =
                            (self.tasks.changes[p].files.iter()).position(|f| f.path == path)?;
                        Some((p, f))
                    });
                    let first = (self.tasks.changes.iter())
                        .position(|c| !c.files.is_empty())
                        .map(|p| (p, 0));
                    self.tasks.current = None;
                    if let Some((p, f)) = reopen.or(first) {
                        self.open_task_file(p, f);
                    }
                }
                TaskLoaded::Diff {
                    request,
                    at,
                    title,
                    view,
                } => {
                    if request != self.tasks.diff_request {
                        continue;
                    }
                    match view {
                        Ok(view) => {
                            let index = self.editor.show_diff_in(self.tasks.preview, &title, *view);
                            self.tasks.preview = Some(self.editor.documents[index].id);
                            self.tasks.current = Some(at);
                            // 键盘跟着打开的差异走，不留在已经看不见的文件树上。
                            self.focus = Focus::Editor;
                            self.reveal_task_file(at);
                            self.relayout();
                            self.clamp_scroll();
                            self.land_diff_file();
                        }
                        Err(e) => {
                            self.diff.land = None;
                            self.tasks.current = Some(at);
                            self.flash(e);
                        }
                    }
                }
            }
        }
        self.request_redraw();
    }

    /// 滚动列表，使文件 `at` 所在的行可见；所在的项目收起时先展开。
    fn reveal_task_file(&mut self, at: (usize, usize)) {
        if let Some(project) = self.tasks.changes.get(at.0) {
            let root = project.root.clone();
            self.tasks.collapsed.remove(&root);
        }
        let rows = self.task_rows();
        let Some(row) = rows.iter().position(|r| *r == TaskRow::File(at.0, at.1)) else {
            return;
        };
        let (top, list) = (row as f32 * TASK_ROW, self.task_list());
        if top < self.tasks.scroll {
            self.tasks.scroll = top;
        } else if top + TASK_ROW > self.tasks.scroll + list.h {
            self.tasks.scroll = top + TASK_ROW - list.h;
        }
        self.tasks.scroll = self.tasks.scroll.clamp(0.0, self.task_max_scroll());
    }

    /// 标签 `id` 是否为任务视图打开的差异。
    pub(in crate::app) fn steps_task_files(&self, id: DocumentId) -> bool {
        self.tasks.preview == Some(id) && self.tasks.current.is_some()
    }

    /// 任务里的下一个（`forward`）或上一个文件，跨项目：（项目，文件，序号（从 0 起），文件名，
    /// 文件总数）；当前标签不是任务的差异，或没有更多文件时返回 `None`。
    pub(in crate::app) fn next_task_file(
        &self,
        forward: bool,
    ) -> Option<(usize, usize, usize, String, usize)> {
        let id = self.editor.active()?.id;
        if !self.steps_task_files(id) {
            return None;
        }
        let all: Vec<(usize, usize)> = (self.tasks.changes.iter().enumerate())
            .flat_map(|(p, project)| (0..project.files.len()).map(move |f| (p, f)))
            .collect();
        let here = all.iter().position(|at| Some(*at) == self.tasks.current)?;
        let next = if forward {
            here + 1
        } else {
            here.checked_sub(1)?
        };
        let &(p, f) = all.get(next)?;
        let file = &self.tasks.changes[p].files[f];
        let name = winc_editor::display_name(&file.path).into_owned();
        Some((p, f, next, name, all.len()))
    }

    /// 在同一个差异标签里打开任务的上一个或下一个文件。
    pub(in crate::app) fn step_task_file(&mut self, forward: bool) -> bool {
        let Some((p, f, at, name, total)) = self.next_task_file(forward) else {
            return false;
        };
        self.open_task_file(p, f);
        let project = self.tasks.changes[p].name.clone();
        let name = format!("{project} / {name}");
        let (index, name) = (at + 1, name.as_str());
        self.flash(if forward {
            t!("git-next-file", name = name, index = index, total = total)
        } else {
            t!(
                "git-previous-file",
                name = name,
                index = index,
                total = total
            )
        });
        true
    }
}

/// 在后台准备任务里一个文件的差异：左侧是 `base` 中的版本，右侧是工作区里的文件。
fn task_diff(
    git: &Git,
    root: &Path,
    base: Option<&str>,
    base_name: &str,
    file: &CommitFile,
) -> Result<DiffView, String> {
    let name = winc_editor::display_name(&file.path).into_owned();
    let text = |bytes: Vec<u8>| -> Result<String, String> {
        if bytes.len() as u64 > LARGE_FILE {
            return Err(t!("git-diff-too-large", name = name.as_str()));
        }
        if looks_binary(&bytes) {
            return Err(t!("git-diff-binary", name = name.as_str()));
        }
        Ok(decode_unknown(&bytes).0)
    };
    let old_path = file.from.clone().unwrap_or_else(|| file.path.clone());
    let old = match base {
        Some(rev) => git
            .content_at(root, rev, &old_path, &Cancel::default())
            .map_err(|e| t!("git-read-file-failed", name = name.as_str(), error = e))?
            .map(text)
            .transpose()?,
        None => None,
    };
    let new = match winc_backend::fs::read(&file.path) {
        Ok(bytes) => Some(text(bytes)?),
        Err(_) => None,
    };
    let language = winc_syntax::detect(&name);
    let old_title = match base {
        Some(rev) if base_name != "HEAD" => format!("{base_name} ({})", log::short(rev)),
        Some(rev) => format!("HEAD ({})", log::short(rev)),
        None => t!("git-diff-side-before-root").to_owned(),
    };
    let mut old_side = DiffSide::new(old_title, old.as_deref().unwrap_or(""), None, language)
        .with_missing(base.is_some() && old.is_none());
    if let Some(rev) = base {
        old_side = old_side.with_blame(BlameSource {
            root: root.to_path_buf(),
            path: old_path,
            rev: Some(rev.to_owned()),
        });
    }
    let relative = file.path.strip_prefix(root).unwrap_or(&file.path);
    let new_side = DiffSide::new(
        relative.display().to_string(),
        new.as_deref().unwrap_or(""),
        new.is_some().then(|| file.path.clone()),
        language,
    )
    .with_missing(new.is_none());
    let mut view = DiffView::new(old_side, new_side);
    view.snapshot_path = Some(file.path.clone());
    Ok(view)
}
