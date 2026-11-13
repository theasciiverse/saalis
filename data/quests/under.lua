-- What Lies Under Kide: the story's second chapter (STORY.md). Ilta Rask
-- digs out what was under her claim while you hold the jumpers off.

local LANTERN = "the Lantern"

-- Waits until the tagged ships are knocked out or gone; false if you left
-- the region or landed first (they vanish then, so that isn't a win).
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
    wait(function() return done("claim") end)
    wait_hours(6)
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
      else
        say("ilta", text.later)
        wait_talk("ilta")
      end
    end
    say("ilta", text.go)
    return "cover"
  end,

  cover = function()
    objective("Fly cover over Ilta's claim in Kide Drift: hold off the jumpers while her crew digs.")
    wait(function() return region() == "Kide Drift" and flying() end)
    wait_hours(0.1)
    spawn{ tag = "diggers1", class = "Karppa", count = 2, skill = 0.4, callsign = "Jumper" }
    say("ilta", text.first)
    if not hold("diggers1", "Kide Drift") then
      say("ilta", text.left)
      return "cover"
    end
    wait_hours(0.05)
    spawn{ tag = "diggers2", class = "Ahma", skill = 0.45, callsign = "Ahma Routa" }
    spawn{ tag = "diggers2", class = "Karppa", skill = 0.4, callsign = "Jumper" }
    say("ilta", text.second)
    if not hold("diggers2", "Kide Drift") then
      say("ilta", text.left)
      return "cover"
    end
    return "free"
  end,

  free = function()
    say("ilta", text.free)
    say("ilta", text.stow)
    if has(LANTERN) == 0 then give_item(LANTERN) end
    say("ilta", text.where)
    if not vars.paid then
      vars.paid = true
      pay(1200)
      shift("coop", 5)
    end
    remove("ilta")
    finish(text.done)
  end,
}
