//! Presentation: the suggestion card and the approval gate.
//!
//! M1 keeps this plain ANSI styling (console), in the normal scrollback — the
//! M2 ratatui inline-viewport card replaces this module's `card`/`gate`.

use crate::context::EnvContext;
use crate::risk::{Assessment, Risk};
use crate::suggest::Suggestion;
use console::style;
use nobodywho::miette;

/// Print the suggestion card. Stays in the terminal scrollback.
pub fn card(ctx: &EnvContext, suggestion: &Suggestion, assessment: &Assessment, model: &str) {
    let main = suggestion
        .commands
        .first()
        .expect("grammar guarantees at least one command");

    let risk_styled = match assessment.risk {
        Risk::High => style(assessment.risk.label()).red().bold(),
        Risk::Medium => style(assessment.risk.label()).yellow(),
        Risk::Low => style(assessment.risk.label()).green(),
    };

    println!();
    println!(
        "  {} {}",
        style("❯").cyan().bold(),
        style(&main.command).bold()
    );
    println!("  {}", style(&main.explanation).dim());
    println!();
    println!(
        "  {} {risk_styled} {} {} {}",
        style("risk:").dim(),
        style("·").dim(),
        style(format!("shell: {}", ctx.shell)).dim(),
        style(format!("· model: {model}")).dim(),
    );
    for reason in &assessment.reasons {
        println!("  {} {}", style("⚠").yellow(), style(reason).yellow());
    }
    println!();
}

/// The approval gate. Nothing runs without passing it.
///
/// Low/medium risk: plain confirm, defaulting to no.
/// High risk: requires typing the word `run`.
pub fn confirm_run(assessment: &Assessment) -> miette::Result<bool> {
    match assessment.risk {
        Risk::High => {
            let reasons = assessment.reasons.join("; ");
            let answer: String = cliclack::input(format!(
                "DANGEROUS ({reasons}). Type 'run' to execute anyway"
            ))
            .placeholder("run")
            .interact()
            .map_err(|e| miette::miette!("prompt failed: {e}"))?;
            Ok(answer.trim().eq_ignore_ascii_case("run"))
        }
        _ => cliclack::confirm("Run this command?")
            .initial_value(false)
            .interact()
            .map_err(|e| miette::miette!("prompt failed: {e}")),
    }
}
