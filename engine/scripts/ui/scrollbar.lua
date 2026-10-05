-- scrollbar.lua — the engine's Scrollbar widget (#422), Unity's `Scrollbar`.
--
-- `Sliding Area/Handle` spans `size` (0..1) of the track at `value` (0..1). Drag
-- the handle to move it; press the track beside it to page one handle-length that
-- way. While focused, the arrows along its axis step it (a tenth, or one step with
-- `number_of_steps`); across it they move the focus. A scroll view drives its
-- bars through the same API.
--
-- Owner API (via `Scene.GetScript(id, "scrollbar")`):
--   value, size                                   -- read them
--   on_value_changed = function(id, value) … end
--   set_value(v) / set_value_without_notify(v)     -- clamped to 0..1 (and stepped)
--   set_size(s)                                    -- the handle's share of the track

local Scrollbar = {
    fields = {
        value = { type = "number", default = 0, range = { 0, 1 }, tooltip = "Position, 0..1" },
        size = { type = "number", default = 0.2, range = { 0, 1 }, tooltip = "Handle length, share of the track" },
        number_of_steps = { type = "number", default = 0, tooltip = "Distinct positions (0 = continuous)" },
        direction = { type = "text", default = "LeftToRight", tooltip = "LeftToRight, RightToLeft, BottomToTop or TopToBottom" },
    },
}

local self_id, area, handle
local vertical, reversed = false, false
local grab -- where on the handle the pointer holds it (0..1 of the track), or nil

local function quantize(v)
    v = math.max(0, math.min(1, v))
    local n = math.floor(Scrollbar.number_of_steps)
    if n > 1 then v = math.floor(v * (n - 1) + 0.5) / (n - 1) end
    return v
end

-- The handle's span along the track, 0..1, in track coordinates.
local function span()
    local s = math.max(0, math.min(1, Scrollbar.size))
    local v = reversed and 1 - Scrollbar.value or Scrollbar.value
    local lo = v * (1 - s)
    return lo, lo + s
end

local function place()
    if not handle then return end
    local lo, hi = span()
    if vertical then
        RectTransform.SetAnchorMin(handle, 0, lo)
        RectTransform.SetAnchorMax(handle, 1, hi)
    else
        RectTransform.SetAnchorMin(handle, lo, 0)
        RectTransform.SetAnchorMax(handle, hi, 1)
    end
    RectTransform.SetSizeDelta(handle, 0, 0)
    RectTransform.SetAnchoredPosition(handle, 0, 0)
end

local function set(v, notify)
    v = quantize(v)
    if v == Scrollbar.value then return end
    Scrollbar.value = v
    place()
    if notify and Scrollbar.on_value_changed then Scrollbar.on_value_changed(self_id, v) end
end

function Scrollbar.set_value(v) set(v, true) end
function Scrollbar.set_value_without_notify(v) set(v, false) end

function Scrollbar.set_size(s)
    Scrollbar.size = math.max(0, math.min(1, s))
    place()
end

-- The pointer as a 0..1 position along the track (canvas units, so it works on a
-- world canvas too).
local function track_t(event)
    local r = UI.GetRect(area or self_id)
    local p = event.canvas_position
    if not r or not p then return 0 end
    if vertical then
        return r.height > 0 and (p.y - r.y) / r.height or 0
    end
    return r.width > 0 and (p.x - r.x) / r.width or 0
end

-- Put the handle's start at track position `lo` (0..1).
local function set_start(lo)
    local room = 1 - math.max(0, math.min(1, Scrollbar.size))
    local v = room > 0 and lo / room or 0
    Scrollbar.set_value(reversed and 1 - v or v)
end

function Scrollbar.Awake(id)
    self_id = id
    area = Scene.FindChild(id, "Sliding Area")
    handle = area and Scene.FindChild(area, "Handle")
    local d = Scrollbar.direction
    vertical = d == "BottomToTop" or d == "TopToBottom"
    reversed = d == "RightToLeft" or d == "TopToBottom"
    Scrollbar.value = quantize(Scrollbar.value)
    place()
end

-- On the handle: hold it where pressed. Beside it: page toward the pointer.
function Scrollbar.OnPointerDown(id, event)
    if event.button ~= "Left" then return end
    local t = track_t(event)
    local lo, hi = span()
    if t >= lo and t <= hi then
        grab = t - lo
        return
    end
    grab = nil
    local page = math.max(0, math.min(1, Scrollbar.size))
    set_start(t < lo and lo - page or lo + page)
end

function Scrollbar.OnDrag(id, event)
    if event.button ~= "Left" then return end
    local s = math.max(0, math.min(1, Scrollbar.size))
    set_start(track_t(event) - (grab or s * 0.5))
end

function Scrollbar.OnEndDrag(id, event)
    grab = nil
end

function Scrollbar.OnMove(id, event)
    local along = vertical and event.y or event.x
    if along == 0 then
        local to = UI.FindSelectable(id, event.direction)
        if to then UI.SetSelected(to) end
        return
    end
    local n = math.floor(Scrollbar.number_of_steps)
    local step = n > 1 and 1 / (n - 1) or 0.1
    if reversed then along = -along end
    Scrollbar.set_value(Scrollbar.value + along * step)
end

return Scrollbar
