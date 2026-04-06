use crate::app::{App, QuitAction};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
const HINTS: &[(&str, &str)] = &[
    ("\u{2191}\u{2193}/jk", "Navigate"),
    ("Space", "Select"),
    ("a/d", "All/None"),
    ("Enter/r", "Quit"),
    ("f", "Force"),
    ("R", "Restart"),
    ("e", "Refresh"),
    ("/", "Filter"),
    ("s", "Sort"),
    ("g", "Group"),
    ("l", "History"),
    ("Tab/p", "Preview"),
    ("Q", "Quit All"),
    ("q", "Exit"),
];

fn hint_lines(inner_width: u16) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used: u16 = 0;

    for &(key, label) in HINTS {
        let w = 1 + key.len() as u16 + 2 + label.len() as u16 + 2;
        if used + w > inner_width && !spans.is_empty() {
            lines.push(Line::from(std::mem::take(&mut spans)));
            used = 0;
        }
        spans.push(Span::raw(" "));
        spans.push(Span::styled(key, Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(format!(": {}  ", label)));
        used += w;
    }
    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }
    lines
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let inner_width = frame.area().width.saturating_sub(2);
    let hint_row_count = hint_lines(inner_width).len() as u16;
    let status_row = app.status_message.is_some() as u16;
    let footer_height = hint_row_count + status_row + 2;

    let chunks = Layout::vertical([
        Constraint::Length(8),
        Constraint::Min(5),
        Constraint::Length(footer_height),
    ])
    .split(frame.area());

    draw_header(frame, chunks[0]);
    draw_app_list(frame, chunks[1], app);
    draw_footer(frame, chunks[2], app);

    if let Some(ref dialog) = app.confirm_dialog {
        draw_confirm_dialog(frame, &dialog.app_names, dialog.action);
    }

    if app.show_history {
        draw_history_overlay(frame, app);
    }
}

fn draw_header(frame: &mut Frame, area: Rect) {
    let logo_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let sub_style = Style::default().fg(Color::DarkGray);

    let raw = [
        r"     _         __  __      ___        _  _   ",
        r"    / \   _ __ \ \/ /     / _ \  _  _(_)| |_ ",
        r"   / _ \ | '__| \  / ___ | | | || || | || __|",
        r"  / ___ \| |    /  \|___|| |_| || || | || |_ ",
        r" /_/   \_\_|   /_/\_\     \__\_\ \__,_|_| \__|",
    ];
    let max_w = raw.iter().map(|l| l.len()).max().unwrap_or(0);
    let mut lines: Vec<Line> = raw
        .iter()
        .map(|l| Line::styled(format!("{:<width$}", l, width = max_w), logo_style))
        .collect();
    lines.push(Line::styled(format!("{:<width$}", "App Manager", width = max_w), sub_style));

    let header = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
    frame.render_widget(header, area);
}

fn format_memory(kb: u64) -> String {
    if kb < 1024 {
        "< 1MB".to_string()
    } else {
        format!("{}MB", kb / 1024)
    }
}

fn draw_app_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let visible: Vec<crate::process::GuiApp> = app.filtered_sorted_apps().into_iter().cloned().collect();
    let visible_refs: Vec<&crate::process::GuiApp> = visible.iter().collect();

    // Split for filter bar if active
    let list_area = if app.filter_active {
        let sub = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(3),
        ])
        .split(area);

        let filter_line = Line::from(vec![
            Span::styled(" Filter: ", Style::default().fg(Color::Yellow)),
            Span::styled(&app.filter_query, Style::default().fg(Color::White)),
            Span::styled("_", Style::default().fg(Color::Yellow)),
        ]);
        frame.render_widget(Paragraph::new(filter_line), sub[0]);
        sub[1]
    } else {
        area
    };

    // Split for preview pane if active
    if app.show_preview {
        let h_split = Layout::horizontal([
            Constraint::Percentage(60),
            Constraint::Percentage(40),
        ])
        .split(list_area);

        draw_app_list_inner(frame, h_split[0], app, &visible_refs);
        draw_preview_pane(frame, h_split[1], app, &visible_refs);
    } else {
        draw_app_list_inner(frame, list_area, app, &visible_refs);
    }
}

fn app_group_name(bundle_id: &str) -> &'static str {
    if bundle_id.starts_with("com.apple.") {
        "Apple"
    } else if bundle_id.starts_with("com.google.") {
        "Google"
    } else if bundle_id.starts_with("com.microsoft.") {
        "Microsoft"
    } else if bundle_id.starts_with("com.jetbrains.") {
        "JetBrains"
    } else if bundle_id.starts_with("com.github.") || bundle_id.starts_with("io.github.") {
        "GitHub / Electron"
    } else {
        "Other"
    }
}

