-- The Toll: a Crown side story (STORY.md). A smuggler's manifest, for the
-- Crown's collections office. The co-op doesn't like it.

local MANIFEST = "Lokki Musta's manifest"

return {
  start = function()
    objective(nil)
    wait(function() return hours() >= 80 end)
    place("saga")
    objective(text.hook)
    wait_talk("saga")
    say("saga", text.hello)
    while choose("saga", text.ask, { text.yes, text.no }) ~= 1 do
      say("saga", text.fine)
      wait_talk("saga")
    end
    say("saga", text.thanks)
    return "hunt"
  end,

  hunt = function()
    objective(text.hunt)
    wait(function() return region() == "Tuisku Belt" and flying() end)
    wait_hours(0.1)
    spawn{ tag = "musta", class = "Haahka", callsign = "Lokki Musta", item = MANIFEST, skill = 0.5 }
    spawn{ tag = "musta_escort", class = "Karppa", count = 2, skill = 0.45, callsign = "Escort" }
    say("saga", text.spotted)
    while true do
      if has(MANIFEST) > 0 then return "home" end
      if alive("musta") == 0 then
        if region() == "Tuisku Belt" and flying() then say("saga", text.lost) end
        return "hunt"
      end
      pause()
    end
  end,

  home = function()
    say("You", text.found)
    objective(text.back)
    wait_talk("saga")
    take_item(MANIFEST)
    say("saga", text.done1)
    if not vars.paid then
      vars.paid = true
      pay(2500)
      shift("crown", 10)
      shift("coop", -5)
    end
    finish(text.done)
  end,
}
