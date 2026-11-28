-- The pause menu: Escape opens and closes it; open, the sim stops and the
-- cursor is free. A frosted, rounded window holds a column of labelled rows,
-- one per setting. Built in Start, wired the tick after, once the widgets'
-- own scripts have loaded.

local Menu = {}

local FADE = 6 -- alpha a second, unscaled
local ROW = 48 -- reference units
local BINDINGS = {
    { "Move", "W A S D" }, { "Look", "Mouse" }, { "Fire", "Left mouse" },
    { "Reload", "R" }, { "Smoke grenade", "G" }, { "Jump", "Space" },
    { "This menu", "Escape" },
}

local function node(name, parent)
    local id = Scene.CreateEntity(name)
    Scene.SetParent(id, parent)
    Scene.AddComponent(id, "RectTransform")
    return id
end

local function stretch(id)
    RectTransform.SetAnchorMin(id, 0, 0)
    RectTransform.SetAnchorMax(id, 1, 1)
    RectTransform.SetSizeDelta(id, 0, 0)
end

local function text(parent, s, size, align)
    local id = UI.Create("Text", parent)
    Text.SetText(id, s)
    Text.SetFontSize(id, size)
    Text.SetAlignment(id, align)
    Text.SetColor(id, 0.92, 0.94, 0.96, 1)
    Text.SetRaycastTarget(id, false)
    return id
end

local function row(column, label, kind, height)
    local id = node(label, column)
    Scene.AddComponent(id, "LayoutGroup")
    LayoutGroup.SetKind(id, "Horizontal")
    LayoutGroup.SetSpacing(id, 20, 0)
    LayoutGroup.SetControlChildSize(id, true, true)
    LayoutGroup.SetChildForceExpand(id, false, true)
    Scene.AddComponent(id, "LayoutElement")
    LayoutElement.SetPreferredSize(id, nil, height or ROW)
    local name = text(id, label, 28, "MiddleLeft")
    Scene.AddComponent(name, "LayoutElement")
    LayoutElement.SetPreferredSize(name, 280, nil)
    local widget = UI.Create(kind, id)
    Scene.AddComponent(widget, "LayoutElement")
    LayoutElement.SetFlexibleSize(widget, 1, nil)
    return widget
end

local function window(root)
    local id = node("Window", root)
    RectTransform.SetSizeDelta(id, 900, 900)
    Scene.AddComponent(id, "Shape")
    Shape.SetKind(id, "Rect")
    Shape.SetCorner(id, "Round")
    Shape.SetRadius(id, 28, 28, 28, 28)
    Shape.SetColor(id, 0.08, 0.1, 0.14, 0.55)
    Shape.SetBorder(id, 2, 1, 1, 1, 0.15)
    Scene.AddComponent(id, "BackdropFilter")
    BackdropFilter.SetBlurRadius(id, 24)
    BackdropFilter.SetTint(id, 0, 0, 0, 0.25)
    BackdropFilter.SetSaturation(id, 0.6)
    Scene.AddComponent(id, "Mask")
    return id
end

local function build(self)
    local canvas = UI.Create("Canvas")
    Scene.SetName(canvas, "Menu_Canvas")
    Canvas.SetSortOrder(canvas, 20)
    local root = node("Menu_Root", canvas)
    stretch(root)
    Scene.AddComponent(root, "CanvasGroup")
    local dim = UI.Create("Image", root)
    stretch(dim)
    Image.SetColor(dim, 0, 0, 0, 0.35)
    local column = node("Settings", window(root))
    stretch(column)
    Scene.AddComponent(column, "LayoutGroup")
    LayoutGroup.SetKind(column, "Vertical")
    LayoutGroup.SetPadding(column, 40, 32, 40, 32)
    LayoutGroup.SetSpacing(column, 0, 14)
    LayoutGroup.SetControlChildSize(column, true, true)
    LayoutGroup.SetChildForceExpand(column, true, false)
    self.title = text(column, "Paused", 44, "MiddleCenter")
    Scene.AddComponent(self.title, "LayoutElement")
    LayoutElement.SetPreferredSize(self.title, nil, 60)
    self.parts = {
        sensitivity = row(column, "Look sensitivity", "Slider"),
        volume = row(column, "Volume", "Slider"),
        quality = row(column, "Quality", "Dropdown"),
        invert = row(column, "Invert look", "Toggle"),
        crosshair = row(column, "Crosshair", "Toggle Group", 100),
        name = row(column, "Player name", "Input Field", 56),
        bindings = row(column, "Key bindings", "Scroll View", 200),
    }
    local content = Scene.FindChild(self.parts.bindings, "Viewport/Content")
    for _, b in ipairs(BINDINGS) do
        text(content, b[1] .. ":  " .. b[2], 26, "MiddleLeft")
    end
    LayoutGroup.SetPadding(content, 12, 6, 12, 6)
    Scene.SetActive(Scene.FindChild(self.parts.bindings, "Scrollbar Horizontal"), false)
    RectMask.SetFeather(Scene.FindChild(self.parts.bindings, "Viewport"), 12)
    local buttons = node("Buttons", column)
    Scene.AddComponent(buttons, "LayoutGroup")
    LayoutGroup.SetKind(buttons, "Horizontal")
    LayoutGroup.SetSpacing(buttons, 24, 0)
    LayoutGroup.SetChildAlignment(buttons, "MiddleCenter")
    LayoutGroup.SetControlChildSize(buttons, false, false)
    Scene.AddComponent(buttons, "LayoutElement")
    LayoutElement.SetPreferredSize(buttons, nil, 64)
    self.parts.resume = UI.Create("Button", buttons)
    self.parts.quit = UI.Create("Button", buttons)
    self.root, self.canvas = root, canvas
