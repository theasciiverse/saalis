//! The HUD around the field: ordinary Ratatui widgets, never Braille text.
//! Top: shields and armour per facing, energy, hull, speed, burner. Right:
//! the radar and the target box. Bottom: guns, comms, the tally. Gauges are
//! block eighths; schematics and frames are box-drawing lines.

use super::glyphs;
use super::palette::{self as pal, rgb, Rgb};
use crate::sim::ship::{Facing, Side};
use crate::sim::{Comm, Nav, World};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget};

const PARTIAL: [char; 7] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉'];
const LEVEL: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub fn frame(title: &str) -> Block<'_> {
    let b = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(pal::FRAME)).style(Style::default().bg(pal::CHROME_BG));
    if title.is_empty() { b } else { b.title(Span::styled(format!(" {title} "), Style::default().fg(pal::DIM))) }
}

/// A distance for the HUD: metres up close, kilometres further out.
pub fn distance(d: f32) -> String {
    if d >= 1000.0 { format!("{:.1} km", d / 1000.0) } else { format!("{:.0} m", d) }
}

/// A horizontal gauge `width` cells long, in block eighths.
fn bar(frac: f32, width: usize, color: Color) -> Vec<Span<'static>> {
    let eighths = (frac.clamp(0.0, 1.0) * (width * 8) as f32).round() as usize;
    let full = eighths / 8;
    let part = eighths % 8;
    let mut filled = "█".repeat(full);
    if part > 0 {
        filled.push(PARTIAL[part - 1]);
    }
    let used = full + (part > 0) as usize;
    vec![Span::styled(filled, Style::default().fg(color)), Span::styled("▁".repeat(width.saturating_sub(used)), Style::default().fg(pal::TRACE))]
}

/// One cell's level, a vertical eighth.
fn level(frac: f32) -> char {
    if frac <= 0.0 { ' ' } else { LEVEL[((frac.clamp(0.0, 1.0) * 8.0).ceil() as usize).clamp(1, 8) - 1] }
}

fn label(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), Style::default().fg(pal::DIM))
}

/// The top bar: two rows of the player's own state.
pub fn top(buf: &mut Buffer, area: Rect, w: &World) {
    let mut block = frame("").title(Span::styled(format!(" SAALIS · {} · {} ", w.region_name(), w.date()), Style::default().fg(pal::DIM)));
    if let (Some(n), Some(p)) = (w.nav, w.player()) {
        let d = (w.nav_pos(n) - p.pos).len();
        let col = match n {
            Nav::Base(_) => pal::TRAFFIC,
            Nav::Mouth(_) => rgb(pal::LANE),
        };
        let mut spans = vec![Span::styled(" NAV ▸ ", Style::default().fg(pal::DIM)), Span::styled(w.nav_name(n), Style::default().fg(col)), Span::styled(format!(" {} ", distance(d)), Style::default().fg(pal::TEXT))];
        if w.autopilot {
            spans.push(Span::styled(" AUTO ", Style::default().fg(Color::Rgb(10, 14, 20)).bg(col)));
            spans.push(Span::raw(" "));
        }
        block = block.title(Line::from(spans).right_aligned());
    }
    let inner = block.inner(area);
    block.render(area, buf);
    let Some(p) = w.ships.first() else { return };
    let c = p.class;
    let wide = inner.width >= 96;
    let gw = if wide { 5 } else { 3 };
    let facing = |row: &mut Vec<Span<'static>>, vals: [f32; 4], max: [f32; 4], shield: bool| {
        for f in Facing::ALL {
            let i = f.index();
            let hit = p.alive && p.hit_flash[i] < 0.2;
            let st = if hit { Style::default().fg(Color::Rgb(20, 10, 10)).bg(Color::Rgb(255, 90, 70)) } else { Style::default().fg(pal::DIM) };
            row.push(Span::styled(f.letter().to_string(), st));
            let frac = vals[i] / max[i];
            let col = if shield { rgb(pal::lerp((40, 70, 110), pal::SHIELD, frac)) } else { pal::ramp(&pal::GAUGE, frac) };
            row.extend(bar(frac, gw, col));
            row.push(Span::raw(" "));
        }
    };
    let mut r1 = vec![label("SHIELD ")];
    facing(&mut r1, p.shields, [p.max_shield(); 4], true);
    r1.push(label(" ENERGY "));
    r1.extend(bar(p.energy / p.max_energy(), if wide { 12 } else { 8 }, rgb(pal::ENERGY)));
    r1.push(label("  HULL "));
    r1.extend(bar(p.hull / c.hull, if wide { 10 } else { 6 }, pal::ramp(&pal::GAUGE, p.hull / c.hull)));
    let mut r2 = vec![label("ARMOUR ")];
    facing(&mut r2, p.armour, p.max_armour(), false);
    r2.push(label(" SPEED "));
    let spd = if p.alive { p.speed() } else { 0.0 };
    r2.push(Span::styled(format!("{:>3}", spd.round() as i32), Style::default().fg(if p.burning { rgb(pal::BURN) } else { pal::TEXT })));
    r2.push(label(" ▸ "));
    r2.push(Span::styled(format!("{:<3}", p.set_speed.round() as i32), Style::default().fg(pal::TEXT)));
    r2.push(label(" BURN "));
    r2.extend(bar(p.fuel / c.fuel, if wide { 8 } else { 5 }, rgb(pal::BURN)));
    Paragraph::new(vec![Line::from(r1), Line::from(r2)]).render(inner, buf);
}

