-- A shot here settles eight frames first; the showroom runs no TAA, so a settle
-- changes no pixel (scenario shots take one render).

return {
    { name = "floor_to_mirror", position = { 4.5, 0.5, -1.0 }, target = { -6, 0.6, 0 } },
    { name = "glass_mirror", position = { -3.6, 1.1, -0.6 }, target = { -4.8, 0.9, 0 } },
    { name = "placard", position = { 0.5, 1.5, -0.5 }, target = { 2.5, 1.3, 3.5 } },
    { name = "glasses_window", position = { 1.0, 1.4, 0.0 }, target = { 6, 1.8, 0.3 } },
}
