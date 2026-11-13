//! The nav map (N): this region from above, or the whole Reach (Tab).
//! Routes and rocks are Braille; bases and lane mouths are marked in text;
//! the frame is box-drawing.

use super::canvas::Canvas;
use super::hud::{distance, frame};
use super::palette::{self as pal, rgb};
use crate::app::App;
use crate::sim::sector::REGION_RADIUS;
use crate::sim::v2::V2;
use crate::sim::Nav;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Widget};

const LIST_W: u16 = 32;

pub fn draw(buf: &mut Buffer, area: Rect, app: &App, aspect: f32) {
    Clear.render(area, buf);
    let w = &app.world;
    let title = if app.map_sector { "the Northern Reach".to_string() } else { format!("{} · {}", w.region_name(), w.sector.regions[w.region].note) };
    let block = frame("");
    let block = block.title(Span::styled(format!(" NAV · {title} "), Style::default().fg(pal::TEXT)));
    let inner = block.inner(area);
    block.render(area, buf);
    if inner.width < LIST_W + 10 || inner.height < 6 {
        return;
    }
    let map = Rect::new(inner.x, inner.y, inner.width - LIST_W, inner.height - 1);
    let list = Rect::new(inner.right() - LIST_W, inner.y, LIST_W, inner.height - 1);
    let keys = Rect::new(inner.x, inner.bottom() - 1, inner.width, 1);
    let hint = if app.map_sector { "↑↓ choose · Enter plot a course · Tab this region · N close" } else { "↑↓ choose · Enter set nav · G go (autopilot) · Tab the Reach · N close" };
    Paragraph::new(Line::from(Span::styled(hint, Style::default().fg(pal::DIM)))).render(keys, buf);
    if app.map_sector {
        sector(buf, map, list, app);
    } else {
        region(buf, map, list, app, aspect);
    }
}

