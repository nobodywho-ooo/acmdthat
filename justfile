# acmdthat — dev tasks

default:
    @just --list

# Build everything (lib + acmdthat CLI + eval harness).
build:
    cargo build --bin acmdthat --bin eval

# Full candidate model sweep (downloads on first use; fast on a GPU).
eval:
    cargo build --bin eval
    ./target/debug/eval \
        --model qwen3.5-0.8b=hf://NobodyWho/Qwen_Qwen3.5-0.8B-GGUF/Qwen_Qwen3.5-0.8B-Q4_K_M-vendor-sampling.gguf \
        --model qwen3.5-2b=hf://NobodyWho/Qwen_Qwen3.5-2B-GGUF/Qwen_Qwen3.5-2B-Q4_K_M-vendor-sampling.gguf \
        --model qwen3.5-4b=hf://NobodyWho/Qwen_Qwen3.5-4B-GGUF/Qwen_Qwen3.5-4B-Q4_K_M-vendor-sampling.gguf \
        --model gemma4-e2b=hf://NobodyWho/Google_Gemma4-E2B-GGUF/gemma-4-E2B-it-Q4_K_M.gguf \
        --model gemma4-e4b=hf://NobodyWho/Google_Gemma4-E4B-GGUF/gemma-4-E4B-it-Q4_K_M.gguf \
        --model qwen3.5-9b=hf://unsloth/Qwen3.5-9B-GGUF/Qwen3.5-9B-Q4_K_M.gguf \
        --jsonl evals/results.jsonl

# Evaluate a single model: just eval-one "name=hf://…"
eval-one spec:
    cargo build --bin eval
    ./target/debug/eval --model {{spec}} --jsonl evals/results.jsonl

# A/B the prompt versions on one model.
eval-ab spec:
    cargo build --bin eval
    ./target/debug/eval --model {{spec}} --prompt v1 --jsonl evals/results.jsonl
    ./target/debug/eval --model {{spec}} --prompt v2 --jsonl evals/results.jsonl

test:
    cargo test
    cargo fmt --all -- --check
    cargo clippy --no-deps --all-targets -- -D warnings