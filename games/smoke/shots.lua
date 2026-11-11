-- shots.lua — the fixed cameras the smoke game is screenshotted from.
-- Each shot: a name (the PNG's), the camera's world position, and the point it
-- looks at.

return {
    { name = "overview", position = { 0, 6, -10 }, target = { 0, 0, 0 } },
    { name = "crate", position = { 2.5, 1.5, -2.5 }, target = { 0, 0.5, 0 } },
}
