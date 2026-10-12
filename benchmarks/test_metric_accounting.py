"""Synthetic receipt controls only; no provider, campaign or compiler calls."""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
import live_campaign_common as common


def load_campaign(folder):
    # CLI's established trusted sibling imports require its harness directory.
    sys.path.insert(0, str(ROOT / "cli-tokens-v1"))
    spec = importlib.util.spec_from_file_location("accounting_" + folder.replace("-", "_"), ROOT / folder / "codex_campaign.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class MetricAccountingTests(unittest.TestCase):
    def receipt(self, events):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "provider.jsonl"
            path.write_text("\n".join(json.dumps(event) for event in events) + "\n")
            return common.stream_usage(path, "model-a")

    def test_partial_message_buckets_stay_unknown_and_final_total_is_distinct(self):
        complete = {"input_tokens": 5, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 2, "output_tokens": 3}
        events = [{"type": "assistant", "message": {"id": "a", "model": "model-a", "usage": complete}},
                  {"type": "assistant", "message": {"id": "b", "model": "model-a", "usage": {"input_tokens": 7}}}]
        observed = self.receipt(events)
        self.assertEqual(observed["usage"]["input_tokens"], 12)
        self.assertIsNone(observed["usage"]["output_tokens"])
        self.assertIsNone(observed["usage"]["cache_read_input_tokens"])
        self.assertIsNone(observed["legacy_net_input"]["net_input_tokens"])
        self.assertIsNone(common.rate_card_estimate_details(observed["usage"])["usd"])
        final = {**complete, "input_tokens": 12, "output_tokens": 9}
        observed = self.receipt([*events, {"type": "result", "usage": final}])
        self.assertEqual(observed["usage"]["output_tokens"], 9)
        self.assertEqual(observed["provider_result_usage"]["output_tokens"], 9)
        self.assertIsNone(observed["turn_usage_by_message"][1]["usage"]["output_tokens"])
        self.assertIsNone(observed["fixed_context_tokens_in_this_session"])
        self.assertEqual(observed["result_event"]["usage"], final)

        # Missing entire usage and mixed message models remain independent controls.
        usage = {"input_tokens": 5, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0, "output_tokens": 3}
        events = [{"type": "assistant", "message": {"id": "a", "model": "model-a", "usage": usage}},
                  {"type": "assistant", "message": {"id": "b", "model": "model-b"}}]
        observed = self.receipt(events)
        self.assertEqual(observed["turns_with_usage"], 2)
        self.assertIsNone(observed["usage"]["input_tokens"])
        self.assertIsNone(observed["usage"]["output_tokens"])
        events[1]["message"]["usage"] = usage
        observed = self.receipt(events)
        self.assertEqual(observed["usage"]["input_tokens"], 10)
        self.assertEqual(observed["pricing_model_identities_observed"], ["model-a", "model-b"])
        self.assertEqual(observed["model_usage_keys_observed"], [])
        self.assertIsNone(common.rate_card_estimate_details(observed["usage"],
            model_usage_keys=observed["pricing_model_identities_observed"])["usd"])

    def test_tool_ids_do_not_replace_message_identity_or_duplicate_visible_output(self):
        def message(text, tools):
            return {"type": "assistant", "message": {"id": "message-a", "model": "model-a", "usage": {"output_tokens": 8},
                "content": [{"type": "text", "text": text}, *[{"type": "tool_use", "id": identity} for identity in tools]]}}
        observed = self.receipt([message("partial", ["tool-1"]), message("final", ["tool-1", "tool-2"])])
        self.assertEqual(observed["visible_output_bytes"], len("final\n".encode()))
        self.assertEqual(observed["turns_with_usage"], 1)
        self.assertEqual(observed["tool_use_events"], 2)
        self.assertEqual(observed["usage_updates_per_message"], {"message-a": 2})
        self.assertEqual(observed["provider_output_tokens"], 8)

    def test_multi_model_receipt_totals_all_models_without_single_model_billing(self):
        a = {"inputTokens": 2, "cacheCreationInputTokens": 0, "cacheReadInputTokens": 3, "outputTokens": 4}
        b = {"inputTokens": 5, "cacheCreationInputTokens": 1, "cacheReadInputTokens": 2, "outputTokens": 6}
        observed = self.receipt([{"type": "result", "modelUsage": {"model-a": a, "model-b": b}, "total_cost_usd": 0.25}])
        self.assertEqual(observed["usage"]["input_tokens"], 7)
        self.assertEqual(observed["usage"]["output_tokens"], 10)
        self.assertEqual(observed["model_usage_keys_observed"], ["model-a", "model-b"])
        self.assertEqual(observed["provider_reported_api_equivalent_total_cost_usd"], 0.25)
        estimate = common.rate_card_estimate_details(observed["usage"], model_usage_keys=observed["model_usage_keys_observed"])
        self.assertIsNone(estimate["usd"])
        authoritative = {"input_tokens": 8, "output_tokens": 11}
        observed = self.receipt([{"type": "result", "modelUsage": {"model-a": a, "model-b": b}, "usage": authoritative}])
        self.assertIsNone(observed["usage"]["input_tokens"])
        self.assertEqual(observed["final_total_discrepancies"]["input_tokens"], {"provider_result": 8, "all_model_usage": 7})
        self.assertEqual(observed["model_usage_totals"]["input_tokens"], 7)
        self.assertEqual(observed["result_event"]["modelUsage"], {"model-a": a, "model-b": b})

    def test_dated_saved_rate_card_prices_cache_subsets_and_missing_card_is_unknown(self):
        usage = {"input_tokens": 100, "cached_input_tokens": 30, "cache_write_input_tokens": 2, "output_tokens": 7, "reasoning_output_tokens": 3}
        card = {"date": "2020-01-02", "standard_short_context_usd_per_million": {"input": 10, "cache_read": 1, "cache_write": 20, "output": 100}}
        for folder in ("cli-tokens-v1", "webapp-tokens-v2"):
            with self.subTest(folder=folder):
                campaign = load_campaign(folder)
                estimate = campaign.list_price_estimate([{"usage": usage}], card)
                self.assertEqual(estimate["standard_short_context_api_equivalent_usd"], 0.00145)
                self.assertEqual(estimate["cache_write_cost_usd"], 0.00004)
                self.assertIsNone(estimate["actual_billed_usd"])
                for bad in ({}, {**card, "date": None}, {**card, "standard_short_context_usd_per_million": {"input": float("nan")}}):
                    self.assertIsNone(campaign.list_price_estimate([{"usage": usage}], bad)["standard_short_context_api_equivalent_usd"])
                self.assertIsNone(campaign.list_price_estimate([{"usage": {**usage, "cache_write_input_tokens": None}}], card)["standard_short_context_api_equivalent_usd"])


if __name__ == "__main__":
    unittest.main()
