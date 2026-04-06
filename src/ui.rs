use crate::app::{App, QuitAction};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(8),
        Constraint::Min(5),
        Constraint::Length(3),
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

    let lines = vec![
        Line::styled(r"     _          __  __      ___        _  _   ", logo_style),
        Line::styled(r"    / \   _ __ \ \/ /     / _ \  _  _(_)| |_ ", logo_style),
        Line::styled(r"   / _ \ | '__| \  / ___ | | | || || | || __|", logo_style),
        Line::styled(r"  / ___ \| |    /  \|___|| |_| || || | || |_ ", logo_style),
        Line::styled(r" /_/   \_\_|   /_/\_\     \__\_\ \__,_|_| \__|", logo_style),
        Line::styled("                                macOS App Manager", sub_style),
    ];

    let header = Paragraph::new(lines)
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

fn draw_app_list(frame: &mut Frame, area: Rect, app: &App) {
    let visible = app.filtered_sorted_apps();

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

        draw_app_list_inner(frame, h_split[0], app, &visible);
        draw_preview_pane(frame, h_split[1], app, &visible);
    } else {
        draw_app_list_inner(frame, list_area, app, &visible);
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
        ("\u{1f512} ", Color::DarkGray)
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

    let mut spans = vec![
        Span::styled(marker.to_string(), Style::default().fg(marker_color)),
    ];
    spans.extend(frozen_prefix);
    spans.push(Span::styled(
        format!("{:<30}", a.name),
        Style::default().fg(name_color),
    ));
    spans.push(Span::styled(
        format!("  {:<36}", a.bundle_id),
        Style::default().fg(Color::DarkGray),
    ));
    spans.push(Span::styled(
        format!("  PID: {:<8}", a.pid),
        Style::default().fg(Color::DarkGray),
    ));
    spans.push(Span::styled(
        format!("  {}  {}", mem_str, cpu_str),
        Style::default().fg(Color::DarkGray),
    ));

    Line::from(spans)
}

fn draw_app_list_inner(frame: &mut Frame, area: Rect, app: &App, visible: &[&crate::process::GuiApp]) {
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
                items.push(ListItem::new(make_app_line(visible[idx], app)));
                if idx == app.selected_index {
                    highlight_visual = Some(items.len() - 1);
                }
            }
        }
        (items, highlight_visual)
    } else {
        let items: Vec<ListItem> = visible
            .iter()
            .map(|a| ListItem::new(make_app_line(a, app)))
            .collect();
        (items, Some(app.selected_index))
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(title),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{25b6} ");

    let mut state = ListState::default();
    state.select(highlight);
    frame.render_stateful_widget(list, area, &mut state);
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
    let mut lines = vec![Line::from(vec![
        Span::styled(" \u{2191}\u{2193}/jk", Style::default().fg(Color::Yellow)),
        Span::raw(": Navigate  "),
        Span::styled("Space", Style::default().fg(Color::Yellow)),
        Span::raw(": Select  "),
        Span::styled("a", Style::default().fg(Color::Yellow)),
        Span::raw("/"),
        Span::styled("d", Style::default().fg(Color::Yellow)),
        Span::raw(": All/None  "),
        Span::styled("Enter/r", Style::default().fg(Color::Yellow)),
        Span::raw(": Quit  "),
        Span::styled("f", Style::default().fg(Color::Yellow)),
        Span::raw(": Force  "),
        Span::styled("R", Style::default().fg(Color::Yellow)),
        Span::raw(": Restart  "),
        Span::styled("e", Style::default().fg(Color::Yellow)),
        Span::raw(": Refresh  "),
        Span::styled("/", Style::default().fg(Color::Yellow)),
        Span::raw(": Filter  "),
        Span::styled("s", Style::default().fg(Color::Yellow)),
        Span::raw(": Sort  "),
        Span::styled("g", Style::default().fg(Color::Yellow)),
        Span::raw(": Group  "),
        Span::styled("l", Style::default().fg(Color::Yellow)),
        Span::raw(": History  "),
        Span::styled("Tab/p", Style::default().fg(Color::Yellow)),
        Span::raw(": Preview  "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(": Exit"),
    ])];

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