fn region(buf: &mut Buffer, map: Rect, list: Rect, app: &App, aspect: f32) {
    let w = &app.world;
    // metres per column, so the whole region fits and stays round
    let scale = (REGION_RADIUS * 2.1 / map.width as f32).max(REGION_RADIUS * 2.1 / (map.height as f32 * aspect));
    let (cx, cy) = (map.width as f32 / 2.0, map.height as f32 / 2.0);
    let cell_f = |p: V2| -> (f32, f32) { (cx + p.x / scale, cy + p.y / (scale * aspect)) };
    let mut c = Canvas::new(map.width, map.height);
    for r in &w.rocks {
        let (x, y) = cell_f(r.pos);
        c.dot((x * 2.0) as i32, (y * 4.0) as i32, Color::Rgb(100, 92, 84));
        if r.radius > 250.0 {
            c.dot((x * 2.0) as i32 + 1, (y * 4.0) as i32, Color::Rgb(100, 92, 84));
        }
    }
    let navs = w.navs();
    let sel = navs.get(app.map_sel).copied();
    // a dotted line from the ship to the highlighted nav point
    if let (Some(n), Some(p)) = (sel.or(w.nav), w.player()) {
        let (a, b) = (cell_f(p.pos), cell_f(w.nav_pos(n)));
        let steps = (((b.0 - a.0).abs() * 2.0).max((b.1 - a.1).abs() * 4.0) as i32).max(1);
        for i in (0..=steps).step_by(2) {
            let t = i as f32 / steps as f32;
            c.dot(((a.0 + (b.0 - a.0) * t) * 2.0) as i32, ((a.1 + (b.1 - a.1) * t) * 4.0) as i32, rgb(pal::scale(pal::PLAYER, 0.6)));
        }
    }
    c.blit(buf, map, pal::CHROME_BG);
    let put = |buf: &mut Buffer, p: V2, mark: &str, name: &str, style: Style| {
        let (x, y) = cell_f(p);
        let (x, y) = (x as i32, y as i32);
        if x < 0 || y < 0 || x >= map.width as i32 || y >= map.height as i32 {
            return;
        }
        buf.set_string(map.x + x as u16, map.y + y as u16, mark, style);
        let room = map.width as i32 - x - 2;
        if room > 2 {
            let text: String = name.chars().take(room as usize).collect();
            buf.set_string(map.x + x as u16 + 2, map.y + y as u16, text, style);
        }
    };
    for (i, &n) in navs.iter().enumerate() {
        let (mark, col) = match n {
            Nav::Base(_) => ("◆", pal::TRAFFIC),
            Nav::Mouth(_) => ("◇", rgb(pal::LANE)),
        };
        let mut st = Style::default().fg(col);
        if i == app.map_sel {
            st = st.add_modifier(Modifier::REVERSED);
        }
        let name = match n {
            Nav::Base(b) => w.bases[b].name.to_string(),
            Nav::Mouth(to) => format!("▸ {}", w.sector.regions[to].name),
        };
        put(buf, w.nav_pos(n), mark, &name, st);
    }
    if let Some(p) = w.player() {
        const ARROWS: [&str; 8] = ["↑", "↗", "→", "↘", "↓", "↙", "←", "↖"];
        let i = ((p.heading / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(8) as usize;
        put(buf, p.pos, ARROWS[i], "you", Style::default().fg(rgb(pal::PLAYER)).add_modifier(Modifier::BOLD));
    }
    // the list
    let mut lines = vec![Line::from(Span::styled("NAV POINTS", Style::default().fg(pal::DIM))), Line::from("")];
    for (i, &n) in navs.iter().enumerate() {
        let d = w.player().map_or(0.0, |p| (w.nav_pos(n) - p.pos).len());
        let mut name = w.nav_name(n);
        let room = LIST_W as usize - 10;
        if name.chars().count() > room {
            name = name.chars().take(room - 1).collect::<String>() + "…";
        }
        let current = w.nav == Some(n);
        let mut st = Style::default().fg(if current { pal::TARGET } else { pal::TEXT });
        if i == app.map_sel {
            st = st.add_modifier(Modifier::REVERSED);
        }
        lines.push(Line::from(vec![Span::styled(format!("{name:<room$}"), st), Span::styled(format!("{:>9}", distance(d)), Style::default().fg(pal::DIM))]));
    }
    if !w.course.is_empty() {
        let dest = w.sector.regions[*w.course.last().unwrap()].name;
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(format!("course: {dest}, {} lanes", w.course.len()), Style::default().fg(rgb(pal::LANE)))));
    }
    if !w.missions.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("JOBS", Style::default().fg(pal::DIM))));
        for m in &w.missions {
            let d: String = m.describe(&w.sector).chars().take(LIST_W as usize - 8).collect();
            let left = crate::sim::mission::time_left(m.deadline, w.clock);
            lines.push(Line::from(vec![Span::styled(d, Style::default().fg(pal::TEXT)), Span::styled(format!(" {left}"), Style::default().fg(pal::DIM))]));
        }
    }
    let quests = super::journal::lines(app, list.width as usize, false, (list.height as usize).saturating_sub(lines.len() + 1));
    if !quests.is_empty() {
        lines.push(Line::from(""));
        lines.extend(quests);
    }
    Paragraph::new(lines).render(list, buf);
}

