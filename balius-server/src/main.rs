use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    routing::{get, post},
};
use clap::Parser;
use config::{AppConfig, Args};
use dashmap::DashMap;
use error::{ApiError, ApiResult};
use include_dir::{Dir, include_dir};
use pallas_crypto::key::ed25519::SecretKey;
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, sync::Mutex};
use tracing::info;
use worker::{Worker, WorkerService};

mod config;
mod error;
mod worker;

async fn hello_world() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

async fn create_project(
    State(AppState { projects, .. }): State<AppState>,
    Json(req): Json<CreateProjectRequest>,
) -> ApiResult<Json<CreateProjectResponse>> {
    let new_id = (projects.len() + 1).to_string();
    projects.insert(new_id.clone(), ProjectState {});
    Ok(Json(CreateProjectResponse {
        id: new_id.clone(),
        name: req.name,
        namespace: "idk".to_string(),
    }))
}

#[derive(Deserialize)]
struct CreateProjectRequest {
    name: String,
}

#[derive(Serialize)]
struct CreateProjectResponse {
    id: String,
    name: String,
    namespace: String,
}

async fn create_resource(
    State(AppState {
        workers,
        worker_service,
        ..
    }): State<AppState>,
    Json(req): Json<CreateResourceRequest>,
) -> ApiResult<Json<CreateResourceResponse>> {
    let mut public_keys = HashMap::new();

    let key = SecretKey::new(rand::thread_rng());
    public_keys.insert("default".to_string(), key.public_key().to_string());

    let keys = vec![("default", key)];

    let worker = worker_service
        .lock()
        .await
        .create_worker(&req.spec, keys)
        .await?;
    let id = worker.id.clone();
    workers.insert(id.clone(), worker);

    Ok(Json(CreateResourceResponse {
        id: id.clone(),
        name: id,
        kind: req.kind,
        public_keys,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateResourceRequest {
    #[allow(unused)]
    project_id: String,
    kind: ResourceKind,
    spec: String,
}

#[derive(Serialize, Deserialize)]
enum ResourceKind {
    BaliusWorker,
}

#[derive(Serialize)]
struct CreateResourceResponse {
    id: String,
    name: String,
    kind: ResourceKind,
    public_keys: HashMap<String, String>,
}

async fn invoke_worker(
    State(AppState { workers, .. }): State<AppState>,
    Path(worker_id): Path<String>,
    Json(req): Json<InvokeWorkerRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let Some(mut worker) = workers.get_mut(&worker_id) else {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "worker not found"));
    };
    let response = worker.invoke(&req.method, &req.params).await?;
    Ok(Json(response))
}

#[derive(Serialize, Deserialize)]
struct InvokeWorkerRequest {
    method: String,
    params: serde_json::Value,
}

static PRECOMPILED_WORKERS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/workers");

async fn serve_worker(Path(filename): Path<String>) -> ApiResult<Vec<u8>> {
    let Some(worker) = PRECOMPILED_WORKERS.get_file(&filename) else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!("worker {filename} not found"),
        ));
    };
    Ok(worker.contents().to_vec())
}

#[derive(Clone)]
struct AppState {
    projects: Arc<DashMap<String, ProjectState>>,
    workers: Arc<DashMap<String, Worker>>,
    worker_service: Arc<Mutex<WorkerService>>,
}

struct ProjectState {}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::try_parse()?;
    let config = AppConfig::load(args)?;
    tracing_subscriber::fmt::init();
    info!("Hello, world!");

    let mut predefined_workers = HashMap::new();
    for file in PRECOMPILED_WORKERS.files() {
        let path = format!("/workers/{}", file.path().to_string_lossy());
        predefined_workers.insert(path, file.contents().to_vec());
    }

    let state = AppState {
        projects: Arc::new(DashMap::new()),
        workers: Arc::new(DashMap::new()),
        worker_service: Arc::new(Mutex::new(WorkerService::new(
            config.clone(),
            predefined_workers,
        )?)),
    };

    let app = Router::new()
        .route("/", get(hello_world))
        .route("/projects", post(create_project))
        .route("/resources", post(create_resource))
        .route("/worker/{workerId}", post(invoke_worker))
        .route("/workers/{filename}", get(serve_worker))
        .with_state(state);

    let listener = TcpListener::bind(("0.0.0.0", config.port)).await?;
    info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}
