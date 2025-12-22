use axum::{
    extract::{Multipart, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncReadExt;

use crate::error::{AppError, Result};
use crate::state::AppState;

fn resolve_path(base: &str, path: &str) -> PathBuf {
    if path.starts_with('/') {
        PathBuf::from(path)
    } else {
        PathBuf::from(base).join(path)
    }
}

// Read file
#[derive(Debug, Deserialize)]
pub struct FileReadQuery {
    pub path: String,
    #[serde(default = "default_encoding")]
    #[allow(dead_code)]
    pub encoding: String,
}

fn default_encoding() -> String {
    "utf-8".into()
}

#[derive(Debug, Serialize)]
pub struct FileReadResponse {
    pub content: String,
    pub size: u64,
    pub mime_type: String,
}

pub async fn read_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileReadQuery>,
) -> Result<Json<FileReadResponse>> {
    let full_path = resolve_path(&state.config.workspace, &query.path);

    if !full_path.exists() {
        return Err(AppError::NotFound("File not found".into()));
    }

    let content = fs::read_to_string(&full_path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let metadata = fs::metadata(&full_path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(FileReadResponse {
        content,
        size: metadata.len(),
        mime_type: "text/plain".into(),
    }))
}

// Write file
#[derive(Debug, Deserialize)]
pub struct FileWriteRequest {
    pub path: String,
    pub content: String,
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_mode() -> String {
    "644".into()
}

#[derive(Debug, Serialize)]
pub struct FileWriteResponse {
    pub path: String,
    pub size: u64,
}

pub async fn write_file(
    State(state): State<Arc<AppState>>,
    Json(req): Json<FileWriteRequest>,
) -> Result<Json<FileWriteResponse>> {
    let full_path = resolve_path(&state.config.workspace, &req.path);

    // Create parent directories
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    fs::write(&full_path, &req.content)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Set file mode (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = u32::from_str_radix(&req.mode, 8).unwrap_or(0o644);
        let perms = std::fs::Permissions::from_mode(mode);
        fs::set_permissions(&full_path, perms)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    let size = req.content.len() as u64;

    Ok(Json(FileWriteResponse {
        path: full_path.to_string_lossy().into_owned(),
        size,
    }))
}

// List directory
#[derive(Debug, Deserialize)]
pub struct FileListQuery {
    pub path: String,
    #[serde(default)]
    pub recursive: bool,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub file_type: String,
    pub size: u64,
    pub modified: String,
}

#[derive(Debug, Serialize)]
pub struct FileListResponse {
    pub path: String,
    pub entries: Vec<FileEntry>,
}

pub async fn list_files(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileListQuery>,
) -> Result<Json<FileListResponse>> {
    let full_path = resolve_path(&state.config.workspace, &query.path);

    if !full_path.exists() {
        return Err(AppError::NotFound("Path not found".into()));
    }

    let mut entries = Vec::new();

    if query.recursive {
        collect_entries_recursive(&full_path, &mut entries).await?;
    } else {
        let mut dir = fs::read_dir(&full_path)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        while let Some(entry) = dir
            .next_entry()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
        {
            if let Some(file_entry) = entry_to_file_entry(&entry).await {
                entries.push(file_entry);
            }
        }
    }

    Ok(Json(FileListResponse {
        path: full_path.to_string_lossy().into_owned(),
        entries,
    }))
}

async fn collect_entries_recursive(path: &PathBuf, entries: &mut Vec<FileEntry>) -> Result<()> {
    let mut dir = fs::read_dir(path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    while let Some(entry) = dir
        .next_entry()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
    {
        if let Some(file_entry) = entry_to_file_entry(&entry).await {
            let is_dir = file_entry.file_type == "directory";
            entries.push(file_entry);

            if is_dir {
                Box::pin(collect_entries_recursive(&entry.path(), entries)).await?;
            }
        }
    }

    Ok(())
}

async fn entry_to_file_entry(entry: &fs::DirEntry) -> Option<FileEntry> {
    let metadata = entry.metadata().await.ok()?;
    let modified = metadata.modified().ok()?;
    let datetime: chrono::DateTime<chrono::Utc> = modified.into();

    Some(FileEntry {
        name: entry.file_name().to_string_lossy().into_owned(),
        path: entry.path().to_string_lossy().into_owned(),
        file_type: if metadata.is_dir() {
            "directory"
        } else {
            "file"
        }
        .into(),
        size: metadata.len(),
        modified: datetime.to_rfc3339(),
    })
}

// Upload file (multipart)
pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<FileWriteResponse>> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_path: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AppError::Internal(e.to_string()))?
                        .to_vec(),
                );
            }
            "path" => {
                file_path = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::Internal(e.to_string()))?,
                );
            }
            _ => {}
        }
    }

    let data = file_data.ok_or_else(|| AppError::BadRequest("Missing file field".into()))?;
    let path = file_path.ok_or_else(|| AppError::BadRequest("Missing path field".into()))?;

    let full_path = resolve_path(&state.config.workspace, &path);

    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    fs::write(&full_path, &data)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(FileWriteResponse {
        path: full_path.to_string_lossy().into_owned(),
        size: data.len() as u64,
    }))
}

// Download file
#[derive(Debug, Deserialize)]
pub struct FileDownloadQuery {
    pub path: String,
}

pub async fn download_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileDownloadQuery>,
) -> Result<Response> {
    let full_path = resolve_path(&state.config.workspace, &query.path);

    if !full_path.exists() {
        return Err(AppError::NotFound("File not found".into()));
    }

    let mut file = fs::File::open(&full_path)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let filename = full_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".into());

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        contents,
    )
        .into_response())
}
