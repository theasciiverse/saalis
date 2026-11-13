-- Cold Light: the story's third chapter (STORY.md). Dr. Aino Halme knows
-- the Lantern for a lane seed, and the jumpers' courier carries a ledger.

local LEDGER = "the jumpers' ledger"

return {
  start = function()
    objective(nil)
    wait(function() return done("under") end)
    place("aino")
    objective(text.hook)
    wait_talk("aino")
    say("aino", text.hello)
    say("aino", text.seed)
    say("aino", text.awake)
    say("The Lantern", text.lantern1)
    say("The Lantern", text.lantern2)
    say("aino", text.gone)
    say("aino", text.paid)
    say("aino", text.courier)
    return "hunt"
  end,

  hunt = function()
    objective(text.hunt)
    wait(function() return region() == "Revontuli Gap" and flying() end)
    wait_hours(0.1)
    spawn{ tag = "courier", class = "Karppa", callsign = "Kärppä Viesti", item = LEDGER, skill = 0.4 }
    spawn{ tag = "escort", class = "Karppa", count = 2, skill = 0.45, callsign = "Escort" }
    say("aino", text.spotted)
    while true do
      if has(LEDGER) > 0 then return "home" end
      if alive("courier") == 0 then
        if region() == "Revontuli Gap" and flying() then
          say("aino", text.lost)
        else
          say("aino", text.fled)
        end
        return "hunt"
      end
      pause()
    end
  end,

  home = function()
    say("aino", text.found)
    objective(text.home)
    wait_talk("aino")
    take_item(LEDGER)
    say("aino", text.read)
    say("The Lantern", text.lantern3)
    if not vars.paid then
      vars.paid = true
      pay(2000)
      shift("traders", 5)
    end
    finish(text.done)
  end,
}