fn make_app_line(a: &crate::process::GuiApp, app: &App) -> Line<'static> {
    let is_protected = app.is_protected(&a.name);
    let is_selected = app.selected_pids.contains(&a.pid);

    let (marker, marker_color) = if is_protected {
        ("\u{1f512}", Color::DarkGray)
    } else if is_selected {
        ("\u{25cf} ", Color::Cyan)
    } else {
        ("  ", Color::White)
    };

    let name_color = if is_protected {
        Color::DarkGray
    } else if is_selected {
        Color::Cyan
    } else {
        Color::White
    };

    let frozen_prefix = if a.is_frozen {
        vec![Span::styled("\u{26a0} ", Style::default().fg(Color::Red))]
    } else {
        vec![]
    };

    let mem_str = format_memory(a.memory_kb);
    let cpu_str = format!("{:.1}%", a.cpu_percent);

    let name_col = truncate(&a.name, 24);
    let bundle_col = truncate(&a.bundle_id, 30);

    let mut spans = vec![
        Span::styled(marker.to_string(), Style::default().fg(marker_color)),
    ];
    spans.extend(frozen_prefix);
    spans.push(Span::styled(
        format!("{:<24}", name_col),
        Style::default().fg(name_color),
    ));
    spans.push(Span::styled(
        format!("  {:<30}", bundle_col),
        Style::default().fg(Color::DarkGray),
    ));
    spans.push(Span::styled(
        format!("  {:>6}", a.pid),
        Style::default().fg(Color::DarkGray),
    ));
    spans.push(Span::styled(
        format!("  {:>6}", mem_str),
        Style::default().fg(Color::DarkGray),
    ));
    spans.push(Span::styled(
        format!("  {:>5}", cpu_str),
        Style::default().fg(Color::DarkGray),
    ));

    Line::from(spans)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}\u{2026}", &s[..max - 1]) }
}

fn draw_app_list_inner(frame: &mut Frame, area: Rect, app: &mut App, visible: &[&crate::process::GuiApp]) {
    let sort_label = app.sort_mode.label();
    let grouped_tag = if app.group_mode { " [Grouped]" } else { "" };
    let filter_tag = if app.filter_active && !app.filter_query.is_empty() {
        format!(" Filter: {}", app.filter_query)
    } else {
        String::new()
    };
    let select_tag = if app.selected_pids.is_empty() {
        String::new()
    } else {
        format!(" \u{2014} {} selected", app.selected_pids.len())
    };
    let title = format!(" Applications ({}) [{}]{}{}{} ", visible.len(), sort_label, grouped_tag, filter_tag, select_tag);

    let (items, highlight) = if app.group_mode {
        let group_order = ["Apple", "Google", "Microsoft", "JetBrains", "GitHub / Electron", "Other"];
        let mut items: Vec<ListItem> = Vec::new();
        let mut highlight_visual: Option<usize> = None;

        for &group_name in &group_order {
            let indices: Vec<usize> = visible.iter().enumerate()
                .filter(|(_, a)| app_group_name(&a.bundle_id) == group_name)
                .map(|(i, _)| i)
                .collect();
            if indices.is_empty() { continue; }

            items.push(ListItem::new(Line::styled(
                format!("\u{2500}\u{2500} {} ({}) \u{2500}\u{2500}", group_name, indices.len()),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));

            for idx in indices {
                let item = ListItem::new(vec![
                    make_app_line(visible[idx], app),
                    Line::raw(""),
                ]);
                items.push(item);
                if idx == app.selected_index {
                    highlight_visual = Some(items.len() - 1);
                }
            }
        }
        (items, highlight_visual)
    } else {
        let items: Vec<ListItem> = visible
            .iter()
            .map(|a| ListItem::new(vec![
                make_app_line(a, app),
                Line::raw(""),
            ]))
            .collect();
        (items, Some(app.selected_index))
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner: 1-line column header + separator + list
    let [header_area, sep_area, list_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ]).areas(inner);

    let header_line = Line::from(vec![
        Span::raw("    "),
        Span::styled(format!("{:<24}", "Name"),    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {:<30}", "Bundle ID"), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {:>6}", "PID"),    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {:>6}", "Mem"),    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {:>5}", "CPU"),    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]);
    frame.render_widget(Paragraph::new(header_line), header_area);

    let sep = "\u{2500}".repeat(inner.width as usize);
    frame.render_widget(
        Paragraph::new(Line::styled(sep, Style::default().fg(Color::DarkGray))),
        sep_area,
    );

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{25b6} ");

    app.list_state.select(highlight);
    frame.render_stateful_widget(list, list_area, &mut app.list_state);
}