/// The bottom bar: guns ready, the newest comms, the tally.
pub fn bottom(buf: &mut Buffer, area: Rect, w: &World, note: Option<&str>) {
    let block = frame("comms");
    let inner = block.inner(area);
    block.render(area, buf);
    let mut left = vec![label("GUNS ")];
    if let Some(p) = w.ships.first() {
        for (g, gun) in p.guns().iter().enumerate() {
            let ready = p.alive && p.cooldown[g] <= 0.0 && p.energy >= gun.energy;
            left.push(Span::styled(if ready { "●" } else { "○" }, Style::default().fg(if ready { rgb(pal::ENERGY) } else { pal::DIM })));
        }
    }
    if let Some(p) = w.ships.first().filter(|p| p.kit.launcher) {
        let lock = w.target.and_then(|t| w.ship(t)).is_some_and(|t| p.kit.seekers > 0 && w.locked(p, t));
        left.push(Span::styled(format!(" MSL {}+{}", p.kit.seekers, p.kit.dumb), Style::default().fg(pal::DIM)));
        if lock {
            left.push(Span::styled(" LOCK", Style::default().fg(pal::TARGET).add_modifier(Modifier::BOLD)));
        }
    }
    if w.missile_warning() && (w.time * 4.0) as i32 % 2 == 0 {
        left.push(Span::styled(" MISSILE ", Style::default().fg(Color::Rgb(20, 10, 10)).bg(Color::Rgb(255, 80, 70))));
    }
    left.push(Span::raw("  "));
    let max = w.ships.first().map_or(0, |p| p.cargo_cap());
    let tally = format!("{} cr  HOLD {}/{}  KILLS {}", w.credits, w.cargo_used(), max, w.kills);
    let mut right = Vec::new();
    if let Some(n) = note {
        right.push(Span::styled(format!("{n}  "), Style::default().fg(pal::TRACE)));
    }
    right.push(Span::styled(tally.clone(), Style::default().fg(pal::DIM)));
    let right_w: usize = right.iter().map(|s| s.content.chars().count()).sum();
    let left_w: usize = left.iter().map(|s| s.content.chars().count()).sum();
    let room = (inner.width as usize).saturating_sub(left_w + right_w + 2);
    let comm: Option<&Comm> = w.comms.last().filter(|c| c.age < 9.0);
    match comm {
        Some(c) => {
            let col = match c.side {
                Some(Side::Pirate) => rgb(pal::PIRATE),
                Some(Side::Player) => pal::TEXT,
                Some(Side::Patrol) => rgb(pal::faction(crate::sim::faction::Faction::Coop)),
                Some(Side::Trader) | None => pal::TRAFFIC,
            };
            let mut text = format!("{}: {}", c.from, c.text);
            if text.chars().count() > room {
                text = text.chars().take(room.saturating_sub(1)).collect::<String>() + "…";
            }
            left.push(Span::styled(text, Style::default().fg(col)));
        }
        None => left.push(Span::styled("…", Style::default().fg(pal::TRACE))),
    }
    let used: usize = left.iter().map(|s| s.content.chars().count()).sum();
    left.push(Span::raw(" ".repeat((inner.width as usize).saturating_sub(used + right_w))));
    left.extend(right);
    Paragraph::new(Line::from(left)).render(inner, buf);
}

