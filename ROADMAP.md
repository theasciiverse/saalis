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
- [x] Catalog entry and first release: tagged with 0.8.0 on 2026-11-13, after a
      round of play and tuning (the autopilot, the first fights)

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

## 0.9.0: the story begins

Built 2026-11-13. The written story is in `STORY.md`.

- [x] The quest engine grows for a story: other quests' progress
      (`done`), shared flags, who holds a region, handing regions over,
      opening lanes, an ending card, empty wrecks to salvage
- [x] The Lantern, chapters 2 to 4: What Lies Under Kide, Cold Light,
      The Ledger
- [x] Side stories: Thin Air (co-op), Old Debts, Licence Revoked (Inner
      Customs)
- [x] More of the Reach: Tuisku Belt, Hopea Ring, Usva Deep, and
      Aamunkoi waiting off the map
- [x] Saves from 0.8 load into the bigger Reach

## 1.0.0

- [ ] The Lantern's end: four choices, four endings, the new lane
- [ ] The rest of the side stories: The Toll (Crown), Running Dry (Free
      Traders)
- [ ] Balance: a pass over pay, prices and fights with the story in
