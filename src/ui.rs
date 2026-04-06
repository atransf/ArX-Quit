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

    // If filter is active, split the area to show filter input bar
    if app.filter_active {
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
        draw_app_list_inner(frame, sub[1], app, &visible);
    } else {
        draw_app_list_inner(frame, area, app, &visible);
    }
}

fn draw_app_list_inner(frame: &mut Frame, area: Rect, app: &App, visible: &[&crate::process::GuiApp]) {
    let items: Vec<ListItem> = visible
        .iter()
        .map(|a| {
            let is_selected = app.selected_pids.contains(&a.pid);
            let marker = if is_selected { "\u{25cf} " } else { "  " };
            let name_color = if is_selected { Color::Cyan } else { Color::White };
            let mem_str = format_memory(a.memory_kb);
            let cpu_str = format!("{:.1}%", a.cpu_percent);
            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{:<30}", a.name),
                    Style::default().fg(name_color),
                ),
                Span::styled(
                    format!("  {:<36}", a.bundle_id),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("  PID: {:<8}", a.pid),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("  {}  {}", mem_str, cpu_str),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let sort_label = app.sort_mode.label();
    let title = if app.filter_active && !app.filter_query.is_empty() {
        if app.selected_pids.is_empty() {
            format!(" Applications ({}) [{}] Filter: {} ", visible.len(), sort_label, app.filter_query)
        } else {
            format!(" Applications ({}) [{}] Filter: {} \u{2014} {} selected ", visible.len(), sort_label, app.filter_query, app.selected_pids.len())
        }
    } else if app.selected_pids.is_empty() {
        format!(" Applications ({}) [{}] ", visible.len(), sort_label)
    } else {
        format!(" Applications ({}) [{}] \u{2014} {} selected ", visible.len(), sort_label, app.selected_pids.len())
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
    state.select(Some(app.selected_index));
    frame.render_stateful_widget(list, area, &mut state);
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
        Span::styled("/", Style::default().fg(Color::Yellow)),
        Span::raw(": Filter  "),
        Span::styled("s", Style::default().fg(Color::Yellow)),
        Span::raw(": Sort  "),
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
