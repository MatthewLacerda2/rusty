-- shots.lua — the fixed cameras the runner game is screenshotted from (the
-- start; `dodge.lua` and `long.lua` take a lane change mid-air and a run at speed).

return {
    { name = "start", position = { 0, 3.2, -6.5 }, target = { 0, 1.5, 20 } },
    { name = "side", position = { 9, 4, 8 }, target = { 0, 0.5, 18 } },
}