fn draw_preview_pane(frame: &mut Frame, area: Rect, app: &App, visible: &[&crate::process::GuiApp]) {
    let content = if let Some(a) = visible.get(app.selected_index) {
        vec![
            Line::from(Span::styled(
                &a.name,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )),
            Line::raw(""),
            Line::from(vec![
                Span::styled("PID:       ", Style::default().fg(Color::Yellow)),
                Span::raw(a.pid.to_string()),
            ]),
            Line::from(vec![
                Span::styled("Bundle ID: ", Style::default().fg(Color::Yellow)),
                Span::raw(&a.bundle_id),
            ]),
            Line::from(vec![
                Span::styled("Memory:    ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{:.1} MB", a.memory_kb as f64 / 1024.0)),
            ]),
            Line::from(vec![
                Span::styled("CPU:       ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{:.1}%", a.cpu_percent)),
            ]),
            Line::from(vec![
                Span::styled("Status:    ", Style::default().fg(Color::Yellow)),
                if a.is_frozen {
                    Span::styled("Not Responding", Style::default().fg(Color::Red))
                } else {
                    Span::styled("Running", Style::default().fg(Color::Green))
                },
            ]),
        ]
    } else {
        vec![Line::styled(
            "No app selected",
            Style::default().fg(Color::DarkGray),
        )]
    };

    let preview = Paragraph::new(content).block(
        Block::default()
            .title(" Preview ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(preview, area);
}

fn draw_history_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 60, frame.area());
    frame.render_widget(Clear, area);

    let title = format!(" Quit History ({}) ", app.quit_history.len());

    let lines: Vec<Line> = app.quit_history.iter().rev().map(|entry| {
        let time_str = if let Ok(dur) = entry.timestamp.duration_since(std::time::UNIX_EPOCH) {
            let secs = dur.as_secs();
            let h = (secs / 3600) % 24;
            let m = (secs / 60) % 60;
            let s = secs % 60;
            format!("{:02}:{:02}:{:02}", h, m, s)
        } else {
            "??:??:??".to_string()
        };

        let (icon, icon_color) = if entry.success {
            ("\u{2713}", Color::Green)
        } else {
            ("\u{2717}", Color::Red)
        };

        let action_str = match entry.action {
            QuitAction::Graceful => "quit",
            QuitAction::Force => "force",
        };

        Line::from(vec![
            Span::styled(format!("  [{}]  ", time_str), Style::default().fg(Color::DarkGray)),
            Span::styled(icon, Style::default().fg(icon_color)),
            Span::raw(format!("  {:<6} {}", action_str, entry.app_name)),
        ])
    }).collect();

    let content = if lines.is_empty() {
        vec![Line::styled("  No history yet", Style::default().fg(Color::DarkGray))]
    } else {
        lines
    };

    let overlay = Paragraph::new(content).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(overlay, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let inner_width = area.width.saturating_sub(2);
    let mut lines = hint_lines(inner_width);

    if let Some((ref msg, success)) = app.status_message {
        let color = if success { Color::Green } else { Color::Red };
        let prefix = if success { " \u{2713} " } else { " \u{2717} " };
        lines.push(Line::from(Span::styled(
            format!("{}{}", prefix, msg),
            Style::default().fg(color),
        )));
    }

    let footer = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(footer, area);
}
fn draw_confirm_dialog(frame: &mut Frame, app_names: &[String], action: QuitAction) {
    let action_text = match action {
        QuitAction::Graceful => "Quit",
        QuitAction::Force => "Force Quit",
    };

    let action_color = match action {
        QuitAction::Graceful => Color::Yellow,
        QuitAction::Force => Color::Red,
    };

    let title = if app_names.len() == 1 {
        format!(" {} {}? ", action_text, app_names[0])
    } else {
        format!(" {} {} apps? ", action_text, app_names.len())
    };

    let mut text = vec![Line::raw("")];

    if app_names.len() == 1 {
        text.push(Line::from(vec![
            Span::raw("  Action: "),
            Span::styled(action_text, Style::default().fg(action_color).add_modifier(Modifier::BOLD)),
            Span::raw(format!(" \"{}\"", app_names[0])),
        ]));
    } else {
        text.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(action_text, Style::default().fg(action_color).add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {} apps:", app_names.len())),
        ]));
        for name in app_names.iter().take(8) {
            text.push(Line::from(Span::styled(
                format!("    \u{2022} {}", name),
                Style::default().fg(Color::White),
            )));
        }
        if app_names.len() > 8 {
            text.push(Line::from(Span::styled(
                format!("    ... and {} more", app_names.len() - 8),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    text.push(Line::raw(""));
    text.push(Line::from(vec![
        Span::raw("  Press "),
        Span::styled("y/Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" to confirm, "),
        Span::styled("n/Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" to cancel"),
    ]));

    let height_pct = if app_names.len() > 1 { 50 } else { 30 };
    let area = centered_rect(50, height_pct, frame.area());

    let dialog = Paragraph::new(text).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(action_color)),
    );

    frame.render_widget(Clear, area);
    frame.render_widget(dialog, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
