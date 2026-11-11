-- runner.lua — the endless runner: the run itself (speed, lane changes, jump and
-- slide, the crash), on the fixed tick. The track and its crates are
-- `runner/track.lua`; the effects, camera and HUD are `runner/fx.lua`.
--
-- Keys: Left/Right (or A/D) change lane, Up/Space/W jump, Down/S slides (in the
-- air it drops fast first), Enter runs again after a crash.
--
-- A scenario reads the state through `Scene.GetScript(id, "runner")`: `distance`
-- (metres run), `speed`, `over`, `killer` (the kind of hazard hit), `lane`,
-- `jumps`, `slides`, `lane_changes`, `near_misses`, `landings` (crates come to
-- rest) and `dropped` (crates dropped). It may write `hold` (the runner stands
-- still and the speed stops rising), and call `game:Restart()`, `game:View()`
-- (what a bot sees: `runner/bot.lua`), `game:Drop(z, lanes)` and `game:Crates()`.

local Track = require("runner.track")
local Fx = require("runner.fx")

local Runner = {
    fields = {
        seed = { type = "number", default = 1, tooltip = "Seeds the track" },
        start_speed = { type = "number", default = 10, range = { 1, 60 }, tooltip = "Metres per second at the start" },
        max_speed = { type = "number", default = 25, range = { 1, 60 }, tooltip = "The fastest the run gets" },
        accel = { type = "number", default = 0.15, range = { 0, 5 }, tooltip = "Speed gained per second" },
    },
}

local GRAVITY = 40.0      -- m/s²: a snappy, arcade fall
local JUMP = 10.0         -- m/s up at take-off: a 1.25 m apex, 0.5 s in the air
local LATERAL = 18.0      -- m/s across while changing lane
local SLIDE_TIME = 0.75   -- seconds a slide lasts (longer while there is no room to stand)
local HEIGHT, SLIDE_HEIGHT = 1.8, 0.9
local NEAR = 0.6          -- metres of clearance that count as a near miss
local REACH = 0.45        -- the crash test's half width: the body's radius and a little more
local FOOT = 0.3          -- the ground probe's half width
-- Every layer but the runner's own (8), for the probe and the crash test.
local OTHERS = 0xFFFFFFFF & ~(1 << 8)

local runner, body
local track, fx
local vy, slide_left, grounded = 0, 0, true
local passed = {}         -- the hazards already behind the runner

local function height() return slide_left > 0 and SLIDE_HEIGHT or HEIGHT end

local function set_height(h)
    Transform.SetLocalPosition(body, 0, h / 2, 0)
    Transform.SetLocalScale(body, 0.8, h / 2, 0.8)
end

-- The top of whatever is under the runner's feet at (`x`, `z`), looked for from
-- `above` metres over `feet` down to `reach` below them; nil over nothing.
local function ground(x, feet, z, above, reach)
    local hit, _, dist = Physics.BoxCast(x, feet + above, z, FOOT, 0.01, FOOT, 0, -1, 0,
        0, 0, 0, nil, OTHERS, above + reach, false)
    if hit then return feet + above - dist - 0.01 end
end

-- Whether a body `h` tall fits with its feet at `feet`.
local function room(x, feet, z, h)
    return not Physics.CheckBox(x, feet + h / 2 + 0.06, z, 0.4, h / 2 - 0.05, 0.4, 0, 0, 0, OTHERS, false)
end

local function input(x, feet, z)
    local step = (Input.GetKeyDown("RIGHT") or Input.GetKeyDown("D")) and 1 or 0
    step = step - ((Input.GetKeyDown("LEFT") or Input.GetKeyDown("A")) and 1 or 0)
    if step ~= 0 and math.abs(Runner.lane + step) <= 1 then
        Runner.lane = Runner.lane + step
        Runner.lane_changes = Runner.lane_changes + 1
        Fx.sound("whoosh", x, feet + 1, z, 0.3)
    end
    local jump = Input.GetKeyDown("UP") or Input.GetKeyDown("SPACE") or Input.GetKeyDown("W")
    if jump and grounded and (slide_left == 0 or room(x, feet, z, HEIGHT)) then
        if slide_left > 0 then slide_left = 0; set_height(HEIGHT) end
        vy, grounded = JUMP, false
        Runner.jumps = Runner.jumps + 1
        Fx.sound("jump", x, feet + 1, z)
    elseif (Input.GetKeyDown("DOWN") or Input.GetKeyDown("S")) and slide_left == 0 then
        if not grounded then vy = math.min(vy, -JUMP) end
        slide_left = SLIDE_TIME
        set_height(SLIDE_HEIGHT)
        Runner.slides = Runner.slides + 1
        Fx.sound("slide", x, feet, z, 0.5)
    end
end

-- The first hazard the runner's body, a little wider than drawn, touches: its
-- id and kind. Standing on a box top or a crate is not a touch.
local function touching(x, feet, z)
    local lo, hi = feet + 0.12, feet + height() + 0.02
    local ids = Physics.OverlapBox(x, (lo + hi) / 2, z, REACH, (hi - lo) / 2, REACH, 0, 0, 0, OTHERS, false)
    for _, id in ipairs(ids) do
        if track.hazards[id] then return id, track.hazards[id] end
    end
end

