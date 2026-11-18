use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};

use crate::app::{App, ConfigureField, Screen, MAIN_MENU_ITEMS};
use crate::banner;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(0)])
        .split(area);

    draw_banner(frame, chunks[0]);

    match &app.screen {
        Screen::MainMenu { selected } => draw_main_menu(frame, chunks[1], *selected),
        Screen::Download {
            apps,
            selected,
            status,
        } => draw_download(frame, chunks[1], apps, *selected, status.as_deref()),
        Screen::InstallSelect {
            files,
            selected,
            status,
        } => draw_install_select(frame, chunks[1], files, *selected, status.as_deref()),
        Screen::InstallConfigure(state) => draw_configure(frame, chunks[1], state),
        Screen::InstallDone { message, is_error } => draw_done(frame, chunks[1], message, *is_error),
        Screen::Update { apps } => draw_update(frame, chunks[1], apps),
        Screen::DeleteList {
            apps,
            selected,
            confirm,
        } => draw_delete(frame, chunks[1], apps, *selected, *confirm),
        Screen::Help { scroll } => draw_help(frame, chunks[1], *scroll),
    }

    if let Some(msg) = &app.global_message {
        draw_toast(frame, area, msg);
    }
}

fn draw_banner(frame: &mut Frame, area: Rect) {
    let lines = banner::title_lines("Universal AppImage Installer");
    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn centered_list_area(area: Rect, width_pct: u16, height: u16) -> Rect {
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(area);
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(horizontal[1]);
    vertical[1]
}

fn draw_main_menu(frame: &mut Frame, area: Rect, selected: usize) {
    let list_area = centered_list_area(area, 50, MAIN_MENU_ITEMS.len() as u16 + 2);
    let items: Vec<ListItem> = MAIN_MENU_ITEMS
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let style = if i == selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("  {label}  ")).style(style)
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Main Menu — ↑/↓ select, Enter confirm, q quit "),
    );
    frame.render_widget(list, list_area);
}

fn draw_download(
    frame: &mut Frame,
    area: Rect,
    apps: &[crate::catalog::CatalogApp],
    selected: usize,
    status: Option<&str>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    if apps.is_empty() {
        let empty = Paragraph::new("The catalog is empty. Edit your apps.json to add entries.")
            .block(Block::default().borders(Borders::ALL).title(" Download AppImage "));
        frame.render_widget(empty, chunks[0]);
        return;
    }

    let items: Vec<ListItem> = apps
        .iter()
        .enumerate()
        .map(|(i, app)| {
            let style = if i == selected {
                Style::default().fg(Color::Black).bg(Color::Green)
            } else {
                Style::default()
            };
            let text = format!(
                "{name}  [{cat}]  ({id})\n  {desc}\n  {url}",
                name = app.entry.name,
                id = app.id,
                cat = if app.entry.category.is_empty() {
                    "misc"
                } else {
                    &app.entry.category
                },
                desc = if app.entry.description.is_empty() {
                    &app.entry.homepage
                } else {
                    &app.entry.description
                },
                url = app.entry.download_url
            );
            ListItem::new(text).style(style)
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Download AppImage — ↑/↓ select, Enter download, Esc back "),
    );
    frame.render_widget(list, chunks[0]);

    let status_text = status.unwrap_or("Select an app and press Enter to fetch it with curl.");
    let status_widget = Paragraph::new(status_text).block(Block::default().borders(Borders::ALL));
    frame.render_widget(status_widget, chunks[1]);
}

fn draw_install_select(
    frame: &mut Frame,
    area: Rect,
    files: &[crate::appimage::DiscoveredAppImage],
    selected: usize,
    status: Option<&str>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    let items: Vec<ListItem> = files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let style = if i == selected {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default()
            };
            ListItem::new(f.path.display().to_string()).style(style)
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Install — pick a downloaded .AppImage — Enter continue, Esc back "),
    );
    frame.render_widget(list, chunks[0]);

    let status_text = status.unwrap_or("Enter inspects the AppImage and lets you configure the install.");
    let status_widget = Paragraph::new(status_text).block(Block::default().borders(Borders::ALL));
    frame.render_widget(status_widget, chunks[1]);
}

fn field_line(label: &str, value: String, focused: bool, editable: bool) -> Line<'static> {
    let marker = if focused { "▶ " } else { "  " };
    let style = if focused {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default()
    };
    let suffix = if editable && focused { "▏" } else { "" };
    Line::from(Span::styled(
        format!("{marker}{label}: {value}{suffix}"),
        style,
    ))
}

fn draw_configure(frame: &mut Frame, area: Rect, state: &crate::app::ConfigureState) {
    let mut lines = vec![
        Line::from(format!(
            "AppImage: {}",
            state.appimage_path.display()
        )),
        Line::from(""),
        field_line(
            "Advanced mode (Enter to toggle)",
            if state.advanced { "ON".into() } else { "OFF".into() },
            state.focus == ConfigureField::AdvancedToggle,
            false,
        ),
    ];

    if state.advanced {
        lines.push(Line::from(""));
        lines.push(field_line(
            "Custom name",
            state.name.clone(),
            state.focus == ConfigureField::Name,
            true,
        ));
        lines.push(field_line(
            "Custom version",
            state.version.clone(),
            state.focus == ConfigureField::Version,
            true,
        ));
        lines.push(field_line(
            "Delete temp folder after install",
            if state.delete_temp { "ON".into() } else { "OFF".into() },
            state.focus == ConfigureField::DeleteTemp,
            false,
        ));
        lines.push(field_line(
            "Delete source .AppImage after install",
            if state.delete_source { "ON".into() } else { "OFF".into() },
            state.focus == ConfigureField::DeleteSource,
            false,
        ));
    } else {
        lines.push(Line::from(format!(
            "  (using defaults: name = '{}', version = '{}', delete temp folder = ON, delete source = OFF)",
            state.name, state.version
        )));
    }

    lines.push(Line::from(""));
    lines.push(field_line(
        "Install now",
        String::new(),
        state.focus == ConfigureField::Confirm,
        false,
    ));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Configure install — ↑/↓ move, type to edit text fields, Enter activate, Esc back "),
    );
    frame.render_widget(paragraph, area);
}

