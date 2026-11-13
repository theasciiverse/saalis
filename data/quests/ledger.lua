-- The Ledger: the story's fourth chapter (STORY.md). Sten Korpi makes his
-- offer, and the Lantern's key waits in the hulk of its builder ship.

local KEY = "the Lantern's key"

return {
  start = function()
    objective(nil)
    wait(function() return done("cold") end)
    place("korpi")
    objective(text.hook)
    wait_talk("korpi")
    say("korpi", text.hello)
    say("korpi", text.why)
    say("korpi", text.offer)
    local a = choose("korpi", text.q, { text.a1, text.a2, text.a3 })
    if a == 1 then
      say("korpi", text.think)
    elseif a == 2 then
      say("korpi", text.robbed)
    else
      say("korpi", text.never)
    end
    set_flag("korpi_offer", true)
    return "key"
  end,

  key = function()
    objective(text.kalmisto)
    if not vars.asked then
      wait_flying()
      say("The Lantern", text.lantern1)
      say("The Lantern", text.lantern2)
      vars.asked = true
    end
    wait(function() return region() == "Kalmisto" and flying() end)
    wait_hours(0.1)
    spawn{ tag = "hulk", class = "Haahka", callsign = "Ensimmäinen", item = KEY, wreck = true, crew = 2, distance = 2500 }
    spawn{ tag = "scavengers", class = "Ahma", skill = 0.5, callsign = "Scavenger" }
    spawn{ tag = "scavengers", class = "Karppa", count = 2, skill = 0.45, callsign = "Scavenger" }
    say("The Lantern", text.spotted)
    say("Lane beacon", text.squatters)
    while true do
      if has(KEY) > 0 then return "found" end
      if alive("hulk") == 0 then
        say("The Lantern", text.lost)
        return "key"
      end
      pause()
    end
  end,

  found = function()
    say("You", text.found)
    say("The Lantern", text.lantern3)
    set_flag("key", true)
    finish(text.done)
  end,
}
