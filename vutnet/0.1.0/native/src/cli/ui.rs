use std::{
    fmt::Write as FmtWrite,
    io::{IsTerminal as _, Write as _},
    sync::atomic::{AtomicBool, Ordering},
};

use anstyle::{AnsiColor, Style};

pub struct Ui {
    color: bool,
    interactive: bool,
    progress_active: AtomicBool,
}

pub struct RouteRow {
    pub method: String,
    pub path: String,
}

impl Ui {
    #[must_use]
    pub fn new() -> Self {
        let stdout = std::io::stdout();
        let interactive = stdout.is_terminal();
        Self {
            color: interactive && std::env::var_os("NO_COLOR").is_none(),
            interactive,
            progress_active: AtomicBool::new(false),
        }
    }

    #[cfg(test)]
    fn with_color(color: bool) -> Self {
        Self {
            color,
            interactive: false,
            progress_active: AtomicBool::new(false),
        }
    }

    pub fn header(&self) {
        Self::stdout(&format!(
            "  {}  {}\n",
            self.paint(
                "VUTNET",
                Style::new()
                    .fg_color(Some(AnsiColor::Magenta.into()))
                    .bold()
            ),
            self.paint(
                &format!("v{}", env!("VUTNET_VERSION")),
                Style::new().dimmed()
            )
        ));
    }

    pub fn status(&self, message: &str) {
        Self::stdout(&format!("  {} {}\n", self.paint("✓", green()), message));
    }

    pub fn detail(&self, label: &str, value: &str) {
        Self::stdout(&format!(
            "  {}  {}\n",
            self.paint(label, Style::new().bold()),
            value
        ));
    }

    pub fn heading(&self, text: &str) {
        Self::stdout(&format!(
            "  {}\n\n",
            self.paint(
                text,
                Style::new()
                    .fg_color(Some(AnsiColor::Magenta.into()))
                    .bold()
            )
        ));
    }

    pub fn dim(&self, text: &str) {
        Self::stdout(&format!("  {}\n", self.paint(text, Style::new().dimmed())));
    }

    pub fn info_table(&self, values: &[(&str, String)]) {
        let width = values
            .iter()
            .map(|(label, _)| unicode_width::UnicodeWidthStr::width(*label))
            .max()
            .unwrap_or(0);
        for (label, value) in values {
            let spaces = width.saturating_sub(unicode_width::UnicodeWidthStr::width(*label));
            Self::stdout(&format!(
                "  {}{}  {}",
                self.paint(label, Style::new().bold()),
                " ".repeat(spaces),
                value
            ));
        }
    }

    pub fn route_table(&self, rows: &[RouteRow]) {
        Self::stdout(&self.format_route_table(rows));
    }

    fn format_route_table(&self, rows: &[RouteRow]) -> String {
        let method_width = rows
            .iter()
            .map(|row| unicode_width::UnicodeWidthStr::width(row.method.as_str()))
            .max()
            .unwrap_or(6)
            .max(6);
        let mut output = format!(
            "  {}  {}\n",
            self.paint("METHOD", Style::new().bold()),
            self.paint("PATH", Style::new().bold())
        );
        for row in rows {
            let method_color = match row.method.as_str() {
                "GET" | "HEAD" | "OPTIONS" => AnsiColor::Cyan,
                "POST" => AnsiColor::Green,
                "PUT" | "PATCH" => AnsiColor::Yellow,
                "DELETE" => AnsiColor::Red,
                _ => AnsiColor::White,
            };
            let _ = writeln!(
                output,
                "  {}{}  {}",
                self.paint(
                    &row.method,
                    Style::new().fg_color(Some(method_color.into()))
                ),
                " ".repeat(
                    method_width
                        .saturating_sub(unicode_width::UnicodeWidthStr::width(row.method.as_str()))
                ),
                self.paint(
                    &row.path,
                    Style::new().fg_color(Some(AnsiColor::Cyan.into()))
                )
            );
        }
        output
    }

    pub fn warning(&self, message: &str) {
        Self::stderr(&format!(
            "  {} {}\n",
            self.paint("⚠ WARN", yellow().bold()),
            message
        ));
    }

    pub fn error(&self, message: &str) {
        Self::stderr(&format!(
            "  {}\n\n  {}\n",
            self.paint("✗ ERROR", red().bold()),
            self.paint(message, red())
        ));
    }

    pub fn changed(&self, path: &str) {
        Self::stdout(&format!(
            "  {} {} changed\n",
            self.paint("↻", cyan()),
            self.paint(path, Style::new().dimmed())
        ));
    }

    pub fn progress(&self, message: &str) {
        Self::stdout(&format!("  {} {}\n", self.paint("↻", cyan()), message));
    }

    pub fn begin_live_progress(&self, message: &str) {
        if self.interactive {
            self.progress_active.store(true, Ordering::Release);
            self.write_live_progress(message);
        } else {
            self.progress(message);
            self.progress_active.store(true, Ordering::Release);
        }
    }

    pub fn update_live_progress(&self, message: &str) {
        if self.progress_active.load(Ordering::Acquire) {
            if self.interactive {
                self.write_live_progress(message);
            } else {
                self.progress(message);
            }
        }
    }

    pub fn finish_live_progress(&self) {
        if self.progress_active.swap(false, Ordering::AcqRel) && self.interactive {
            Self::stdout("\n");
        }
    }

    fn write_live_progress(&self, message: &str) {
        let mut output = anstream::stdout();
        let line = format!("\r  {} {}", self.paint("↻", cyan()), message);
        let _ = output.write_all(line.as_bytes());
        let _ = output.flush();
    }

    pub fn paint(&self, text: &str, style: Style) -> String {
        if self.color {
            format!("{style}{text}{style:#}")
        } else {
            text.to_owned()
        }
    }

    pub fn stdout(text: &str) {
        let mut out = anstream::stdout();
        let _ = out.write_all(text.as_bytes());
        let _ = out.flush();
    }

    pub fn stderr(text: &str) {
        let mut out = anstream::stderr();
        let _ = out.write_all(text.as_bytes());
        let _ = out.flush();
    }
}

fn green() -> Style {
    Style::new().fg_color(Some(AnsiColor::Green.into()))
}
fn cyan() -> Style {
    Style::new().fg_color(Some(AnsiColor::Cyan.into()))
}
fn yellow() -> Style {
    Style::new().fg_color(Some(AnsiColor::Yellow.into()))
}
fn red() -> Style {
    Style::new().fg_color(Some(AnsiColor::Red.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_color_disables_style_sequences() {
        let ui = Ui::with_color(false);
        assert_eq!(ui.paint("VUTNET", red().bold()), "VUTNET");
    }

    #[test]
    fn color_formatter_uses_anstyle_sequences() {
        let ui = Ui::with_color(true);
        assert!(ui.paint("VUTNET", red().bold()).contains("VUTNET"));
    }

    #[test]
    fn route_table_aligns_methods_and_has_plain_non_color_mode() {
        let ui = Ui::with_color(false);
        let rows = [
            RouteRow {
                method: "GET".to_owned(),
                path: "/".to_owned(),
            },
            RouteRow {
                method: "POST".to_owned(),
                path: "/users".to_owned(),
            },
        ];
        let rendered = ui.format_route_table(&rows);
        assert!(!rendered.contains('\u{1b}'));
        let get = rendered.lines().nth(1).unwrap();
        let post = rendered.lines().nth(2).unwrap();
        assert_eq!(get.find('/'), post.find('/'));
    }
}
