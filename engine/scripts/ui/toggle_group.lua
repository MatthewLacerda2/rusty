-- toggle_group.lua — the engine's ToggleGroup (#422), Unity's `ToggleGroup`.
--
-- Put it on any ancestor of some toggles: they become radio buttons — at most one
-- is on, and (unless `allow_switch_off`) exactly one, so clicking the on one does
-- nothing. A group starting with none on turns its first on; with several, keeps
-- the first.
--
-- Owner API (via `Scene.GetScript(id, "toggle_group")`):
--   active()           -- the `toggle` script table that is on, or nil
--   set_all_off()      -- turn every toggle off (only when allow_switch_off)

local Group = {
    fields = {
        allow_switch_off = { type = "boolean", default = false, tooltip = "Whether all toggles may be off" },
    },
}

local toggles = {}

-- Called by each member toggle in its Awake.
function Group.register(t)
    toggles[#toggles + 1] = t
end

function Group.active()
    for _, t in ipairs(toggles) do
        if t.is_on then return t end
    end
end

-- Whether `t` may not turn off: the group needs one on and no other is.
function Group.is_locked(t)
    if Group.allow_switch_off then return false end
    for _, other in ipairs(toggles) do
        if other ~= t and other.is_on then return false end
    end
    return true
end

-- A member turned on: turn the others off (each announcing its change).
function Group.notify_on(on)
    for _, t in ipairs(toggles) do
        if t ~= on and t.is_on then t.set_is_on(false) end
    end
end

function Group.set_all_off()
    if not Group.allow_switch_off then return end
    for _, t in ipairs(toggles) do t.set_is_on(false) end
end

function Group.Start(id)
    local first = Group.active()
    if first then
        Group.notify_on(first)
    elseif toggles[1] and not Group.allow_switch_off then
        toggles[1].set_is_on(true)
    end
end

return Group
