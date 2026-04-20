use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Task {
    Ping { message: String },
    ProcessImage { path: String },
    ProcessDataset { dir_path: String }, // NEW
}
