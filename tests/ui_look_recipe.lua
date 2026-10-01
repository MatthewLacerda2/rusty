-- A Cyberpunk panel: dark glass with cut corners, a neon edge and glow, a
-- gradient header strip and an additive dashed scan line. `hud` is a canvas.
local function element(name, parent, x, y, w, h)
  local id = Scene.CreateEntity(name)
  Scene.SetParent(id, parent)
  Scene.AddComponent(id, "Shape") -- also adds a RectTransform
  RectTransform.SetAnchorMin(id, 0, 0)
  RectTransform.SetAnchorMax(id, 0, 0)
  RectTransform.SetPivot(id, 0, 0)
  RectTransform.SetAnchoredPosition(id, x, y)
  RectTransform.SetSizeDelta(id, w, h)
  return id
end

local panel = element("Panel", hud, 64, 64, 480, 220)
Shape.SetCorner(panel, "Chamfer")
Shape.SetRadius(panel, 24, 0, 24, 0)            -- cut top-left and bottom-right
Shape.SetColor(panel, 0.02, 0.05, 0.08, 0.85)   -- dark glass
Shape.SetBorder(panel, 2, 0.0, 0.95, 1.0, 1.0)  -- neon cyan edge
Shape.SetGlow(panel, 14, 0.8, 0.0, 0.95, 1.0, 1.0)
Shape.SetShadow(panel, 6, -6, 8, 0, 0, 0, 0.6)

local header = element("Header", panel, 24, 180, 432, 24)
Shape.SetCorner(header, "Chamfer")
Shape.SetRadius(header, 0, 12, 0, 0)
Shape.SetGradient(header, { angle = 0, stops = {
  { t = 0, color = { 1.0, 0.1, 0.4, 0.9 } },
  { t = 1, color = { 1.0, 0.1, 0.4, 0.0 } },
} })

local scan = element("ScanLine", panel, 24, 160, 432, 4)
Shape.SetKind(scan, "Line")
Shape.SetThickness(scan, 2)
Shape.SetDash(scan, 12, 6)
Shape.SetColor(scan, 0.0, 0.95, 1.0, 0.6)
Shape.SetBlend(scan, "Additive")
