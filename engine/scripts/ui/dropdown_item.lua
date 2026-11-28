-- lint: allow(ui-coverage) a Dropdown attaches it to its own list items: not a widget
-- One clickable part of an open Dropdown's list (an `Item`, or the `Blocker`).
-- A click forwards `index` to the dropdown: the 0-based option, or -1 for the
-- blocker, which closes without picking.

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
