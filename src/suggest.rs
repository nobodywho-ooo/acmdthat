//! Suggestion generation: schema-grammar-constrained output from NobodyWho.

use crate::context::EnvContext;
use nobodywho::chat::ChatHandle;
use nobodywho::miette;
use nobodywho::sampler::{GrammarStep, SampleStep, SamplerBuilder, SamplerConfig, ShiftStep};
use serde::{Deserialize, Serialize};

/// JSON schema the model's output is grammar-constrained to (llguidance).
///
/// `maxItems: 1` for M1: exactly one command, no picker yet. M2 widens this to
/// 1–3 with a picker UI; the grammar keeps the same shape either way.
pub const SUGGESTION_SCHEMA: &str = r##"{
    "type": "object",
    "properties": {
        "commands": {
            "type": "array",
            "minItems": 1,
            "maxItems": 1,
            "items": {
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "explanation": { "type": "string" }
                },
                "required": ["command", "explanation"],
                "additionalProperties": false
            }
        }
    },
    "required": ["commands"],
    "additionalProperties": false
}"##;

/// What the model returns: guaranteed parseable by the grammar constraint.
#[derive(Debug, Serialize, Deserialize)]
pub struct Suggestion {
    pub commands: Vec<CommandSuggestion>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandSuggestion {
    pub command: String,
    pub explanation: String,
}

/// Grammar + sampling chain: JSON-schema constraint first (grammar masks before
/// shifts truncate), then default top-k/top-p, low temperature, dist sample.
pub fn sampler() -> SamplerConfig {
    SamplerBuilder::new()
        .shift(ShiftStep::TopK { top_k: 20 })
        .shift(ShiftStep::TopP {
            top_p: 0.95,
            min_keep: 1,
        })
        .shift(ShiftStep::Temperature { temperature: 0.2 })
        .add_grammar(GrammarStep::JsonSchema(SUGGESTION_SCHEMA.to_string()))
        .sample(SampleStep::Dist)
}

pub fn system_prompt(ctx: &EnvContext) -> String {
    let entries = if ctx.entries.is_empty() {
        "(empty or unreadable)".to_string()
    } else {
        ctx.entries
            .iter()
            .map(|e| format!("  {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "You are the command generator for acmdthat, a terminal assistant. \
Convert the user's request into exactly one {shell} command line to run on {os} \
from the working directory {cwd}.\n\
Rules:\n\
- One single command line, valid {shell}, ready to run as-is: no markdown, no \
backticks, no leading '$'.\n\
- Prefer widely available modern tools (rg, fd, jq, awk, sed, ffmpeg, …) when \
they fit the request.\n\
- Use paths relative to the working directory unless the user asks otherwise.\n\
- Multiple steps may be joined with '&&' (only when a later step depends on an \
earlier one) or ';' otherwise. Never more than one line.\n\
- explanation: one short sentence saying what the command does and its key flags.\n\
\n\
Working directory contains:\n{entries}",
        shell = ctx.shell,
        os = ctx.os,
        cwd = ctx.cwd_display(),
    )
}

/// Stream a suggestion for `query`, reporting each token piece to `on_token`
/// with the running token count. Returns the raw model output (a JSON object,
/// by construction of the grammar).
pub fn generate(
    chat: &ChatHandle,
    query: &str,
    mut on_token: impl FnMut(&str, usize),
) -> miette::Result<String> {
    let mut stream = chat.ask(query)?;
    let mut count = 0usize;
    while let Some(token) = stream.next_token()? {
        count += 1;
        on_token(&token, count);
    }
    Ok(stream.completed()?)
}

impl Suggestion {
    /// Parse from raw model output. Only fails if the grammar constraint was
    /// somehow violated, which is a bug (or a nobodywho/llguidance bug).
    pub fn parse(raw: &str) -> miette::Result<Self> {
        serde_json::from_str(raw.trim()).map_err(|e| {
            miette::miette!(
                "model produced unparseable JSON ({e}) — impossible under the grammar \
                 constraint, please report this bug"
            )
        })
    }

    /// The command to run: the first (and in M1, only) suggested command.
    pub fn command(&self) -> &str {
        &self
            .commands
            .first()
            .expect("grammar guarantees at least one command")
            .command
    }
}
