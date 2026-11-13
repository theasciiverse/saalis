-- Licence Revoked: an Inner Customs side story (STORY.md). It brings the
-- player to Captain Vala Marr before the story's end does.

local CRATE = "a crate of unlicensed cores"

return {
  start = function()
    objective(nil)
    wait(function() return done("cold") end)
    place("marr")
    objective(text.hook)
    wait_talk("marr")
    say("marr", text.hello)
    say("marr", text.why)
    while choose("marr", text.ask, { text.yes, text.no }) ~= 1 do
      say("marr", text.fine)
      wait_talk("marr")
    end
    say("marr", text.thanks)
    return "hunt"
  end,

  hunt = function()
    objective(text.hunt)
    wait(function() return region() == "Usva Deep" and flying() end)
    wait_hours(0.1)
    spawn{ tag = "runner", class = "Haukka", callsign = "Haukka Tuli", item = CRATE, skill = 0.5 }
    spawn{ tag = "runner_escort", class = "Ahma", skill = 0.45, callsign = "Escort" }
    say("marr", text.spotted)
    while true do
      if has(CRATE) > 0 then return "home" end
      if alive("runner") == 0 then
        if region() == "Usva Deep" and flying() then say("marr", text.lost) end
        return "hunt"
      end
      pause()
    end
  end,

  home = function()
    say("You", text.found)
    objective(text.back)
    wait_talk("marr")
    take_item(CRATE)
    say("marr", text.done1)
    say("marr", text.done2)
    if not vars.paid then
      vars.paid = true
      pay(3000)
      shift("inner", 10)
    end
    finish(text.done)
  end,
}
