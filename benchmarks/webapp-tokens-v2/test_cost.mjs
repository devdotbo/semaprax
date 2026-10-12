import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const costScript = resolve(fileURLToPath(new URL(".", import.meta.url)), "cost.mjs");

function runCost(events) {
  const root = mkdtempSync(join(tmpdir(), "semaprax-token-cost-"));
  try {
    mkdirSync(join(root, "node_modules", "@anthropic-ai", "tokenizer"), { recursive: true });
    writeFileSync(
      join(root, "node_modules", "@anthropic-ai", "tokenizer", "index.js"),
      "exports.countTokens = (text) => text.length;\n",
    );
    const transcript = join(root, "transcript.jsonl");
    writeFileSync(transcript, events.map((event) => JSON.stringify(event)).join("\n") + "\n");
    const output = execFileSync("node", [costScript, "--tokenizer", root, `sample=${transcript}`], {
      encoding: "utf8",
    });
    return JSON.parse(output).runs.sample;
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("provider output and thinking counters stay separate from visible proxy and cost", () => {
  const run = runCost([{
    type: "assistant",
    message: {
      id: "turn-1",
      model: "claude-sonnet-5-5-20261001",
      usage: {
        input_tokens: 10,
        cache_creation_input_tokens: 2,
        cache_read_input_tokens: 3,
        output_tokens: 9000,
        thinking_tokens: 4000,
      },
      content: [{ type: "text", text: "a".repeat(5000) }],
    },
  }]);
  assert.equal(run.tokens.output_provider_reported, 9000);
  assert.equal(run.tokens.thinking_provider_reported, 4000);
  assert.equal(run.tokens.output_visible, 5001);
  assert.equal(run.usd.output_lower_bound, 0.05);
  assert.equal(run.usd.output_provider_usage_rate_card_estimate, 0.09);
  assert.notEqual(run.usd.output_provider_usage_rate_card_estimate, run.usd.output_lower_bound);
});

test("unavailable provider usage stays null instead of becoming a zero-cost bucket", () => {
  const run = runCost([{
    type: "assistant",
    message: {
      id: "turn-1",
      usage: { input_tokens: 10, output_tokens: 5 },
      content: [],
    },
  }]);
  assert.equal(run.tokens.input, 10);
  assert.equal(run.tokens.cache_write, null);
  assert.equal(run.tokens.output_provider_reported, 5);
  assert.equal(run.usd.input, null);
  assert.equal(run.usd.total_lower_bound, null);
});

function completeMessage(id, model, usage = {}) {
  return { type: "assistant", message: {
    id, model, usage: { input_tokens: 10, cache_creation_input_tokens: 2,
      cache_read_input_tokens: 3, output_tokens: 7, ...usage },
    content: [{ type: "text", text: "visible" }],
  } };
}

test("unknown and mixed models retain counters without borrowing the Sonnet rate card", () => {
  for (const events of [
    [completeMessage("a", "gpt-6.1-sol")],
    [completeMessage("a", "claude-sonnet-5-5"), completeMessage("b", "gpt-6.1-sol")],
    [completeMessage("a", "claude-sonnet-5-5"), completeMessage("a", "gpt-6.1-sol")],
  ]) {
    const run = runCost(events);
    assert.ok(run.tokens.input > 0);
    assert.ok(run.tokens.output_visible > 0);
    assert.equal(run.actual_billed_usd, null);
    for (const value of Object.values(run.usd)) assert.equal(value, null);
    assert.notEqual(run.pricing_status, "matched_archived_rate_card");
  }
});

test("an observed request without usage cannot vanish from a partial aggregate", () => {
  const run = runCost([completeMessage("a", "claude-sonnet-5-5"), {
    type: "assistant", message: { id: "b", model: "claude-sonnet-5-5", content: [] },
  }]);
  assert.equal(run.turns, 2);
  assert.equal(run.tokens.input, null);
  assert.equal(run.tokens.output_provider_reported, null);
  assert.equal(run.usd.input, null);
  assert.equal(run.usd.output_provider_usage_rate_card_estimate, null);
  assert.equal(run.usd.total_lower_bound, null);
});

test("split events bind one request model and merge its available counters once", () => {
  const run = runCost([{
    type: "assistant", message: { id: "a", model: "claude-sonnet-5-5", content: [] },
  }, completeMessage("a", undefined), completeMessage("a", undefined)]);
  assert.equal(run.turns, 1);
  assert.deepEqual(run.reported_models, ["claude-sonnet-5-5"]);
  assert.equal(run.pricing_status, "matched_archived_rate_card");
  assert.equal(run.tokens.input, 10);
  assert.equal(run.tokens.output_provider_reported, 7);
  assert.equal(run.tokens.output_visible, 8);
  assert.equal(run.actual_billed_usd, null);
});

test("unattributed assistant requests make aggregate counters and prices unavailable", () => {
  for (const event of [{ type: "assistant" }, completeMessage(undefined, "claude-sonnet-5-5")]) {
    const run = runCost([completeMessage("a", "claude-sonnet-5-5"), event]);
    assert.equal(run.turn_count_status, "incomplete");
    assert.equal(run.pricing_status, "unresolved_request_model");
    assert.equal(run.tokens.input, null);
    assert.equal(run.tokens.output_provider_reported, null);
    for (const value of Object.values(run.usd)) assert.equal(value, null);
  }
});
