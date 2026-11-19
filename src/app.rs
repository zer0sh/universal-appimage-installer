use std::path::PathBuf;

use anyhow::Result;

use crate::appimage::{self, DiscoveredAppImage, ExtractedInfo, InstallOptions};
use crate::catalog::{self, CatalogApp};
use crate::config::Paths;
use crate::downloader;
use crate::registry::{self, InstalledApp};

pub const MAIN_MENU_ITEMS: [&str; 6] = [
    "Download AppImage",
    "Install",
    "Update",
    "Delete",
    "Help",
    "Quit",
];

pub enum Screen {
    MainMenu {
        selected: usize,
    },
    Download {
        apps: Vec<CatalogApp>,
        selected: usize,
        status: Option<String>,
    },
    InstallSelect {
        files: Vec<DiscoveredAppImage>,
        selected: usize,
        status: Option<String>,
    },
    InstallConfigure(ConfigureState),
    InstallDone {
        message: String,
        is_error: bool,
    },
    Update {
        apps: Vec<InstalledApp>,
    },
    DeleteList {
        apps: Vec<InstalledApp>,
        selected: usize,
        confirm: bool,
    },
    Help {
        scroll: u16,
    },
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ConfigureField {
    AdvancedToggle,
    Name,
    Version,
    DeleteTemp,
    DeleteSource,
    Confirm,
}

pub struct ConfigureState {
    pub appimage_path: PathBuf,
    pub info: ExtractedInfo,
    pub name: String,
    pub version: String,
    pub delete_temp: bool,
    pub delete_source: bool,
    pub advanced: bool,
    pub focus: ConfigureField,
}

impl ConfigureState {
    fn visible_fields(&self) -> Vec<ConfigureField> {
        if self.advanced {
            vec![
                ConfigureField::AdvancedToggle,
                ConfigureField::Name,
                ConfigureField::Version,
                ConfigureField::DeleteTemp,
                ConfigureField::DeleteSource,
                ConfigureField::Confirm,
            ]
        } else {
            vec![ConfigureField::AdvancedToggle, ConfigureField::Confirm]
        }
    }

