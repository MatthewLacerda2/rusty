-- range_axis.lua — the axis a Slider or a Scrollbar runs along (#996).
--
-- A helper module, not a widget: it has no callbacks and is never attached to an
-- entity. Both widgets `require("ui.range_axis")`, so the maths they share — the
-- direction, where the pointer is along the track, anchoring a part over a span
-- of it, and arrow keys along it versus across it — exists once.

local RangeAxis = {}

-- `direction` (LeftToRight, RightToLeft, BottomToTop or TopToBottom) as two flags.
function RangeAxis.parse(direction)
    local vertical = direction == "BottomToTop" or direction == "TopToBottom"
    local reversed = direction == "RightToLeft" or direction == "TopToBottom"
    return vertical, reversed
end

-- Where `event`'s pointer is along entity `id`'s rect, 0..1 from its start edge
-- (left or bottom) and clamped to it; nil when the rect or the pointer is
-- missing. Canvas units, so it works on a world canvas too.
function RangeAxis.track(id, event, vertical)
    local r = UI.GetRect(id)
    local p = event.canvas_position
    if not r or not p then return nil end
    local t
    if vertical then
        t = r.height > 0 and (p.y - r.y) / r.height or 0
    else
        t = r.width > 0 and (p.x - r.x) / r.width or 0
    end
    return math.max(0, math.min(1, t))
end

-- Anchor `id` over `lo..hi` of the track along the axis, filling it across.
function RangeAxis.span(id, vertical, lo, hi)
    if vertical then
        RectTransform.SetAnchorMin(id, 0, lo)
        RectTransform.SetAnchorMax(id, 1, hi)
    else
        RectTransform.SetAnchorMin(id, lo, 0)
        RectTransform.SetAnchorMax(id, hi, 1)
    end
end

-- An `OnMove` on entity `id`: the arrows across the axis move the focus (and
-- this returns 0); along it, this returns the steps to take toward the value's
-- end, -1 or 1.
function RangeAxis.move(id, event, vertical, reversed)
    local along = vertical and event.y or event.x
    if along == 0 then
        local to = UI.FindSelectable(id, event.direction)
        if to then UI.SetSelected(to) end
        return 0
    end
    return reversed and -along or along
end

return RangeAxis
