-- crates/script/src/api/particle/live.lua — the Lua half of the live-particle
-- calls (#2171). `live.rs` hands an emitter's particles over as one packed
-- string (`read`) and takes them back as one (`write`) or as a list of edits
-- (`edit`), so the per-particle work is done here, by the VM, not one Rust call
-- per field.
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
--
-- `for_each` hands `fn` one table per call, refilled for each particle; after
-- `fn` each field is compared with the value read, and only the changed ones go
-- back, as edits (an index-and-slot key, a value), so nothing is packed, checked
-- or allocated per particle a rule leaves alone. The table is emptied and killed
-- when the call ends, so one kept past it errors instead of reading on.
local read, write, edit, PARTICLE, RECORD, names = ...
local pack, unpack, concat, type, pcall, error = string.pack, string.unpack, table.concat,
    type, pcall, error
local next, setmetatable = next, setmetatable
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

-- The fields in wire order, and each one's 0-based slot (an edit's key is
-- `index * SLOTS + slot`, which `live.rs` splits the same way).
local FIELDS, SLOT, SLOTS = {}, {}, 32
for name in names:gmatch("[^,]+") do
    FIELDS[#FIELDS + 1] = name
    SLOT[name] = #FIELDS - 1
end

-- The newest read of each emitter: `at` maps a handed-out table to its 0-based
-- source, `age` holds each source's age at that read. Dropped by a write.
local reads = {}

local function bad(verb, i, field, x)
    error(("Particles.%s: entry %d's %s is a %s, not a number or nil")
        :format(verb, i, field, type(x)), 4)
end

-- Fail on an entry field that is neither nil nor a number. Run only once the
-- fast path in `record` has failed, to name the entry and the field.
local function check(verb, i, p)
    for k = 1, #FIELDS do
        local x = p[FIELDS[k]]
        if x ~= nil and type(x) ~= "number" then bad(verb, i, FIELDS[k], x) end
    end
end

-- The record for table `p`, read from live particle `src` (0-based) at age
-- `stamp`. A field left out is sent as NaN, which `live.rs` keeps. The `< INF`
-- comparisons are the type check: comparing a string, a boolean or a table with
-- a number raises (`pack` alone would take "2" for 2), with no call per field.
local function record(src, stamp, p)
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
    return pack(RECORD, src, stamp, x or NAN, y or NAN, z or NAN, vx or NAN, vy or NAN,
        vz or NAN, age or NAN, life or NAN, size or NAN, rot or NAN, r or NAN, g or NAN,
        b or NAN, a or NAN, c1 or NAN, c2 or NAN, c3 or NAN, c4 or NAN)
end

local function live_id(id, verb)
    if id == nil then
        error("Particles." .. verb .. ": no entity (nil id)", 3)
    end
end

local function get(id)
    local s, count = read(id, false)
    local list, at, ages, pos = {}, setmetatable({}, WEAK), {}, 1
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
        list[i], at[p], ages[i] = p, i - 1, age
    end
    if id ~= nil then
        reads[id] = { at = at, age = ages }
    end
    return list
end

-- Every entry's record into `out`; raises on the first bad entry, unnamed.
local function records(list, out, at, ages)
    for i = 1, #list do
        local p = list[i]
        if type(p) ~= "table" then error("not a table") end
        local src = at[p]
        if src then
            out[i] = record(src, ages[src + 1], p)
        else
            out[i] = record(NO_SOURCE, NAN, p)
        end
    end
end

local function set(id, list)
    live_id(id, "SetParticles")
    if type(list) ~= "table" then
        error("Particles.SetParticles: the list is not a table", 2)
    end
    local newest = reads[id]
    local at, ages = newest and newest.at or NONE, newest and newest.age or NONE
    local out = {}
    local ok, err = pcall(records, list, out, at, ages)
    if not ok then
        for i = 1, #list do
            if type(list[i]) ~= "table" then
                error(("Particles.SetParticles: entry %d is not a particle table"):format(i), 2)
            end
            check("SetParticles", i, list[i])
        end
        error(err, 0)
    end
    reads[id] = nil
    return write(id, "SetParticles", concat(out))
end

-- What a view kept past its `ForEach` does on any read or write.
local function dead()
    error("Particles.ForEach: a particle view was used after its ForEach returned; "
        .. "copy the numbers you need out of it inside fn", 2)
end
local DEAD = { __index = dead, __newindex = dead, __metatable = false }


-- Log `v` as edit `key` at `edits[ne]`. The `< INF` is the type check: a
-- string, a boolean or a table raises there, and `for_each` names the field.
local function log(edits, ne, key, v)
    local _ = v < INF
    edits[ne + 1], edits[ne + 2] = key, v
    return ne + 2
end

-- The first field of `p` that is neither nil nor a number, and its type.
local function wrong(p)
    for k = 1, #FIELDS do
        local x = p[FIELDS[k]]
        if x ~= nil and type(x) ~= "number" then return FIELDS[k], type(x) end
    end
end

local function for_each(id, fn)
    live_id(id, "ForEach")
    if type(fn) ~= "function" then
        error("Particles.ForEach: fn is a " .. type(fn) .. ", not a function", 2)
    end
    local s, count, snapshot = read(id, true)
    local p, edits, ne, kills, nk, n = {}, {}, 0, {}, 0, 0
    -- Fill `p` with each particle in turn, run `fn`, and log what it changed:
    -- a field is compared with the value read, so nothing is called for one
    -- left alone.
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
                v = p.x; if v ~= x and v ~= nil then ne = log(edits, ne, at + 0, v) end
                v = p.y; if v ~= y and v ~= nil then ne = log(edits, ne, at + 1, v) end
                v = p.z; if v ~= z and v ~= nil then ne = log(edits, ne, at + 2, v) end
                v = p.vx; if v ~= vx and v ~= nil then ne = log(edits, ne, at + 3, v) end
                v = p.vy; if v ~= vy and v ~= nil then ne = log(edits, ne, at + 4, v) end
                v = p.vz; if v ~= vz and v ~= nil then ne = log(edits, ne, at + 5, v) end
                v = p.age; if v ~= age and v ~= nil then ne = log(edits, ne, at + 6, v) end
                v = p.lifetime; if v ~= life and v ~= nil then ne = log(edits, ne, at + 7, v) end
                v = p.size; if v ~= size and v ~= nil then ne = log(edits, ne, at + 8, v) end
                v = p.rotation; if v ~= rot and v ~= nil then ne = log(edits, ne, at + 9, v) end
                v = p.r; if v ~= r and v ~= nil then ne = log(edits, ne, at + 10, v) end
                v = p.g; if v ~= g and v ~= nil then ne = log(edits, ne, at + 11, v) end
                v = p.b; if v ~= b and v ~= nil then ne = log(edits, ne, at + 12, v) end
                v = p.a; if v ~= a and v ~= nil then ne = log(edits, ne, at + 13, v) end
                v = p.custom1; if v ~= c1 and v ~= nil then ne = log(edits, ne, at + 14, v) end
                v = p.custom2; if v ~= c2 and v ~= nil then ne = log(edits, ne, at + 15, v) end
                v = p.custom3; if v ~= c3 and v ~= nil then ne = log(edits, ne, at + 16, v) end
                v = p.custom4; if v ~= c4 and v ~= nil then ne = log(edits, ne, at + 17, v) end
            end
        end
    end
    local ok, err = pcall(visit)
    local field, kind = wrong(p)
    for k in next, p do p[k] = nil end
    setmetatable(p, DEAD)
    if not ok then
        if field then
            error(("Particles.ForEach: entry %d's %s is a %s, not a number or nil")
                :format(n, field, kind), 2)
        end
        error(err, 0)
    end
    reads[id] = nil
    return edit(id, snapshot, edits, kills)
end

return get, set, for_each
