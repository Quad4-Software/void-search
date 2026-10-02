use anyhow::Context;
use std::path::Path;
use std::sync::{Arc, RwLock};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, STRING, TEXT, FAST, INDEXED};
use tantivy::{doc, DocAddress, Index, IndexWriter, TantivyDocument};

/// tantivy index for crawled docs: bm25 on title+body with an authority
/// boost computed from inlinks, freshness weight on top
pub struct SearchIndex {
    pub index: Index,
    pub writer: Arc<RwLock<IndexWriter>>,
    pub url: Field,
    pub host: Field,
    pub title: Field,
    pub body: Field,
    pub description: Field,
    pub fetched_at: Field,
    pub authority: Field,
    pub inlinks: Field,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub url: String,
    pub host: String,
    pub title: String,
    pub description: String,
    pub score: f32,
}

impl SearchIndex {
    pub fn open(dir: &Path, heap_mb: usize) -> anyhow::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let mut schema = Schema::builder();
        let url = schema.add_text_field("url", STRING | STORED);
        let host = schema.add_text_field("host", STRING | STORED | FAST);
        let title = schema.add_text_field("title", TEXT | STORED);
        let body = schema.add_text_field("body", TEXT);
        let description = schema.add_text_field("description", TEXT | STORED);
        let fetched_at = schema.add_u64_field("fetched_at", STORED | FAST | INDEXED);
        let authority = schema.add_f64_field("authority", STORED | FAST);
        let inlinks = schema.add_u64_field("inlinks", STORED | FAST);
        let schema = schema.build();

        let index = Index::open_or_create(tantivy::directory::MmapDirectory::open(dir)?, schema)?;
        let writer = index.writer(heap_mb * 1024 * 1024)?;

        Ok(Self {
            index,
            writer: Arc::new(RwLock::new(writer)),
            url,
            host,
            title,
            body,
            description,
            fetched_at,
            authority,
            inlinks,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_doc(&self, url: &str, host: &str, title: &str, body: &str, desc: &str, fetched_at: u64, authority: f64, inlinks: u64) -> anyhow::Result<()> {
        let w = self.writer.write().unwrap();
        // replace by url to keep the index fresh on recrawl
        w.delete_term(tantivy::Term::from_field_text(self.url, url));
        w.add_document(doc!(
            self.url => url,
            self.host => host,
            self.title => title,
            self.body => body,
            self.description => desc,
            self.fetched_at => fetched_at,
            self.authority => authority,
            self.inlinks => inlinks,
        ))?;
        Ok(())
    }

    pub fn commit(&self) -> anyhow::Result<()> {
        self.writer.write().unwrap().commit()?;
        Ok(())
    }

    /// bm25 search across title (x4) and body (x1) with description (x2),
    /// then rerank top-k with authority and freshness weights
    pub fn search(&self, q: &str, limit: usize) -> anyhow::Result<Vec<SearchHit>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        let mut qp = QueryParser::for_index(&self.index, vec![self.title, self.body, self.description]);
        qp.set_field_boost(self.title, 4.0);
        qp.set_field_boost(self.description, 2.0);
        let query = qp.parse_query(q).context("parse query")?;

        let top_n = (limit * 4).max(50);
        let hits: Vec<(f32, DocAddress)> =
            searcher.search(&*query, &TopDocs::with_limit(top_n).order_by_score())?;
        let mut out: Vec<SearchHit> = Vec::with_capacity(hits.len());
        let now = now_epoch();
        for (bm25, addr) in hits {
            let doc: TantivyDocument = searcher.doc(addr)?;
            let url = doc.get_first(self.url).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let host = doc.get_first(self.host).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let title = doc.get_first(self.title).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let description = doc.get_first(self.description).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let authority = doc.get_first(self.authority).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let fetched = doc.get_first(self.fetched_at).and_then(|v| v.as_u64()).unwrap_or(now);

            // authority boost: inlink-derived, log-scaled, modest weight
            let authority_boost = 1.0 + authority.min(10.0) * 0.15;
            // freshness decay: pages older than a year get slight penalty
            let age_days = now.saturating_sub(fetched) as f64 / 86400.0;
            let freshness = 1.0 / (1.0 + (age_days / 365.0).max(0.0) * 0.2);

            let score = bm25 * authority_boost as f32 * freshness as f32;
            out.push(SearchHit { url, host, title, description, score });
        }
        out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(limit);
        Ok(out)
    }

    pub fn num_docs(&self) -> u64 {
        self.index.reader().ok().map(|r| r.searcher().num_docs()).unwrap_or(0)
    }
}

pub fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
