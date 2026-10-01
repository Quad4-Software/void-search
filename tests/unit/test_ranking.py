# SPDX-License-Identifier: AGPL-3.0-or-later
# pylint: disable=missing-class-docstring,invalid-name
"""Tests for Void result ranking."""

from searx.ranking import calculate_score, rank_results, relevance_multiplier, tokenize_query
from tests import SearxTestCase


class _DictResult(dict):
    def __getattr__(self, name):
        try:
            return self[name]
        except KeyError as exc:
            raise AttributeError(name) from exc

    def __setattr__(self, name, value):
        self[name] = value


class RankingTestCase(SearxTestCase):
    TEST_SETTINGS = "test_result_container.yml"

    def test_tokenize_drops_stopwords(self):
        tokens = tokenize_query("the rust programming language")
        self.assertEqual(tokens, ["rust", "programming", "language"])

    def test_https_and_authority_outrank_seo_http(self):
        wiki = _DictResult(
            url="https://en.wikipedia.org/wiki/Reticulum",
            title="Reticulum (network stack)",
            content="Reticulum is a cryptography-based networking stack.",
            engines={"wikipedia", "duckduckgo"},
            positions=[1, 2],
            priority=None,
            parsed_url=None,
            publishedDate=None,
        )
        farm = _DictResult(
            url="http://pinterest.com/pin/reticulum-network",
            title="Reticulum network ideas",
            content="Pin this reticulum network stack guide.",
            engines={"duckduckgo"},
            positions=[1],
            priority=None,
            parsed_url=None,
            publishedDate=None,
        )
        ranked = rank_results([farm, wiki], query="reticulum network stack")
        self.assertEqual(ranked[0].url, wiki["url"])
        self.assertGreater(ranked[0].score, ranked[1].score)

    def test_title_match_beats_unrelated_https(self):
        related = _DictResult(
            url="https://example.org/searxng-fork",
            title="Void SearXNG fork ranking",
            content="A privacy metasearch fork.",
            engines={"brave"},
            positions=[4],
            priority=None,
            parsed_url=None,
            publishedDate=None,
        )
        unrelated = _DictResult(
            url="https://example.org/cooking",
            title="Best pasta recipes",
            content="Tomato sauce and basil.",
            engines={"brave"},
            positions=[1],
            priority=None,
            parsed_url=None,
            publishedDate=None,
        )
        ranked = rank_results([unrelated, related], query="void searxng fork", lang="en")
        self.assertEqual(ranked[0]["title"], related["title"])

    def test_low_priority_stays_zero(self):
        result = _DictResult(
            url="https://example.org",
            title="Example",
            content="",
            engines={"duckduckgo"},
            positions=[1],
            priority="low",
            parsed_url=None,
            publishedDate=None,
        )
        self.assertEqual(calculate_score(result, "low", query="example"), 0.0)

    def test_relevance_prefers_exact_title(self):
        exact = relevance_multiplier(
            {"title": "Mesh networking", "content": "", "url": "https://quad4.io/mesh"},
            "mesh networking",
        )
        loose = relevance_multiplier(
            {"title": "Random post", "content": "mentions mesh once", "url": "https://example.net/a/b/c/d/e/f/g"},
            "mesh networking",
        )
        self.assertGreater(exact, loose)

    def test_word_boundary_does_not_match_substring(self):
        from searx.ranking import _coverage  # pylint: disable=import-outside-toplevel

        self.assertEqual(_coverage("trust no one", ["rust"]), 0.0)
        self.assertEqual(_coverage("the rust book", ["rust"]), 1.0)

    def test_navigational_official_host_wins(self):
        official = _DictResult(
            url="https://github.com/",
            title="GitHub",
            content="GitHub is where people build software.",
            engines={"brave"},
            positions=[3],
            priority=None,
            parsed_url=None,
            publishedDate=None,
        )
        blog = _DictResult(
            url="https://example.net/best-github-tips",
            title="Best GitHub tips for 2026",
            content="A list of github tricks.",
            engines={"brave", "bing"},
            positions=[1, 1],
            priority=None,
            parsed_url=None,
            publishedDate=None,
        )
        ranked = rank_results([blog, official], query="github", lang="en")
        self.assertEqual(ranked[0]["url"], official["url"])
