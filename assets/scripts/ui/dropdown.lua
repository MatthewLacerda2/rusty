-- dropdown.lua — the engine's Dropdown widget (#422), Unity's `Dropdown`.
--
-- Shows the chosen option on its `Label`. Click it (or Enter while focused) to
-- open the list: the inactive `Template` (a scroll view) is lifted onto a popup
-- canvas above every other canvas, behind which a transparent `Blocker` covers the
-- screen — a click there closes the list and reaches nothing underneath, like
-- Unity's. The list holds one `Item` per option (a Selectable, so it highlights);
-- click one, or pick it with the arrows and Enter. Escape closes it.
--
-- `options` is the list, `|`-separated in the inspector; `value` is 0-based.
--
-- Owner API (via `Scene.GetScript(id, "dropdown")`):
--   value                                        -- 0-based index of the choice
--   on_value_changed = function(id, value, text) … end
--   set_value(i) / set_value_without_notify(i)
--   get_options() / set_options({ "Low", "High" })
--   show() / hide() / is_expanded()

local Dropdown = {
    fields = {
        options = { type = "text", default = "Option A|Option B|Option C", tooltip = "Choices, separated by |" },
        value = { type = "number", default = 0, tooltip = "The chosen option, 0-based" },
        item_height = { type = "number", default = 50, tooltip = "Each item's height in the list" },
        font_size = { type = "number", default = 28, tooltip = "The list items' font size" },
    },
}

local POPUP_ORDER = 30000
local DRAG_SLOP = 10
local self_id, label, template
local list = {}                   -- the options
local items = {}                  -- item entity per option, built on open
local stale = true                -- items no longer match the options
local popup, blocker, home        -- open state: popup canvas, blocker, template's own placement
local opened_tick, press_at

