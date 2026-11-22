-- This bot is not part of the game: scenarios press its keys with `Input.Press`,
-- the same input the player uses.

local Track = require("runner.track")

local Bot = {}

local LANE_HALF = 0.6  -- share of a lane the body sweeps, with a margin

-- The nearest hazard ahead in `lane`: its distance and how the bot passes it
-- (`slide`, `jump` or `block`), or nil when clear. Hazards overlapping it along
-- the track count with it, so stacked crates block.
local function ahead(view, lane, past_bars)
    local x = Track.lane_x(lane)
    local near, lo, hi, kinds, back
    for _, h in ipairs(view.hazards) do
        local seen = not (past_bars and h.kind == "bar")
        if seen and h.max[3] > view.z - 0.5 and h.min[1] < x + LANE_HALF and h.max[1] > x - LANE_HALF then
            if not near then
                near, lo, hi, kinds, back = h, h.min[2], h.max[2], h.kind, h.max[3]
            elseif h.min[3] < back + 0.5 then
                lo, hi, kinds = math.min(lo, h.min[2]), math.max(hi, h.max[2]), kinds .. h.kind
                back = math.max(back, h.max[3])
            end
        end
    end
    if not near then return nil end
    local how = "block"
    if kinds == "bar" then how = "slide" -- a crate still falling is no bar
    elseif hi < 0.95 and back - near.min[3] < 1.6 then how = "jump" end
    local d = math.max(near.min[3] - (view.z + 0.5), 0)
    if how == "jump" then
        local wait = math.abs(x - view.x) / view.lateral + 0.15
        if not view.grounded then
            wait = wait + (view.vy + math.sqrt(view.vy * view.vy + 2 * view.gravity * math.max(view.feet, 0))) / view.gravity
        end
        if d < view.speed * wait then how = "block" end
    end
    return d, how
end

local function score(view, lane)
    local d, how = ahead(view, lane, true)
    if not d then return 1e6 end
    if how ~= "block" then return 1e5 + d end
    return d
end

-- Whether the body can cross to `lane` without running into anything but a bar:
-- nothing in the strip it sweeps until across, except what is straight ahead.
local function clear_across(view, lane)
    local to = Track.lane_x(lane)
    local across = view.speed * (math.abs(to - view.x) / view.lateral + 0.15) + 1.0
    local out = view.speed * (2 * LANE_HALF / view.lateral + 0.05) + 0.5
    local x0, x1 = math.min(view.x, to) - LANE_HALF, math.max(view.x, to) + LANE_HALF
    for _, h in ipairs(view.hazards) do
        local d = h.min[3] - (view.z + 0.5)
        local in_way = h.min[1] < x1 and h.max[1] > x0
        local here = h.min[1] < view.x + LANE_HALF and h.max[1] > view.x - LANE_HALF
        local beyond
        if to > view.x then beyond = h.min[1] > view.x + LANE_HALF - 0.2
        else beyond = h.max[1] < view.x - LANE_HALF + 0.2 end
        if h.kind ~= "bar" and in_way and h.max[3] > view.z - 0.5 then
            if d < (here and not beyond and out or across) then return false end
        end
    end
    return true
end

function Bot.keys(view)
    local keys = {}
    local horizon = view.speed * 1.0 + 4
    local lane = view.lane
    local d, how = ahead(view, lane)
    local d2 = ahead(view, lane, true)
    if d2 and d2 < horizon then
        local best, best_score = lane, score(view, lane)
        for _, l in ipairs({ lane - 1, lane + 1, lane - 2, lane + 2 }) do
            if math.abs(l) <= 1 and score(view, l) > best_score then best, best_score = l, score(view, l) end
        end
        if best ~= lane then
            local step = best > lane and 1 or -1
            if clear_across(view, lane + step) then
                keys[#keys + 1] = step > 0 and "RIGHT" or "LEFT"
                return keys
            end
        end
    end
    if d and how == "jump" and d < view.speed * 0.2 + 0.8 and view.grounded then
        keys[#keys + 1] = "UP"
    elseif d and how == "slide" and d < view.speed * 0.3 + 1.0 and not view.sliding then
        keys[#keys + 1] = "DOWN"
    end
    return keys
end

function Bot.play(game, ticks, each)
    local held = {}
    for _ = 1, ticks do
        if game.over then return end
        for _, key in ipairs(held) do Input.Release(key) end
        local view = game:View()
        if each then each(view) end
        held = #held == 0 and Bot.keys(view) or {}
        for _, key in ipairs(held) do Input.Press(key) end
        Harness.Step(1)
    end
    for _, key in ipairs(held) do Input.Release(key) end
end

return Bot
