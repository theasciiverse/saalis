# Saalis

An Asciiverse space game of flight, combat, trading and missions in the
line of Elite, Wing Commander: Privateer and above all **Ascii Sector**
(Laserbrain Studios), rebuilt for the terminal in Rust + Ratatui with a
universe of our own.

**Status (2026-11-07):** 0.2.0 is built.
- Nine belt regions joined by lanes, with seven bases and their markets.
- The nav map, plotted courses, the autopilot (it drops out when
  claim-jumpers turn up on the way), jumps, landing, the exchange,
  repairs, and saving at the dock.
- Tested by unit tests and by a scripted tmux run: buy, launch,
  autopilot, jump, autopilot, land, autosave, continue.
- No person has played it yet, so prices, encounter rates and balance
  are untuned.

Design decisions are below, and the release plan is in `ROADMAP.md`.

## Running

```bash
cargo run --release                   # title (the game flies itself behind it)
cargo run --release -- --play         # straight into a new game, docked at Routa
cargo run --release -- --demo --seed 7
cargo test                            # sim, input, audio, frames at every size
SAALIS_PRINT=1 cargo test a_frame -- --nocapture   # one 100x30 frame as text
asciiverse/record-preview.sh          # the 100x30 catalog cast (needs asciinema 3)
```

Saved game: `~/.local/share/saalis/save.json` (`SAALIS_DATA_DIR` moves
it). Saves happen only at the dock: on every landing, and with F.

Session log: `~/.local/state/saalis/session.log`, started fresh each run.
It holds every raw key, every state change, the comms, kills and waves,
and a snapshot every 500 ms. `SAALIS_LOG=info` gives less;
`SAALIS_LOG_DIR` puts it somewhere else.

Screenshots: run in tmux on a private socket (`tmux -L saalis`), then
`capture-pane -e -p` and `../panzer/tools/ansi2png.py` (its font lacks
Braille, so stars and the radar show as boxes there).

## Code layout

- `src/sim/`: the simulation, with no terminal code.
  - `ship.rs`: hull classes, flight, facings and damage.
  - `ai.rs`: pilots (lead aim, break, flee); also the demo autopilot.
  - `mod.rs`: the world: regions, bases, the clock (a game minute per
    second of flight), the autopilot, jumps, landing, trade and repair,
    shots, collisions, encounters, comms, the log snapshot.
  - `sector.rs`: the Reach's regions, lanes, bases and rocks.
  - `econ.rs`: goods and markets. Prices follow stock, and stock drifts
    back by the hour.
  - `save.rs`: the one save slot, as versioned JSON.
  - `v2.rs`: vectors. Heading 0 is up and angles grow clockwise.
  - `rng.rs`, `cue.rs`.
- `src/render/`: drawing.
  - `view.rs`: the camera, with world metres to cells, pixels and dots.
    The field shows about 1500 m across.
  - `space.rs`: Braille stars, shots and sparks; half-block rocks, the
    station and fireballs; the box-drawing dock, brackets and edge arrow.
  - `glyphs.rs`: ships (a glyph, plus the Braille nose line and wake with
    crowd detail) and the box-drawing schematics.
  - `hud.rs`: gauges, the radar, the target box and comms.
  - `navmap.rs`: the region and sector maps.
  - `dock.rs`: the base and the exchange.
  - `mod.rs`: the layout and the cards (title, keys, pause, quit, ship
    lost).
- `app.rs`: screens (title, flight, map, dock) and the menus' keys.
- Other modules:
  - `input.rs`: Kaiku's three-tier held keys plus one-shot commands.
  - `pad.rs`: gilrs.
  - `audio.rs`: Kaiku's synth with Saalis cues.
  - `log.rs`: the session log.
- Ship classes are in code for 0.1 (`sim/ship.rs`). They move to
  `data/` when the shipyard arrives (0.4).