local function split(s)
    local out = {}
    for part in string.gmatch(s, "([^|]+)") do out[#out + 1] = part end
    return out
end

local function refresh_label()
    if label then Text.SetText(label, list[Dropdown.value + 1] or "") end
end

local function set(i, notify)
    i = math.max(0, math.min(#list - 1, math.floor(i)))
    if i == Dropdown.value then return end
    Dropdown.value = i
    refresh_label()
    if notify and Dropdown.on_value_changed then
        Dropdown.on_value_changed(self_id, i, list[i + 1])
    end
end

function Dropdown.set_value(i) set(i, true) end
function Dropdown.set_value_without_notify(i) set(i, false) end
function Dropdown.get_options() return { table.unpack(list) } end
function Dropdown.is_expanded() return popup ~= nil end

function Dropdown.set_options(opts)
    list = { table.unpack(opts) }
    stale = true
    Dropdown.value = math.max(0, math.min(#list - 1, Dropdown.value))
    refresh_label()
end

-- A child of `parent` stretched over it, inset `l, r` from the sides.
local function child(parent, name, l, r)
    local e = Scene.CreateEntity(name)
    Scene.AddComponent(e, "RectTransform")
    Scene.SetParent(e, parent)
    RectTransform.SetAnchorMin(e, 0, 0)
    RectTransform.SetAnchorMax(e, 1, 1)
    RectTransform.SetSizeDelta(e, -(l + r), 0)
    RectTransform.SetAnchoredPosition(e, (l - r) / 2, 0)
    return e
end

-- One Item per option under the template's Content, navigable top to bottom.
local function build_items()
    for _, e in ipairs(items) do
        Scene.SetActive(e, false)
        Scene.DestroyEntity(e)
    end
    items = {}
    local content = Scene.FindChild(template, "Viewport/Content")
    for i, text in ipairs(list) do
        local e = Scene.CreateEntity("Item " .. (i - 1) .. ": " .. text)
        Scene.AddComponent(e, "Image")
        Scene.AddComponent(e, "Selectable")
        Scene.AddComponent(e, "LayoutElement")
        LayoutElement.SetPreferredSize(e, nil, Dropdown.item_height)
        Scene.SetParent(e, content)
        local mark = child(e, "Item Checkmark", 0, 0)
        RectTransform.SetAnchorMax(mark, 0, 1)
        RectTransform.SetSizeDelta(mark, 12, -Dropdown.item_height + 12)
        RectTransform.SetAnchoredPosition(mark, 16, 0)
        Scene.AddComponent(mark, "Image")
        Image.SetColor(mark, 0.196, 0.196, 0.196, 1)
        Image.SetRaycastTarget(mark, false)
        local l = child(e, "Item Label", 34, 10)
        Scene.AddComponent(l, "Text")
        Text.SetText(l, text)
        Text.SetFontSize(l, Dropdown.font_size)
        Text.SetAlignment(l, "MiddleLeft")
        Text.SetWrap(l, false)
        Text.SetRichText(l, false)
        Text.SetRaycastTarget(l, false)
        Text.SetColor(l, 0.196, 0.196, 0.196, 1)
        items[i] = e
    end
    for i, e in ipairs(items) do
        Selectable.SetNavigation(e, "Explicit")
        Selectable.SetSelectOn(e, "Up", items[i - 1])
        Selectable.SetSelectOn(e, "Down", items[i + 1])
    end
    stale = false
end

-- Which option's item `hit` is (or is inside), or nil.
local function item_of(hit)
    while hit do
        for i, e in ipairs(items) do
            if e == hit then return i end
        end
        hit = Scene.GetParent(hit)
    end
end

function Dropdown.show()
    if popup or not template or #list == 0 then return end
    local r = UI.GetRect(self_id)
    if not r then return end
    if stale then build_items() end
    local rw, rh = Canvas.GetReferenceResolution(r.canvas)
    popup = Scene.CreateEntity("Dropdown List")
    Scene.AddComponent(popup, "Canvas")
    Canvas.SetSortOrder(popup, POPUP_ORDER)
    Canvas.SetReferenceResolution(popup, rw, rh)
    Canvas.SetMatchWidthOrHeight(popup, Canvas.GetMatchWidthOrHeight(r.canvas))
    blocker = child(popup, "Blocker", 0, 0)
    Scene.AddComponent(blocker, "Image")
    Image.SetColor(blocker, 0, 0, 0, 0)
    local _, height = RectTransform.GetSizeDelta(template)
    home = {
        { RectTransform.GetAnchorMin(template) }, { RectTransform.GetAnchorMax(template) },
        { RectTransform.GetPivot(template) }, { RectTransform.GetAnchoredPosition(template) },
        { RectTransform.GetSizeDelta(template) },
    }
    Scene.SetParent(template, popup)
    RectTransform.SetAnchorMin(template, 0, 0)
    RectTransform.SetAnchorMax(template, 0, 0)
    -- Below the dropdown, or above it when the screen has no room below.
    local below = r.y - height >= 0
    RectTransform.SetPivot(template, 0, below and 1 or 0)
    RectTransform.SetAnchoredPosition(template, r.x, below and r.y or r.y + r.height)
    RectTransform.SetSizeDelta(template, r.width, height)
    for i, e in ipairs(items) do
        Scene.SetActive(Scene.FindChild(e, "Item Checkmark"), i == Dropdown.value + 1)
    end
    Scene.SetActive(template, true)
    UI.SetSelected(items[Dropdown.value + 1])
    opened_tick = Time.frameCount()
end

function Dropdown.hide()
    if not popup then return end
    Scene.SetActive(template, false)
    Scene.SetParent(template, self_id)
    RectTransform.SetAnchorMin(template, home[1][1], home[1][2])
    RectTransform.SetAnchorMax(template, home[2][1], home[2][2])
    RectTransform.SetPivot(template, home[3][1], home[3][2])
    RectTransform.SetAnchoredPosition(template, home[4][1], home[4][2])
    RectTransform.SetSizeDelta(template, home[5][1], home[5][2])
    Scene.DestroyEntity(blocker)
    Scene.DestroyEntity(popup)
    popup, blocker = nil, nil
    UI.SetSelected(self_id)
end

local function choose(i)
    Dropdown.hide()
    Dropdown.set_value(i - 1)
end

function Dropdown.Awake(id)
    self_id = id
    label = Scene.FindChild(id, "Label")
    template = Scene.FindChild(id, "Template")
    list = split(Dropdown.options)
    Dropdown.value = math.max(0, math.min(#list - 1, math.floor(Dropdown.value)))
    refresh_label()
end

function Dropdown.OnPointerClick(id, event)
    if event.button == "Left" then Dropdown.show() end
end

function Dropdown.OnSubmit(id)
    Dropdown.show()
end

-- While open: the list's own clicks, Enter and Escape (the items carry no script).
function Dropdown.Update(id)
    if not popup or Time.frameCount() == opened_tick then return end
    if Input.GetKeyDown("ESCAPE") then return Dropdown.hide() end
    local picked = item_of(UI.GetSelected())
    if picked and (Input.GetKeyDown("ENTER") or Input.GetKeyDown("KEYPADENTER")) then
        return choose(picked)
    end
    local mx, my = Input.GetMousePosition()
    local _, sh = UI.GetScreenSize()
    local hit = UI.Raycast(mx, sh - my)
    if Input.GetKeyDown("MOUSE0") then
        press_at = { mx, my }
        if hit == blocker then return Dropdown.hide() end
    end
    if Input.GetKeyUp("MOUSE0") and press_at then
        local still = math.abs(mx - press_at[1]) + math.abs(my - press_at[2]) <= DRAG_SLOP
        press_at = nil
        local i = still and item_of(hit)
        if i then choose(i) end
    end
end

function Dropdown.OnDisable(id)
    Dropdown.hide()
end

return Dropdown
