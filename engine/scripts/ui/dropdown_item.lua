-- dropdown_item.lua — one clickable part of an open Dropdown's list (#996).
--
-- The dropdown attaches it to each `Item` it builds and to its `Blocker`, so their
-- clicks come through the UI event system's own rules (press and release on the
-- same entity, no drag in between) instead of a second copy of them in Lua — the
-- role of Unity's `DropdownItem` and the blocker's `Button`. A click forwards
-- `index` to the dropdown: the option it picks, 0-based, or -1 for the blocker,
-- which closes the list without picking.

local DropdownItem = {
    fields = {
        dropdown = { type = "number", default = 0, tooltip = "The dropdown this part belongs to" },
        index = { type = "number", default = -1, tooltip = "The option it picks, 0-based (-1 closes the list)" },
    },
}

function DropdownItem.OnPointerClick(id, event)
    if event.button ~= "Left" then return end
    local dropdown = Scene.GetScript(DropdownItem.dropdown, "dropdown")
    if dropdown then dropdown.select_item(DropdownItem.index) end
end

return DropdownItem
