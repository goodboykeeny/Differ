//! 任务视图的绘制：关键字筛选行、改动范围行，以及任务列表或选中任务在各项目里的改动。
//!
//! 行的样式与文件树相同；可点击的位置在绘制时记入 `Hits::tasks`。

use std::f32::consts::FRAC_PI_2;

use winc_editor::git::{Change, TaskRange};
use winc_i18n::t;

use super::*;
use crate::app::git::tasks::{TASK_ROW, TaskRow, TaskTarget};

/// 筛选项、范围按钮里文字左右的留白（逻辑像素）。
const CHIP_PAD: f32 = 9.0;
/// 筛选项、范围按钮之间的间距（逻辑像素）。
const CHIP_GAP: f32 = 4.0;

/// 绘制任务视图。
pub(in crate::view) fn draw_tasks(app: &mut App, frame: &mut Frame<'_>) {
    let p = app.palette;
    let panel = app.panel_rect();
    let clip = app.layout.sidebar.intersect(panel);
    let header = app.layout.sidebar_header;
    let mut hits = Vec::new();

    // 标题行：选中任务后是返回箭头和任务名，否则是「任务」；右端是刷新。
    let refresh = Rect::new(
        header.right() - 8.0 - BUTTON_SMALL,
        header.y + (header.h - BUTTON_SMALL) / 2.0,
        BUTTON_SMALL,
        BUTTON_SMALL,
    );
    let hovered = app.pointer.hover == Hover::Tasks(TaskTarget::Refresh);
    draw_button(frame, p, refresh, clip, hovered, false);
    let color = if hovered { p.text } else { p.text_muted };
    icons::refresh(frame, center(refresh), color, clip);
    hits.push((refresh, TaskTarget::Refresh));
    let band = Rect::new(
        header.x + 8.0,
        header.y,
        refresh.x - header.x - 12.0,
        header.h,
    );
    match app.tasks.selected.clone() {
        Some(name) => {
            if app.pointer.hover == Hover::Tasks(TaskTarget::Back) {
                frame.rect_clipped(band.inset(0.0, 3.0), p.hover, RADIUS_ROW, clip);
            }
            let cy = band.y + band.h / 2.0;
            icons::chevron_turned(
                frame,
                (band.x + 10.0, cy),
                6.0,
                2.0 * FRAC_PI_2,
                p.text_muted,
                clip,
            );
            let count = t!("tasks-file-count", count = app.task_file_count());
            let count_w = frame.measure(FontKind::Ui, app.fonts.ui_small, &count);
            let room = (band.w - 28.0 - count_w - 8.0).max(0.0);
            let name = ellipsize(frame, &name, app.fonts.ui, room);
            ui_text(frame, app, band.x + 22.0, band, &name, p.text, clip);
            let x = band.right() - count_w - 4.0;
            label(
                frame,
                app,
                app.fonts.ui_small,
                (x, band),
                &count,
                p.text_muted,
                clip,
            );
            hits.push((band, TaskTarget::Back));
        }
        None => {
            ui_text(
                frame,
                app,
                band.x + 6.0,
                band,
                t!("tasks-title"),
                p.text,
                clip,
            );
        }
    }

    // 筛选行和范围行。
    let row = |i: usize| {
        let y = header.bottom() + i as f32 * (TASK_ROW + 6.0) + 3.0;
        Rect::new(panel.x + 8.0, y, panel.w - 16.0, TASK_ROW)
    };
    let mut chips: Vec<(String, TaskTarget, bool)> = vec![(
        t!("tasks-filter-all").to_owned(),
        TaskTarget::Keyword(None),
        app.tasks.keyword.is_none(),
    )];
    for (i, keyword) in app.task_keywords().into_iter().enumerate() {
        let on = app.tasks.keyword == Some(i);
        chips.push((keyword, TaskTarget::Keyword(Some(i)), on));
    }
    draw_chips(app, frame, row(0), &chips, clip, &mut hits);
    let ranges = [
        (t!("tasks-range-branch"), TaskRange::Branch),
        (t!("tasks-range-uncommitted"), TaskRange::Uncommitted),
    ];
    let chips: Vec<_> = (ranges.iter())
        .map(|(name, range)| {
            (
                (*name).to_owned(),
                TaskTarget::Range(*range),
                app.tasks.range == *range,
            )
        })
        .collect();
    draw_chips(app, frame, row(1), &chips, clip, &mut hits);

    let list = app.task_list();
    let rows = app.task_rows();
    let clip = clip.intersect(list);
    if rows.is_empty() {
        let title = if app.tasks.scanning || app.tasks.loading {
            t!("tasks-loading")
        } else if app.tasks.selected.is_some() {
            t!("tasks-no-changes")
        } else {
            t!("tasks-empty")
        };
        draw_empty_state(app, frame, (list, list), title, None);
        app.hits.tasks = hits;
        return;
    }
    let scroll = app.tasks.scroll.clamp(0.0, app.task_max_scroll());
    app.tasks.scroll = scroll;
    for (v, row) in rows.iter().enumerate() {
        let y = list.y + v as f32 * TASK_ROW - scroll;
        if y + TASK_ROW <= list.y || y >= list.bottom() {
            continue;
        }
        let line = Rect::new(list.x, y, list.w, TASK_ROW);
        let pill = Rect::new(
            line.x + 8.0,
            y + 1.0,
            (line.w - 16.0).max(0.0),
            TASK_ROW - 2.0,
        );
        let target = match *row {
            TaskRow::Task(i) => TaskTarget::Task(i),
            TaskRow::Project(i) => TaskTarget::Project(i),
            TaskRow::File(i, f) => TaskTarget::File(i, f),
        };
        let hovered = app.pointer.hover == Hover::Tasks(target);
        let current = matches!(*row, TaskRow::File(i, f) if app.tasks.current == Some((i, f)));
        if let Some(color) = row_color(p, current, true, hovered) {
            frame.rect_clipped(pill, color, RADIUS_ROW, clip);
        }
        let cy = icon_center_y(app, line, app.fonts.ui);
        let small = app.fonts.ui_small;
        // 行右端小一号的次要色文字，返回它的左边。
        let tail = |frame: &mut Frame<'_>, text: &str, color: Color, right: f32| {
            let x = right - frame.measure(FontKind::Ui, small, text);
            label(frame, app, small, (x, line), text, color, clip);
            x - 8.0
        };
        let right = pill.right() - 6.0;
        match *row {
            TaskRow::Task(i) => {
                let task = &app.tasks.tasks[i];
                let names: Vec<&str> = task.projects.iter().map(|p| p.name.as_str()).collect();
                let count = task.projects.len().to_string();
                let right = tail(frame, &count, p.text_muted, right);
                icons::branch_small(frame, (pill.x + 12.0, cy), p.accent, clip);
                let name_w = frame.measure(FontKind::Ui, app.fonts.ui, &task.name);
                let x = pill.x + 24.0;
                let name = ellipsize(frame, &task.name, app.fonts.ui, (right - x).max(0.0));
                ui_text(frame, app, x, line, &name, p.text, clip);
                let x = x + name_w + 10.0;
                if right - x > 24.0 {
                    let names = names.join(" · ");
                    let names = ellipsize(frame, &names, small, right - x);
                    label(frame, app, small, (x, line), &names, p.text_dim, clip);
                }
            }
            TaskRow::Project(i) => {
                let project = &app.tasks.changes[i];
                let open = !app.tasks.collapsed.contains(&project.root);
                let turned = if open { FRAC_PI_2 } else { 0.0 };
                icons::chevron_turned(frame, (pill.x + 10.0, cy), 6.0, turned, p.text_dim, clip);
                let detail = match &project.error {
                    Some(error) => error.clone(),
                    None => format!("{} · {}", project.base_name, project.files.len()),
                };
                let color = if project.error.is_some() {
                    p.error
                } else {
                    p.text_muted
                };
                let x = pill.x + 22.0;
                let name_w = frame.measure(FontKind::Ui, app.fonts.ui, &project.name);
                let room = (right - x - name_w - 10.0).max(0.0);
                let detail = ellipsize(frame, &detail, small, room);
                let right = tail(frame, &detail, color, right);
                let name = ellipsize(frame, &project.name, app.fonts.ui, (right - x).max(0.0));
                ui_text(frame, app, x, line, &name, p.text, clip);
            }
            TaskRow::File(i, f) => {
                let project = &app.tasks.changes[i];
                let file = &project.files[f];
                let color = match file.change {
                    Change::Deleted => p.git_deleted,
                    Change::Added | Change::Untracked => p.git_added,
                    Change::Conflicted => p.git_conflict,
                    Change::Renamed | Change::Modified => p.git_modified,
                };
                let letter = file.change.letter().to_string();
                let mut right = tail(frame, &letter, color, right);
                if let Some((added, removed)) = file.lines {
                    right = tail(frame, &format!("-{removed}"), p.git_deleted, right + 4.0);
                    right = tail(frame, &format!("+{added}"), p.git_added, right + 4.0);
                }
                let relative = file.path.strip_prefix(&project.root).unwrap_or(&file.path);
                let name = winc_editor::display_name(&file.path);
                let dir = relative.parent().map(|d| d.display().to_string());
                let x = pill.x + 30.0;
                let name_w = frame.measure(FontKind::Ui, app.fonts.ui, &name);
                let shown = ellipsize(frame, &name, app.fonts.ui, (right - x).max(0.0));
                ui_text(frame, app, x, line, &shown, color, clip);
                let x = x + name_w + 8.0;
                if let Some(dir) = dir.filter(|d| !d.is_empty() && right - x > 24.0) {
                    let dir = ellipsize(frame, &dir, small, right - x);
                    label(frame, app, small, (x, line), &dir, p.text_dim, clip);
                }
            }
        }
        hits.push((pill.intersect(clip), target));
    }
    app.hits.tasks = hits;
}

/// 在 `row` 里从左到右绘制一排可以选中的小按钮，选中的铺强调色；放不下的不绘制。
fn draw_chips(
    app: &App,
    frame: &mut Frame<'_>,
    row: Rect,
    chips: &[(String, TaskTarget, bool)],
    clip: Rect,
    hits: &mut Vec<(Rect, TaskTarget)>,
) {
    let p = app.palette;
    let size = app.fonts.ui_small;
    let mut x = row.x;
    for (name, target, on) in chips {
        let w = frame.measure(FontKind::Ui, size, name) + 2.0 * CHIP_PAD;
        if x + w > row.right() {
            break;
        }
        let chip = Rect::new(x, row.y + 1.0, w, row.h - 2.0);
        let hovered = app.pointer.hover == Hover::Tasks(*target);
        let (fill, color) = if *on {
            (p.accent_fill, p.text_on_accent)
        } else if hovered {
            (p.hover, p.text)
        } else {
            (p.group_bg, p.text_muted)
        };
        frame.rect_clipped(chip, fill, chip.h / 2.0, clip);
        label(frame, app, size, (x + CHIP_PAD, chip), name, color, clip);
        hits.push((chip, *target));
        x += w + CHIP_GAP;
    }
}
