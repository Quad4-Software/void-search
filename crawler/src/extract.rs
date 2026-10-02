use scraper::{Html, Selector};
use std::sync::OnceLock;
use url::Url;

static SEL_A: OnceLock<Selector> = OnceLock::new();
static SEL_TITLE: OnceLock<Selector> = OnceLock::new();
static SEL_META_ROBOTS: OnceLock<Selector> = OnceLock::new();
static SEL_META_DESC: OnceLock<Selector> = OnceLock::new();
static SEL_DROP: OnceLock<Selector> = OnceLock::new();

#[allow(dead_code)]
pub struct Extracted {
    pub title: String,
    pub description: String,
    pub text: String,
    pub links: Vec<String>,
    pub noindex: bool,
    pub nofollow: bool,
}

/// pull title, meta description, visible text and absolute links out of html.
/// meta robots noindex/nofollow honored.
pub fn extract(body: &str, base: &Url) -> Extracted {
    let sel_a = SEL_A.get_or_init(|| Selector::parse("a[href]").unwrap());
    let sel_title = SEL_TITLE.get_or_init(|| Selector::parse("title").unwrap());
    let sel_meta_robots =
        SEL_META_ROBOTS.get_or_init(|| Selector::parse("meta[name=robots]").unwrap());
    let sel_meta_desc =
        SEL_META_DESC.get_or_init(|| Selector::parse("meta[name=description]").unwrap());
    let sel_drop = SEL_DROP.get_or_init(|| {
        Selector::parse(
            "script, style, noscript, template, iframe, svg, nav, footer, header, form, aside",
        )
        .unwrap()
    });

    let doc = Html::parse_document(body);

    let title = doc
        .select(sel_title)
        .next()
        .map(|t| t.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    let description = doc
        .select(sel_meta_desc)
        .next()
        .and_then(|m| m.value().attr("content").map(|s| s.trim().to_string()))
        .unwrap_or_default();

    let mut noindex = false;
    let mut nofollow = false;
    if let Some(m) = doc.select(sel_meta_robots).next()
        && let Some(content) = m.value().attr("content")
    {
        let c = content.to_lowercase();
        noindex = c.contains("noindex") || c.contains("none");
        nofollow = c.contains("nofollow");
    }

    let links: Vec<String> = if nofollow {
        Vec::new()
    } else {
        doc.select(sel_a)
            .filter_map(|a| a.value().attr("href"))
            .filter_map(|href| base.join(href).ok().map(|u| u.to_string()))
            .collect()
    };

    // text extraction: walk the tree, skip script/style/nav subtrees
    let mut text = String::with_capacity(body.len() / 4);
    let drop_ids: std::collections::HashSet<ego_tree::NodeId> =
        doc.select(sel_drop).map(|e| e.id()).collect();
    for node in doc.tree.root().descendants() {
        if let Some(t) = node.value().as_text() {
            // skip text nodes inside dropped elements
            let mut skip = false;
            let mut cur = node.parent();
            while let Some(p) = cur {
                if drop_ids.contains(&p.id()) {
                    skip = true;
                    break;
                }
                cur = p.parent();
            }
            if !skip {
                let s = t.trim();
                if !s.is_empty() {
                    text.push_str(s);
                    text.push(' ');
                }
            }
        }
    }

    // cap stored text - pages can be huge, ranking only needs the front
    if text.len() > 64 * 1024 {
        text.truncate(64 * 1024);
    }

    Extracted {
        title,
        description,
        text,
        links,
        noindex,
        nofollow,
    }
}

/// pull item/entry links out of an rss or atom feed body. returns empty for
/// non-feeds. feeds themselves are not indexed, just mined for urls.
pub fn feed_links(body: &str) -> Vec<String> {
    let head = &body[..body.len().min(8192)];
    let is_feed = head.contains("<rss") || head.contains("<feed") || head.contains("<rdf:RDF");
    if !is_feed {
        return Vec::new();
    }
    let mut links = Vec::new();
    // rss: <link>https://x</link> inside <item>; atom: <link href="..."/>
    let mut rest = body;
    while let Some(i) = rest.find("<link>") {
        let after = &rest[i + 6..];
        if let Some(j) = after.find("</link>") {
            let l = after[..j].trim();
            if l.starts_with("http") {
                links.push(l.to_string());
            }
            rest = &after[j + 7..];
        } else {
            break;
        }
    }
    for cap in rest.split("<link").skip(1) {
        if let Some(a) = cap.find("href=") {
            let q = &cap[a + 5..];
            let end = q.find('"').or_else(|| q.find('\''));
            let start = if q.starts_with('"') || q.starts_with('\'') {
                1
            } else {
                0
            };
            if let Some(e) = end {
                let l = q[start..e].trim();
                if l.starts_with("http") {
                    links.push(l.to_string());
                }
            }
        }
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_basics() {
        let base = Url::parse("https://x.test/dir/").unwrap();
        let html = r#"<html><head><title>Hello</title>
        <meta name="description" content="desc">
        </head><body><p>Visible text</p><script>bad()</script>
        <a href="/a">link</a><a href="rel2">link2</a></body></html>"#;
        let e = extract(html, &base);
        assert_eq!(e.title, "Hello");
        assert_eq!(e.description, "desc");
        assert!(e.text.contains("Visible text"));
        assert!(!e.text.contains("bad()"));
        assert!(e.links.iter().any(|l| l == "https://x.test/a"));
        assert!(e.links.iter().any(|l| l == "https://x.test/dir/rel2"));
        assert!(!e.noindex);
    }

    #[test]
    fn respects_nofollow() {
        let base = Url::parse("https://x.test/").unwrap();
        let html = r#"<html><head><meta name="robots" content="noindex,nofollow"></head>
        <body><a href="/x">l</a></body></html>"#;
        let e = extract(html, &base);
        assert!(e.noindex);
        assert!(e.links.is_empty());
    }
}