/// The radar: a round Braille scope, north up. The inner ring is 3 km, the
/// edge 6 km; contacts further out sit dim on the edge.
pub fn radar(buf: &mut Buffer, area: Rect, w: &World, aspect: f32) {
    let block = frame("radar");
    let inner = block.inner(area);
    block.render(area, buf);
    if inner.width < 6 || inner.height < 3 {
        return;
    }
    let mut c = super::canvas::Canvas::new(inner.width, inner.height);
    // radius in columns, so the circle is round on screen
    let rc = (inner.width as f32 / 2.0).min(inner.height as f32 * aspect / 2.0) - 0.5;
    let (cxd, cyd) = (inner.width as f32, inner.height as f32 * 2.0);
    let to_dot = |dx: f32, dy: f32| -> (i32, i32) { ((cxd + dx * rc * 2.0).floor() as i32, (cyd + dy * rc * 4.0 / aspect).floor() as i32) };
    const EDGE: f32 = 6000.0;
    for k in 0..96 {
        let a = k as f32 / 96.0 * std::f32::consts::TAU;
        let (x, y) = to_dot(a.sin(), -a.cos());
        c.dot(x, y, pal::FRAME);
        if k % 2 == 0 {
            let (x, y) = to_dot(a.sin() * 0.5, -a.cos() * 0.5);
            c.dot(x, y, pal::TRACE);
        }
    }
    let centre = w.ships.first().map(|p| p.pos).unwrap_or_default();
    let place = |p: crate::sim::v2::V2| -> ((i32, i32), bool) {
        let d = (p - centre) * (1.0 / EDGE);
        let l = d.len();
        let (d, out) = if l > 1.0 { (d * (1.0 / l), true) } else { (d, false) };
        (to_dot(d.x, d.y), out)
    };
    for r in &w.rocks {
        let ((x, y), out) = place(r.pos);
        if !out {
            c.dot(x, y, Color::Rgb(90, 82, 74));
        }
    }
    let blob = |c: &mut super::canvas::Canvas, (x, y): (i32, i32), col: Color| {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            c.dot(x + dx, y + dy, col);
        }
    };
    for b in &w.bases {
        blob(&mut c, place(b.pos).0, pal::TRAFFIC);
    }
    for to in w.sector.neighbours(w.region) {
        let (at, out) = place(w.sector.mouth(w.region, to));
        blob(&mut c, at, if out { rgb(pal::scale(pal::LANE, 0.55)) } else { rgb(pal::LANE) });
    }
    for s in w.ships.iter().filter(|s| s.alive && s.side != Side::Player) {
        let ((x, y), out) = place(s.pos);
        let is_target = w.target == Some(s.id);
        // hostiles red whoever they are; everyone else in their faction's colour
        let base = if w.hostile_to_player(s) { pal::PIRATE } else { pal::faction(s.faction) };
        let col = if is_target { pal::TARGET } else if out || w.fleeing(s.id) { rgb(pal::scale(base, 0.55)) } else { rgb(base) };
        c.dot(x, y, col);
        if is_target {
            c.dot(x + 1, y, col);
            c.dot(x, y + 1, col);
            c.dot(x + 1, y + 1, col);
        }
    }
    if let Some(p) = w.player() {
        let f = p.forward();
        for k in 0..3 {
            let (x, y) = to_dot(f.x * 0.04 * k as f32, f.y * 0.04 * k as f32);
            c.dot(x, y, rgb(pal::PLAYER));
        }
    }
    c.blit(buf, inner, pal::CHROME_BG);
}

/// The target box: callsign, role, the hull's wireframe, its shields and
/// armour per facing, its hull, distance and speed.
pub fn target(buf: &mut Buffer, area: Rect, w: &World) {
    let block = frame("target");
    let inner = block.inner(area);
    block.render(area, buf);
    let Some(t) = w.target.and_then(|id| w.ship(id)) else {
        let lines = vec![Line::from(label("no target")), Line::from(label("T next · R nearest"))];
        Paragraph::new(lines).render(inner, buf);
        return;
    };
    let me = w.player();
    let mut lines = vec![
        Line::from(Span::styled(t.callsign.clone(), Style::default().fg(pal::TARGET).add_modifier(Modifier::BOLD))),
        Line::from(vec![
            Span::styled(format!("{} · ", t.faction.name()), Style::default().fg(rgb(pal::faction(t.faction)))),
            label(if w.hostile_to_player(t) { "hostile" } else { t.class.role }),
        ]),
    ];
    let schem = glyphs::schematic(t.class);
    let wire = pal::ramp(&pal::GAUGE, t.hull_frac());
    let facing_row = |name: &str, vals: [f32; 4], max: [f32; 4], base: Option<Rgb>| -> Vec<Span<'static>> {
        let mut v = vec![label(name)];
        for f in Facing::ALL {
            let frac = vals[f.index()] / max[f.index()];
            let col = match base {
                Some(b) => rgb(pal::lerp((40, 70, 110), b, frac)),
                None => pal::ramp(&pal::GAUGE, frac),
            };
            v.push(label(&f.letter().to_string()));
            v.push(Span::styled(level(frac).to_string(), Style::default().fg(col)));
        }
        v
    };
    let info: [Vec<Span<'static>>; 4] = [
        facing_row("SHD ", t.shields, [t.max_shield(); 4], Some(pal::SHIELD)),
        facing_row("ARM ", t.armour, t.max_armour(), None),
        {
            let mut v = vec![label("HULL ")];
            v.extend(bar(t.hull_frac(), 6, wire));
            v
        },
        vec![],
    ];
    let sw = schem.iter().map(|s| s.chars().count()).max().unwrap_or(0).max(8);
    for (k, info) in info.into_iter().enumerate() {
        let s = schem.get(k).copied().unwrap_or("");
        let mut row = vec![Span::styled(format!("{s:^sw$} "), Style::default().fg(wire))];
        row.extend(info);
        lines.push(Line::from(row));
    }
    if let Some(me) = me {
        let dist = distance((t.pos - me.pos).len());
        let mut l = vec![label("RANGE "), Span::styled(dist, Style::default().fg(pal::TEXT)), label("  SPD "), Span::styled(format!("{:.0}", t.speed()), Style::default().fg(pal::TEXT))];
        if w.fleeing(t.id) {
            l.push(Span::styled("  running", Style::default().fg(pal::DIM)));
        }
        lines.push(Line::from(l));
    }
    Paragraph::new(lines).render(inner, buf);
}

