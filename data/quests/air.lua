-- Thin Air: a co-op side story (STORY.md). Machinery for Lumi Farm's
-- failing air plant, and a guard over the restart.

local function hold(tag, where)
  while true do
    if region() ~= where or not flying() then return false end
    if alive(tag) - disabled(tag) == 0 then return true end
    pause()
  end
end

return {
  start = function()
    objective(nil)
    wait(function() return hours() >= 30 end)
    place("kerttu")
    objective(text.hook)
    wait_talk("kerttu")
    say("kerttu", text.hello)
    while choose("kerttu", text.ask, { text.yes, text.no }) ~= 1 do
      say("kerttu", text.fine)
      wait_talk("kerttu")
    end
    say("kerttu", text.thanks)
    return "fetch"
  end,

  fetch = function()
    objective(text.fetch)
    while true do
      wait_talk("kerttu")
      if take_cargo("machinery", 10) then break end
      say("kerttu", text.short)
    end
    say("kerttu", text.fitted)
    return "guard"
  end,

  guard = function()
    objective(text.guard)
    wait(function() return region() == "Lumi Field" and flying() end)
    wait_hours(0.1)
    spawn{ tag = "raid", class = "Karppa", count = 2, skill = 0.45, callsign = "Raider" }
    spawn{ tag = "raid", class = "Ahma", skill = 0.45, callsign = "Raider" }
    say("kerttu", text.raid)
    if not hold("raid", "Lumi Field") then
      say("kerttu", text.left)
      return "guard"
    end
    say("kerttu", text.lit)
    if not vars.paid then
      vars.paid = true
      pay(2500)
      shift("coop", 8)
    end
    finish(text.done)
  end,
}
