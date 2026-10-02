# SPDX-License-Identifier: AGPL-3.0-or-later
"""hister - personal search engine results from a running hister server.

hister (https://github.com/asciimoo/hister) indexes pages you visit and
files you keep. Its search endpoint is ``GET {base}/search?q=...`` which
answers JSON shaped ``{"documents": [{...}], "page_key": ...}``.

Configure with ``base_url`` (the hister server, e.g. ``http://hister:4433``)
and ``token`` matching its ``app.access_token`` when authentication is on.
Both can come from the ``HISTER_API_URL`` and ``HISTER_TOKEN`` env vars so
containers do not need the token in settings.yml.
"""

import os
from urllib.parse import urlencode

about = {
    "website": 'https://github.com/asciimoo/hister',
    "official_api_documentation": 'https://hister.org/docs',
    "use_official_api": True,
    "require_api_key": False,
    "results": 'JSON',
}

categories = ['general']
paging = True
base_url = os.environ.get('HISTER_API_URL', '')
token = os.environ.get('HISTER_TOKEN', '')
timeout = 10.0


def request(query, params):
    if not base_url:
        return None
    params['url'] = f"{base_url}/search?{urlencode({'q': query})}"
    if token:
        params['headers']['X-Access-Token'] = token
    return params


def response(resp):
    docs = resp.json().get('documents') or []
    return [
        {
            'url': d['url'],
            'title': d.get('title') or d['url'],
            'content': d.get('text', '')[:400],
        }
        for d in docs
        if d.get('url')
    ]
