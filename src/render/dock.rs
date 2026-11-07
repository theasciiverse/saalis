//! The dock: the base's name and services on the left, the commodity
//! exchange on the right.

use super::hud::frame;
use super::palette as pal;
use crate::app::App;
use crate::sim::{econ, Phase};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};

const SIDE_W: u16 = 30;

/// The docking bay, in box-drawing lines.
const BAY: [&str; 5] = ["    ╭────────╮    ", "────┤ ▫ ▫ ▫▫ ├────", "    │  ╭──╮  │    ", "────┤  │  │  ├────", "    ╰──┴──┴──╯    "];

pub fn draw(buf: &mut Buffer, area: Rect, app: &App) {
    Clear.render(area, buf);
    let w = &app.world;
    let Phase::Docked(b) = w.phase else { return };
    let base = &w.bases[b];
    if area.width < SIDE_W + 40 {
        return;
    }
    let side = Rect::new(area.x, area.y, SIDE_W, area.height);
    let main = Rect::new(area.x + SIDE_W, area.y, area.width - SIDE_W, area.height);

    let block = frame("docked");
    let inner = block.inner(side);
    block.render(side, buf);
    let dim = Style::default().fg(pal::DIM);
    let mut lines = vec![
        Line::from(Span::styled(base.name, Style::default().fg(pal::TRAFFIC).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(format!("{} · {}", base.kind.name(), w.region_name()), dim)),
        Line::from(Span::styled(w.date(), dim)),
        Line::from(""),
    ];
    for row in BAY {
        lines.push(Line::from(Span::styled(row, Style::default().fg(pal::WIRE))));
    }
    let max = w.ships.first().map_or(0, |p| p.class.cargo);
    let cost = w.repair_cost();
    lines.extend([
        Line::from(""),
        Line::from(vec![Span::styled("credits  ", dim), Span::styled(format!("{}", w.credits), Style::default().fg(pal::TEXT))]),
        Line::from(vec![Span::styled("hold     ", dim), Span::styled(format!("{} / {}", w.cargo_used(), max), Style::default().fg(pal::TEXT))]),
        Line::from(vec![Span::styled("repairs  ", dim), Span::styled(if cost == 0 { "none needed".to_string() } else { format!("{cost} cr") }, Style::default().fg(pal::TEXT))]),
        Line::from(""),
        Line::from(Span::styled("R repair · F save", dim)),
        Line::from(Span::styled("Enter launch · Esc quit", dim)),
    ]);
    Paragraph::new(lines).render(inner, buf);

    let block = frame("exchange");
    let inner = block.inner(main);
    block.render(main, buf);
    let m = &w.markets[base.market];
    let hdr = Style::default().fg(pal::DIM);
    let mut rows = vec![
        Line::from(Span::styled(format!("{:<17}{:>7}{:>7}{:>8}{:>7}{:>7}", "", "buy", "sell", "index", "hold", "paid"), hdr)),
        Line::from(""),
    ];
    for g in 0..econ::N {
        let buy = m.buy_price(g);
        let sell = m.sell_price(g);
        let idx = econ::index(&w.markets, g);
        // cheap to buy here: green; pays well to sell here: green
        let good = Color::Rgb(120, 230, 150);
        let bad = Color::Rgb(240, 120, 100);
        let buy_col = if (buy as f32) < idx as f32 * 0.9 { good } else if (buy as f32) > idx as f32 * 1.1 { bad } else { pal::TEXT };
        let sell_col = if (sell as f32) > idx as f32 * 1.0 { good } else { pal::TEXT };
        let held = w.cargo[g];
        let sel = g == app.dock_row;
        let name_st = if sel { Style::default().fg(pal::TEXT).add_modifier(Modifier::REVERSED) } else { Style::default().fg(pal::TEXT) };
        let stock = if m.has(g) { "" } else { " (none)" };
        rows.push(Line::from(vec![
            Span::styled(format!("{:<17}", format!("{}{stock}", econ::GOODS[g].name)), name_st),
            Span::styled(format!("{buy:>7}"), Style::default().fg(buy_col)),
            Span::styled(format!("{sell:>7}"), Style::default().fg(sell_col)),
            Span::styled(format!("{idx:>8}"), hdr),
            Span::styled(format!("{:>7}", if held > 0 { held.to_string() } else { "·".into() }), Style::default().fg(pal::TEXT)),
            Span::styled(format!("{:>7}", if held > 0 { w.paid[g].to_string() } else { String::new() }), hdr),
        ]));
    }
    rows.push(Line::from(""));
    rows.push(Line::from(Span::styled("↑↓ choose · → or B buy · ← or S sell · Shift: ten at a time", hdr)));
    if let Some(note) = &app.dock_note {
        rows.push(Line::from(""));
        rows.push(Line::from(Span::styled(note.clone(), Style::default().fg(pal::TARGET))));
    }
    Paragraph::new(rows).render(inner, buf);
}