-- Sparks for each hazard that has just gone behind the runner within `NEAR` of it.
local function near_misses(x, feet, z)
    local top = feet + height()
    for _, h in ipairs(Track.bounds(track)) do
        if h.max[3] < z - 0.4 and h.max[3] > z - 3 and not passed[h.id] then
            passed[h.id] = true
            local side = math.max(h.min[1] - (x + 0.4), (x - 0.4) - h.max[1])
            local over_it = math.max(h.min[2] - top, feet - h.max[2])
            local gap = math.max(side, over_it)
            if gap >= 0 and gap < NEAR then
                Runner.near_misses = Runner.near_misses + 1
                local px = math.max(h.min[1], math.min(x, h.max[1]))
                local py = math.max(h.min[2], math.min(feet + 0.9, h.max[2]))
                Fx.sparks(fx, px, py, h.max[3])
            end
        end
    end
end

local function crash(kind, x, feet, z)
    Runner.over, Runner.killer = true, kind
    Fx.sound("crash", x, feet + 1, z)
    Fx.dust(fx, x, feet + 0.5, z + 0.5, 30)
    Trail.SetEmitting(body, false)
end

-- One tick of running: input, the lane change, gravity and the ground, the slide,
-- then the crash test against where the runner ended up.
local function run(dt)
    local x, feet, z = Transform.GetPosition(runner)
    input(x, feet, z)
    local target = Track.lane_x(Runner.lane)
    x = x + math.max(-LATERAL * dt, math.min(LATERAL * dt, target - x))
    z = z + (Runner.hold and 0 or Runner.speed * dt)
    vy = vy - GRAVITY * dt
    local fall = math.max(0, -vy * dt)
    local floor = ground(x, feet, z, 0.3, fall + 0.05)
    local was_grounded = grounded
    if floor and floor <= feet + 0.05 and vy <= 0 and feet + vy * dt <= floor + 0.02 then
        feet, vy, grounded = floor, 0, true
    else
        feet, grounded = feet + vy * dt, false
    end
    if grounded and not was_grounded then Fx.dust(fx, x, feet + 0.05, z, 6) end
    if slide_left > 0 then
        slide_left = math.max(0, slide_left - dt)
        if slide_left == 0 and not room(x, feet, z, HEIGHT) then slide_left = dt end
        if slide_left == 0 then set_height(HEIGHT) end
    end
    Transform.SetPosition(runner, x, feet, z)
    local _, kind = touching(x, feet, z)
    if kind or feet < -5 then return crash(kind or "fall", x, feet, z) end
    near_misses(x, feet, z)
    if not Runner.hold then
        Runner.speed = math.min(Runner.max_speed, Runner.speed + Runner.accel * dt)
    end
    Runner.distance = z
    Fx.follow(x, z)
end

-- Destroy the track and start a new run from `seed`.
function Runner.Restart()
    Track.clear(track)
    Random.SetSeed(Runner.seed)
    Transform.SetPosition(runner, 0, 0, 0)
    vy, slide_left, grounded, passed = 0, 0, true, {}
    set_height(HEIGHT)
    Trail.Clear(body)
    Trail.SetEmitting(body, true)
    Runner.lane, Runner.speed, Runner.distance = 0, Runner.start_speed, 0
    Runner.over, Runner.killer = false, nil
    Runner.jumps, Runner.slides, Runner.lane_changes, Runner.near_misses, Runner.landings = 0, 0, 0, 0, 0
    Runner.dropped = 0
    Track.advance(track, 0, Runner.speed)
    Fx.follow(0, 0)
end

-- What a bot needs to dodge: the runner and every hazard's bounds, nearest first.
function Runner.View()
    local x, feet, z = Transform.GetPosition(runner)
    return { x = x, feet = feet, z = z, vy = vy, gravity = GRAVITY, lateral = LATERAL, lane = Runner.lane, speed = Runner.speed, sliding = slide_left > 0,
        grounded = grounded, hazards = Track.bounds(track) }
end

-- Drop crates on `lanes` at `z` now, in the chunk under `z`.
function Runner.Drop(_, z, lanes)
    for _, chunk in ipairs(track.chunks) do
        if z >= chunk.index * Track.CHUNK and z < (chunk.index + 1) * Track.CHUNK then
            return #Track.drop(track, chunk, z, lanes)
        end
    end
    return 0
end

-- Every crate and barrel on the track: `{n, x, y, z, speed}`, in the order they
-- were dropped (`n`, from 1 each run).
function Runner.Crates()
    local list = {}
    for id, crate in pairs(track.crates) do
        local x, y, z = Transform.GetPosition(id)
        local vx, vy2, vz = Physics.GetVelocity(id)
        list[#list + 1] = { n = crate.n, x = x, y = y, z = z, speed = math.sqrt(vx * vx + vy2 * vy2 + vz * vz) }
    end
    table.sort(list, function(a, b) return a.n < b.n end)
    return list
end

function Runner.Start(id)
    Runner.seed = math.floor(Runner.seed)
    runner = Scene.FindEntityByName("Runner")
    body = Scene.FindChild(runner, "Body")
    track, fx = Track.new(), Fx.new()
    Runner.Restart()
end

function Runner.Update(id, dt)
    if Runner.over then
        if Input.GetKeyDown("ENTER") then Runner.Restart() end
    else
        run(dt)
        Track.advance(track, Runner.distance, Runner.speed)
    end
    for _, p in ipairs(Track.landings(track)) do
        Runner.landings = Runner.landings + 1
        Fx.dust(fx, p[1], p[2], p[3], 12)
        Fx.sound("thud", p[1], p[2], p[3], 0.5)
    end
    Runner.dropped = track.dropped
    Fx.hud(fx, Runner.distance, Runner.over)
end

return Runner
