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

fn draw_app_list(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .apps
        .iter()
        .map(|a| {
            let is_selected = app.selected_pids.contains(&a.pid);
            let marker = if is_selected { "● " } else { "  " };
            let name_color = if is_selected { Color::Cyan } else { Color::White };
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
                    format!("  PID: {}", a.pid),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let title = if app.selected_pids.is_empty() {
        format!(" Applications ({}) ", app.apps.len())
    } else {
        format!(" Applications ({}) — {} selected ", app.apps.len(), app.selected_pids.len())
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
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(app.selected_index));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![Line::from(vec![
        Span::styled(" ↑↓/jk", Style::default().fg(Color::Yellow)),
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
        Span::raw(": Refresh  "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(": Exit"),
    ])];

    if let Some((ref msg, success)) = app.status_message {
        let color = if success { Color::Green } else { Color::Red };
        let prefix = if success { " ✓ " } else { " ✗ " };
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
                format!("    • {}", name),
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

    // Size the dialog based on content
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
