-- shots.lua — the fixed cameras the showroom is screenshotted from. Each shot: a
-- name (the PNG's), the camera's world position, and the point it looks at. The
-- rest are taken by scenarios: the probe volumes drawn as gizmos (probes.lua), and
-- the ball at its start and once it has rolled into the sun (ball.lua). A shot
-- here settles eight frames first and the showroom runs no TAA, so a settle
-- changes no pixel; a scenario's shot takes one render (#2090).

return {
    -- The polished floor at a grazing angle toward the mirror: SSR and its fade.
    { name = "floor_to_mirror", position = { 4.5, 0.5, -1.0 }, target = { -6, 0.6, 0 } },
    -- The drinking glass against the mirror, the tinted pane against the window.
    { name = "glass_mirror", position = { -3.6, 1.1, -0.6 }, target = { -4.8, 0.9, 0 } },
    { name = "glasses_window", position = { 1.0, 1.4, 0.0 }, target = { 6, 1.8, 0.3 } },
}
