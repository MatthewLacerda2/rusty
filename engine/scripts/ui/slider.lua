-- slider.lua — the engine's Slider widget (#422), Unity's `Slider`.
--
-- A value between `min` and `max`: `Fill Area/Fill` stretches from the start to
-- it and `Handle Slide Area/Handle` sits on it. Press or drag anywhere on the
-- slider to set it; while focused, the arrows along its axis step it (a tenth of
-- the range, or 1 with `whole_numbers`) and the arrows across it move the focus.
-- `direction` is LeftToRight, RightToLeft, BottomToTop or TopToBottom.
--
-- Owner API (via `Scene.GetScript(id, "slider")`):
--   value, min, max                            -- read them
--   on_value_changed = function(id, value) … end
--   set_value(v) / set_value_without_notify(v)  -- clamped (and rounded when whole)
--   normalized()                                -- the value as 0..1 of the range

local Slider = {
    fields = {
        min = { type = "number", default = 0, tooltip = "The lowest value" },
        max = { type = "number", default = 1, tooltip = "The highest value" },
        value = { type = "number", default = 0, tooltip = "The starting value" },
        whole_numbers = { type = "boolean", default = false, tooltip = "Round to integers" },
        direction = { type = "text", default = "LeftToRight", tooltip = "LeftToRight, RightToLeft, BottomToTop or TopToBottom" },
    },
}

local RangeAxis = require("ui.range_axis")

local self_id, fill, area, handle
local vertical, reversed = false, false

local function clamp(v)
    local lo, hi = math.min(Slider.min, Slider.max), math.max(Slider.min, Slider.max)
    v = math.max(lo, math.min(hi, v))
    if Slider.whole_numbers then v = math.floor(v + 0.5) end
    return v
end

function Slider.normalized()
    if Slider.max == Slider.min then return 0 end
    return (Slider.value - Slider.min) / (Slider.max - Slider.min)
end

-- Anchor the fill and the handle at the value, along the slider's axis.
local function place()
    local n = Slider.normalized()
    if reversed then n = 1 - n end
    local lo, hi = 0, n
    if reversed then lo, hi = n, 1 end
    if fill then
        RangeAxis.span(fill, vertical, lo, hi)
        RectTransform.SetSizeDelta(fill, 0, 0)
    end
    if handle then
        local w, h = RectTransform.GetSizeDelta(handle)
        local thick = math.max(w, h)
        RangeAxis.span(handle, vertical, n, n)
        if vertical then
            RectTransform.SetSizeDelta(handle, 0, thick)
        else
            RectTransform.SetSizeDelta(handle, thick, 0)
        end
        RectTransform.SetAnchoredPosition(handle, 0, 0)
    end
end

local function set(v, notify)
    v = clamp(v)
    if v == Slider.value then return end
    Slider.value = v
    place()
    if notify and Slider.on_value_changed then Slider.on_value_changed(self_id, v) end
end

function Slider.set_value(v) set(v, true) end
function Slider.set_value_without_notify(v) set(v, false) end

-- Set the value from the pointer along the slide area.
local function from_pointer(event)
    local t = RangeAxis.track(area or self_id, event, vertical)
    if not t then return end
    if reversed then t = 1 - t end
    Slider.set_value(Slider.min + t * (Slider.max - Slider.min))
end

function Slider.Awake(id)
    self_id = id
    fill = Scene.FindChild(id, "Fill Area/Fill")
    area = Scene.FindChild(id, "Handle Slide Area")
    handle = area and Scene.FindChild(area, "Handle")
    vertical, reversed = RangeAxis.parse(Slider.direction)
    Slider.value = clamp(Slider.value)
    place()
end

function Slider.OnPointerDown(id, event)
    if event.button == "Left" then from_pointer(event) end
end

function Slider.OnDrag(id, event)
    if event.button == "Left" then from_pointer(event) end
end

-- Arrows along the axis step the value; across it they navigate.
function Slider.OnMove(id, event)
    local along = RangeAxis.move(id, event, vertical, reversed)
    if along == 0 then return end
    local step = Slider.whole_numbers and 1 or (Slider.max - Slider.min) * 0.1
    Slider.set_value(Slider.value + along * step)
end

return Slider