fn draw_done(frame: &mut Frame, area: Rect, message: &str, is_error: bool) {
    let color = if is_error { Color::Red } else { Color::Green };
    let title = if is_error { " Install failed " } else { " Install complete " };
    let paragraph = Paragraph::new(format!("{message}\n\nPress any key to return to the main menu."))
        .style(Style::default().fg(color))
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(paragraph, area);
}

fn draw_update(frame: &mut Frame, area: Rect, apps: &[crate::registry::InstalledApp]) {
    let text = if apps.is_empty() {
        "No apps installed yet.".to_string()
    } else {
        apps.iter()
            .map(|a| {
                format!(
                    "{} {}  (installed {})\n  {}",
                    a.name,
                    a.version,
                    a.installed_at,
                    a.appimage_path.display()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Update — installed apps (checking for new versions is not implemented yet) — Esc back "),
    );
    frame.render_widget(paragraph, area);
}

fn draw_delete(
    frame: &mut Frame,
    area: Rect,
    apps: &[crate::registry::InstalledApp],
    selected: usize,
    confirm: bool,
) {
    let items: Vec<ListItem> = apps
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let style = if i == selected {
                Style::default().fg(Color::Black).bg(Color::Red)
            } else {
                Style::default()
            };
            ListItem::new(format!("{} {}", a.name, a.version)).style(style)
        })
        .collect();

    let title = if apps.is_empty() {
        " Delete — nothing installed — Esc back "
    } else {
        " Delete — ↑/↓ select, Enter/d delete, Esc back "
    };
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(list, area);

    if confirm {
        if let Some(app) = apps.get(selected) {
            let popup = centered_list_area(area, 60, 5);
            frame.render_widget(Clear, popup);
            let text = format!(
                "Delete {} {}?\n\nThis removes the .AppImage, its launcher and icon.\n\ny = confirm    n/Esc = cancel",
                app.name, app.version
            );
            let paragraph = Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title(" Confirm delete "));
            frame.render_widget(paragraph, popup);
        }
    }
}

const HELP_TEXT: &str = "\
Universal AppImage Installer — Help

Navigation: Up/Down arrows move the selection, Enter confirms/activates, Esc goes back
to the previous screen, q quits from the main menu.

Download AppImage
  Fetches an .AppImage from a small built-in catalog (a key/value JSON file you can
  extend) using curl. Currently ships with ClickUp; add more entries by editing
  ~/.config/universal-appimage-installer/apps.json with the same shape:
  { \"id\": { \"name\", \"category\", \"description\", \"homepage\",
  \"download_url\" } }. Downloaded files land in ~/.applications.

Install
  Lists the .AppImage files sitting in ~/.applications. After you pick one,
  it runs `--appimage-extract` inside a timestamped temp folder to read the
  app's internal launcher (name, version, icon) without running the app itself.
  By default it installs straight away using the detected name/version, deletes
  the temp extraction folder afterwards, and keeps the original .AppImage in
  ~/.applications.
  Advanced mode (toggle on the configure screen) lets you override:
    - Custom name: overrides the menu entry's display name
    - Custom version: overrides the version tag used in the installed filename
    - Delete temp folder: remove the extraction folder after installing (default ON)
    - Delete source AppImage: remove the original downloaded file after installing
      (default OFF, so you keep a copy in ~/.applications)
  Installing copies the AppImage into ~/.applications/installed, copies the
  app's icon into ~/.local/share/icons/hicolor, and writes a .desktop launcher
  in ~/.local/share/applications that points at both, so the app shows up in
  your system's application menu. Missing folders are created automatically.

Update
  Lists the apps this tool has installed (name, version, install date, path).
  This screen is informational only for now — it does not check for or apply
  updates yet.

Delete
  Lists installed apps. Select one and press Enter or 'd' to remove it: this
  deletes the installed .AppImage, its .desktop launcher and its icon after a
  confirmation prompt.

Help
  This screen.
";

fn draw_help(frame: &mut Frame, area: Rect, scroll: u16) {
    let paragraph = Paragraph::new(HELP_TEXT)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .block(Block::default().borders(Borders::ALL).title(" Help — ↑/↓ scroll, Esc back "));
    frame.render_widget(paragraph, area);
}

fn draw_toast(frame: &mut Frame, area: Rect, message: &str) {
    let popup = centered_list_area(area, 70, 5);
    frame.render_widget(Clear, popup);
    let paragraph = Paragraph::new(format!("{message}\n\nPress any key to dismiss."))
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::Red))
        .block(Block::default().borders(Borders::ALL).title(" Notice "));
    frame.render_widget(paragraph, popup);
}
