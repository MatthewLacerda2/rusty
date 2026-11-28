-- `objects` is the scene's exact entity count: buildings fill what the rest leaves.

local p = { size = 400, objects = 3000, levels = 1, seed = 1, origin = 0 }
for key, value in pairs(... or {}) do
    if p[key] == nil then error("city: unknown parameter '" .. key .. "'") end
    p[key] = value
end
local function whole(key, low)
    local v = p[key]
    if type(v) ~= "number" or v ~= v or v < low or v ~= math.floor(v) then
        error("city: " .. key .. " is a whole number of at least " .. low .. ", not " .. tostring(v))
    end
end
whole("objects", 1); whole("levels", 1); whole("seed", 0)
if type(p.size) ~= "number" or not (p.size >= 0) then error("city: size is metres, not " .. tostring(p.size)) end
if type(p.origin) ~= "number" or p.origin ~= p.origin or math.abs(p.origin) > 1e7 then
    error("city: origin is metres within 1e7 of zero, not " .. tostring(p.origin))
end

local AVE, AVE_WALK = 24, 4   -- m: width, and each sidewalk's
local ST, ST_WALK = 16, 3
local BLOCK_X, BLOCK_Z = 150, 60
local CURB = 0.15
local DECK_GAP, DECK_T = 9, 0.6
local POLE_GAP = 15
local LOT = 80                -- m²
local MAX_PARTS = 6
local ALLEY = 0.6
local O = p.origin

local PX, PZ = BLOCK_X + AVE, BLOCK_Z + ST
local NX = math.max(1, math.floor((p.size - AVE) / PX))
local NZ = math.max(1, math.floor((p.size - ST) / PZ))
local X0 = O + (p.size - (NX * PX + AVE)) / 2 + AVE / 2
local Z0 = O + (p.size - (NZ * PZ + ST)) / 2 + ST / 2
local function ave_x(i) return X0 + i * PX end
local function st_z(j) return Z0 + j * PZ end

local state = p.seed * 2654435761 % 2147483647 + 1
local function rand() -- [0, 1), Park–Miller
    state = state * 48271 % 2147483647
    return state / 2147483647
end

Scene.New()
local made = 0
local function entity(name, primitive, parent)
    made = made + 1
    local id = Scene.CreateEntity(name, primitive)
    if parent then Scene.SetParent(id, parent) end
    return id
end
local city = entity("City")
-- No navmesh until it is tiled: a dense grid this size is the bake's wall.
Scene.AddComponent(city, "NavMeshModifier")
NavMeshModifier.SetIgnoreFromBuild(city, true)
local groups = {}
for _, name in ipairs({ "Streets", "Props", "Edge", "Blocks", "Buildings", "Viewpoints" }) do
    groups[name] = entity(name, nil, city)
end

local function noise_map(name, scale, dark, light)
    return Texture.Bake({
        resolution = 64, seed = p.seed,
        nodes = {
            { id = "n", op = "noise", kind = "fbm", scale = scale, octaves = 4 },
            { id = "c", op = "color_ramp", inputs = { "n" },
              stops = { { pos = 0.0, color = dark }, { pos = 1.0, color = light } } },
        },
    }, "assets/textures/" .. name .. "_base_color.png", "base_color")
end
Material.DefineAsset("street", { base_color_map = noise_map("asphalt", 12.0,
    { 0.08, 0.08, 0.09, 1 }, { 0.22, 0.22, 0.23, 1 }), roughness = 0.85 })
Material.DefineAsset("pavement", { base_color = { 0.55, 0.54, 0.5 }, roughness = 0.9 })
Material.DefineAsset("steel", { base_color = { 0.3, 0.32, 0.34 }, metallic = 1.0, roughness = 0.45 })
Material.DefineAsset("signal", { base_color = { 0.9, 0.7, 0.1 }, roughness = 0.5 })
Material.DefineAsset("barrier", { base_color = { 0.6, 0.15, 0.1 }, roughness = 0.7 })
local FACADES = { "brick", "stone", "glass", "render" }
Material.DefineAsset("brick", { base_color = { 0.5, 0.25, 0.18 }, roughness = 0.9 })
Material.DefineAsset("stone", { base_color_map = noise_map("stone", 6.0,
    { 0.45, 0.43, 0.4, 1 }, { 0.7, 0.68, 0.63, 1 }), roughness = 0.85 })
