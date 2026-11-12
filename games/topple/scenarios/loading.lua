-- scenarios/loading.lua — the next level loads in the background (#2123): the
-- loading canvas comes up over the running level, the operation is ready on the
-- next tick and swapped in at its end, and the new level starts with its own
-- loading canvas hidden. Held back, it waits at 0.9 with the old level running.

Harness.Step(1) -- enter Play: the scripts start
local level = Scene.GetScript(Scene.FindEntityByName("Level"), "level")
local op = level.LoadNext()
Harness.Expect(Scene.IsActive(level.loading), "the loading canvas is up")
Harness.Expect(Scene.GetLoadStatus(op) == "loading" and Scene.GetLoadProgress(op) == 0,
    "the operation starts loading")
Scene.SetAllowSceneActivation(op, false)
Harness.Step(5)
Harness.Expect(Scene.GetLoadStatus(op) == "ready" and Scene.GetLoadProgress(op) == 0.9,
    "held back, it waits at 0.9")
Harness.Expect(Scene.GetActivePath() == "assets/scenes/level_1.scene", "the first level still runs")
Harness.Expect(level.Standing() == 1, "and its tower still stands")
Harness.Screenshot("cache/test/shots/loading.png", { settle = 0 })
Scene.SetAllowSceneActivation(op, true)
Harness.Step(1)
Harness.Expect(Scene.IsLoadDone(op) and Scene.GetLoadProgress(op) == 1, "then it is done")
Harness.Expect(Scene.GetActivePath() == "assets/scenes/level_2.scene", "the second level is active")
Harness.Step(1)
local second = Scene.GetScript(Scene.FindEntityByName("Level"), "level")
Harness.Expect(second.number == 2 and not Scene.IsActive(second.loading),
    "it starts with its loading canvas hidden")
