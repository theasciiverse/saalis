-- The Jumped Claim: Ilta Rask's beacon, taken by claim-jumpers in Kide
-- Drift. Each stage reads in order; returning a stage's name moves on, and
-- a loaded game starts the stage it was saved in again from its top.

local BEACON = "Ilta's claim beacon"

return {
  start = function()
    place("ilta")
    objective(text.hook)
    wait_talk("ilta")
    say("ilta", text.hello)
    return "ask"
  end,

  ask = function()
    while true do
      local a = choose("ilta", text.ask, { text.yes, text.pay, text.no })
      if a == 1 then break end
      if a == 2 then
        say("ilta", text.offer)
        vars.offered = true
      else
        say("ilta", text.refused)
        wait_talk("ilta")
      end
    end
    say("ilta", text.thanks)
    if vars.offered and not vars.advance then
      vars.advance = true
      pay(300)
    end
    return "hunt"
  end,

  hunt = function()
    objective("Find the jumpers' Ahma in Kide Drift. Knock it out, board it (L) and take the beacon from its hold.")
    wait(function() return region() == "Kide Drift" and flying() end)
    wait_hours(0.15)
    spawn{ tag = "carrier", class = "Ahma", callsign = "Ahma Varis", item = BEACON, skill = 0.35 }
    spawn{ tag = "jumpers", class = "Karppa", count = 1, skill = 0.3 }
    say("ilta", text.spotted)
    while true do
      if has(BEACON) > 0 then return "home" end
      -- gone without the beacon: scuttled, shot to pieces, or left
      -- behind at a dock or a lane. Jumpers come back to a claim.
      if alive("carrier") == 0 then
        if region() == "Kide Drift" and flying() then
          say("ilta", text.lost)
        else
          say("ilta", text.fled)
        end
        return "hunt"
      end
      pause()
    end
  end,

  home = function()
    say("ilta", text.found)
    objective(text.home)
    wait_talk("ilta")
    take_item(BEACON)
    say("ilta", text.back)
    say("ilta", text.hint)
    pay(1500)
    shift("coop", 5)
    finish(text.done)
  end,
}
