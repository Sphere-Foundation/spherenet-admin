//! Output mode: human-readable text or machine-readable JSON.
//!
//! Commands build a serde-serializable value implementing [`Render`] and hand
//! it to [`emit`], which prints the text box or pretty JSON depending on the
//! global `--output` mode.
//!
//! Discipline: in `json` mode **stdout must contain only the final JSON object**
//! so it pipes cleanly into `jq`. All progress, preamble, and warnings go to
//! **stderr** via [`progress`] (in both modes), never `println!`.

use clap::ValueEnum;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, ValueEnum)]
pub enum OutputMode {
    /// Human-readable output (default).
    #[default]
    Text,
    /// Machine-readable pretty JSON on stdout.
    Json,
}

/// A command result that can render as either a text box or JSON.
pub trait Render: serde::Serialize {
    /// The human-readable rendering (what we historically `println!`-ed).
    fn to_text(&self) -> String;
}

/// Print a command's final result to stdout in the selected mode.
pub fn emit<T: Render>(value: &T, mode: OutputMode) -> eyre::Result<()> {
    match mode {
        OutputMode::Text => println!("{}", value.to_text()),
        OutputMode::Json => println!("{}", serde_json::to_string_pretty(value)?),
    }
    Ok(())
}

/// Progress / preamble / warnings → **stderr**, in both modes.
///
/// Keeps stdout clean for JSON while still showing operators what's happening
/// in text mode (stderr renders in the terminal).
pub fn progress(msg: impl std::fmt::Display) {
    eprintln!("{}", msg);
}

/// Inner width of the box-drawing frame used by `boxed_header`.
const BOX_WIDTH: usize = 64;

/// A newline — for uniform `out.push_str(newline())` section breaks.
pub fn newline() -> &'static str {
    "\n"
}

/// A box-drawing frame with the title centered (3 lines + trailing newline).
///
/// Shared by the `show` commands so headers are consistent and titles don't
/// have to be hand-centered with counted spaces.
pub fn boxed_header(title: &str) -> String {
    let bar = "═".repeat(BOX_WIDTH);
    let pad = BOX_WIDTH.saturating_sub(title.chars().count());
    let left = pad / 2;
    let right = pad - left;
    format!(
        "╔{bar}╗\n║{}{title}{}║\n╚{bar}╝\n",
        " ".repeat(left),
        " ".repeat(right),
    )
}

/// A left-aligned `Label:` padded to a common column, then the value + newline.
///
/// Keeps the value column aligned across every `show` command without manual
/// space-counting.
pub fn field(label: &str, value: impl std::fmt::Display) -> String {
    format!("{:<21}{}\n", format!("{label}:"), value)
}

/// Like [`field`], but indented one level — for nested/sub-entry lines
/// (list entries, result details). The value column stays aligned with
/// top-level `field` values.
pub fn subfield(label: &str, value: impl std::fmt::Display) -> String {
    format!("  {:<19}{}\n", format!("{label}:"), value)
}
