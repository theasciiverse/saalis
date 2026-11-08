# Saalis roadmap

Versions are 0.x.0 while unfinished; 1.0.0 when this list is done.

## 0.1.0: flight and combat sandbox

Built 2026-11-07. Still to do: a person plays it and tunes it.

- [x] Crate set up from Kaiku's loop, input, gamepad, canvas, palette and audio
- [x] Session log, too-small message, centred and resize-following layout
- [x] Privateer-style flight: turn rate, set/actual speed, afterburner
- [x] Guns, one energy pool, four-facing shields over armour
- [x] Two or three enemy types with steering AI (experience, morale)
- [x] Space view: Braille starfield, ASCII fighters, half-block station and asteroids
- [x] HUD: shields/armour, energy, speed, radar, target box, comms line
- [x] Explosions and synthesised sound
- [x] `--demo` autopilot and the first preview cast

## 0.2.0: the Reach

Built 2026-11-07.

- [x] Belt regions joined by drive lanes, nav and sector maps, autopilot
- [x] Landing, base menus, commodity exchange, saving

## 0.3.0: people

Built 2026-11-07.

- [x] Factions and attitude, comms, encounters by region owner
- [x] Generated missions with deadlines, guilds, fixers

## 0.4.0: the shipyard

Built 2026-11-07.

- [x] Hulls and upgrades, missiles and ECM, turrets, repair
- [x] Contraband, hidden compartments, customs scans

## 0.5.0: a living belt

Built 2026-11-07, all but the release itself.

- [x] Faction fleets and region control
- [ ] Catalog entry and first public release: drafts in `asciiverse/saalis.toml` and `.md`;
      the hub entry, pushing and tagging are David's call

## 0.6.0 to 0.8.0

Decided 2026-11-08: on foot is classic ASCII; quests are Lua 5.4 (mlua) with
TOML data (see `CLAUDE.md`).

- [x] Walkable bases (0.6.0, built 2026-11-08): classic ASCII decks, the
      counters open the old dock panels, the crowd wanders
- [x] On-foot combat and boarding disabled ships (0.7.0, built 2026-11-08):
      ships can be knocked out, boarded with L, the crew fought turn by turn
      on a ship deck, the hold and console looted
- [x] Quest format and loader (0.8.0, built 2026-11-08): Lua 5.4 stages
      with TOML casts, sandboxed; the game's first quest, The Jumped Claim;
      players' own quests from the save folder (`QUESTS.md`)

## 1.0.0
- [ ] Written story, balance, full content
