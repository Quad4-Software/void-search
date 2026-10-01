# SPDX-License-Identifier: AGPL-3.0-or-later
"""Tests for Void engine policy and result cache keys."""

from searx.result_cache import cache_key
from searx.search.models import EngineRef, SearchQuery
from searx.void_policy import apply_engine_policy
from tests import SearxTestCase


class VoidPolicyTestCase(SearxTestCase):
    TEST_SETTINGS = "test_result_container.yml"

    def test_disables_block_prone_engines(self):
        settings = {
            "engines": [
                {"name": "google", "timeout": 3.0},
                {"name": "duckduckgo", "timeout": 3.0},
                {"name": "bing", "timeout": 3.0},
                {"name": "slow scraper", "timeout": 20.0},
            ]
        }
        apply_engine_policy(settings)
        by_name = {item["name"]: item for item in settings["engines"]}
        self.assertTrue(by_name["google"]["disabled"])
        self.assertTrue(by_name["duckduckgo"]["disabled"])
        self.assertTrue(by_name["slow scraper"]["disabled"])
        self.assertFalse(by_name["bing"].get("disabled"))

    def test_disables_brave(self):
        settings = {"engines": [{"name": "brave", "timeout": 3.0}]}
        apply_engine_policy(settings)
        self.assertTrue(settings["engines"][0]["disabled"])

    def test_strips_client_headers(self):
        from searx.void_policy import anonymize_outgoing_headers

        headers = {
            "Accept-Language": "de-DE,de;q=0.9",
            "X-Forwarded-For": "1.2.3.4",
            "X-Real-IP": "1.2.3.4",
            "Referer": "https://example.test/",
            "Cookie": "sid=abc",
            "User-Agent": "SearXNG/1.0 leaked",
        }
        anonymize_outgoing_headers(headers)
        self.assertNotIn("X-Forwarded-For", headers)
        self.assertNotIn("X-Real-IP", headers)
        self.assertNotIn("Referer", headers)
        self.assertNotIn("Cookie", headers)
        self.assertFalse(headers.get("User-Agent", "").startswith("SearXNG/"))
        self.assertEqual(headers["Accept-Language"], "de-DE,de;q=0.9")

    def test_blocklist_matches_subdomain(self):
        from searx.void_blocklist import is_blocked_url

        self.assertTrue(is_blocked_url("https://www.pinterest.com/pin/1"))
        self.assertFalse(is_blocked_url("https://en.wikipedia.org/wiki/X"))

    def test_english_search_drops_cyrillic(self):
        from searx.ranking import is_foreign_language

        foreign = {
            "title": "Сеть Ретикулум документация",
            "content": "Криптографический сетевой стек для автономных сетей",
            "url": "https://example.ru/reticulum",
        }
        english = {
            "title": "Reticulum network stack",
            "content": "A cryptography based networking stack",
            "url": "https://reticulum.network/",
        }
        self.assertTrue(is_foreign_language(foreign, "en"))
        self.assertFalse(is_foreign_language(english, "en"))
        self.assertFalse(is_foreign_language(foreign, "all"))

    def test_logs_never_print_query_or_ip(self):
        import logging

        from searx.void_logging import NoQueryFilter

        record = logging.LogRecord(
            "searx.webapp",
            logging.INFO,
            __file__,
            1,
            "GET /search?q=secret-query&language=en from 203.0.113.9",
            (),
            None,
        )
        filt = NoQueryFilter()
        self.assertTrue(filt.filter(record))
        message = record.getMessage()
        self.assertNotIn("secret-query", message)
        self.assertNotIn("203.0.113.9", message)

    def test_cache_key_ignores_client_identity(self):
        refs = [EngineRef("bing", "general")]
        left = SearchQuery("reticulum", refs, lang="en", safesearch=0, pageno=1)
        right = SearchQuery("reticulum", refs, lang="en", safesearch=0, pageno=1)
        self.assertEqual(cache_key(left), cache_key(right))
        other = SearchQuery("other", refs, lang="en", safesearch=0, pageno=1)
        self.assertNotEqual(cache_key(left), cache_key(other))
