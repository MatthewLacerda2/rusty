-- crates/script/src/api/particle/live.lua — the Lua half of the live-particle
-- calls (#2171). `live.rs` hands an emitter's particles over as one packed
-- string (`read`) and takes them back as one (`write`), so the per-particle
-- tables are built here, by the VM, not one Rust call per field.
--
-- `live.rs` owns the layout and passes it in: `PARTICLE`, the format of one
-- particle read; `RECORD`, the format of one record written back (a u32 source,
-- the age that source had when read, then the particle's fields); and `names`,
-- the fields in wire order. The unpacking and packing below are written out by
-- position for speed, so `ORDER` must equal `names`, and loading fails if not.
--
-- A table `get` hands out carries its identity: `reads[id]` maps it to the index
-- it was read from, for the emitter's newest read only. `set` sends that source
-- with each entry, so a survivor keeps its own hidden fields wherever the script
-- moved it; a table with no identity there is sent as an added particle.
local read, write, PARTICLE, RECORD, names = ...
local pack, unpack, concat, type = string.pack, string.unpack, table.concat, type
local ORDER = "position.x,position.y,position.z,velocity.x,velocity.y,velocity.z,"
    .. "age,lifetime,size,rotation,color.r,color.g,color.b,color.a,"
    .. "custom.1,custom.2,custom.3,custom.4"
if names ~= ORDER then
    error("particles_live: live.rs's field order\n  " .. names
        .. "\nis not the one live.lua packs\n  " .. ORDER, 0)
end
local NAN = 0 / 0
local NO_SOURCE = 0xFFFFFFFF
local NONE = {}
local WEAK = { __mode = "k" }

-- The fields an entry is checked for, from `names`: plain numbers, and the
-- vector groups with their components.
local SCALARS, GROUPS = {}, {}
for name in names:gmatch("[^,]+") do
    local group, key = name:match("^(%w+)%.(%w+)$")
    if not group then
        SCALARS[#SCALARS + 1] = name
    else
        local g = GROUPS[#GROUPS]
        if not g or g[1] ~= group then
            g = { group }
            GROUPS[#GROUPS + 1] = g
        end
        g[#g + 1] = tonumber(key) or key
    end
end

-- The newest read of each emitter: `at` maps a handed-out table to its 0-based
-- source, `age` holds each source's age at that read. Dropped by a write.
local reads = {}

local function bad(verb, i, field, x)
    error(("Particles.%s: entry %d's %s is a %s, not a number or nil")
        :format(verb, i, field, type(x)), 4)
end

-- Fail, before anything is written, on an entry field that is neither nil nor
-- a number (a vector group: nil or a table of those).
local function check(verb, i, p)
    for k = 1, #SCALARS do
        local x = p[SCALARS[k]]
        if x ~= nil and type(x) ~= "number" then bad(verb, i, SCALARS[k], x) end
    end
    for g = 1, #GROUPS do
        local group = GROUPS[g]
        local t = p[group[1]]
        if t ~= nil then
            if type(t) ~= "table" then
                error(("Particles.%s: entry %d's %s is a %s, not a table or nil")
                    :format(verb, i, group[1], type(t)), 3)
            end
            for k = 2, #group do
                local x = t[group[k]]
                if x ~= nil and type(x) ~= "number" then
                    bad(verb, i, group[1] .. "." .. group[k], x)
                end
            end
        end
    end
end

-- The record for table `p`, read from live particle `src` (0-based) at age
-- `stamp`. A field left out is sent as NaN, which `live.rs` keeps. Inlined
-- (`or`, not a helper per field): a Lua call per field was most of the cost.
local function record(src, stamp, p)
    local q, v, c, k = p.position or NONE, p.velocity or NONE, p.color or NONE, p.custom or NONE
    return pack(RECORD, src, stamp,
        q.x or NAN, q.y or NAN, q.z or NAN, v.x or NAN, v.y or NAN, v.z or NAN,
        p.age or NAN, p.lifetime or NAN, p.size or NAN, p.rotation or NAN,
        c.r or NAN, c.g or NAN, c.b or NAN, c.a or NAN,
        k[1] or NAN, k[2] or NAN, k[3] or NAN, k[4] or NAN)
end

local function live_id(id, verb)
    if id == nil then
        error("Particles." .. verb .. ": no entity (nil id)", 3)
    end
end

local function get(id)
    local s, count = read(id)
    local list, at, ages, pos = {}, setmetatable({}, WEAK), {}, 1
    for i = 1, count do
        local px, py, pz, vx, vy, vz, age, life, size, rot, r, g, b, a, c1, c2, c3, c4, nxt =
            unpack(PARTICLE, s, pos)
        pos = nxt
        local p = {
            position = { x = px, y = py, z = pz },
            velocity = { x = vx, y = vy, z = vz },
            age = age, lifetime = life, size = size, rotation = rot,
            color = { r = r, g = g, b = b, a = a },
            custom = { c1, c2, c3, c4 },
        }
        list[i], at[p], ages[i] = p, i - 1, age
    end
    if id ~= nil then
        reads[id] = { at = at, age = ages }
    end
    return list
end

local function set(id, list)
    live_id(id, "SetParticles")
    if type(list) ~= "table" then
        error("Particles.SetParticles: the list is not a table", 2)
    end
    local newest = reads[id]
    local at, ages = newest and newest.at or NONE, newest and newest.age or NONE
    local out = {}
    for i = 1, #list do
        local p = list[i]
        if type(p) ~= "table" then
            error(("Particles.SetParticles: entry %d is not a particle table"):format(i), 2)
        end
        check("SetParticles", i, p)
        local src = at[p]
        if src then
            out[i] = record(src, ages[src + 1], p)
        else
            out[i] = record(NO_SOURCE, NAN, p)
        end
    end
    reads[id] = nil
    return write(id, "SetParticles", concat(out))
end

local function for_each(id, fn)
    live_id(id, "ForEach")
    if type(fn) ~= "function" then
        error("Particles.ForEach: fn is a " .. type(fn) .. ", not a function", 2)
    end
    local s, count = read(id)
    local P, V, C, K, p = {}, {}, {}, {}, {}
    local out, m, pos = {}, 0, 1
    for i = 1, count do
        local age, nxt
        P.x, P.y, P.z, V.x, V.y, V.z, age, p.lifetime, p.size, p.rotation,
            C.r, C.g, C.b, C.a, K[1], K[2], K[3], K[4], nxt = unpack(PARTICLE, s, pos)
        pos, p.age = nxt, age
        p.position, p.velocity, p.color, p.custom = P, V, C, K
        if fn(p) ~= false then
            check("ForEach", i, p)
            m = m + 1
            out[m] = record(i - 1, age, p)
        end
    end
    reads[id] = nil
    return write(id, "ForEach", concat(out))
end

return get, set, for_each
