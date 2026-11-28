-- A shooter's HUD built by script: health, ammo, a crosshair opening with the spread,
-- the minimap's image, an FPS text rewritten twice a second, floating damage numbers
-- and a hit flash. hud_cameras.lua beside it draws the gun and the minimap. Fed through
-- Scene.GetScript(id, "hud"); a setter stores, LateUpdate writes what changed.

local Hud = {}

local DAMAGE_NUMBERS = 8
local NUMBER_LIFE = 0.8 -- s
local NUMBER_RISE = 1.2 -- m over a life
local FLASH_FADE = 2.5  -- alpha a second
local FPS_EVERY = 0.5   -- s
local CROSS_GAP = 10    -- reference units
local CROSS_OPEN = 1500 -- reference units per radian of spread

-- Before Start, so an owner may feed it first.
Hud.health, Hud.max_health, Hud.mag, Hud.reserve = 100, 100, 0, 0
Hud.spread, Hud.alpha, Hud.next_number = 0, 0, 1
Hud.dirty = { health = true, ammo = true, spread = true }

local function rect(id, ax, ay, x, y, w, h)
    RectTransform.SetAnchorMin(id, ax, ay)
    RectTransform.SetAnchorMax(id, ax, ay)
    RectTransform.SetPivot(id, ax, ay)
    RectTransform.SetAnchoredPosition(id, x, y)
    RectTransform.SetSizeDelta(id, w, h)
end

local function image(parent, name, ax, ay, x, y, w, h, r, g, b, a)
    local id = UI.Create("Image", parent)
    Scene.SetName(id, name)
    rect(id, ax, ay, x, y, w, h)
    Image.SetColor(id, r, g, b, a)
    Image.SetRaycastTarget(id, false)
    return id
end

local function text(parent, name, ax, ay, x, y, w, h, size, align)
    local id = UI.Create("Text", parent)
    Scene.SetName(id, name)
    rect(id, ax, ay, x, y, w, h)
    Text.SetFontSize(id, size)
    Text.SetAlignment(id, align)
    Text.SetColor(id, 0.95, 0.95, 0.9, 1)
    Text.SetOutline(id, 0.12, 0, 0, 0, 0.9)
    Text.SetRaycastTarget(id, false)
    return id
end

local function minimap(canvas)
    image(canvas, "Minimap_Frame", 1, 1, -26, -26, 268, 268, 0, 0, 0, 0.6)
    local map = image(canvas, "Minimap", 1, 1, -32, -32, 256, 256, 1, 1, 1, 1)
    Image.SetTexture(map, "rt:minimap")
    image(map, "Minimap_Player", 0.5, 0.5, 0, 0, 10, 10, 0.2, 1, 0.3, 1)
end

local function crosshair(self, canvas)
    self.cross = {
        { image(canvas, "Cross_Up", 0.5, 0.5, 0, 0, 3, 14, 1, 1, 1, 0.9), 0, 1 },
        { image(canvas, "Cross_Down", 0.5, 0.5, 0, 0, 3, 14, 1, 1, 1, 0.9), 0, -1 },
        { image(canvas, "Cross_Left", 0.5, 0.5, 0, 0, 14, 3, 1, 1, 1, 0.9), -1, 0 },
        { image(canvas, "Cross_Right", 0.5, 0.5, 0, 0, 14, 3, 1, 1, 1, 0.9), 1, 0 },
    }
end

