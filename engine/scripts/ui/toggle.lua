-- toggle.lua — the engine's Toggle widget (#422), Unity's `Toggle`.
--
-- A checkbox: `Background/Checkmark` fades in (on) and out (off) with a `Tween` on
-- unscaled time, so it animates under a paused game's menu. Under an ancestor
-- carrying `toggle_group.lua` it is a radio button: turning it on turns the
-- group's others off.
--
-- Owner API (via `Scene.GetScript(id, "toggle")`):
--   is_on                                   -- the state (read it; set it with set_is_on)
--   on_value_changed = function(id, on) … end
--   set_is_on(on)                           -- change it, firing on_value_changed
--   set_is_on_without_notify(on)            -- change it silently

local Toggle = {
    fields = {
        is_on = { type = "boolean", default = true, tooltip = "Whether the toggle starts on" },
        fade = { type = "number", default = 0.1, range = { 0, 1 }, tooltip = "Checkmark fade, unscaled seconds" },
    },
}

local self_id, check, group

-- Show the checkmark for the current state, fading unless `instant`.
local function show(instant)
    if not check then return end
    local r, g, b = Image.GetColor(check)
    local a = Toggle.is_on and 1 or 0
    Tween.KillAll(check)
    local faded = not instant and Toggle.fade > 0
        and pcall(Tween.To, check, "Image.color", { r, g, b, a }, Toggle.fade, { unscaled = true })
    if not faded then
        Image.SetColor(check, r, g, b, a)
    end
end

local function set(on, notify)
    if Toggle.is_on == on then return end
    if not on and group and group.is_locked(Toggle) then return end
    Toggle.is_on = on
    show(false)
    if on and group then group.notify_on(Toggle) end
    if notify and Toggle.on_value_changed then Toggle.on_value_changed(self_id, on) end
end

function Toggle.set_is_on(on) set(on, true) end
function Toggle.set_is_on_without_notify(on) set(on, false) end

-- The nearest ancestor's toggle group, if any.
local function find_group(id)
    local at = Scene.GetParent(id)
    while at do
        local g = Scene.GetScript(at, "toggle_group")
        if g then return g end
        at = Scene.GetParent(at)
    end
end

function Toggle.Awake(id)
    self_id = id
    check = Scene.FindChild(id, "Background/Checkmark")
    group = find_group(id)
    if group then group.register(Toggle) end
    show(true)
end

function Toggle.OnPointerClick(id, event)
    if event.button == "Left" then Toggle.set_is_on(not Toggle.is_on) end
end

function Toggle.OnSubmit(id)
    Toggle.set_is_on(not Toggle.is_on)
end

return Toggle
