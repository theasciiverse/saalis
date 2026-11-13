//! At a counter on the base: the base and you on the left, the counter's
//! service (the exchange, the mission boards, the bar, the yard) on the
//! right. Walking about the base is `deck.rs`.

use super::hud::frame;
use super::palette as pal;
use crate::app::App;
use crate::sim::{econ, Phase};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};

pub const SIDE_W: u16 = 30;

/// The base and you: credits, hold, repairs and standing.
pub fn side(buf: &mut Buffer, side: Rect, app: &App) {
    let w = &app.world;
    let Phase::Docked(b) = w.phase else { return };
    let base = &w.bases[b];
    let block = frame("docked");
    let inner = block.inner(side);
    block.render(side, buf);
    let dim = Style::default().fg(pal::DIM);
    let mut lines = vec![
        Line::from(Span::styled(base.name, Style::default().fg(pal::TRAFFIC).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(format!("{} · {}", base.kind.name(), w.region_name()), dim)),
        Line::from(Span::styled(w.date(), dim)),
    ];
    let max = w.ships.first().map_or(0, |p| p.cargo_cap());
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
    let room = (inner.height as usize).saturating_sub(lines.len() + 1);
    let quests = super::journal::lines(app, inner.width as usize, false, room);
    if !quests.is_empty() {
        lines.push(Line::from(""));
        lines.extend(quests);
    }

    Paragraph::new(lines).render(inner, buf);
}

pub fn draw(buf: &mut Buffer, area: Rect, app: &App) {
    Clear.render(area, buf);
    let w = &app.world;
    let Phase::Docked(b) = w.phase else { return };
    let base = &w.bases[b];
    if area.width < SIDE_W + 40 {
        return;
    }
    let main = Rect::new(area.x + SIDE_W, area.y, area.width - SIDE_W, area.height);
    side(buf, Rect::new(area.x, area.y, SIDE_W, area.height), app);
    let name = ["exchange", "mission boards", "bar", if base.kind.shipyard() { "shipyard" } else { "outfitter" }][app.dock_tab.min(3)];
    let block = frame(name);
    let inner = block.inner(main);
    block.render(main, buf);
    match app.dock_tab {
        1 => return board(buf, inner, app),
        2 => return bar(buf, inner, app),
        3 => return yard(buf, inner, app),
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
        // contraband here (banned by the owner's law) shows red
        let banned_here = crate::sim::econ::illegal(w.owner(), g);
        let base_st = Style::default().fg(if banned_here { Color::Rgb(240, 120, 100) } else { pal::TEXT });
        let name_st = if sel { base_st.add_modifier(Modifier::REVERSED) } else { base_st };
        let stock = if !m.open[g] { " (banned)" } else if m.has(g) { "" } else { " (none)" };
        rows.push(Line::from(vec![
            Span::styled(format!("{:<17}", format!("{}{stock}", econ::GOODS[g].name)), name_st),
            Span::styled(if m.open[g] { format!("{buy:>7}") } else { format!("{:>7}", "-") }, Style::default().fg(buy_col)),
            Span::styled(if m.open[g] { format!("{sell:>7}") } else { format!("{:>7}", "-") }, Style::default().fg(sell_col)),
            Span::styled(format!("{idx:>8}"), hdr),
            Span::styled(format!("{:>7}", if held > 0 { held.to_string() } else { "·".into() }), Style::default().fg(pal::TEXT)),
            Span::styled(format!("{:>7}", if held > 0 { w.paid[g].to_string() } else { String::new() }), hdr),
        ]));
    }
    rows.push(Line::from(""));
    rows.push(Line::from(Span::styled("↑↓ choose · → or B buy · ← or S sell · Shift: ten at a time", hdr)));
    rows.push(Line::from(Span::styled("F save · Esc step away", hdr)));
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
    let quests = super::journal::lines(app, width, true, (area.height as usize).saturating_sub(rows.len() + 6));
    if !quests.is_empty() {
        rows.push(Line::from(""));
        rows.extend(quests);
    }
    rows.push(Line::from(""));
    for (k, g) in Guild::ALL.iter().enumerate() {
        let status = if w.members[g.index()] { format!("member, rating {}", w.rating[g.index()]) } else { format!("{} to join: press {}", g.fee(), k + 1) };
        rows.push(Line::from(vec![Span::styled(format!("{:<22}", g.name()), Style::default().fg(pal::rgb(pal::faction(g.faction())))), Span::styled(status, dim)]));
    }
    rows.push(Line::from(Span::styled("↑↓ choose · A take the job · Esc step away", dim)));
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
            rows.push(Line::from(Span::styled("A take it · Esc step away", dim)));
        }
        None => rows.push(Line::from(Span::styled("Quiet tonight. Nobody's buying.", dim))),
    }
    note(&mut rows, app);
    Paragraph::new(rows).render(area, buf);
}

fn yard(buf: &mut Buffer, area: Rect, app: &App) {
    let w = &app.world;
    let dim = Style::default().fg(pal::DIM);
    let text = Style::default().fg(pal::TEXT);
    let shop = w.shop();
    let width = area.width as usize;
    let mut rows = vec![];
    if let Some(p) = w.ships.first() {
        let k = &p.kit;
        rows.push(Line::from(vec![
            Span::styled(format!("{} ", p.class.name), Style::default().fg(pal::rgb(pal::PLAYER)).add_modifier(Modifier::BOLD)),
            Span::styled(format!("hold {} · armour {} · shields {} · power {} · ECM {}", p.cargo_cap(), k.armour_grade, k.shield_grade, k.engine_grade, k.ecm), dim),
        ]));
        let mut extra = Vec::new();
        if k.launcher {
            extra.push(format!("missiles {} dumb, {} seeking", k.dumb, k.seekers));
        }
        if k.turret {
            extra.push("rear turret".into());
        }
        if k.hidden > 0 {
            extra.push(format!("{} hidden units", k.hidden));
        }
        rows.push(Line::from(Span::styled(if extra.is_empty() { " ".into() } else { extra.join(" · ") }, dim)));
    }
    // the shop is long: news goes at the top, where it is seen
    rows.push(Line::from(Span::styled(app.dock_note.clone().unwrap_or_default(), Style::default().fg(pal::TARGET))));
    if shop.is_empty() {
        rows.push(Line::from(Span::styled("No shipyard or outfitter here.", dim)));
    }
    for (i, o) in shop.iter().enumerate() {
        let price = if o.price == 0 { "-".to_string() } else { format!("{} cr", o.price) };
        let room = width.saturating_sub(12);
        let name: String = o.name.chars().take(room).collect();
        let mut st = if o.ok { text } else { dim };
        if i == app.yard_row {
            st = st.add_modifier(Modifier::REVERSED);
        }
        rows.push(Line::from(vec![Span::styled(format!("{name:<room$}"), st), Span::styled(format!("{price:>11}"), if o.ok { text } else { dim })]));
    }
    if let Some(o) = shop.get(app.yard_row) {
        rows.push(Line::from(""));
        rows.push(Line::from(Span::styled(o.note.clone(), dim)));
    }
    rows.push(Line::from(Span::styled("↑↓ choose · B buy · Esc step away", dim)));
    Paragraph::new(rows).render(area, buf);
}
