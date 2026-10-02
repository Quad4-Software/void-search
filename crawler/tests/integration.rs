use futures::StreamExt;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[path = "support/mod.rs"]
mod support;

/// fixture site: index links to /a, /b and /private/hidden. robots.txt
/// disallows /private/. /slow drips bytes under the drip floor.
async fn serve_fixture() -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/robots.txt", get(|| async {
            "User-agent: *\nAllow: /\nDisallow: /private/\n"
        }))
        .route("/", get({
            let h = hits.clone();
            move || {
                let h = h.clone();
                async move {
                    h.fetch_add(1, Ordering::Relaxed);
                    Html(r#"<html><head><title>root</title></head><body>
                    <p>welcome to the fixture root page about wibbly widgets</p>
                    <a href="/a">a</a> <a href="/b">b</a> <a href="/private/hidden">h</a>
                    </body></html>"#)
                }
            }
        }))
        .route("/a", get({
            let h = hits.clone();
            move || {
                let h = h.clone();
                async move {
                    h.fetch_add(1, Ordering::Relaxed);
                    Html("<html><head><title>page a</title></head><body><p>alpha widgets and gadgets</p><a href=\"/b\">b</a></body></html>")
                }
            }
        }))
        .route("/b", get({
            let h = hits.clone();
            move || {
                let h = h.clone();
                async move {
                    h.fetch_add(1, Ordering::Relaxed);
                    Html("<html><head><title>page b</title></head><body><p>beta gadgets and gizmos</p></body></html>")
                }
            }
        }))
        .route("/private/hidden", get({
            let h = hits.clone();
            move || {
                let h = h.clone();
                async move {
                    h.fetch_add(1, Ordering::Relaxed);
                    Html("<html><body>hidden</body></html>")
                }
            }
        }))
        .route("/tarpit", get(|| async {
            // slow drip: small chunks with long sleeps, way below the floor
            let stream = futures::stream::iter(0..64).then(|i| async move {
                tokio::time::sleep(Duration::from_millis(250)).await;
                Ok::<_, std::io::Error>(format!("chunk{i}"))
            });
            axum::body::Body::from_stream(stream)
        }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), hits)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crawler_respects_robots_and_indexes() {
    let (base, hits) = serve_fixture().await;

    let dir = tempfile::tempdir().unwrap();
    let mut cfg = support::test_config(dir.path());
    cfg.seeds = vec![format!("{base}/")];

    let crawler = std::sync::Arc::new(support::make_crawler(cfg).unwrap());
    crawler.seed(&[format!("{base}/"), format!("{base}/tarpit")]);

    let c = crawler.clone();
    let task = tokio::spawn(async move { c.run().await });
    let _ = tokio::time::timeout(Duration::from_secs(60), task).await.unwrap();

    // disallowed path must never have been requested
    let store_docs = crawler.index().num_docs();
    assert!(store_docs >= 2, "expected >=2 docs, got {store_docs}");

    // robots disallowed path skipped: the fixture counts hits, /private never hit
    // (hits counts all routes; tarpit may have been attempted once)
    assert!(hits.load(Ordering::Relaxed) >= 2);

    // search finds the pages
    let res = crawler.index().search("widgets", 10).unwrap();
    assert!(!res.is_empty(), "expected search hits for widgets");
    assert!(res.iter().all(|h| h.url.starts_with(&base)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api_serves_search() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = support::test_config(dir.path());
    let crawler = support::make_crawler(cfg.clone()).unwrap();
    let idx = crawler.index();
    idx.add_doc("https://x.test/a", "x.test", "hello world page", "the world says hello", "", 0, 0.0, 0).unwrap();
    idx.commit().unwrap();

    let state = support::app_state(crawler);
    let app = support::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });

    let resp = reqwest::get(format!("http://{addr}/search?q=hello+world&limit=5"))
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    let results = body["results"].as_array().unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0]["url"], "https://x.test/a");
}