    pub fn move_focus(&mut self, delta: i32) {
        let fields = self.visible_fields();
        let current = fields.iter().position(|f| *f == self.focus).unwrap_or(0);
        let len = fields.len() as i32;
        let next = ((current as i32 + delta).rem_euclid(len)) as usize;
        self.focus = fields[next];
    }
}

pub struct App {
    pub paths: Paths,
    pub screen: Screen,
    pub should_quit: bool,
    pub global_message: Option<String>,
}

impl App {
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            screen: Screen::MainMenu { selected: 0 },
            should_quit: false,
            global_message: None,
        }
    }

    pub fn go_main_menu(&mut self) {
        self.screen = Screen::MainMenu { selected: 0 };
    }

    pub fn enter_download(&mut self) {
        match catalog::load(&self.paths.catalog_file) {
            Ok(apps) => {
                self.screen = Screen::Download {
                    apps,
                    selected: 0,
                    status: None,
                };
            }
            Err(err) => {
                self.global_message = Some(format!("Error loading catalog: {err}"));
            }
        }
    }

    pub fn enter_install(&mut self) {
        if let Err(err) = appimage::cleanup_stale_temp_dirs(&self.paths.appimages_dir) {
            self.global_message = Some(format!("Warning: could not clean up temp folder: {err}"));
        }
        match appimage::list_appimages(&self.paths.appimages_dir) {
            Ok(files) => {
                let status = if files.is_empty() {
                    Some(format!(
                        "No .AppImage files found in {}",
                        self.paths.appimages_dir.display()
                    ))
                } else {
                    None
                };
                self.screen = Screen::InstallSelect {
                    files,
                    selected: 0,
                    status,
                };
            }
            Err(err) => {
                self.global_message = Some(format!("Error listing AppImages: {err}"));
            }
        }
    }

    pub fn enter_update(&mut self) {
        match registry::load(&self.paths.registry_file) {
            Ok(apps) => self.screen = Screen::Update { apps },
            Err(err) => self.global_message = Some(format!("Error loading registry: {err}")),
        }
    }

    pub fn enter_delete(&mut self) {
        match registry::load(&self.paths.registry_file) {
            Ok(apps) => {
                self.screen = Screen::DeleteList {
                    apps,
                    selected: 0,
                    confirm: false,
                }
            }
            Err(err) => self.global_message = Some(format!("Error loading registry: {err}")),
        }
    }

    pub fn enter_help(&mut self) {
        self.screen = Screen::Help { scroll: 0 };
    }

    pub fn start_download(&mut self, url: String, app_name: String) {
        self.global_message = None;
        let result = crossterm_scope(|| downloader::download_with_curl(&url, &self.paths.appimages_dir));
        match result {
            Ok(path) => {
                if let Screen::Download { status, .. } = &mut self.screen {
                    *status = Some(format!(
                        "Downloaded {app_name} -> {}",
                        path.display()
                    ));
                }
            }
            Err(err) => {
                if let Screen::Download { status, .. } = &mut self.screen {
                    *status = Some(format!("Download failed: {err}"));
                }
            }
        }
    }

    pub fn begin_configure(&mut self, appimage_path: PathBuf) {
        match appimage::extract_info(&self.paths.appimages_dir, &appimage_path) {
            Ok(info) => {
                let default_name = info
                    .name
                    .clone()
                    .unwrap_or_else(|| filename_stem(&appimage_path));
                let default_version = info.version.clone().unwrap_or_else(|| "unknown".into());
                self.screen = Screen::InstallConfigure(ConfigureState {
                    appimage_path,
                    info,
                    name: default_name,
                    version: default_version,
                    delete_temp: true,
                    delete_source: false,
                    advanced: false,
                    focus: ConfigureField::AdvancedToggle,
                });
            }
            Err(err) => {
                self.screen = Screen::InstallDone {
                    message: format!("Failed to inspect AppImage: {err}"),
                    is_error: true,
                };
            }
        }
    }

    pub fn confirm_install(&mut self) {
        let Screen::InstallConfigure(state) = &self.screen else {
            return;
        };
        let opts = InstallOptions {
            name: if state.name.trim().is_empty() {
                "App".to_string()
            } else {
                state.name.trim().to_string()
            },
            version: if state.version.trim().is_empty() {
                "unknown".to_string()
            } else {
                state.version.trim().to_string()
            },
            delete_temp_folder: state.delete_temp,
            delete_source_appimage: state.delete_source,
        };
        let result = appimage::install(&self.paths, &state.appimage_path, &state.info, &opts);
        match result {
            Ok(installed) => {
                let msg = format!(
                    "Installed {} {} -> {}\nLauncher: {}",
                    installed.name,
                    installed.version,
                    installed.appimage_path.display(),
                    installed.desktop_path.display()
                );
                if let Err(err) = registry::add(&self.paths.registry_file, installed) {
                    self.screen = Screen::InstallDone {
                        message: format!("Installed, but failed to update registry: {err}"),
                        is_error: true,
                    };
                } else {
                    self.screen = Screen::InstallDone {
                        message: msg,
                        is_error: false,
                    };
                }
            }
            Err(err) => {
                self.screen = Screen::InstallDone {
                    message: format!("Install failed: {err}"),
                    is_error: true,
                };
            }
        }
    }

    pub fn confirm_delete(&mut self) -> Result<()> {
        let Screen::DeleteList {
            apps, selected, ..
        } = &self.screen
        else {
            return Ok(());
        };
        let Some(app) = apps.get(*selected) else {
            return Ok(());
        };
        appimage::uninstall(app)?;
        registry::remove(&self.paths.registry_file, *selected)?;
        self.enter_delete();
        Ok(())
    }
}

fn filename_stem(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("App")
        .to_string()
}

/// Runs `f` after leaving the alternate screen / raw mode so a child process
/// (curl) can draw its own progress bar directly on the terminal, then
/// restores the TUI afterwards.
fn crossterm_scope<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
    use crossterm::execute;
    use std::io::stdout;

    let _ = execute!(stdout(), LeaveAlternateScreen);
    let _ = disable_raw_mode();

    let result = f();

    let _ = enable_raw_mode();
    let _ = execute!(stdout(), EnterAlternateScreen);

    result
}