Material.DefineAsset("glass", { base_color = { 0.25, 0.35, 0.45 }, metallic = 0.6, roughness = 0.15 })
Material.DefineAsset("render", { base_color = { 0.8, 0.76, 0.66 }, roughness = 0.8 })

-- Each quad is wound to face along `n`.
local function mesh_builder()
    local m = { positions = {}, indices = {}, n = 0 }
    function m.quad(c, n)
        local a, b, d = c[1], c[2], c[3]
        local u = { b[1] - a[1], b[2] - a[2], b[3] - a[3] }
        local v = { d[1] - a[1], d[2] - a[2], d[3] - a[3] }
        local cross = { u[2] * v[3] - u[3] * v[2], u[3] * v[1] - u[1] * v[3], u[1] * v[2] - u[2] * v[1] }
        if cross[1] * n[1] + cross[2] * n[2] + cross[3] * n[3] < 0 then c = { c[4], c[3], c[2], c[1] } end
        for _, q in ipairs(c) do
            for k = 1, 3 do m.positions[#m.positions + 1] = q[k] end
        end
        for _, k in ipairs({ 0, 1, 2, 0, 2, 3 }) do m.indices[#m.indices + 1] = m.n + k end
        m.n = m.n + 4
    end
    function m.top(x0, x1, z0, z1, y) m.quad({ { x0, y, z0 }, { x0, y, z1 }, { x1, y, z1 }, { x1, y, z0 } }, { 0, 1, 0 }) end
    function m.bottom(x0, x1, z0, z1, y) m.quad({ { x0, y, z0 }, { x0, y, z1 }, { x1, y, z1 }, { x1, y, z0 } }, { 0, -1, 0 }) end
    function m.wall_x(x, z0, z1, y0, y1, nx) m.quad({ { x, y0, z0 }, { x, y1, z0 }, { x, y1, z1 }, { x, y0, z1 } }, { nx, 0, 0 }) end
    function m.wall_z(z, x0, x1, y0, y1, nz) m.quad({ { x0, y0, z }, { x0, y1, z }, { x1, y1, z }, { x1, y0, z } }, { 0, 0, nz }) end
    return m
end

-- One metre along Z, scaled to its length.
local function strip(name, w, walk, deck)
    local m, h, a = mesh_builder(), w / 2, w / 2 - walk
    local low = deck and -DECK_T or 0
    m.top(-h, -a, -0.5, 0.5, CURB); m.top(a, h, -0.5, 0.5, CURB); m.top(-a, a, -0.5, 0.5, 0)
    m.wall_x(-a, -0.5, 0.5, 0, CURB, 1); m.wall_x(a, -0.5, 0.5, 0, CURB, -1)
    m.wall_x(-h, -0.5, 0.5, low, CURB, -1); m.wall_x(h, -0.5, 0.5, low, CURB, 1)
    if deck then m.bottom(-h, h, -0.5, 0.5, low) end
    Mesh.Create(name, { positions = m.positions, indices = m.indices })
end
local function tile(name, deck)
    local m = mesh_builder()
    local hx, hz, ax, az = AVE / 2, ST / 2, AVE / 2 - AVE_WALK, ST / 2 - ST_WALK
    for _, sx in ipairs({ -1, 1 }) do
        for _, sz in ipairs({ -1, 1 }) do
            m.top(math.min(sx * ax, sx * hx), math.max(sx * ax, sx * hx),
                math.min(sz * az, sz * hz), math.max(sz * az, sz * hz), CURB)
            m.wall_x(sx * ax, math.min(sz * az, sz * hz), math.max(sz * az, sz * hz), 0, CURB, -sx)
            m.wall_z(sz * az, math.min(sx * ax, sx * hx), math.max(sx * ax, sx * hx), 0, CURB, -sz)
        end
    end
    m.top(-ax, ax, -az, az, 0)
    m.top(-ax, ax, az, hz, 0); m.top(-ax, ax, -hz, -az, 0)
    m.top(ax, hx, -az, az, 0); m.top(-hx, -ax, -az, az, 0)
    if deck then m.bottom(-hx, hx, -hz, hz, -DECK_T) end
    Mesh.Create(name, { positions = m.positions, indices = m.indices })
end
strip("Avenue", AVE, AVE_WALK); strip("Street", ST, ST_WALK); tile("Crossing")
if p.levels > 1 then strip("AvenueDeck", AVE, AVE_WALK, true); tile("CrossingDeck", true) end

local function road(name, mesh, x, y, z, length, yaw)
    local id = entity(name, nil, groups.Streets)
    Mesh.SetSharedMesh(id, mesh)
    Material.Set(id, "street")
    Transform.SetPosition(id, x, y, z)
    if yaw then Transform.SetRotation(id, 0, yaw, 0) end
    if length then Transform.SetLocalScale(id, 1, 1, length) end
    Scene.AddComponent(id, "Collider")
    Physics.SetColliderShape(id, { kind = "Mesh", convex = false })
    Scene.SetStatic(id, true)
end
local function box(name, parent, x, y, z, sx, sy, sz, material, collide)
    local id = entity(name, "Box", parent)
    Transform.SetPosition(id, x, y + sy / 2, z)
    Transform.SetLocalScale(id, sx, sy, sz)
    Material.Set(id, material)
    if collide then
        Scene.AddComponent(id, "Collider")
        Physics.SetColliderShape(id, { kind = "Box", x = 1, y = 1, z = 1 })
    end
    Scene.SetStatic(id, true)
    return id
end
local function pole(x, z, kind, out_x, out_z)
    local h = ({ lamp = 6, sign = 2.6, signal = 4 })[kind]
    local id = entity("Pole", "Cylinder", groups.Props)
    Transform.SetPosition(id, x, CURB + h / 2, z)
    Transform.SetLocalScale(id, 0.18, h / 2, 0.18)
    Material.Set(id, "steel")
    Scene.SetStatic(id, true)
    if kind == "lamp" then
        box("Lamp", groups.Props, x + out_x * 0.6, CURB + h - 0.2, z + out_z * 0.6,
            0.3 + math.abs(out_x) * 0.9, 0.2, 0.3 + math.abs(out_z) * 0.9, "steel")
    elseif kind == "sign" then
        box("Sign", groups.Props, x, CURB + h - 0.6, z, 0.06 + math.abs(out_z) * 0.7, 0.6,
            0.06 + math.abs(out_x) * 0.7, "signal")
    else
        box("Signal", groups.Props, x, CURB + h - 0.9, z, 0.3, 0.9, 0.3, "signal")
    end
end

local deck_y = function(level) return (level - 1) * DECK_GAP end
for i = 0, NX do
    for j = 0, NZ do
        local x, z = ave_x(i), st_z(j)
        road("Crossing", "Crossing", x, 0, z)
        for level = 2, p.levels do road("CrossingDeck", "CrossingDeck", x, deck_y(level), z) end
        if p.levels > 1 then
            box("Pillar", groups.Streets, x, 0, z, 2, deck_y(p.levels) - DECK_T, 2, "pavement", true)
        end
        for _, s in ipairs({ { -1, -1 }, { 1, -1 }, { -1, 1 }, { 1, 1 } }) do
            pole(x + s[1] * (AVE / 2 - AVE_WALK + 0.5), z + s[2] * (ST / 2 - ST_WALK + 0.5), "signal")
        end
        if j < NZ then
            local mid = (st_z(j) + st_z(j + 1)) / 2
            road("Avenue", "Avenue", x, 0, mid, BLOCK_Z)
            for level = 2, p.levels do road("AvenueDeck", "AvenueDeck", x, deck_y(level), mid, BLOCK_Z) end
            for side = -1, 1, 2 do
                local px, n = x + side * (AVE / 2 - AVE_WALK + 0.7), 0
                for d = POLE_GAP / 2, BLOCK_Z, POLE_GAP do
                    n = n + 1
                    pole(px, z + ST / 2 + d, n % 2 == 1 and "lamp" or "sign", -side, 0)
                end
            end
        end
        if i < NX then
            local mid = (ave_x(i) + ave_x(i + 1)) / 2
            road("Street", "Street", mid, 0, z, BLOCK_X, 90)
            for side = -1, 1, 2 do
                local pz, n = z + side * (ST / 2 - ST_WALK + 0.7), 0
                for d = POLE_GAP / 2, BLOCK_X, POLE_GAP do
                    n = n + 1
                    pole(x + AVE / 2 + d, pz, n % 2 == 1 and "lamp" or "sign", 0, -side)
                end
            end
        end
    end
end

local EX0, EX1 = ave_x(0) - AVE / 2, ave_x(NX) + AVE / 2
local EZ0, EZ1 = st_z(0) - ST / 2, st_z(NZ) + ST / 2
local WALL_H, WALL_T = 6, 1
box("Barrier_S", groups.Edge, (EX0 + EX1) / 2, 0, EZ0 - WALL_T / 2, EX1 - EX0 + 2 * WALL_T, WALL_H, WALL_T, "barrier", true)
box("Barrier_N", groups.Edge, (EX0 + EX1) / 2, 0, EZ1 + WALL_T / 2, EX1 - EX0 + 2 * WALL_T, WALL_H, WALL_T, "barrier", true)
box("Barrier_W", groups.Edge, EX0 - WALL_T / 2, 0, (EZ0 + EZ1) / 2, WALL_T, WALL_H, EZ1 - EZ0, "barrier", true)
box("Barrier_E", groups.Edge, EX1 + WALL_T / 2, 0, (EZ0 + EZ1) / 2, WALL_T, WALL_H, EZ1 - EZ0, "barrier", true)

local sun = entity("Sun", "DirectionalLight", city)
Light.SetMode(sun, "Mixed")
Transform.SetRotation(sun, -50, 35, 0)
-- The bench's viewpoints; Play keeps the first.
local VIEWS = { "Street", "CrossStreet", "Rooftop", "High", "Inside", "FarCorner" }
if p.levels > 1 then VIEWS[#VIEWS + 1] = "Deck" end
local views = {}
for _, name in ipairs(VIEWS) do
    views[name] = entity(name, nil, groups.Viewpoints)
    Scene.AddComponent(views[name], "Camera")
    Camera.SetClipPlanes(views[name], 0.1, 4000)
end
Scene.AddScript(groups.Viewpoints, "assets/scripts/viewpoints.lua")

local blocks = {}
for i = 0, NX - 1 do
    for j = 0, NZ - 1 do
        local b = { x0 = ave_x(i) + AVE / 2, x1 = ave_x(i + 1) - AVE / 2,
                    z0 = st_z(j) + ST / 2, z1 = st_z(j + 1) - ST / 2 }
        box("Block", groups.Blocks, (b.x0 + b.x1) / 2, 0, (b.z0 + b.z1) / 2, BLOCK_X, CURB, BLOCK_Z, "pavement", true)
        blocks[#blocks + 1] = b
    end
end
local budget = p.objects - made
if budget < 0 then
    error(string.format("city: objects=%d is fewer than the %d its streets, poles and edge take at size %g",
        p.objects, made, p.size))
end
local ideal = #blocks * math.floor(BLOCK_X * BLOCK_Z / LOT)
local parts = math.max(1, math.min(MAX_PARTS, math.floor(budget / ideal)))
local buildings = math.floor(budget / parts)
local extra = budget - buildings * parts
local cx, cz = (EX0 + EX1) / 2, (EZ0 + EZ1) / 2
local reach = math.sqrt((EX1 - cx) ^ 2 + (EZ1 - cz) ^ 2)
local tallest, built = { h = 0 }, 0

local function building(x0, x1, z0, z1, n)
    local x, z, w, d = (x0 + x1) / 2, (z0 + z1) / 2, x1 - x0 - ALLEY, z1 - z0 - ALLEY
    local core = 1 - math.sqrt((x - cx) ^ 2 + (z - cz) ^ 2) / reach
    local h = (12 + 180 * core * core) * (0.35 + 0.65 * rand())
    local facade = FACADES[1 + math.floor(rand() * #FACADES)]
    local y, share = CURB, (n == 1) and 1 or 0.55
    for k = 1, n do
        local sy = (k == 1) and h * share or h * (1 - 0.55) / math.max(1, n - 1)
        box("Building", groups.Buildings, x, y, z, w, sy, d, facade, k == 1)
        y = y + sy
        w, d = w * 0.8, d * 0.8
    end
    if y > tallest.h then tallest = { h = y, x = x, z = z } end
end
for index, b in ipairs(blocks) do
    local count = math.floor(buildings / #blocks) + ((index <= buildings % #blocks) and 1 or 0)
    if count > 0 then
        local rows = math.max(1, math.floor(math.sqrt(count * BLOCK_Z / BLOCK_X) + 0.5))
        rows = math.min(rows, count)
        for r = 0, rows - 1 do
            local in_row = math.floor(count / rows) + ((r < count % rows) and 1 or 0)
            local rz0 = b.z0 + r * BLOCK_Z / rows
            for c = 0, in_row - 1 do
                built = built + 1
                local lx0 = b.x0 + c * BLOCK_X / in_row
                building(lx0, lx0 + BLOCK_X / in_row, rz0, rz0 + BLOCK_Z / rows,
                    parts + ((built <= extra) and 1 or 0))
            end
        end
    end
end
if made ~= p.objects then
    error(string.format("city: made %d entities, not the %d asked for", made, p.objects))
end

local function aim(id, x, y, z, tx, ty, tz)
    Transform.SetPosition(id, x, y, z)
    local dx, dy, dz = tx - x, ty - y, tz - z
    local yaw = math.deg(math.atan(-dx, -dz))
    local pitch = math.deg(math.atan(dy, math.sqrt(dx * dx + dz * dz)))
    Transform.SetRotation(id, pitch, yaw, 0)
end
local mid_i, mid_j = math.floor(NX / 2), math.floor(NZ / 2)
local first = blocks[1]
aim(views.Street, ave_x(mid_i) + 3, 1.7, st_z(0) - ST / 4, ave_x(mid_i) + 3, 1.7, st_z(NZ))
aim(views.CrossStreet, ave_x(0) - AVE / 4, 1.7, st_z(mid_j) + 2, ave_x(NX), 1.7, st_z(mid_j) + 2)
if tallest.x then
    aim(views.Rooftop, tallest.x, tallest.h + 1.7, tallest.z, EX1, tallest.h / 3, EZ1)
else
    aim(views.Rooftop, cx, 60, cz, cx + 1, 0, cz + 100)
end
aim(views.High, cx - 1, 500, cz - 300, cx, 0, cz)
aim(views.Inside, first.x0 + 4, 2, first.z0 + 4, first.x1, 2, first.z1)
aim(views.FarCorner, ave_x(0), 1.7, st_z(0), EX1, 1.7, EZ1)
if views.Deck then
    aim(views.Deck, ave_x(mid_i) + 3, deck_y(p.levels) + 1.7, st_z(0), ave_x(mid_i) + 3, deck_y(p.levels), st_z(NZ))
end

local SCENE = "assets/scenes/city.scene"
Scene.Save(SCENE)
Application.SetStartupScene(SCENE)
