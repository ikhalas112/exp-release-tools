use std::sync::mpsc::Receiver;

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetSelectionKind {
    File,
    Folder,
}

impl AssetSelectionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Folder => "Folder",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetSelection {
    pub path: String,
    pub kind: AssetSelectionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiConfig {
    pub input_exe: String,
    pub output_dir: String,
    pub asset_entries: Vec<AssetSelection>,
    pub protect_types: String,
    pub skip_types: String,
    pub compression_level: u32,
    pub use_phase2: bool,
    pub enable_compress: bool,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            input_exe: String::new(),
            output_dir: String::new(),
            asset_entries: Vec::new(),
            protect_types: String::new(),
            skip_types: String::new(),
            compression_level: 6,
            use_phase2: false,
            enable_compress: true,
        }
    }
}

#[derive(Debug)]
pub struct WorkerResult {
    pub success: bool,
    pub summary: String,
    pub full_log: String,
}

pub struct MaxionGuiApp {
    pub config: GuiConfig,
    pub log_text: String,
    pub status_text: String,
    pub status_color: Color32,
    pub worker_rx: Option<Receiver<WorkerResult>>,
    pub running: bool,
}
