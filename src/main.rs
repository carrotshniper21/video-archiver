mod models;
mod utils;

use axum::{
    body::StreamBody,
    extract::{DefaultBodyLimit, Multipart, Path},
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use http::HeaderMap;
use log::info;
use std::{net::SocketAddr, path::Path as FilePath, str::FromStr, time::Duration};
use tokio::{
    fs::{read_dir, remove_file, File},
    io::AsyncWriteExt,
};
use tokio_util::io::ReaderStream;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::Span;

async fn fallback_func() -> (StatusCode, Json<models::ResponseError>) {
    (
        StatusCode::NOT_FOUND,
        Json(models::ResponseError {
            message: String::new(),
            error: String::from("page not found"),
        }),
    )
}

async fn video_stream_handler(
    Path(file_name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let file_path = format!("./archive/{}", file_name);

    // Check if the file exists
    if !FilePath::new(&file_path).exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Open the video file
    let file = File::open(file_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Stream the file in chunks
    let stream = ReaderStream::new(file);
    let body = StreamBody::new(stream);

    // Set appropriate headers
    let mut response_headers = HeaderMap::new();
    response_headers.insert("Content-Type", "video/mp4".parse().unwrap());
    response_headers.insert("Accept-Ranges", "bytes".parse().unwrap());

    Ok((response_headers, body))
}

// Function to save the file in chunks
async fn save_file(
    field: &mut axum::extract::multipart::Field<'_>,
    file_path: &str,
) -> anyhow::Result<()> {
    let mut file = File::create(file_path).await?;

    while let Some(chunk) = field.chunk().await? {
        file.write_all(&chunk).await?;
    }

    Ok(())
}

async fn video_upload(mut multipart: Multipart) -> Result<(StatusCode, String), anyhow::Error> {
    // Create archive directory if it doesn't exist
    if !FilePath::new("./archive").exists() {
        tokio::fs::create_dir("./archive")
            .await
            .expect("Failed to create archive directory");
    }

    while let Some(mut field) = multipart.next_field().await? {
        let file_name = field
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Missing file name"))?
            .to_string();

        let file_path = format!("./archive/{}", file_name);
        info!("Uploading file: {}", file_name);

        // Attempt to save the file
        save_file(&mut field, &file_path)
            .await
            .expect(&format!("Error uploading file: {}", file_name));
        info!("File {} uploaded successfully", file_name);
    }

    // Return successful response
    Ok((StatusCode::OK, "Video uploaded successfully".to_string()))
}

async fn video_upload_handler(multipart: Multipart) -> impl IntoResponse {
    match video_upload(multipart).await {
        Ok(response) => response,
        Err(err) => {
            eprintln!("Error: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        }
    }
}

async fn archive_handler() -> Result<(StatusCode, Json<models::ArchiveResponse>), StatusCode> {
    std::fs::create_dir_all("./archive").expect("Failed to create archive directory!!");
    let mut file_names = vec![];

    // Read the directory contents
    match read_dir("./archive").await {
        Ok(mut entries) => {
            while let Some(entry) = entries.next_entry().await.unwrap() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    file_names.push(file_name);
                }
            }

            // If the directory is empty, file_names will remain empty.
            let response = models::ArchiveResponse { files: file_names };
            Ok((StatusCode::OK, Json(response)))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// New function to delete a file
async fn delete_file_handler(
    Path(file_name): Path<String>,
) -> Result<(StatusCode, String), StatusCode> {
    let file_path = format!("./archive/{}", file_name);

    // Check if the file exists
    if !FilePath::new(&file_path).exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Attempt to delete the file
    match remove_file(&file_path).await {
        Ok(_) => {
            info!("File {} deleted successfully", file_name);
            Ok((StatusCode::OK, "File deleted successfully".to_string()))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    utils::logger::initialize();

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_methods([Method::POST, Method::GET, Method::DELETE])
        .allow_headers(Any)
        .allow_origin(Any);

    // Create the Axum app
    let app = Router::new()
        .route("/upload", post(video_upload_handler))
        .route("/archive", get(archive_handler))
        .route("/stream/:file_name", get(video_stream_handler))
        .route("/delete/:file_name", delete(delete_file_handler)) // Add delete route
        .fallback(fallback_func)
        .layer(cors)
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024 * 1024)) // 20GB
        .layer(TraceLayer::new_for_http().on_response(
            |response: &http::Response<axum::body::BoxBody>, latency: Duration, span: &Span| {
                let status = response.status();

                // Log the response status and latency
                info!(
                    "Time: {:?}ms, Response Status: {}",
                    latency.as_millis(),
                    status.as_u16()
                );

                span.in_scope(|| {});
            },
        ));

    // Set the address to bind the server
    let addr = SocketAddr::from_str("0.0.0.0:8080").unwrap();
    info!("Server started on http://{}\n", addr);

    // Start the server
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
