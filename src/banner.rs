use std::process::Command;

use ratatui::style::Color;
use ratatui::text::{Line, Span};

const RAINBOW: [Color; 6] = [
    Color::Rgb(255, 0, 0),
    Color::Rgb(255, 140, 0),
    Color::Rgb(255, 230, 0),
    Color::Rgb(0, 200, 80),
    Color::Rgb(0, 120, 255),
    Color::Rgb(160, 0, 220),
];

/// Renders `text` with the system `figlet`/`toilet` binary using the
/// "pagga" font, then re-colors it as a rainbow gradient (à la `toilet -F
/// gay`) using ratatui spans instead of raw ANSI, so it composes cleanly
/// with the rest of the UI. Falls back to a plain bold line if figlet isn't
/// installed.
pub fn title_lines(text: &str) -> Vec<Line<'static>> {
    match Command::new("figlet").args(["-f", "pagga", text]).output() {
        Ok(output) if output.status.success() => {
            let rendered = String::from_utf8_lossy(&output.stdout);
            rendered
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(colorize_line)
                .collect()
        }
        _ => vec![Line::from(Span::styled(
            text.to_string(),
            ratatui::style::Style::default()
                .add_modifier(ratatui::style::Modifier::BOLD),
        ))],
    }
}

fn colorize_line(line: &str) -> Line<'static> {
    let spans: Vec<Span<'static>> = line
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let color = RAINBOW[i % RAINBOW.len()];
            Span::styled(
                c.to_string(),
                ratatui::style::Style::default()
                    .fg(color)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )
        })
        .collect();
    Line::from(spans)
}