function Hud.Start(id)
    local self = Hud
    self.id = id
    local canvas = UI.Create("Canvas")
    Scene.SetName(canvas, "HUD_Canvas")
    Canvas.SetSortOrder(canvas, 10)
    self.flash = image(canvas, "Hit_Flash", 0, 0, 0, 0, 0, 0, 0.8, 0, 0, 0)
    RectTransform.SetAnchorMax(self.flash, 1, 1)
    image(canvas, "Health_Back", 0, 0, 40, 40, 360, 28, 0, 0, 0, 0.55)
    self.health_bar = image(canvas, "Health_Bar", 0, 0, 44, 44, 352, 20, 0.85, 0.2, 0.15, 1)
    Image.SetType(self.health_bar, "Filled")
    Image.SetFillMethod(self.health_bar, "Horizontal")
    self.health_text = text(canvas, "Health", 0, 0, 40, 74, 200, 56, 44, "BottomLeft")
    self.ammo_text = text(canvas, "Ammo", 1, 0, -40, 40, 320, 64, 52, "BottomRight")
    self.fps_text = text(canvas, "FPS", 0, 1, 24, -16, 300, 36, 26, "TopLeft")
    self.numbers = {}
    for i = 1, DAMAGE_NUMBERS do
        local n = text(canvas, "Damage_" .. i, 0.5, 0.5, 0, 0, 160, 48, 34, "MiddleCenter")
        Text.SetText(n, "")
        self.numbers[i] = { id = n, life = 0 }
    end
    crosshair(self, canvas)
    minimap(canvas)
    self.fps_clock, self.fps_ticks = 0, 0
end

function Hud.SetHealth(value, max)
    Hud.health, Hud.max_health = value, math.max(1, max or Hud.max_health)
    Hud.dirty.health = true
end

function Hud.SetAmmo(mag, reserve)
    Hud.mag, Hud.reserve = mag, reserve
    Hud.dirty.ammo = true
end

function Hud.SetSpread(radians)
    Hud.spread = radians
    Hud.dirty.spread = true
end

function Hud.Fired()
    local cams = Hud.id and Scene.GetScript(Hud.id, "hud_cameras")
    if cams then cams.Fired() end
end

function Hud.Hurt(amount)
    Hud.alpha = math.min(0.6, Hud.alpha + amount / 100)
end

function Hud.Hit(x, y, z, amount)
    if Hud.numbers == nil then return end
    local n = Hud.numbers[Hud.next_number]
    Hud.next_number = Hud.next_number % DAMAGE_NUMBERS + 1
    n.x, n.y, n.z, n.life = x, y, z, NUMBER_LIFE
    Text.SetText(n.id, string.format("%d", math.floor(amount + 0.5)))
    RectTransform.SetWorldAnchor(n.id, nil, x, y, z)
end

local function numbers(self, dt)
    for _, n in ipairs(self.numbers) do
        if n.life > 0 then
            n.life = math.max(0, n.life - dt)
            local t = 1 - n.life / NUMBER_LIFE
            RectTransform.SetWorldAnchor(n.id, nil, n.x, n.y + t * NUMBER_RISE, n.z)
            local a = n.life / NUMBER_LIFE
            Text.SetColor(n.id, 1, 0.85, 0.2, a)
            Text.SetOutline(n.id, 0.12, 0, 0, 0, 0.9 * a)
            if n.life == 0 then Text.SetText(n.id, "") end
        end
    end
end

local function changed(self)
    local d = self.dirty
    if d.health then
        Image.SetFillAmount(self.health_bar, math.max(0, self.health) / self.max_health)
        Text.SetText(self.health_text, string.format("%d", math.max(0, math.floor(self.health + 0.5))))
    end
    if d.ammo then
        Text.SetText(self.ammo_text, string.format("%d / %d", self.mag, self.reserve))
    end
    if d.spread then
        local gap = CROSS_GAP + self.spread * CROSS_OPEN
        for _, arm in ipairs(self.cross) do
            RectTransform.SetAnchoredPosition(arm[1], arm[2] * gap, arm[3] * gap)
        end
    end
    d.health, d.ammo, d.spread = false, false, false
end

function Hud.LateUpdate(id, dt)
    local self = Hud
    changed(self)
    numbers(self, dt)
    if self.alpha > 0 then
        self.alpha = math.max(0, self.alpha - FLASH_FADE * dt)
        Image.SetColor(self.flash, 0.8, 0, 0, self.alpha)
    end
    self.fps_ticks = self.fps_ticks + 1
    self.fps_clock = self.fps_clock + Time.unscaledDeltaTime()
    if self.fps_clock >= FPS_EVERY then
        local fps = self.fps_ticks / self.fps_clock
        Text.SetText(self.fps_text, string.format("%.0f FPS  tick %d", fps, Time.frameCount()))
        self.fps_clock, self.fps_ticks = 0, 0
    end
end

return Hud
