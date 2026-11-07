# Saalis

**THE HIGH BELT DOESN'T FORGIVE.**

Flight, combat and trade in the Northern Reach, the cold, high fringe of
the asteroid belt. You fly a light courier-fighter out of Routa station,
and claim-jumpers want your hull. Turn, throttle, burn and shoot in real
time from a top-down view. Shields recharge from one energy pool and guard
four facings. Under them is armour that doesn't come back until the dock
crews patch it.

## Play

With [av](https://asciiverse.sh), the Asciiverse client:

```
av install saalis
av play saalis
```

To build it yourself: `cargo run --release`.

Needs a truecolour terminal of at least 80x24; 100x30 or more is
comfortable.

## Keys

| Key | |
|---|---|
| ← → or A D | turn |
| ↑ ↓ or W S | throttle up, down |
| Home | full speed |
| End, Backspace | stop |
| M | match the target's speed |
| Space | fire guns (hold) |
| Tab | afterburner (hold) |
| T | next target |
| R | nearest target |
| P | pause |
| F8 | sound on or off |
| F1 | keys |
| Esc | quit |

Gamepad: the left stick turns and throttles, A fires, B burns, the right
bumper picks the next target, the left bumper the nearest, Start pauses.

## Options

```
saalis --play      straight into the cockpit
saalis --demo      the game plays itself (any key ends it)
saalis --seed N    the same region and waves each time
saalis --mute      no sound
```

On terminals that can't report key releases, held keys are read from key
repeat. `SAALIS_INPUT=latch` makes turning, the guns and the afterburner
toggles instead.

Session log for bug reports: `~/.local/state/saalis/session.log`
(`%LOCALAPPDATA%\saalis\session.log` on Windows), written fresh each run.

## Credits

Design and code: Asciiverse, in Rust with Ratatui. Inspired by Ascii
Sector and the space trading games before it. The universe, ships, names,
art and sound are original. The sound is synthesised in the game.

## Licence

MIT, see `LICENSE`.
