use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use eframe::egui::{self, Color32};

use crate::model::{AssetSelection, AssetSelectionKind, MaxionGuiApp, WorkerResult};
use crate::services::{
    dedupe_entries, default_output_path, detect_pe_architecture, input_parent_dir, load_config,
    migrate_legacy_defaults, run_protect, save_config,
};
use crate::view;

impl MaxionGuiApp {
    pub fn new() -> Self {
        let mut config = load_config().unwrap_or_default();
        config.asset_entries = dedupe_entries(config.asset_entries);
        migrate_legacy_defaults(&mut config);

        if config.output_dir.is_empty() {
            if let Some(default_dir) = default_output_path(&config.input_exe) {
                config.output_dir = default_dir.display().to_string();
            }
        }

        Self {
            config,
            log_text: "Ready.\n".to_string(),
            status_text: "Idle".to_string(),
            status_color: Color32::from_rgb(54, 44, 34),
            worker_rx: None,
            running: false,
        }
    }

    pub fn expected_output_exe(&self) -> String {
        crate::services::expected_output_exe(&self.config.input_exe, &self.config.output_dir)
            .map(|path| path.display().to_string())
            .unwrap_or_default()
    }

    pub fn detected_is_x64(&self) -> Option<bool> {
        let input = Path::new(self.config.input_exe.trim());
        if !input.is_file() {
            return None;
        }
        detect_pe_architecture(input).ok()
    }

    pub fn add_asset_entries(&mut self, entries: Vec<AssetSelection>) {
        self.config.asset_entries.extend(entries);
        self.config.asset_entries = dedupe_entries(std::mem::take(&mut self.config.asset_entries));
        let _ = save_config(&self.config);
    }

    pub fn pick_input_exe(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Windows Executable", &["exe"])
            .pick_file()
        {
            self.config.input_exe = path.display().to_string();
            if let Some(default_dir) = default_output_path(&self.config.input_exe) {
                self.config.output_dir = default_dir.display().to_string();
            }
            let _ = save_config(&self.config);
        }
    }

    pub fn pick_output_dir(&mut self) {
        let start_dir = Path::new(&self.config.output_dir);
        let dialog = if start_dir.is_dir() {
            rfd::FileDialog::new().set_directory(start_dir)
        } else {
            rfd::FileDialog::new()
        };

        if let Some(path) = dialog.pick_folder() {
            self.config.output_dir = path.display().to_string();
            let _ = save_config(&self.config);
        }
    }

    pub fn browse_files(&mut self) {
        let base_dir = input_parent_dir(&self.config.input_exe);
        let dialog = if let Some(dir) = base_dir.as_ref() {
            rfd::FileDialog::new().set_directory(dir)
        } else {
            rfd::FileDialog::new()
        };

        if let Some(paths) = dialog.pick_files() {
            let entries = paths
                .into_iter()
                .filter(|path| path.is_file())
                .map(|path| AssetSelection {
                    path: path.display().to_string(),
                    kind: AssetSelectionKind::File,
                })
                .collect();
            self.add_asset_entries(entries);
        }
    }

    pub fn browse_folders(&mut self) {
        let base_dir = input_parent_dir(&self.config.input_exe);
        let dialog = if let Some(dir) = base_dir.as_ref() {
            rfd::FileDialog::new().set_directory(dir)
        } else {
            rfd::FileDialog::new()
        };

        if let Some(paths) = dialog.pick_folders() {
            let entries = paths
                .into_iter()
                .filter(|path| path.is_dir())
                .map(|path| AssetSelection {
                    path: path.display().to_string(),
                    kind: AssetSelectionKind::Folder,
                })
                .collect();
            self.add_asset_entries(entries);
        }
    }

    pub fn begin_protect(&mut self) {
        if self.running {
            return;
        }

        let config = self.config.clone();
        self.log_text.clear();
        self.status_text = "Protecting...".to_string();
        self.status_color = Color32::from_rgb(207, 143, 71);
        self.running = true;

        let (tx, rx): (mpsc::Sender<WorkerResult>, Receiver<WorkerResult>) = mpsc::channel();
        self.worker_rx = Some(rx);

        thread::spawn(move || {
            let result = match run_protect(&config) {
                Ok(full_log) => WorkerResult {
                    success: true,
                    full_log,
                    summary: "Protect completed successfully.".to_string(),
                },
                Err(err) => WorkerResult {
                    success: false,
                    summary: "Protect failed.".to_string(),
                    full_log: format!("{err:#}"),
                },
            };
            let _ = tx.send(result);
        });
    }

    pub fn poll_worker(&mut self) {
        if let Some(rx) = &self.worker_rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.running = false;
                    self.status_text = result.summary.clone();
                    self.status_color = if result.success {
                        Color32::from_rgb(67, 119, 74)
                    } else {
                        Color32::from_rgb(176, 54, 54)
                    };
                    self.log_text = result.full_log;
                    self.worker_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.running = false;
                    self.worker_rx = None;
                    self.status_text = "Protect process disconnected.".to_string();
                    self.status_color = Color32::from_rgb(176, 54, 54);
                }
            }
        }
    }
}

impl eframe::App for MaxionGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker();
        let _ = save_config(&self.config);

        if self.running {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        view::render(self, ctx);
    }
}
