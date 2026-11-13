-- The Lantern: the story's end (STORY.md). The player chooses who the
-- seed is for; each choice is a fight, an ending card, and a Reach changed
-- for good (a lane to Aamunkoi, regions changing hands, standing).

local LANTERN = "the Lantern"
local KEY = "the Lantern's key"
local WHO = { "ilta", "korpi", "marr", "aino" }
local SIDE = { ilta = "coop", korpi = "crown", marr = "inner", aino = "lantern" }

-- the fights on the way: the first wave, then a heavier one
local WAVES = {
  coop = { { "Ahma", 1 }, { "Karppa", 2 } },
  crown = { { "Ahma", 1 }, { "Karppa", 2 } },
  inner = { { "Ahma", 1 }, { "Karppa", 1 } },
  lantern = { { "Ahma", 2 }, { "Karppa", 2 } },
}
local HEAVY = {
  coop = { { "Karhu", 1 }, { "Karppa", 1 } },
  crown = { { "Ahma", 1 }, { "Karppa", 2 } },
  inner = { { "Ahma", 1 }, { "Karppa", 2 } },
  lantern = { { "Karhu", 1 }, { "Ahma", 1 } },
}

local function hold(tag, where)
  while true do
    if region() ~= where or not flying() then return false end
    if alive(tag) - disabled(tag) == 0 then return true end
    pause()
  end
end

local function wave(tag, list)
  for _, w in ipairs(list) do
    spawn{ tag = tag, class = w[1], count = w[2], skill = 0.55, callsign = "Jumper" }
  end
end

return {
  start = function()
    objective(nil)
    wait(function() return done("ledger") end)
    for _, id in ipairs(WHO) do place(id) end
    return "choose"
  end,

  choose = function()
    objective(text.hook)
    while true do
      local who
      while not who do
        for _, id in ipairs(WHO) do
          if talked(id) then who = id end
        end
        if not who then pause() end
      end
      say(who, text[who .. "_pitch"])
      if choose(who, text[who .. "_ask"], { text.yes, text.no }) == 1 then
        say(who, text[who .. "_go"])
        set_flag("choice", SIDE[who])
        return "plant"
      end
      say(who, text.wait)
    end
  end,

  plant = function()
    local side = flag("choice")
    local where = text[side .. "_where"]
    objective(text[side .. "_obj"])
    wait(function() return region() == where and flying() end)
    wait_hours(0.1)
    wave("final1", WAVES[side])
    say("Lane beacon", text[side .. "_wave"])
    if not hold("final1", where) then
      say("Lane beacon", text.left)
      return "plant"
    end
    wait_hours(0.05)
    wave("final2", HEAVY[side])
    say("Lane beacon", text.more)
    if not hold("final2", where) then
      say("Lane beacon", text.left)
      return "plant"
    end
    return "planted"
  end,

  planted = function()
    local side = flag("choice")
    if side == "coop" then
      say("The Lantern", text.lantern_coop)
      open_lane("Kide Drift", "Aamunkoi")
      hand_over("Aamunkoi", "coop")
      if not vars.paid then
        vars.paid = true
        pay(5000)
        shift("coop", 20)
        shift("crown", -20)
      end
    elseif side == "crown" then
      say("The Lantern", text.lantern_crown)
      open_lane("Halla Shoal", "Aamunkoi")
      hand_over("Aamunkoi", "crown")
      if not vars.paid then
        vars.paid = true
        pay(25000)
        shift("crown", 25)
        shift("coop", -25)
      end
    elseif side == "inner" then
      say("The Lantern", text.lantern_inner)
      if not vars.paid then
        vars.paid = true
        pay(8000)
        shift("inner", 25)
      end
    else
      say("The Lantern", text.lantern_free)
      open_lane("Revontuli Gap", "Aamunkoi")
      hand_over("Aamunkoi", "traders")
      if not vars.paid then
        vars.paid = true
        pay(3000)
        shift("traders", 20)
        shift("crown", -10)
        shift("inner", -10)
      end
    end
    take_item(LANTERN)
    take_item(KEY)
    set_flag("ending", side)
    ending(text[side .. "_title"], text[side .. "_end"])
    finish(text.done)
  end,
}
