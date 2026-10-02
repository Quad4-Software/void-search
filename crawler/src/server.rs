use crate::index::SearchIndex;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub index: Arc<SearchIndex>,
    pub doc_count: Arc<std::sync::atomic::AtomicU64>,
}

#[derive(Deserialize)]
pub struct SearchParams {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

fn default_limit() -> usize {
    20
}

#[derive(Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<crate::index::SearchHit>,
    pub took_ms: u64,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub docs: u64,
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub docs: u64,
    pub index_docs: u64,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/search", get(search))
        .route("/healthz", get(healthz))
        .route("/stats", get(stats))
        .with_state(state)
}

async fn search(State(s): State<AppState>, Query(p): Query<SearchParams>) -> Result<Json<SearchResponse>, StatusCode> {
    if p.q.trim().is_empty() || p.q.len() > 512 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let limit = p.limit.clamp(1, 100);
    let offset = p.offset.min(10_000);
    let started = std::time::Instant::now();
    let results = s
        .index
        .search(&p.q, limit + offset)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .skip(offset)
        .collect();
    Ok(Json(SearchResponse {
        query: p.q,
        results,
        took_ms: started.elapsed().as_millis() as u64,
    }))
}

async fn healthz(State(s): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        docs: s.doc_count.load(std::sync::atomic::Ordering::Relaxed),
    })
}

async fn stats(State(s): State<AppState>) -> Json<StatsResponse> {
    Json(StatsResponse {
        docs: s.doc_count.load(std::sync::atomic::Ordering::Relaxed),
        index_docs: s.index.num_docs(),
    })
}

pub async fn serve(state: AppState, listen: &str) -> anyhow::Result<()> {
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(listen).await?;
    info!(listen, "search api up");
    axum::serve(listener, app).await?;
    Ok(())
}
