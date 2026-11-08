# Saalis

**THE HIGH BELT DOESN'T FORGIVE.**

Flight, combat and trade in the Northern Reach, the cold, high fringe of
the asteroid belt. You fly a light courier-fighter out of Routa Station,
with 2,000 credits and an empty hold. Nine belt regions are joined by
drive lanes. Mines sell ice and ore, refineries want them, farms grow the
Reach's food and air, and every market moves with what you buy and sell.
Claim-jumpers work the lanes, worst where no station watches. The
cooperatives' patrols hunt them, freighters run between the bases, and
the guild boards and the fixers in the bars always have a job with a
deadline. Shipyards sell bigger hulls, guns, plating, shields, missiles
and turrets. Stims and unlicensed AI cores pay well where they're
banned, if the customs scans miss the hidden compartment. And the
claim-jumpers' fleets are taking the Reach region by region, unless
someone stops them. Fly and
fight in real time from a top-down view. Shields recharge from one energy
pool and guard four facings. Under them is armour that only a dock can
patch.

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
| F | launch a missile (it seeks when LOCK shows) |
| T | next target |
| R | nearest target |
| N | nav map (Tab in it: the whole Reach) |
| [ ] | previous, next nav point |
| G | autopilot to the nav point |
| J | jump through a lane mouth |
| L | land at a base (close and slow) |
| C | hail the target, or the nearest ship |
| P | pause |
| F8 | sound on or off |
| F1 | keys |
| Esc | quit |

Docked, you walk the base on foot: arrows, WASD or hjkl move (y u b n
and the numpad go diagonally), and walking into someone or something uses
it. The broker's counter opens the exchange, the guild office's terminals
the mission boards, the yard master's counter the shipyard, the fixer in
the bar their job; the mechanic beside your ship repairs it, and walking
into your ship launches. Enter talks to or uses whatever is beside you;
walking into a passer-by squeezes past them. At the exchange ↑ ↓ choose
a good, B or → buys, S or ← sells (Shift for ten). On the boards and in the bar A takes a
job, and 1 or 2 joins a guild. Esc steps away from a counter. F saves;
every landing saves the game, and C on the title continues it.

Boarding: a ship's last hit sometimes leaves it dead in space instead of
blowing it apart (it shows grey, and the target box says so). Target it,
come alongside slowly and press L to clamp on and board. Aboard, the crew
fight you a turn at a time: F fires at the nearest one in sight (marked),
walking into someone hits them, M uses a medkit. Some give up. Walk into
crates and the cockpit's console to take what they hold, and back into
the hatch (`<`) to cast off; the wreck is scuttled behind you. Boarding a
lawful ship is piracy. Sidearms, vests and medkits are sold at
outfitters; a landing heals you. `--board` starts a new game already
clamped onto a disabled raider.

Quests: some people on the decks have a story to tell. They stand
underlined; talk to them with Enter. Their lines come up in a box: Enter
reads on, and a question is answered with its number. What each quest
asks of you next is in the journal beside the deck, on the nav map and on
the mission boards. Quests are written in Lua, and you can write your own:
see `QUESTS.md`.

Gamepad: the left stick turns and throttles, A fires, B burns, the right
bumper picks the next target, the left bumper the nearest, Start pauses.

## Options

```
saalis --play      straight into a new game
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
