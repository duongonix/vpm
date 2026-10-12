use std::{net::SocketAddr, time::Duration};

use anstyle::{AnsiColor, Style};
use http::StatusCode;

use super::Event;

const METHOD_WIDTH: usize = 7;
const PATH_WIDTH: usize = 40;

pub(super) enum Stream {
    Stdout,
    Stderr,
}

pub(super) struct Rendered {
    pub(super) text: String,
    pub(super) stream: Stream,
}

pub(super) fn render(event: Event, color: bool) -> Rendered {
    match event {
        Event::Ready {
            version,
            address,
            elapsed,
            mode,
            watching,
        } => Rendered {
            text: ready(
                version,
                address,
                elapsed,
                mode.as_deref(),
                watching.as_deref(),
                color,
            ),
            stream: Stream::Stdout,
        },
        Event::DevReady {
            address,
            elapsed,
            mode,
            watching,
        } => Rendered {
            text: ready_without_header(
                address,
                elapsed,
                mode.as_deref(),
                watching.as_deref(),
                color,
            ),
            stream: Stream::Stdout,
        },
        Event::Request {
            method,
            path,
            status,
            elapsed,
        } => Rendered {
            text: request(&method, &path, status, elapsed, color),
            stream: Stream::Stdout,
        },
        Event::RequestError {
            method,
            path,
            status,
            elapsed,
            stage,
            message,
        } => Rendered {
            text: request_error(&method, &path, status, elapsed, stage, &message, color),
            stream: Stream::Stderr,
        },
        Event::Error { message } => Rendered {
            text: error(&message, color),
            stream: Stream::Stderr,
        },
        Event::Stopped => Rendered {
            text: stopped(color),
            stream: Stream::Stdout,
        },
    }
}

fn ready(
    version: &str,
    address: SocketAddr,
    elapsed: Duration,
    mode: Option<&str>,
    watching: Option<&str>,
    color: bool,
) -> String {
    ready_content(Some(version), address, elapsed, mode, watching, color)
}

fn ready_without_header(
    address: SocketAddr,
    elapsed: Duration,
    mode: Option<&str>,
    watching: Option<&str>,
    color: bool,
) -> String {
    ready_content(None, address, elapsed, mode, watching, color)
}

fn ready_content(
    version: Option<&str>,
    address: SocketAddr,
    elapsed: Duration,
    mode: Option<&str>,
    watching: Option<&str>,
    color: bool,
) -> String {
    let mut output = String::new();
    if let Some(version) = version {
        output.push_str("  ");
        output.push_str(&styled("VUTNET", magenta().bold(), color));
        output.push_str("  ");
        output.push_str(&styled(&format!("v{version}"), dim(), color));
        output.push_str("\n\n");
    }
    output.push_str("  ");
    output.push_str(&styled("✓ Server ready", green().bold(), color));
    output.push(' ');
    output.push_str(&styled(&format!("in {}", duration(elapsed)), dim(), color));
    output.push_str("\n\n");
    info_row(
        &mut output,
        "Local",
        &local_url(address),
        cyan().underline(),
        color,
    );
    if let Some(mode) = mode {
        info_row(&mut output, "Mode", mode, green(), color);
    }
    if let Some(watching) = watching {
        info_row(&mut output, "Watching", watching, dim(), color);
    }
    output
}

fn info_row(output: &mut String, label: &str, value: &str, value_style: Style, color: bool) {
    output.push_str("  ");
    output.push_str(&styled("➜", cyan(), color));
    output.push(' ');
    output.push_str(&styled(label, bold(), color));
    output.push_str(&" ".repeat(10usize.saturating_sub(label.chars().count())));
    output.push_str(&styled(value, value_style, color));
    output.push('\n');
}

