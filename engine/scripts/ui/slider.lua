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
        if vertical then
            RectTransform.SetAnchorMin(fill, 0, lo)
            RectTransform.SetAnchorMax(fill, 1, hi)
        else
            RectTransform.SetAnchorMin(fill, lo, 0)
            RectTransform.SetAnchorMax(fill, hi, 1)
        end
        RectTransform.SetSizeDelta(fill, 0, 0)
    end
    if handle then
        local w, h = RectTransform.GetSizeDelta(handle)
        local thick = math.max(w, h)
        if vertical then
            RectTransform.SetAnchorMin(handle, 0, n)
            RectTransform.SetAnchorMax(handle, 1, n)
            RectTransform.SetSizeDelta(handle, 0, thick)
        else
            RectTransform.SetAnchorMin(handle, n, 0)
            RectTransform.SetAnchorMax(handle, n, 1)
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

-- Set the value from the pointer on the canvas (reference units, so it works on
-- a world canvas too).
local function from_pointer(event)
    local r = UI.GetRect(area or self_id)
    local p = event.canvas_position
    if not r or not p then return end
    local t
    if vertical then
        t = r.height > 0 and (p.y - r.y) / r.height or 0
    else
        t = r.width > 0 and (p.x - r.x) / r.width or 0
    end
    t = math.max(0, math.min(1, t))
    if reversed then t = 1 - t end
    Slider.set_value(Slider.min + t * (Slider.max - Slider.min))
end

function Slider.Awake(id)
    self_id = id
    fill = Scene.FindChild(id, "Fill Area/Fill")
    area = Scene.FindChild(id, "Handle Slide Area")
    handle = area and Scene.FindChild(area, "Handle")
    local d = Slider.direction
    vertical = d == "BottomToTop" or d == "TopToBottom"
    reversed = d == "RightToLeft" or d == "TopToBottom"
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
    local along = vertical and event.y or event.x
    if along == 0 then
        local to = UI.FindSelectable(id, event.direction)
        if to then UI.SetSelected(to) end
        return
    end
    local step = Slider.whole_numbers and 1 or (Slider.max - Slider.min) * 0.1
    if reversed then along = -along end
    Slider.set_value(Slider.value + along * step)
end

return Slider
