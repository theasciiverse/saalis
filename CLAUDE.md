# Saalis

An Asciiverse space game of flight, combat, trading and missions in the
line of Elite, Wing Commander: Privateer and above all **Ascii Sector**
(Laserbrain Studios), rebuilt for the terminal in Rust + Ratatui with a
universe of our own.

**Status:** folder and notes only, 2026-10-20. No code. The direction is
settled (a spiritual successor, see below); scope, universe and art are
still open (see the end).

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

## Open questions (David's to decide)

- [ ] The universe: setting, factions, why the player is a privateer
- [ ] Scope of a first playable version (flight and combat core first?)
- [ ] Art: classic ASCII ships as in Ascii Sector, or Braille and
      half-block sub-cell drawing like other Asciiverse titles
- [ ] On-foot parts and boarding: in the first version or later
- [ ] Quest scripting language: own format, or an embedded language
- [ ] Subtitle and motto, if any
- [ ] Trademark checks (EUIPO, PRH, USPTO) before announcing
