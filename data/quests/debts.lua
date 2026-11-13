-- Old Debts: a side story (STORY.md). A retired Watch pilot's wingman,
-- lost in Kalmisto twelve years ago.

local RECORDER = "Lauri's flight recorder"

return {
  start = function()
    objective(nil)
    wait(function() return done("claim") end)
    place("harri")
    objective(text.hook)
    wait_talk("harri")
    say("harri", text.hello)
    while choose("harri", text.ask, { text.yes, text.no }) ~= 1 do
      say("harri", text.fine)
      wait_talk("harri")
    end
    say("harri", text.thanks)
    return "hunt"
  end,

  hunt = function()
    objective(text.hunt)
    wait(function() return region() == "Kalmisto" and flying() end)
    wait_hours(0.3)
    spawn{ tag = "lauri", class = "Kotka", side = "patrol", faction = "coop", callsign = "Watch Two", item = RECORDER, wreck = true, distance = 3000 }
    say("Lane beacon", text.seen)
    while true do
      if has(RECORDER) > 0 then return "home" end
      if alive("lauri") == 0 then
        if region() == "Kalmisto" and flying() then say("Lane beacon", text.gone) end
        return "hunt"
      end
      pause()
    end
  end,

  home = function()
    say("You", text.found)
    objective(text.back)
    wait_talk("harri")
    say("harri", text.listen)
    take_item(RECORDER)
    say("Lauri Aalto", text.lauri)
    say("harri", text.after)
    if not vars.paid then
      vars.paid = true
      pay(1800)
      shift("coop", 3)
    end
    finish(text.done)
  end,
}
