-- First Light: the story's epilogue (STORY.md). The first landing at
-- Aamunkoi Light, once a lane reaches it.

local WHO = { coop = "ilta", crown = "saga", lantern = "aino" }

return {
  start = function()
    objective(nil)
    wait(function() local e = flag("ending"); return e ~= nil and WHO[e] ~= nil end)
    local who = WHO[flag("ending")]
    place(who)
    objective(text.hook)
    wait_talk(who)
    say(who, text[flag("ending")])
    if not vars.paid then
      vars.paid = true
      pay(flag("ending") == "crown" and 5000 or 2500)
      give_item(text.gift)
    end
    finish(text.done)
  end,
}
