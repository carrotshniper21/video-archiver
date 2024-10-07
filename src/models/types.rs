use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ResponseError {
    pub message: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct ArchiveResponse {
    pub files: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct File {
    pub filetype: String,
    pub contents: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FileUpload {
    pub file: File,
}
