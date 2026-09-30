-- button.lua — the engine's Button widget (#422), Unity's `Button`.
--
-- A script component, not a first-class component: fork it (copy it under another
-- name) to change what a button does. The look — the colour per state — is the
-- entity's `Selectable` (ColorTint on its own Image); this script only raises the
-- click.
--
-- Owner API (via `Scene.GetScript(id, "button")`):
--   on_click = function(id) … end   -- a left click, or Enter while focused
--   press()                         -- click it from code (fires on_click)

local Button = {}

local self_id

function Button.Awake(id)
    self_id = id
end

-- Fire `on_click` the way a click would.
function Button.press()
    if Button.on_click and Selectable.IsInteractable(self_id) then
        Button.on_click(self_id)
    end
end

function Button.OnPointerClick(id, event)
    if event.button == "Left" then
        Button.press()
    end
end

function Button.OnSubmit(id)
    Button.press()
end

return Button
