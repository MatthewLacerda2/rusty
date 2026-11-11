-- shots.lua — the fixed cameras the arena is screenshotted from (block 1).
-- The mid-fight shot, a bot firing with a smoke behind it, is taken by
-- scenarios/match.lua, since only Play has a fight in it.

return {
    { name = "floors", position = { -4, 22, -4 }, target = { 13, 0, 13 } },
    { name = "upper", position = { 14, 9, 2 }, target = { 21, 3, 6 } },
    { name = "stairs", position = { 13.2, 2.4, 6.5 }, target = { 16.5, 1.2, 1.4 } },
}
