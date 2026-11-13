# Writing quests for Saalis

A quest is two files with the same name:

- `<name>.toml` holds what the quest is called, its people, and its lines.
- `<name>.lua` holds what happens, written in Lua 5.4.

Put both in the `quests` folder inside Saalis's save folder:

| System | Folder |
|---|---|
| Linux | `~/.local/state/saalis/quests/` |
| macOS | `~/Library/Application Support/saalis/quests/` |
| Windows | `%APPDATA%\saalis\quests\` |

Every quest there is loaded when a game starts or is continued. The game's
own quest, *The Jumped Claim*, is in `data/quests/` in the source and is
the example to copy.

If a quest has a mistake, it does not stop the game. The quest shows as
*broken* in the journal (beside the deck, on the map and on the mission
boards), with the first line of the error. The whole message, with the
file and line, is in the session log (`~/.local/state/saalis/session.log`
on Linux).

## The TOML file

```toml
title = "The Jumped Claim"        # shown in the journal

[people.ilta]                     # "ilta" is the id the script uses
name = "Ilta Rask"
glyph = "i"                       # their letter on the deck
faction = "coop"                  # coop, crown, traders, inner or pirates: their colour
base = "Routa Station"            # where they stand, by the base's name
room = "bar"                      # bar, exchange, boards, yard, hall or hangar; the corridor if left out
idle = "Kide Drift. You can't miss it."   # what they say when the script isn't listening

[text]                            # lines, read in the script as text.hello and so on
hello = "You fly? Sit."
```

People appear only after the script calls `place("ilta")`. They are
underlined on the deck, and you talk to them with Enter or by walking into
them.

## The Lua file

The file returns a table of **stages**. Each stage is a function, and the
game starts with `start`. A stage runs in order and pauses wherever it
waits. When it returns the name of another stage, that stage begins. When
it returns nothing, the quest is over.

```lua
return {
  start = function()
    place("ilta")
    objective("Someone in Routa Station's bar is asking for a pilot.")
    wait_talk("ilta")
    say("ilta", text.hello)
    if choose("ilta", "Will you help?", { "Yes", "No" }) == 1 then
      return "hunt"
    end
    return "start"
  end,

  hunt = function()
    objective("Find the jumpers in Kide Drift.")
    wait(function() return region() == "Kide Drift" and flying() end)
    spawn{ tag = "jumpers", class = "Ahma", item = "a claim beacon" }
    wait(function() return has("a claim beacon") > 0 end)
    pay(1500)
    finish("The claim is safe.")
  end,
}
```

**Saving.** The game saves which stage each quest is in, its `vars` table
and its objective. A loaded game starts that stage again from its top, so
think of each stage as a checkpoint:

- Things that must happen once (paying, spawning) belong just before a
  stage returns, or behind a flag in `vars`.
- `vars` keeps numbers, strings, true and false, and tables of these:
  lists without gaps (`{"a", "b"}`) and tables with string keys
  (`{seen = true}`). Other keys, such as `vars.done[3] = true` with no
  1 and 2, are lost when the game saves.

```lua
if not vars.paid then vars.paid = true; pay(300) end
```

**Waiting.** A script must wait now and then. A loop that runs for a long
time without waiting is stopped and the quest breaks. Use `pause()` inside
any loop that checks something every moment.

## What a script can ask

| Function | Returns |
|---|---|
| `hours()` | the game clock in hours from the start (a number) |
| `day()` | the day, from 1 |
| `region()` | the region you are in, e.g. `"Kide Drift"` |
| `docked()` | the base you are docked at, or nil |
| `flying()` | true while you fly |
| `aboard()` | true while you are aboard a boarded ship |
| `on_foot()` | true while you walk a deck |
| `credits()`, `room()`, `kills()` | your credits, free hold space, pirates downed |
| `cargo(good)` | how many of a good you carry (`"Water ice"`, or one word of it: `"ice"`) |
| `standing(faction)` | your standing with a faction, -125 to 125 |
| `has(item)` | how many of an item you hold |
| `alive(tag)` | how many ships with that tag are still out there, knocked out or not |
| `disabled(tag)` | how many of them are knocked out |
| `boarded(tag)` | true while you are aboard one of them |
| `talked(id)` | true at the moment you talk to that person |
| `random(m, n)` | a whole number from m to n (`math.random` is the same) |

## What a script can do

| Function | Does |
|---|---|
| `objective(text)` | sets the line in the journal; `objective(nil)` hides the quest |
| `place(id)`, `remove(id)` | puts a person from the TOML on their deck, or takes them off |
| `say(who, text)` | on foot, a line in the talk box; in flight, a line on the comms. `who` is a person's id or any name |
| `choose(who, question, {options})` | asks, and waits for the answer (1 for the first option). Shown on foot only, so it waits until you are walking |
| `pay(n)`, `charge(n)` | gives credits; takes them if you have enough (returns true if it did) |
| `give_cargo(good, n)` | puts cargo in your hold; returns how many fitted |
| `take_cargo(good, n)` | takes cargo if you carry that much (true if it did) |
| `give_item(name)`, `take_item(name)` | items are named things you carry: a beacon, a letter, a key |
| `shift(faction, n)` | changes your standing |
| `spawn{ ... }` | ships near you, while you fly; returns how many came (see below) |
| `finish(text)`, `fail(text)` | ends the quest, saying `text` |
| `print(...)`, `log(...)` | writes to the session log |

The `wait` family:

| Function | Waits until |
|---|---|
| `wait(f)` | `f()` returns true |
| `pause()` | the next moment |
| `wait_hours(h)` | `h` game hours have passed |
| `wait_docked(base)` | you are docked at that base |
| `wait_region(name)` | you are in that region |
| `wait_flying()` | you fly |
| `wait_talk(id)` | you talk to that person |
| `wait_boarded(tag)` | you are aboard a ship with that tag |

In flight, one real second is one game minute.

### spawn

```lua
spawn{
  tag = "jumpers",        -- required: the name alive() and boarded() know them by
  class = "Ahma",         -- Kärppä (or Karppa), Ahma, Karhu, Kotka, Haahka, Tiira, Kuikka, Lokki, Haukka
  side = "pirate",        -- pirate (attacks you), patrol, or trader
  faction = "coop",       -- for patrols and traders
  count = 1,              -- up to 8
  callsign = "Ahma Varis",
  item = "a claim beacon",  -- found in a crate in the first ship's hold when you board it
  wreck = false,          -- true: already dead in space, waiting to be boarded
  skill = 0.5,            -- 0 to 1: how well they fly and shoot
  distance = 3500,        -- metres from you
}
```

A ship carrying an item is always knocked out rather than destroyed by
the last hit, so it can be boarded, and it doesn't leave the region
however far you fly. Shooting the wreck, or casting off from it without
the item, destroys it and the item with it. Pirates that are far from
you leave the region, and every ship is gone once you dock or take a
lane. So a quest that spawns ships should be ready to spawn them again:
see `hunt` in `data/quests/claim.lua`.

## The sandbox

Quests run in a sandbox. There is no `io`, `os`, `require`, `dofile`,
`loadfile` or `debug`, so a quest cannot read or write files or run
programs. `load` accepts text only. The libraries are `table`, `string`
and `math`, and of `coroutine` only `yield` (the game runs each stage as
a coroutine; a quest can't make its own). `__gc` metamethods are refused.
Each quest has its own globals and its own copies of the libraries, so
one quest can't change another. Memory and running time are limited.
The time limit counts Lua's own steps, not time spent inside a single
library call, so a monstrous string pattern on a long string can still
stall the game.