end

local function label(id, path, s)
    local t = Scene.FindChild(id, path)
    Text.SetText(t, s)
    return t
end

local function shooter()
    return Scene.GetScript(Scene.FindEntityByName("Player"), "shooter")
end

local function hud() return Scene.GetScript(Scene.FindEntityByName("HUD"), "hud") end

local function quality_names()
    local names = {}
    for _, preset in ipairs(Graphics.QualityPresets()) do names[#names + 1] = preset.name end
    return names
end

local function wire(self)
    local p = self.parts
    local look = Scene.GetScript(p.sensitivity, "slider")
    look.min, look.max = 0.2, 3
    look.set_value_without_notify(1)
    look.on_value_changed = function(_, v) local s = shooter(); if s then s.look_scale = v end end
    local volume = Scene.GetScript(p.volume, "slider")
    volume.set_value_without_notify(Audio.GetMasterVolume())
    volume.on_value_changed = function(_, v) Audio.SetMasterVolume(v) end
    local quality, names = Scene.GetScript(p.quality, "dropdown"), quality_names()
    if #names == 0 then names = { "Default" } end
    quality.set_options(names)
    for i, n in ipairs(names) do
        if n == Graphics.GetQuality() then quality.set_value_without_notify(i - 1) end
    end
    quality.on_value_changed = function(_, _, name)
        if Graphics.GetQuality() ~= nil then Graphics.SetQuality(name) end
    end
    label(p.invert, "Label", "")
    Scene.GetScript(p.invert, "toggle").on_value_changed = function(_, on)
        local s = shooter(); if s then s.invert_look = on end
    end
    for i, name in ipairs({ "Dynamic", "Static" }) do
        local option = Scene.FindChild(p.crosshair, "Option " .. i)
        Text.SetColor(label(option, "Label", name), 0.92, 0.94, 0.96, 1)
        Scene.GetScript(option, "toggle").on_value_changed = function(_, on)
            local h = hud()
            if h and on then h.static_cross, h.dirty.spread = i == 2, true end
        end
    end
    Scene.GetScript(Scene.FindChild(p.crosshair, "Option 1"), "toggle").set_is_on_without_notify(true)
    label(p.name, "Text Area/Placeholder", "Your name")
    local field = Scene.GetScript(p.name, "input_field")
    field.char_limit = 16
    field.on_end_edit = function(_, t) Text.SetText(self.title, t == "" and "Paused" or "Paused: " .. t) end
    Scene.GetScript(p.bindings, "scroll_view").horizontal = false
    self.bar = Scene.GetScript(Scene.FindChild(p.bindings, "Scrollbar Vertical"), "scrollbar")
    label(p.resume, "Label", "Resume")
    label(p.quit, "Label", "Quit")
    Selectable.SetColor(p.resume, "Highlighted", 0.75, 0.9, 1, 1)
    Scene.GetScript(p.resume, "button").on_click = function() Menu.close() end
    Scene.GetScript(p.quit, "button").on_click = function() Application.Quit() end
    self.wired = true
end

local function show(open)
    local s = shooter()
    if s then s.paused = open end
    Time.SetTimeScale(open and 0 or 1)
    Input.SetCursorLocked(not open)
    Input.SetCursorVisible(open)
    CanvasGroup.SetInteractable(Menu.root, open)
    CanvasGroup.SetBlocksRaycasts(Menu.root, open)
    Menu.open = open
end

function Menu.open_menu()
    Scene.SetActive(Menu.root, true)
    show(true)
    if Menu.bar then Menu.bar.set_value(1) end -- the bindings list starts at its top
    UI.SetSelected(Menu.parts.resume)
end

function Menu.close()
    show(false)
    UI.SetSelected(nil)
end

function Menu.Start(id)
    Menu.open, Menu.alpha, Menu.wired = false, 0, false
    build(Menu)
    CanvasGroup.SetAlpha(Menu.root, 0)
    CanvasGroup.SetInteractable(Menu.root, false)
    CanvasGroup.SetBlocksRaycasts(Menu.root, false)
end

function Menu.Update(id)
    if not Menu.wired then
        wire(Menu)
        Scene.SetActive(Menu.root, false)
        return
    end
    if Input.GetKeyDown("ESCAPE") then
        if Menu.open then Menu.close() else Menu.open_menu() end
    end
    local target = Menu.open and 1 or 0
    if Menu.alpha ~= target then
        local step = FADE * Time.unscaledDeltaTime()
        Menu.alpha = target > Menu.alpha and math.min(1, Menu.alpha + step)
            or math.max(0, Menu.alpha - step)
        CanvasGroup.SetAlpha(Menu.root, Menu.alpha)
        if Menu.alpha == 0 then Scene.SetActive(Menu.root, false) end
    end
end

return Menu
