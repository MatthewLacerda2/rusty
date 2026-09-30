-- scroll_view.lua — the engine's Scroll View widget (#422), Unity's `ScrollRect`.
--
-- Moves `Viewport/Content` (pinned to the viewport's top-left) by drag and wheel,
-- inside the bounds its size allows. `movement_type`: Elastic springs back past an
-- edge (over `elasticity` seconds, rubber-banding while dragged), Clamped stops at
-- it, Unrestricted never stops. With `inertia` a released drag coasts, slowing by
-- `deceleration_rate` per second. Everything runs on unscaled time, so a paused
-- game's menus still scroll. `Scrollbar Horizontal` / `Scrollbar Vertical`
-- children, when present, follow the content and drive it.
--
-- Owner API (via `Scene.GetScript(id, "scroll_view")`):
--   on_value_changed = function(id, x, y) … end   -- normalized position moved
--   get_normalized_position()   -- x (0 = left), y (1 = top), each 0..1
--   set_normalized_position(x, y)
--   stop_movement()

local ScrollView = {
    fields = {
        horizontal = { type = "boolean", default = true, tooltip = "Scroll sideways" },
        vertical = { type = "boolean", default = true, tooltip = "Scroll up and down" },
        movement_type = { type = "text", default = "Elastic", tooltip = "Elastic, Clamped or Unrestricted" },
        elasticity = { type = "number", default = 0.1, tooltip = "Seconds to spring back past an edge" },
        inertia = { type = "boolean", default = true, tooltip = "Coast after a drag" },
        deceleration_rate = { type = "number", default = 0.135, range = { 0, 1 }, tooltip = "Speed kept per second while coasting" },
        scroll_sensitivity = { type = "number", default = 40, tooltip = "Reference units per wheel line" },
    },
}

local self_id, viewport, content, hbar, vbar
local pos = { 0, 0 }      -- content's anchored position
local vel = { 0, 0 }      -- units / second
local prev = { 0, 0 }
local dragging = false
local drag_start, drag_sum = { 0, 0 }, { 0, 0 }
local last_norm = { -1, -1 }

local function sizes()
    local v, c = UI.GetRect(viewport), UI.GetRect(content)
    if not v or not c then return 0, 0, 0, 0 end
    return v.width, v.height, c.width, c.height
end

-- The allowed range per axis: x in [lo, 0], y in [0, hi].
local function bounds()
    local vw, vh, cw, ch = sizes()
    return { math.min(0, vw - cw), 0 }, { 0, math.max(0, ch - vh) }, vw, vh
end

local function clamp_axis(i, v)
    local lo, hi = bounds()
    return math.max(lo[i], math.min(hi[i], v))
end

local function rubber(over, view)
    if view <= 0 then return 0 end
    local s = over < 0 and -1 or 1
    return (1 - 1 / (math.abs(over) * 0.55 / view + 1)) * view * s
end

local function smooth_damp(cur, target, v, t, dt)
    t = math.max(0.0001, t)
    local omega = 2 / t
    local x = omega * dt
    local e = 1 / (1 + x + 0.48 * x * x + 0.235 * x * x * x)
    local change = cur - target
    local temp = (v + omega * change) * dt
    v = (v - omega * temp) * e
    local out = target + (change + temp) * e
    if (target - cur > 0) == (out > target) then return target, 0 end
    return out, v
end

local function enabled(i)
    if i == 1 then return ScrollView.horizontal end
    return ScrollView.vertical
end

function ScrollView.get_normalized_position()
    local lo, hi = bounds()
    local x = lo[1] < 0 and pos[1] / lo[1] or 0
    local y = hi[2] > 0 and 1 - pos[2] / hi[2] or 1
    return math.max(0, math.min(1, x)), math.max(0, math.min(1, y))
end

-- Write the position, then keep the bars and the owner in step.
local function apply()
    RectTransform.SetAnchoredPosition(content, pos[1], pos[2])
    local x, y = ScrollView.get_normalized_position()
    local vw, vh, cw, ch = sizes()
    if hbar then
        hbar.set_size(cw > 0 and vw / cw or 1)
        hbar.set_value_without_notify(x)
    end
    if vbar then
        vbar.set_size(ch > 0 and vh / ch or 1)
        vbar.set_value_without_notify(y)
    end
    if x ~= last_norm[1] or y ~= last_norm[2] then
        last_norm = { x, y }
        if ScrollView.on_value_changed then ScrollView.on_value_changed(self_id, x, y) end
    end
end

function ScrollView.set_normalized_position(x, y)
    local lo, hi = bounds()
    if x and enabled(1) then pos[1] = x * lo[1] end
    if y and enabled(2) then pos[2] = (1 - y) * hi[2] end
    vel = { 0, 0 }
    apply()
end

function ScrollView.stop_movement()
    vel = { 0, 0 }
end

function ScrollView.Awake(id)
    self_id = id
    viewport = Scene.FindChild(id, "Viewport") or id
    content = Scene.FindChild(viewport, "Content")
    local h = Scene.FindChild(id, "Scrollbar Horizontal")
    local v = Scene.FindChild(id, "Scrollbar Vertical")
    if h and not ScrollView.horizontal then Scene.SetActive(h, false); h = nil end
    if v and not ScrollView.vertical then Scene.SetActive(v, false); v = nil end
    hbar = h and Scene.GetScript(h, "scrollbar")
    vbar = v and Scene.GetScript(v, "scrollbar")
    if content then pos[1], pos[2] = RectTransform.GetAnchoredPosition(content) end
end

function ScrollView.Start(id)
    if not content then return end
    if hbar then hbar.on_value_changed = function(_, x) ScrollView.set_normalized_position(x, nil) end end
    if vbar then vbar.on_value_changed = function(_, y) ScrollView.set_normalized_position(nil, y) end end
    apply()
end

function ScrollView.OnBeginDrag(id, event)
    if event.button ~= "Left" or not content then return end
    dragging = true
    vel = { 0, 0 }
    drag_start = { pos[1], pos[2] }
    drag_sum = { 0, 0 }
    prev = { pos[1], pos[2] }
end

function ScrollView.OnDrag(id, event)
    if not dragging then return end
    local r = UI.GetRect(id)
    local sf = r and r.scale_factor or 1
    drag_sum[1] = drag_sum[1] + event.delta.x / sf
    drag_sum[2] = drag_sum[2] + event.delta.y / sf
    local _, _, vw, vh = bounds()
    local view = { vw, vh }
    for i = 1, 2 do
        if enabled(i) then
            local raw = drag_start[i] + drag_sum[i]
            local mode = ScrollView.movement_type
            if mode == "Unrestricted" then
                pos[i] = raw
            else
                local b = clamp_axis(i, raw)
                pos[i] = mode == "Elastic" and b + rubber(raw - b, view[i]) or b
            end
        end
    end
    apply()
end

function ScrollView.OnEndDrag(id, event)
    dragging = false
end

function ScrollView.OnScroll(id, event)
    if not content then return end
    local d = event.delta.y * ScrollView.scroll_sensitivity
    if ScrollView.vertical then
        pos[2] = pos[2] - d
    elseif ScrollView.horizontal then
        pos[1] = pos[1] + d
    end
    vel = { 0, 0 }
    if ScrollView.movement_type ~= "Unrestricted" then
        pos[1], pos[2] = clamp_axis(1, pos[1]), clamp_axis(2, pos[2])
    end
    apply()
end

function ScrollView.Update(id)
    if not content then return end
    local dt = Time.unscaledDeltaTime()
    if dt <= 0 then return end
    if dragging then
        for i = 1, 2 do
            local v = (pos[i] - prev[i]) / dt
            vel[i] = vel[i] + (v - vel[i]) * math.min(1, dt * 10)
            prev[i] = pos[i]
        end
        return
    end
    local moved = false
    for i = 1, 2 do
        if enabled(i) then
            local b = clamp_axis(i, pos[i])
            local mode = ScrollView.movement_type
            if mode == "Elastic" and b ~= pos[i] then
                pos[i], vel[i] = smooth_damp(pos[i], b, vel[i], ScrollView.elasticity, dt)
                moved = true
            elseif ScrollView.inertia and vel[i] ~= 0 then
                vel[i] = vel[i] * ScrollView.deceleration_rate ^ dt
                if math.abs(vel[i]) < 1 then vel[i] = 0 end
                pos[i] = pos[i] + vel[i] * dt
                if mode == "Clamped" and clamp_axis(i, pos[i]) ~= pos[i] then
                    pos[i], vel[i] = clamp_axis(i, pos[i]), 0
                end
                moved = true
            else
                vel[i] = 0
            end
        end
    end
    if moved then apply() end
end

return ScrollView
