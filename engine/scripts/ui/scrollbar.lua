-- Unity's `Scrollbar`. `Sliding Area/Handle` spans `size` (0..1) of the track at
-- `value` (0..1). Pressing the track beside the handle pages one handle-length.
-- Arrows along the axis step a tenth (or one step with `number_of_steps`).
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

local RangeAxis = require("ui.range_axis")

local self_id, area, handle
local vertical, reversed = false, false
local grab -- where on the handle the pointer holds it (0..1 of the track), or nil

local function quantize(v)
    v = math.max(0, math.min(1, v))
    local n = math.floor(Scrollbar.number_of_steps)
    if n > 1 then v = math.floor(v * (n - 1) + 0.5) / (n - 1) end
    return v
end

local function span()
    local s = math.max(0, math.min(1, Scrollbar.size))
    local v = reversed and 1 - Scrollbar.value or Scrollbar.value
    local lo = v * (1 - s)
    return lo, lo + s
end

local function place()
    if not handle then return end
    RangeAxis.span(handle, vertical, span())
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

local function set_start(lo)
    local room = 1 - math.max(0, math.min(1, Scrollbar.size))
    local v = room > 0 and lo / room or 0
    Scrollbar.set_value(reversed and 1 - v or v)
end

function Scrollbar.Awake(id)
    self_id = id
    area = Scene.FindChild(id, "Sliding Area")
    handle = area and Scene.FindChild(area, "Handle")
    vertical, reversed = RangeAxis.parse(Scrollbar.direction)
    Scrollbar.value = quantize(Scrollbar.value)
    place()
end

local function track_t(event)
    return RangeAxis.track(area or self_id, event, vertical)
end

function Scrollbar.OnPointerDown(id, event)
    if event.button ~= "Left" then return end
    local t = track_t(event)
    if not t then return end
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
    local t = track_t(event)
    if not t then return end
    local s = math.max(0, math.min(1, Scrollbar.size))
    set_start(t - (grab or s * 0.5))
end

function Scrollbar.OnEndDrag(id, event)
    grab = nil
end

function Scrollbar.OnMove(id, event)
    local along = RangeAxis.move(id, event, vertical, reversed)
    if along == 0 then return end
    local n = math.floor(Scrollbar.number_of_steps)
    local step = n > 1 and 1 / (n - 1) or 0.1
    Scrollbar.set_value(Scrollbar.value + along * step)
end

return Scrollbar
