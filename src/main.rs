mod app;
mod appimage;
mod banner;
mod catalog;
mod config;
mod downloader;
mod registry;
mod ui;

use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use app::{App, ConfigureField, Screen};
use config::Paths;

fn main() -> Result<()> {
    let paths = Paths::discover()?;
    let mut app = App::new(paths);

    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal, &mut app);
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                handle_key(app, key.code);
            }
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, code: KeyCode) {
    if app.global_message.is_some() {
        app.global_message = None;
        return;
    }

    match &mut app.screen {
        Screen::MainMenu { selected } => match code {
            KeyCode::Up => *selected = selected.checked_sub(1).unwrap_or(app::MAIN_MENU_ITEMS.len() - 1),
            KeyCode::Down => *selected = (*selected + 1) % app::MAIN_MENU_ITEMS.len(),
            KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
            KeyCode::Enter => {
                let selected = *selected;
                match selected {
                    0 => app.enter_download(),
                    1 => app.enter_install(),
                    2 => app.enter_update(),
                    3 => app.enter_delete(),
                    4 => app.enter_help(),
                    _ => app.should_quit = true,
                }
            }
            _ => {}
        },

        Screen::Download { apps, selected, .. } => match code {
            KeyCode::Up if !apps.is_empty() => {
                *selected = selected.checked_sub(1).unwrap_or(apps.len() - 1)
            }
            KeyCode::Down if !apps.is_empty() => *selected = (*selected + 1) % apps.len(),
            KeyCode::Esc => app.go_main_menu(),
            KeyCode::Enter => {
                if let Screen::Download { apps, selected, .. } = &app.screen {
                    if let Some(target) = apps.get(*selected) {
                        let url = target.entry.download_url.clone();
                        let name = target.entry.name.clone();
                        app.start_download(url, name);
                    }
                }
            }
            _ => {}
        },

        Screen::InstallSelect { files, selected, .. } => match code {
            KeyCode::Up if !files.is_empty() => {
                *selected = selected.checked_sub(1).unwrap_or(files.len() - 1)
            }
            KeyCode::Down if !files.is_empty() => *selected = (*selected + 1) % files.len(),
            KeyCode::Esc => app.go_main_menu(),
            KeyCode::Enter => {
                if let Screen::InstallSelect { files, selected, .. } = &app.screen {
                    if let Some(file) = files.get(*selected) {
                        let path = file.path.clone();
                        app.begin_configure(path);
                    }
                }
            }
            _ => {}
        },

        Screen::InstallConfigure(state) => match code {
            KeyCode::Esc => app.enter_install(),
            KeyCode::Up => state.move_focus(-1),
            KeyCode::Down => state.move_focus(1),
            KeyCode::Enter => match state.focus {
                ConfigureField::AdvancedToggle => {
                    state.advanced = !state.advanced;
                    state.focus = ConfigureField::AdvancedToggle;
                }
                ConfigureField::DeleteTemp => state.delete_temp = !state.delete_temp,
                ConfigureField::DeleteSource => state.delete_source = !state.delete_source,
                ConfigureField::Confirm => app.confirm_install(),
                ConfigureField::Name | ConfigureField::Version => {}
            },
            KeyCode::Backspace => match state.focus {
                ConfigureField::Name => {
                    state.name.pop();
                }
                ConfigureField::Version => {
                    state.version.pop();
                }
                _ => {}
            },
            KeyCode::Char(c) => match state.focus {
                ConfigureField::Name => state.name.push(c),
                ConfigureField::Version => state.version.push(c),
                _ => {}
            },
            _ => {}
        },

        Screen::InstallDone { .. } => {
            app.go_main_menu();
        }

        Screen::Update { .. } => {
            if let KeyCode::Esc = code {
                app.go_main_menu();
            }
        }

        Screen::DeleteList {
            apps,
            selected,
            confirm,
        } => {
            if *confirm {
                match code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        if let Err(err) = app.confirm_delete() {
                            app.global_message = Some(format!("Delete failed: {err}"));
                        }
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => *confirm = false,
                    _ => {}
                }
            } else {
                match code {
                    KeyCode::Up if !apps.is_empty() => {
                        *selected = selected.checked_sub(1).unwrap_or(apps.len() - 1)
                    }
                    KeyCode::Down if !apps.is_empty() => *selected = (*selected + 1) % apps.len(),
                    KeyCode::Esc => app.go_main_menu(),
                    KeyCode::Enter | KeyCode::Char('d') if !apps.is_empty() => *confirm = true,
                    _ => {}
                }
            }
        }

        Screen::Help { scroll } => match code {
            KeyCode::Up => *scroll = scroll.saturating_sub(1),
            KeyCode::Down => *scroll = scroll.saturating_add(1),
            KeyCode::Esc => app.go_main_menu(),
            _ => {}
        },
    }
}
