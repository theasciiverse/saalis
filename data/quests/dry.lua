-- Running Dry: a Free Traders side story (STORY.md). Medicine one side of
-- the law and contraband the other, past three factions' scanners.

return {
  start = function()
    objective(nil)
    wait(function() return hours() >= 56 end)
    place("mirja")
    objective(text.hook)
    wait_talk("mirja")
    say("mirja", text.hello)
    while choose("mirja", text.ask, { text.yes, text.no }) ~= 1 do
      say("mirja", text.fine)
      wait_talk("mirja")
    end
    return "load"
  end,

  load = function()
    while room() < 10 do
      say("mirja", text.room)
      wait_talk("mirja")
    end
    give_cargo("stims", 10)
    say("mirja", text.loaded)
    place("nurse")
    return "run"
  end,

  run = function()
    objective(text.run)
    while true do
      wait_talk("nurse")
      if take_cargo("stims", 10) then break end
      say("nurse", text.short)
      return "reload"
    end
    say("nurse", text.got)
    if not vars.paid then
      vars.paid = true
      pay(3000)
      shift("traders", 10)
    end
    finish(text.done)
  end,

  reload = function()
    objective("Go back to Mirja Salo at Usva Den for more stims.")
    remove("nurse")
    wait_talk("mirja")
    return "load"
  end,
}