- Names are placeholders until the Reach's own names are settled:
  - hulls: Tiira (the player's, `#`), Kärppä (`x`), Ahma (`H`), Karhu
    (`M`)
  - the regions and bases in `sector.rs` (Pallas is real and keeps its
    name)
  - the pirates' callsigns

## Name

**Saalis**, chosen by David 2026-10-20. Finnish for *catch, prey, loot,
plunder, booty*, from *saada* ("to get"), Proto-Finnic *\*saalis*
(en.wiktionary.org/wiki/saalis). One word for both halves of the game:
the trader's catch and the raider's plunder. Said roughly SAA-lis. Sits
beside Kaiku as the second Finnish title.

No subtitle for now (David: keep it simple). Considered and left for
later: "Saalis: He Who Dares Wins" (the SAS motto, Del Boy's catchphrase
in *Only Fools and Horses*, and Alligata's 1985 C64 *Who Dares Wins*),
"Saalis: Fortune Awaits" (clear; only a tabletop zine of that name on
itch.io). A possible in-game motto is the Finnish proverb *Rohkea rokan
syö*, "the brave one eats the soup" (the bold get what they want),
recorded by Henrik Florinus in 1702.

Collision check, 2026-10-20: no game, app, brand or crate named Saalis
found (web search, Steam store search API, itch.io, crates.io). GitHub
has only small personal repos (Finnish hunting and fishing catch logs)
and `ulhaqi12/saalis`, an AI arbitration framework, not a game.
**Not yet checked:** trademark registers (EUIPO, PRH in Finland, USPTO).

Names checked and dropped on the way: Monospace (*Mono-Space* on Steam),
Marque / Letter of Marque (*Marque & Reprisal*, *Caribbean Marque*),
Star Rover (a 2020 Steam game), Space Rover (several itch.io games),
Grid Rover (GridRover, a 2013 C++ console game on SourceForge, now
queued for the catalog itself). Clear but not taken: Rov, Kap, Kapa,
Kapare, Storkap, Kaappari.

## Why not a port of Ascii Sector

Ascii Sector (Christian Knudsen, Laserbrain Studios; 0.7.2 of
2016-01-22, the last release, apparently abandoned) is exactly the kind
of game David wants in the catalog, but it cannot be ported or packaged.
Checked 2026-10-20:

- **Closed source.** Never released as far as anything shows: RogueBasin
  lists it as "Freeware, closed source" (Free Pascal and SDL); the
  official site, the Internet Archive and both GitHub mirrors
  (Irtipp/Ascii-Sector-Mirror, daxime/ASCII-Sector-Resources) hold only
  binaries, data, manuals and community quests. The MS-DOS 0.3.1
  (2008) ships Borland Pascal's protected-mode runtime, also binary only.
- **No redistribution.** The manual's Legalities: "This game is Freeware
  with a few limitations. You may only download it from the official
  website at www.asciisector.net and any sort of distribution
  (cd-magazines, other websites, etc.) is prohibited without prior
  written consent from the author". The two GitHub mirrors carry no
  licence or permission.
- **Not a terminal game.** It draws its ASCII art in its own SDL2 window
  (needs SDL2, SDL2_mixer, SDL2_image, X11).
- **It is itself a Privateer fan remake.** The credits: "Based on the
  computer game Wing Commander: Privateer"; the full title was
  "Privateer: Ascii Sector", with the Gemini sector, Confed, Militia,
  Kilrathi and Privateer's bases. Wing Commander and Privateer are EA
  trademarks (acknowledged in its own manual).

David decided (2026-10-20): no email to Knudsen; Saalis is an original
universe and needs no contact.

## Direction

A spiritual successor, the way Moraen and Ceres keep the play and replace
the content:

- **Kept (mechanics are free to use):** real-time flight and dogfights;
  trading between bases; Guild-style and fixer missions; contraband and
  hidden cargo compartments that patrols can find; ship upgrades and new
  ships; walking around bases and fighting on foot and aboard ships; a
  player quest scripting language (Ascii Sector had one, with a Quest
  Maker's Manual).
- **Ours:** universe, factions, aliens, ships, bases, every name, art,
  music and text. Nothing from Ascii Sector's data files (ships, bases,
  name lists) and nothing from Wing Commander or Privateer.
- **Terminal native** from the first line: Rust + Ratatui, drawn centred
  and following resizes, session logging per the Asciiverse convention.

Reference only, for how the original plays (not to be copied): the
0.7.2 Player's Manual and Quest Maker's Manual, both in
daxime/ASCII-Sector-Resources.

## Decisions (David, 2026-11-07)

### Universe: the Northern Reach

The Northern Reach, *a section of the asteroid belt*. This reading
was confirmed by David on 2026-11-07:

- Our own Solar System's main belt, some centuries ahead.
- "Northern" is literal. The Reach is the high-inclination fringe of the
  belt, above the ecliptic: cold, sparse and expensive to reach. Its
  natural heart is the Pallas family (orbits tilted about 34°).
- The crowded low belt (Ceres, Vesta) is the settled "south", where the
  corporate crown sits.
- **Regions, not star systems:** the map is belt regions (clusters around
  a large body or a family), joined by drive lanes in place of jump
  points. The nav map shows the belt seen from above, with height bands.
- **Factions** (names still to invent): the low-belt corporate crown, the
  Reach mining cooperatives, free traders, Reach pirates and
  claim-jumpers, and an inner-system navy/customs that runs the scans.
- **Commodities** from belt life: ice, volatiles, metals, rare earths,
  hydroponic food, air, medicine and drive parts. Contraband: stims,
  unlicensed AI cores, stolen claims.
- **The inner planets are opposing forces** (David, 2026-11-07). They
  appear as factions in the belt: their fleets, agents, customs and
  corporations show up in the Reach, possibly competing with each other.
  They are not places the player visits. Which planets they are (Earth,
  Mars, ...) and how they relate to each other is still open. The
  inner-system navy/customs in the faction list above belongs to them.
- **The unknown is a found artifact** (David, 2026-11-07), not an alien
  species or visitor. Something was found out in the Reach: what it is,
  who holds it and what it does are still open. It is the seed of the
  story (1.0) and can show up earlier in rumours, fixers' talk and
  contraband.
- All names of rocks, bases, people and ships are our own, with a
  Finnish/Nordic flavour where it fits. Real large bodies (Ceres, Vesta,
  Pallas, Hygiea) may appear under their own names.

### Art: mix by size, one glyph family per role

David: mix by size, "but also deliberately use different [glyphs] for
different parts of the design and UI. For example pipes for clean lines
and silhouettes". Each family has one clear job:

| Role | Family |
|---|---|
| Small and medium craft, pods, pilots | one ASCII glyph per hull, with a Braille nose line and wake (below) |
| Stations, capital ships, planets, asteroids | half-block shaded art (`▀▄█▌▐`, two colours per cell) |
| Clean lines and silhouettes: docking rings, wireframe ship schematics (target box, shipyard), nav-map lanes, HUD frames | box-drawing pipes (`─│╭╮╱╲┼`) |
| Stars, shots, particles, scoop beam, radar, map routes | Braille dots |
| Energy, speed and shield gauges | block eighths (`▁▂▃▅▇`, `▏▎▍▌`) |
| Labels, comms, menus | plain text |

**Ships (David, 2026-11-07).** David chose this after seeing mock-ups.
Ascii Sector used one fixed glyph per ship with a `+` marker on the
heading side. The first Saalis build used rotating stroke clusters, which
read roughly at diagonals. Now each hull is one glyph, bold in its
faction's colour:
- `#` Tiira (the player's), `x` Kärppä, `H` Ahma, `M` Karhu
- **Nose line:** Braille, running out of the nose in the faction's
  colour and brightening to white at the tip. Pirates are red, so they
  don't blend with the wakes.
- **Wake:** Braille engine fire behind the ship, the same for everyone,
  amber cooling to ember.
- **Reading direction:** white always marks the front and dark red the
  back. Braille is finer than a cell, so the line turns smoothly at any
  heading.
- **The wake shows thrust:** an ember at cruise, a flame while speeding
  up, a long hot plume on the afterburner.
- **Toned down in a crowd** (David: "tone down the amount if it gets
  crowded"). The player and the target always draw in full. Others draw
  shorter and dimmer from 5 ships in view, and lose their wakes from 8
  (`glyphs::Detail::crowded`).
- **Capital ships** (later) use half-block art, as above.

Truecolor. Drawn centred and scaled to the window, aspect-correct (circles
stay round). Designed at 100x30 (the preview size), minimum 80x24. The HUD
is ordinary Ratatui widgets around the play field, laid out in Ascii
Sector's proven order with our own look:
- top: the shields and armour diagram, energy, and set and actual speed
- right: the radar and the target box
- bottom: the weapons/damage panel, a lock warning and the comms line

### Flight: Privateer-style

The ship turns at its turn rate, and its velocity eases toward heading x
set speed at the ship's acceleration, with a little drift for feel. An
afterburner gives a boost. A Newtonian assist-off toggle may come later.

### First release: flight and combat sandbox

0.1.0 is one belt region: fly, dogfight two or three enemy types, with the
HUD, radar, sound and `--demo`. Trading, regions and missions follow (see
`ROADMAP.md`).

### Code plan

The template is Kaiku (loop, kitty held-key input, gamepad, Braille and
half-block canvas, palette, cue-driven audio, demo autopilot, headless
frame tests). Other sources:
- Torched: session-log file and TOML options
- Glider: data paths, aspect-correct placement, pause on focus loss,
  `release.yml`
- Armoured Commander II: versioned JSON saves and the dlopen ALSA loader

The simulation stays free of terminal code. It uses its own SplitMix64
RNG.

### House rules that apply (from `asciiverse/CLAUDE.md`)

- **Session logging** with `log` + `env_logger`. Log every raw input
  event where it arrives, before dispatch. Log discrete state changes
  (launch, land, jump, pause, mode switches, death). Log a snapshot
  every 250 ms to 1 s: position, speed, target, region, mode, and what
  is held.
- **Standalone binary**, with its own loop and terminal setup. No shared
  SDK is needed.
- **No sexual content** (a hard rule: bars and pleasure stations stay
  clean). Violence, war, smuggling and drugs as themes are fine.
- **Store text describes the game and nothing else:** no rights hedging,
  no build or layout notes.
- **Versions:** 0.x.0 while unfinished and 1.0.0 when `ROADMAP.md` is
  done, tagged `vX.Y.Z`.
- **The `games/CLAUDE.md` index** lists Saalis under "notes only". Move
  it to "Spiritual successors" once there is code.

## Open questions (David's to decide)

- [ ] On-foot and interior art (half-block floor plans or classic ASCII).
      Deferred by David 2026-11-07: decide once the space view and HUD
      designs are in place.
- [ ] Quest format: own declarative format, or an embedded language
      (such as Rhai). Deferred by David 2026-11-07: review later.
- [ ] Subtitle and motto, if any
- [ ] Trademark checks (EUIPO, PRH, USPTO) before announcing