fn sector(buf: &mut Buffer, map: Rect, list: Rect, app: &App) {
    let w = &app.world;
    let s = &w.sector;
    let place = |m: V2| -> (f32, f32) { (2.0 + m.x * (map.width as f32 - 18.0), 1.0 + m.y * (map.height as f32 - 3.0)) };
    let mut c = Canvas::new(map.width, map.height);
    let on_course = |a: usize, b: usize| {
        let mut path = vec![w.region];
        path.extend(&w.course);
        path.windows(2).any(|p| (p[0] == a && p[1] == b) || (p[0] == b && p[1] == a))
    };
    for &(a, b) in &s.lanes {
        let (p, q) = (place(s.regions[a].map), place(s.regions[b].map));
        let col = if on_course(a, b) { rgb(pal::LANE) } else { rgb(pal::scale(pal::LANE, 0.4)) };
        let steps = (((q.0 - p.0).abs() * 2.0).max((q.1 - p.1).abs() * 4.0) as i32).max(1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            c.dot(((p.0 + (q.0 - p.0) * t) * 2.0) as i32, ((p.1 + (q.1 - p.1) * t) * 4.0) as i32, col);
        }
    }
    c.blit(buf, map, pal::CHROME_BG);
    buf.set_string(map.x + 1, map.y, "high fringe ↑", Style::default().fg(pal::TRACE));
    buf.set_string(map.x + 1, map.bottom() - 1, "↓ the low belt", Style::default().fg(pal::TRACE));
    // fleets on the lanes, partway along by how far they've come
    for f in &w.war.fleets {
        let (a, b) = (place(s.regions[f.from].map), place(s.regions[f.to].map));
        let t = (1.0 - ((f.eta - w.clock) / crate::sim::war::FLEET_HOURS) as f32).clamp(0.1, 0.9);
        let (x, y) = ((a.0 + (b.0 - a.0) * t) as u16, (a.1 + (b.1 - a.1) * t) as u16);
        if x < map.width && y < map.height {
            let ang = crate::sim::v2::v2(b.0 - a.0, (b.1 - a.1) * 2.0).heading();
            const ARROWS: [&str; 8] = ["▲", "◥", "▶", "◢", "▼", "◣", "◀", "◤"];
            let i = ((ang / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(8) as usize;
            buf.set_string(map.x + x, map.y + y, ARROWS[i], Style::default().fg(rgb(pal::faction(f.faction))).add_modifier(Modifier::BOLD));
        }
    }
    for (i, r) in s.regions.iter().enumerate().filter(|&(i, _)| s.shown(i)) {
        let (x, y) = place(r.map);
        let (x, y) = (x as u16, y as u16);
        if x >= map.width || y >= map.height {
            continue;
        }
        // the name in the colour of whoever holds it now
        let col = rgb(pal::faction(w.war.control[i]));
        let mut st = Style::default().fg(col);
        if i == app.map_sel {
            st = st.add_modifier(Modifier::REVERSED);
        }
        let mark = if i == w.region { "◉" } else if r.bases.is_empty() { "○" } else { "●" };
        buf.set_string(map.x + x, map.y + y, mark, Style::default().fg(col).add_modifier(Modifier::BOLD));
        let room = map.width.saturating_sub(x + 2) as usize;
        let name: String = r.name.chars().take(room).collect();
        buf.set_string(map.x + x + 2, map.y + y, name, st);
    }
    let mut lines = vec![Line::from(Span::styled("REGIONS", Style::default().fg(pal::DIM))), Line::from("")];
    for (i, r) in s.regions.iter().enumerate().filter(|&(i, _)| s.shown(i)) {
        let mut st = Style::default().fg(if i == w.region { pal::TARGET } else { pal::TEXT });
        if i == app.map_sel {
            st = st.add_modifier(Modifier::REVERSED);
        }
        let threat = w.war.threat(s, i);
        let danger = "▮".repeat(threat as usize) + &" ".repeat(3 - threat.min(3) as usize);
        let holder = w.war.control[i];
        let short = match holder {
            crate::sim::faction::Faction::Coop => "co-op",
            crate::sim::faction::Faction::Crown => "Crown",
            crate::sim::faction::Faction::Traders => "traders",
            crate::sim::faction::Faction::Inner => "customs",
            crate::sim::faction::Faction::Pirates => "pirates",
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{:<15}", r.name), st),
            Span::styled(danger, Style::default().fg(threat_colour(threat))),
            Span::styled(format!(" {short}"), Style::default().fg(rgb(pal::faction(holder)))),
        ]));
    }
    if let Some(r) = s.regions.get(app.map_sel) {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(r.note.to_string(), Style::default().fg(pal::DIM))));
        let holder = w.war.control[app.map_sel];
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", holder.name()), Style::default().fg(rgb(pal::faction(holder)))),
            Span::styled(format!("hold {:.0}%", w.war.garrison[app.map_sel]), Style::default().fg(pal::DIM)),
        ]));
        let hops = s.route(w.region, app.map_sel).len();
        if app.map_sel != w.region {
            lines.push(Line::from(Span::styled(format!("{hops} lane{} away", if hops == 1 { "" } else { "s" }), Style::default().fg(rgb(pal::LANE)))));
        }
        for b in &r.bases {
            lines.push(Line::from(Span::styled(format!("◆ {} ({})", b.name, b.kind.name()), Style::default().fg(pal::TRAFFIC))));
        }
    }
    Paragraph::new(lines).render(list, buf);
}

pub fn threat_colour(t: u32) -> Color {
    match t {
        0 => Color::Rgb(110, 220, 170),
        1 => Color::Rgb(170, 210, 140),
        2 => Color::Rgb(240, 190, 80),
        _ => Color::Rgb(240, 90, 80),
    }
}
