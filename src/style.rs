//! Terminal styling: ANSI colors emitted only when output is colorized.
//!
//! [`ColorChoice`] is the user-facing knob (`--color`). It resolves to a plain
//! boolean via [`ColorChoice::resolve`], which consults the terminal and the
//! `NO_COLOR` convention. [`Styler`] then wraps text in ANSI escape sequences,
//! or returns it untouched when color is disabled, keeping every call site
//! oblivious to the decision.

use std::io::IsTerminal;

use clap::ValueEnum;

/// When to colorize output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ColorChoice {
    /// Colorize when stdout is a terminal and `NO_COLOR` is unset.
    #[default]
    Auto,
    /// Always colorize.
    Always,
    /// Never colorize.
    Never,
}

impl ColorChoice {
    /// Resolves whether color should be emitted on stdout.
    ///
    /// `Auto` honours the [`NO_COLOR`](https://no-color.org/) convention (any
    /// non-empty value disables color) and only colorizes an interactive
    /// terminal, so piped or redirected output stays plain.
    #[must_use]
    pub fn resolve(self) -> bool {
        match self {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => std::io::stdout().is_terminal() && !no_color_set(),
        }
    }
}

fn no_color_set() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
}

/// Applies ANSI styling when enabled, and is a no-op otherwise.
#[derive(Debug, Clone, Copy)]
pub struct Styler {
    enabled: bool,
}

impl Styler {
    /// Creates a styler that emits escapes only when `enabled`.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    fn wrap(self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_owned()
        }
    }

    /// Wraps `text` in green (success).
    #[must_use]
    pub fn green(self, text: &str) -> String {
        self.wrap("32", text)
    }

    /// Wraps `text` in red (failure).
    #[must_use]
    pub fn red(self, text: &str) -> String {
        self.wrap("31", text)
    }

    /// Wraps `text` in yellow (informational drift).
    #[must_use]
    pub fn yellow(self, text: &str) -> String {
        self.wrap("33", text)
    }

    /// Wraps `text` in a dim style (secondary detail).
    #[must_use]
    pub fn dim(self, text: &str) -> String {
        self.wrap("2", text)
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorChoice, Styler};

    #[test]
    fn explicit_choices_ignore_environment() {
        assert!(ColorChoice::Always.resolve());
        assert!(!ColorChoice::Never.resolve());
    }

    #[test]
    fn auto_is_off_when_not_a_terminal() {
        // Tests do not run attached to a terminal, so Auto resolves to false.
        assert!(!ColorChoice::Auto.resolve());
    }

    #[test]
    fn disabled_styler_is_transparent() {
        let plain = Styler::new(false);
        assert_eq!(plain.green("ok"), "ok");
        assert_eq!(plain.red("no"), "no");
    }

    #[test]
    fn enabled_styler_wraps_in_escapes() {
        let color = Styler::new(true);
        assert_eq!(color.green("ok"), "\u{1b}[32mok\u{1b}[0m");
        assert!(color.dim("x").starts_with("\u{1b}[2m"));
        assert!(color.yellow("x").ends_with("\u{1b}[0m"));
    }
}
