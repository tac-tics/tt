use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
pub enum BufferMode {
    Normal,
    Insert,
    Command,
}

impl Default for BufferMode {
    fn default() -> Self {
        BufferMode::Normal
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct TermTextState {
    pub path: Option<PathBuf>,
    pub command: Option<String>,
    pub data: String,
    pub mode: BufferMode,
}
