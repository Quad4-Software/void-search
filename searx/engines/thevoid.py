# SPDX-License-Identifier: AGPL-3.0-or-later
"""thevoid - the Void Search independent web index, built by the bundled
void-crawler (see ``crawler/`` in this repository).

The crawler is a respectful Rust crawler: robots.txt compliant with
crawl-delay, per-host politeness, blocklists, tarpit protection, native
Anubis proof-of-work solving and optional FlareSolverr support. Results are
ranked by tantivy BM25 blended with crawl-derived authority and freshness.

Configure with ``base_url`` pointing at the crawler's search API, for
example ``http://void-crawler:8088`` in the compose stack or
``http://127.0.0.1:8088`` when run on the host.
"""

import os
from urllib.parse import urlencode

about = {
    "website": 'https://github.com/Quad4-Software/void-search',
    "official_api_documentation": 'https://github.com/Quad4-Software/void-search',
    "use_official_api": True,
    "require_api_key": False,
    "results": 'JSON',
}

categories = ['general']
paging = True
base_url = os.environ.get('THEVOID_API_URL', 'http://127.0.0.1:8088')
results_per_page = 20
timeout = 10.0


def request(query, params):
    offset = results_per_page * (params['pageno'] - 1)
    params['url'] = f"{base_url}/search?{urlencode({'q': query, 'limit': results_per_page, 'offset': offset})}"
    return params


def response(resp):
    return [
        {
            'url': r['url'],
            'title': r.get('title') or r['url'],
            'content': r.get('description', ''),
        }
        for r in resp.json().get('results', [])
    ]
