//! The quests' journal: each begun quest's title and what to do next. It
//! shows beside the deck, on the map and, with the finished ones too, on
//! the mission boards.

use super::deck::wrap;
use super::palette as pal;
use crate::app::App;
use crate::sim::quest::Status;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// The journal's lines at `width`, in at most `rows` lines; `all` adds the
/// finished and failed. When it doesn't all fit, each objective shortens
/// to a line, and the last line says how many more there are.
pub fn lines(app: &App, width: usize, all: bool, rows: usize) -> Vec<Line<'static>> {
    let dim = Style::default().fg(pal::DIM);
    let mut quests = Vec::new();
    for st in app.quests.journal() {
        let (tail, show) = match &st.status {
            Status::Running => (st.objective.clone().unwrap_or_default(), true),
            Status::Done(_) => ("done".to_string(), all),
            Status::Failed(_) => ("failed".to_string(), all),
            Status::Broken(e) => (format!("broken: {}", e.lines().next().unwrap_or_default()), true),
        };
        if show {
            quests.push((st, wrap(&tail, width.saturating_sub(2).max(10))));
        }
    }
    if quests.is_empty() || rows < 3 {
        return Vec::new();
    }
    let full: usize = 1 + quests.iter().map(|(_, t)| 1 + t.len()).sum::<usize>();
    let short = full > rows;
    let mut out = vec![Line::from(Span::styled("QUESTS", dim))];
    for (k, (st, mut tail)) in quests.iter().cloned().enumerate() {
        if short && tail.len() > 1 {
            tail.truncate(1);
            let l = &mut tail[0];
            let keep: String = l.chars().take(width.saturating_sub(3)).collect();
            *l = format!("{keep}…");
        }
        let left = quests.len() - k;
        if out.len() + 1 + tail.len() > rows - usize::from(left > 1) {
            out.push(Line::from(Span::styled(format!("+{left} more on the boards"), dim)));
            break;
        }
        let running = st.status == Status::Running;
        let title = Style::default().fg(if running { pal::TARGET } else { pal::DIM }).add_modifier(Modifier::BOLD);
        out.push(Line::from(Span::styled(st.title.clone(), title)));
        for l in tail {
            out.push(Line::from(Span::styled(format!("  {l}"), if running { Style::default().fg(pal::TEXT) } else { dim })));
        }
    }
    out
}