fn request(method: &str, path: &str, status: StatusCode, elapsed: Duration, color: bool) -> String {
    let method = truncate(method, METHOD_WIDTH);
    let path = truncate(&printable(path), PATH_WIDTH);
    let method_padding = " ".repeat(METHOD_WIDTH.saturating_sub(display_width(&method)));
    let path_padding = " ".repeat(PATH_WIDTH.saturating_sub(display_width(&path)));
    let status_text = status.as_u16().to_string();
    let mut output = String::from("  ");
    output.push_str(&styled(&method, method_style(&method), color));
    output.push_str(&method_padding);
    output.push_str("  ");
    output.push_str(&path);
    output.push_str(&path_padding);
    output.push_str("  ");
    output.push_str(&styled(&status_text, status_style(status), color));
    output.push_str("  ");
    output.push_str(&styled(&duration(elapsed), dim(), color));
    output.push('\n');
    output
}

fn request_error(
    method: &str,
    path: &str,
    status: StatusCode,
    elapsed: Duration,
    stage: &str,
    message: &str,
    color: bool,
) -> String {
    let method = truncate(method, METHOD_WIDTH);
    let path = truncate(&printable(path), PATH_WIDTH);
    let method_padding = " ".repeat(METHOD_WIDTH.saturating_sub(display_width(&method)));
    let path_padding = " ".repeat(PATH_WIDTH.saturating_sub(display_width(&path)));
    let status_text = status.as_u16().to_string();
    format!(
        "  {}\n\n  {}{}  {}{}  {}  {}\n\n    {}  {}\n    {}  {}\n",
        styled("✗ REQUEST ERROR", red().bold(), color),
        styled(&method, method_style(&method), color),
        method_padding,
        styled(&path, cyan(), color),
        path_padding,
        styled(&status_text, status_style(status), color),
        styled(&duration(elapsed), dim(), color),
        styled("Stage", bold(), color),
        styled(stage, yellow(), color),
        styled("Cause", bold(), color),
        styled(&printable(message), red(), color),
    )
}

fn error(message: &str, color: bool) -> String {
    let message = printable(message);
    format!(
        "  {}\n\n  {}\n",
        styled("✗ ERROR", red().bold(), color),
        styled(&message, red(), color)
    )
}

fn stopped(color: bool) -> String {
    format!("  {}\n", styled("✓ Server stopped", green().bold(), color))
}

fn local_url(address: SocketAddr) -> String {
    let host = if address.ip().is_loopback() {
        "localhost".to_owned()
    } else {
        address.ip().to_string()
    };
    format!("http://{host}:{}", address.port())
}

fn method_style(method: &str) -> Style {
    match method {
        "GET" | "HEAD" | "OPTIONS" => cyan(),
        "POST" => green(),
        "PUT" | "PATCH" => yellow(),
        "DELETE" => red(),
        _ => dim(),
    }
}

fn status_style(status: StatusCode) -> Style {
    match status.as_u16() {
        200..=299 => green(),
        300..=399 => cyan(),
        400..=499 => yellow(),
        500..=599 => red(),
        _ => dim(),
    }
}

fn duration(elapsed: Duration) -> String {
    if elapsed < Duration::from_millis(1) {
        "<1ms".to_owned()
    } else {
        format!("{}ms", elapsed.as_millis())
    }
}

fn printable(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_control() {
            output.extend(character.escape_default());
        } else {
            output.push(character);
        }
    }
    output
}

fn truncate(value: &str, width: usize) -> String {
    if display_width(value) <= width {
        return value.to_owned();
    }
    if width <= 1 {
        return "…".chars().take(width).collect();
    }
    let mut output: String = value.chars().take(width - 1).collect();
    output.push('…');
    output
}

fn display_width(value: &str) -> usize {
    value.chars().count()
}

fn styled(value: &str, style: Style, color: bool) -> String {
    if color {
        format!("{style}{value}{style:#}")
    } else {
        value.to_owned()
    }
}

const fn fg(color: AnsiColor) -> Style {
    Style::new().fg_color(Some(anstyle::Color::Ansi(color)))
}

const fn magenta() -> Style {
    fg(AnsiColor::Magenta)
}

const fn green() -> Style {
    fg(AnsiColor::Green)
}

const fn cyan() -> Style {
    fg(AnsiColor::Cyan)
}

const fn yellow() -> Style {
    fg(AnsiColor::Yellow)
}

const fn red() -> Style {
    fg(AnsiColor::Red)
}

const fn dim() -> Style {
    Style::new().dimmed()
}

const fn bold() -> Style {
    Style::new().bold()
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
