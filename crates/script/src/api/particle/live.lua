-- `ORDER` must equal `names` (loading fails if not): the unpacking is written out by position for speed.
local halves, PARTICLE, RECORD, names, SLOTS = ...
local pack, unpack, concat, type, pcall, error = string.pack, string.unpack, table.concat,
    type, pcall, error
local next, setmetatable, rawget, rawset = next, setmetatable, rawget, rawset
local ORDER = "x,y,z,vx,vy,vz,age,lifetime,size,rotation,r,g,b,a,"
    .. "custom1,custom2,custom3,custom4"
if names ~= ORDER then
    error("particles_live: live.rs's field order\n  " .. names
        .. "\nis not the one live.lua packs\n  " .. ORDER, 0)
end
local NAN, INF = 0 / 0, 1 / 0
local NO_SOURCE = 0xFFFFFFFF
local NONE = {}
local WEAK = { __mode = "k" }

local FIELDS, SLOT = {}, {}
for name in names:gmatch("[^,]+") do
    FIELDS[#FIELDS + 1] = name
    SLOT[name] = #FIELDS - 1
end
local S_X, S_Y, S_Z, S_VX, S_VY, S_VZ = SLOT.x, SLOT.y, SLOT.z, SLOT.vx, SLOT.vy, SLOT.vz
local S_AGE, S_LIFE, S_SIZE, S_ROT = SLOT.age, SLOT.lifetime, SLOT.size, SLOT.rotation
local S_R, S_G, S_B, S_A = SLOT.r, SLOT.g, SLOT.b, SLOT.a
local S_C1, S_C2, S_C3, S_C4 = SLOT.custom1, SLOT.custom2, SLOT.custom3, SLOT.custom4

local reads = {}

local function wrong(p)
    for k = 1, #FIELDS do
        local x = rawget(p, FIELDS[k])
        if x ~= nil and type(x) ~= "number" then return FIELDS[k], type(x) end
    end
end

local function bad(verb, i, field, kind, level)
    error(("Particles.%s: entry %d's %s is a %s, not a number or nil")
        :format(verb, i, field, kind), level)
end

-- A field left out is sent as NaN, which `live.rs` keeps. The `< INF` comparisons are the type check: comparing a string, boolean or table raises (`pack` alone would take "2" for 2).
local function record(src, p)
    local x, y, z, vx, vy, vz = p.x, p.y, p.z, p.vx, p.vy, p.vz
    local age, life, size, rot = p.age, p.lifetime, p.size, p.rotation
    local r, g, b, a = p.r, p.g, p.b, p.a
    local c1, c2, c3, c4 = p.custom1, p.custom2, p.custom3, p.custom4
    local _ = x == nil or x < INF, y == nil or y < INF, z == nil or z < INF,
        vx == nil or vx < INF, vy == nil or vy < INF, vz == nil or vz < INF,
        age == nil or age < INF, life == nil or life < INF, size == nil or size < INF,
        rot == nil or rot < INF, r == nil or r < INF, g == nil or g < INF,
        b == nil or b < INF, a == nil or a < INF, c1 == nil or c1 < INF,
        c2 == nil or c2 < INF, c3 == nil or c3 < INF, c4 == nil or c4 < INF
    return pack(RECORD, src, x or NAN, y or NAN, z or NAN, vx or NAN, vy or NAN,
        vz or NAN, age or NAN, life or NAN, size or NAN, rot or NAN, r or NAN, g or NAN,
        b or NAN, a or NAN, c1 or NAN, c2 or NAN, c3 or NAN, c4 or NAN)
end

local function live_id(id, verb)
    if id == nil then
        error("Particles." .. verb .. ": no entity (nil id)", 3)
    end
end

local function get(id)
    local s, count, rev = halves.GetParticles(id, false)
    local list, at, pos = {}, setmetatable({}, WEAK), 1
    for i = 1, count do
        local x, y, z, vx, vy, vz, age, life, size, rot, r, g, b, a, c1, c2, c3, c4, nxt =
            unpack(PARTICLE, s, pos)
        pos = nxt
        local p = {
            x = x, y = y, z = z, vx = vx, vy = vy, vz = vz,
            age = age, lifetime = life, size = size, rotation = rot,
            r = r, g = g, b = b, a = a,
            custom1 = c1, custom2 = c2, custom3 = c3, custom4 = c4,
        }
        list[i], at[p] = p, i - 1
    end
    if id ~= nil then
        reads[id] = { at = at, rev = rev }
    end
    return list
end

local function records(list, out, at)
    for i = 1, #list do
        local p = list[i]
        if type(p) ~= "table" then error("not a table") end
        out[i] = record(at[p] or NO_SOURCE, p)
    end
end

local function set(id, list)
    live_id(id, "SetParticles")
    if type(list) ~= "table" then
        error("Particles.SetParticles: the list is not a table", 2)
    end
    local newest = reads[id]
    local at, rev = newest and newest.at or NONE, newest and newest.rev
    local out = {}
    local ok, err = pcall(records, list, out, at)
    if not ok then
        for i = 1, #list do
            if type(list[i]) ~= "table" then
                error(("Particles.SetParticles: entry %d is not a particle table"):format(i), 2)
            end
            local field, kind = wrong(list[i])
            if field then bad("SetParticles", i, field, kind, 3) end
        end
        error(err, 0)
    end
    reads[id] = nil
    return halves.SetParticles(id, "SetParticles", rev, concat(out))
end

local function dead()
    error("Particles.ForEach: a particle view was used after its ForEach returned; "
        .. "copy the numbers you need out of it inside fn", 2)
end
local DEAD = { __index = dead, __newindex = dead, __metatable = false }

local function log(edits, ne, key, v)
    local _ = v < INF
    edits[ne + 1], edits[ne + 2] = key, v
    return ne + 2
end

local function misfit(edits)
    for k = 2, #edits, 2 do
        local v = edits[k]
        if type(v) ~= "number" then
            local key = edits[k - 1]
            return key // SLOTS + 1, FIELDS[key % SLOTS + 1], type(v)
        end
    end
end

local function for_each(id, fn)
    live_id(id, "ForEach")
    if type(fn) ~= "function" then
        error("Particles.ForEach: fn is a " .. type(fn) .. ", not a function", 2)
    end
    local s, count, _, snapshot = halves.ForEach(id, true)
    local p, edits, ne, kills, nk, n = {}, {}, 0, {}, 0, 0
    local function visit()
        local pos = 1
        for i = 1, count do
            n = i
            local x, y, z, vx, vy, vz, age, life, size, rot, r, g, b, a, c1, c2, c3, c4, nxt =
                unpack(PARTICLE, s, pos)
            pos = nxt
            p.x, p.y, p.z, p.vx, p.vy, p.vz = x, y, z, vx, vy, vz
            p.age, p.lifetime, p.size, p.rotation = age, life, size, rot
            p.r, p.g, p.b, p.a = r, g, b, a
            p.custom1, p.custom2, p.custom3, p.custom4 = c1, c2, c3, c4
            if fn(p) == false then
                nk = nk + 1
                kills[nk] = n - 1
            else
                local at, v = (n - 1) * SLOTS, nil
                v = p.x; if v ~= x and v ~= nil then ne = log(edits, ne, at + S_X, v) end
                v = p.y; if v ~= y and v ~= nil then ne = log(edits, ne, at + S_Y, v) end
                v = p.z; if v ~= z and v ~= nil then ne = log(edits, ne, at + S_Z, v) end
                v = p.vx; if v ~= vx and v ~= nil then ne = log(edits, ne, at + S_VX, v) end
                v = p.vy; if v ~= vy and v ~= nil then ne = log(edits, ne, at + S_VY, v) end
                v = p.vz; if v ~= vz and v ~= nil then ne = log(edits, ne, at + S_VZ, v) end
                v = p.age; if v ~= age and v ~= nil then ne = log(edits, ne, at + S_AGE, v) end
                v = p.lifetime; if v ~= life and v ~= nil then ne = log(edits, ne, at + S_LIFE, v) end
                v = p.size; if v ~= size and v ~= nil then ne = log(edits, ne, at + S_SIZE, v) end
                v = p.rotation; if v ~= rot and v ~= nil then ne = log(edits, ne, at + S_ROT, v) end
                v = p.r; if v ~= r and v ~= nil then ne = log(edits, ne, at + S_R, v) end
                v = p.g; if v ~= g and v ~= nil then ne = log(edits, ne, at + S_G, v) end
                v = p.b; if v ~= b and v ~= nil then ne = log(edits, ne, at + S_B, v) end
                v = p.a; if v ~= a and v ~= nil then ne = log(edits, ne, at + S_A, v) end
                v = p.custom1; if v ~= c1 and v ~= nil then ne = log(edits, ne, at + S_C1, v) end
                v = p.custom2; if v ~= c2 and v ~= nil then ne = log(edits, ne, at + S_C2, v) end
                v = p.custom3; if v ~= c3 and v ~= nil then ne = log(edits, ne, at + S_C3, v) end
                v = p.custom4; if v ~= c4 and v ~= nil then ne = log(edits, ne, at + S_C4, v) end
            end
        end
    end
    local ok, err = pcall(visit)
    local field, kind = wrong(p)
    for k in next, p do rawset(p, k, nil) end
    pcall(setmetatable, p, DEAD)
    if not ok then
        if field then bad("ForEach", n, field, kind, 3) end
        error(err, 0)
    end
    reads[id] = nil
    local done, live = pcall(halves["ForEach/edit"], id, snapshot, edits, kills)
    if not done then
        local i, name, what = misfit(edits)
        if i then bad("ForEach", i, name, what, 3) end
        error(live, 0)
    end
    return live
end

return get, set, for_each
