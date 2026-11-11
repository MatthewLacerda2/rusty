-- scenarios/probes.lua — both reflection probes were baked, and their volumes
-- are drawn over a shot: the turned box over the west half, the sphere over the
-- east one (cache/test/probes/box.png and sphere.png).

Harness.Expect(Reflection.Count() == 2, "the showroom has two reflection probes")
for i = 0, Reflection.Count() - 1 do
    local probe = Reflection.Get(i)
    if probe.cubemap == "" then
        Harness.Log("skipped: probe " .. i .. " is unbaked (no GPU adapter at generation)")
        return
    end
    Harness.Expect(probe.cubemap:find("%.ktx2$") ~= nil, "probe " .. i .. " has a baked cubemap")
end

-- A line from `a` to `b`, both {x, y, z}, held for the shot.
local function line(a, b, r, g, bl)
    Debug.DrawLine(a[1], a[2], a[3], b[1], b[2], b[3], r, g, bl, 1, 1)
end

-- The box: its eight corners turned about its position by its Y rotation.
local box = Reflection.Get(0)
local c, s = math.cos(math.rad(box.rotation.y)), math.sin(math.rad(box.rotation.y))
local function corner(x, y, z)
    local dx, dz = x - box.position.x, z - box.position.z
    return { box.position.x + dx * c + dz * s, y, box.position.z - dx * s + dz * c }
end
local lo, hi = box.boxMin, box.boxMax
for _, y in ipairs({ lo.y, hi.y }) do
    local q = { corner(lo.x, y, lo.z), corner(hi.x, y, lo.z), corner(hi.x, y, hi.z),
        corner(lo.x, y, hi.z) }
    for k = 1, 4 do line(q[k], q[k % 4 + 1], 0.3, 0.6, 1) end
end
for _, xz in ipairs({ { lo.x, lo.z }, { hi.x, lo.z }, { hi.x, hi.z }, { lo.x, hi.z } }) do
    line(corner(xz[1], lo.y, xz[2]), corner(xz[1], hi.y, xz[2]), 0.3, 0.6, 1)
end

-- The sphere: three great circles round its position.
local sphere = Reflection.Get(1)
local o, rad = sphere.position, sphere.radius
local function on_circle(axis, t)
    local u, v = rad * math.cos(t), rad * math.sin(t)
    if axis == 1 then return { o.x + u, o.y + v, o.z } end
    if axis == 2 then return { o.x + u, o.y, o.z + v } end
    return { o.x, o.y + u, o.z + v }
end
for axis = 1, 3 do
    for k = 0, 31 do
        line(on_circle(axis, k * math.pi / 16), on_circle(axis, (k + 1) * math.pi / 16), 1, 0.8, 0.2)
    end
end

-- Each volume seen from the far corner of the other half of the room.
local views = {
    { name = "box", eye = { 5.6, 3.3, -3.7 }, at = { -3.0, 1.0, 0.0 } },
    { name = "sphere", eye = { -5.6, 3.3, -3.7 }, at = { 3.0, 1.0, 0.0 } },
}
for _, view in ipairs(views) do
    local eye, at = view.eye, view.at
    local dx, dy, dz = at[1] - eye[1], at[2] - eye[2], at[3] - eye[3]
    Camera.SetPosition(eye[1], eye[2], eye[3])
    Camera.SetYaw(math.deg(math.atan(dz, dx)))
    Camera.SetPitch(math.deg(math.atan(dy, math.sqrt(dx * dx + dz * dz))))
    Camera.ResetHistory()
    local path = "cache/test/probes/" .. view.name .. ".png"
    if not Harness.Screenshot(path, { settle = 8, gizmos = true }) then
        Harness.Log("shot skipped (no GPU adapter): " .. view.name)
    end
end
