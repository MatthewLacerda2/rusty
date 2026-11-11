-- shots.lua — the fixed cameras the showroom is screenshotted from. Each shot: a
-- name (the PNG's), the camera's world position, and the point it looks at. Two
-- more are taken by scenarios, since a shot here sees the scene as loaded: the
-- probe volumes drawn as gizmos (probes.lua) and the ball once it has rolled into
-- the sun (ball.lua).

return {
    -- The polished floor at a grazing angle toward the mirror: SSR and its fade.
    { name = "floor_to_mirror", position = { 4.5, 0.5, -1.0 }, target = { -6, 0.6, 0 } },
    -- The drinking glass against the mirror, the tinted pane against the window.
    { name = "glass_mirror", position = { -3.6, 1.1, -0.6 }, target = { -4.8, 0.9, 0 } },
    { name = "glasses_window", position = { 1.0, 1.4, 0.0 }, target = { 6, 1.8, 0.3 } },
    -- The ball at its start, in the dark half.
    { name = "ball_dark", position = { -2.0, 1.2, -3.6 }, target = { -5, 0.3, -1.0 } },
}
