# Eval results — model & prompt comparison

Run 2026-11-16 (session 6/7), `evals/cases.toml` (15 cases), prompt **v2**
(v1 A/B'd on Qwen3.5-2B). All rows below are the *last complete run per
(model, prompt)*, on Asbjørn's GPU (RX 7700S + 780M, Vulkan, Q4_K_M quants,
ctx 4096, T=0.2, seed default). Raw per-case rows: `results.jsonl`
(append-only, gitignored — includes the earlier CPU-blocked runs).

## Prompt A/B (Qwen3.5-2B, GPU)

| prompt | exact | program | errors | flags P | flags R | median |
|---|---|---|---|---|---|---|
| v1 (rule-list) | 2/15 | 6/15 | 1 | 0.38 | 0.57 | 1.3s |
| **v2 (+few-shot, ambiguity, quoting)** | **4/15** | **9/15** | **0** | **0.64** | **0.77** | 1.0s |

v2 wins on every metric at equal latency → v2 stays the CLI default.

## Model comparison (prompt v2)

| model | size | download | exact | program | errors | flags P | flags R | median |
|---|---|---|---|---|---|---|---|---|
| **gemma4-e4b** | E4B | 4.98 GB | **8/15** | 12/15 | 0 | 0.68 | 0.80 | 1.4s |
| qwen3.5-9b | 9B | 5.68 GB | 5/15 | **13/15** | 0 | 0.73 | 0.87 | 3.3s |
| qwen3.5-4b | 4B | 2.74 GB | 5/15 | 12/15 | 0 | 0.63 | 0.87 | 2.1s |
| gemma4-e2b | E2B | 3.11 GB | 5/15 | 10/15 | 1 | 0.60 | 0.70 | 0.8s |
| qwen3.5-2b | 2B | 1.28 GB | 4/15 | 9/15 | 0 | 0.64 | 0.77 | 1.0s |
| qwen3.5-0.8b | 0.8B | 0.53 GB | 2/15 | 9/15 | 2 | 0.42 | 0.61 | 0.5s |

## Reading the table (honest caveats)

- **Exact-match undercounts** functionally-equivalent answers — e.g. every
  Qwen's `JOBS=4 cargo build` ≡ reference `env JOBS=4 cargo build`,
  `tar … notes` ≡ `… notes/`, `awk -F','` ≡ `awk -F,`. Program+flags is the
  honest headline metric; exact is the strictest.
- **Single sample per case** at T=0.2 — deltas of ±1–2 cases are noise; needs
  3× repeats for confidence (cheap on GPU now — TODO in harness).
- The suite is 15 cases I wrote; it is indicative, not gospel.

## Notable per-case findings

- `tail-follow` ("last 50 lines AND keep following"): **only E4B** produced the
  exact `tail -n 50 -f app.log`. Others dropped `-f` or invented watch-loops.
- `ss-ports`: nobody exact; most added `| awk` pipelines. E4B's answer added
  **`sudo`** — unnecessary elevation (our risk gate would flag it HIGH).
- `ffmpeg-thumb`: **both Gemmas chose ImageMagick `convert`**, every Qwen chose
  `ffmpeg`. v2's tool list ("rg, fd, jq, awk") dropped ffmpeg from the v1
  examples — prompt-sensitivity finding, worth a v3 tweak or more cases.
- `ffmpeg-thumb`: **all six models** chose ImageMagick `convert` over `ffmpeg` —
  v2's tool examples dropped ffmpeg (v1 listed it, and the v1 2B run chose
  `ffmpeg`): the tool list demonstrably drives tool choice. Strongest
  prompt-sensitivity finding; v3 restores ffmpeg.
- `jq-field`: exact for 5/6 models (all but 2B, which wrapped it in a useless
  `cat |`).
- Rambling (ctx-shift error) = small-model failure mode on JSON-heavy cases:
  0.8B ×2, E2B ×1; 4B/9B/E4B never.
- Gemma4 "thinking" markers didn't bite — clean output across both sizes.

## Current standing

- **Quality/speed winner on this hardware: Gemma4-E4B** — best exact, tied-best
  program, 0 errors, 1.4s median. (Also: 4.98 GB fits the 8 GB GPU with ctx.)
- **Size-conscious pick: Qwen3.5-4B** — near-equal program score at 55% of the
  download and slightly lower latency.
- 9B (current default): best program score but 2.4× E4B latency and no exact
  advantage → likely demote after repeat-runs confirm.

## Next

1. `--repeat N` in the eval harness; 3× runs of E4B / 4B / 9B to confirm before
   switching `DEFAULT_MODEL`.
2. Prompt v3 candidates: restore ffmpeg to tool examples (or add image cases);
   discourage unnecessary sudo/pipeline decoration.
3. Grow the suite (target ~30 cases incl. fish-specific syntax cases).