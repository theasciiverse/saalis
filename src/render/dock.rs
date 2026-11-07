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
        Line::from(Span::styled("STANDING", dim)),
    ]);
    for f in crate::sim::faction::ALL {
        let v = w.standing.of(f);
        let word = crate::sim::faction::Standing::word(v);
        let col = if v <= crate::sim::faction::HOSTILE { Color::Rgb(240, 110, 100) } else { pal::rgb(pal::faction(f)) };
        lines.push(Line::from(vec![Span::styled(format!("{:<18}", f.name()), Style::default().fg(col)), Span::styled(word, dim)]));
    }

    Paragraph::new(lines).render(inner, buf);

    let tabs = ["exchange", "missions", "bar"];
    let mut title = vec![Span::raw(" ")];
    for (i, t) in tabs.iter().enumerate() {
        let st = if i == app.dock_tab { Style::default().fg(pal::TEXT).add_modifier(Modifier::REVERSED) } else { Style::default().fg(pal::DIM) };
        title.push(Span::styled(format!(" {t} "), st));
        title.push(Span::raw(" "));
    }
    title.push(Span::styled("Tab switches ", Style::default().fg(pal::TRACE)));
    let block = frame("").title(Line::from(title));
    let inner = block.inner(main);
    block.render(main, buf);
    match app.dock_tab {
        1 => return board(buf, inner, app),
        2 => return bar(buf, inner, app),
        _ => {}
    }
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
    rows.push(Line::from(Span::styled("R repair · F save · Enter launch · Esc quit", hdr)));
    note(&mut rows, app);
    Paragraph::new(rows).render(inner, buf);
}

fn note(rows: &mut Vec<Line<'static>>, app: &App) {
    if let Some(n) = &app.dock_note {
        rows.push(Line::from(""));
        rows.push(Line::from(Span::styled(n.clone(), Style::default().fg(pal::TARGET))));
    }
}

fn board(buf: &mut Buffer, area: Rect, app: &App) {
    use crate::sim::mission::{time_left, Guild, Issuer};
    let w = &app.world;
    let dim = Style::default().fg(pal::DIM);
    let text = Style::default().fg(pal::TEXT);
    let width = area.width as usize;
    let mut rows = vec![Line::from(Span::styled("ON THE BOARDS", dim))];
    if w.offers.is_empty() {
        rows.push(Line::from(Span::styled("nothing today", dim)));
    }
    for (i, m) in w.offers.iter().enumerate() {
        let who = match &m.issuer {
            Issuer::Guild(Guild::Traders) => "Traders",
            Issuer::Guild(Guild::Watch) => "Watch",
            Issuer::Fixer(_) => "fixer",
        };
        let tail = format!("{:>6} cr {:>8}", m.pay, time_left(m.deadline, w.clock));
        let room = width.saturating_sub(tail.len() + 10);
        let mut d = m.describe(&w.sector);
        if d.chars().count() > room {
            d = d.chars().take(room.saturating_sub(1)).collect::<String>() + "…";
        }
        let st = if i == app.job_row { text.add_modifier(Modifier::REVERSED) } else { text };
        rows.push(Line::from(vec![Span::styled(format!("{who:<8} "), dim), Span::styled(format!("{d:<room$}"), st), Span::styled(format!(" {tail}"), text)]));
    }
    rows.push(Line::from(""));
    rows.push(Line::from(Span::styled(format!("YOUR JOBS ({}/{})", w.missions.len(), crate::sim::mission::MAX_ACTIVE), dim)));
    if w.missions.is_empty() {
        rows.push(Line::from(Span::styled("none", dim)));
    }
    for m in &w.missions {
        let tail = format!("{:>6} cr {:>8}", m.pay, time_left(m.deadline, w.clock));
        let room = width.saturating_sub(tail.len() + 1);
        let d: String = m.describe(&w.sector).chars().take(room).collect();
        rows.push(Line::from(vec![Span::styled(format!("{d:<room$}"), text), Span::styled(format!(" {tail}"), text)]));
    }
    rows.push(Line::from(""));
    for (k, g) in Guild::ALL.iter().enumerate() {
        let status = if w.members[g.index()] { format!("member, rating {}", w.rating[g.index()]) } else { format!("{} to join: press {}", g.fee(), k + 1) };
        rows.push(Line::from(vec![Span::styled(format!("{:<22}", g.name()), Style::default().fg(pal::rgb(pal::faction(g.faction())))), Span::styled(status, dim)]));
    }
    rows.push(Line::from(Span::styled("↑↓ choose · A take the job · Enter launch", dim)));
    note(&mut rows, app);
    Paragraph::new(rows).render(area, buf);
}

fn bar(buf: &mut Buffer, area: Rect, app: &App) {
    use crate::sim::mission::time_left;
    let w = &app.world;
    let dim = Style::default().fg(pal::DIM);
    let mut rows = vec![Line::from(Span::styled("THE BAR", dim)), Line::from("")];
    match &w.fixer {
        Some((name, line, m)) => {
            rows.push(Line::from(Span::styled(name.clone(), Style::default().fg(pal::TARGET).add_modifier(Modifier::BOLD))));
            rows.push(Line::from(Span::styled(format!("\u{201c}{line}\u{201d}"), Style::default().fg(pal::TEXT))));
            rows.push(Line::from(""));
            rows.push(Line::from(Span::styled(format!("The job: {}.", m.describe(&w.sector)), Style::default().fg(pal::TEXT))));
            rows.push(Line::from(Span::styled(format!("{} cr, done within {}.", m.pay, time_left(m.deadline, w.clock)), Style::default().fg(pal::TEXT))));
            rows.push(Line::from(""));
            rows.push(Line::from(Span::styled("A take it · Tab back to the boards", dim)));
        }
        None => rows.push(Line::from(Span::styled("Quiet tonight. Nobody's buying.", dim))),
    }
    note(&mut rows, app);
    Paragraph::new(rows).render(area, buf);
}
