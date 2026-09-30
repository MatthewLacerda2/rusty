-- input_field.lua — the engine's Input Field widget (#422), Unity's `InputField`.
--
-- Text entry while focused: typed characters (`Input.GetTextInput`), Backspace,
-- Delete, Left/Right (Up/Down between lines in MultiLine; to the start/end in
-- SingleLine), Home/End, Shift to select, Ctrl+A to select all, a click to place
-- the caret and a drag to select. Ctrl+C / Ctrl+X copy / cut the selection and
-- Ctrl+V pastes through the same filters as typing (`Input.GetClipboard`, #612);
-- Cmd works for Ctrl on macOS. A Password field never copies or cuts. Focusing it selects everything, as in Unity.
-- Enter submits a SingleLine field (`on_submit`); in MultiLine it breaks the line.
-- Escape restores the text it had when focused and lets go. `content_type`:
-- Standard, Integer, Decimal or Password (shown as `*`). `char_limit` 0 = none.
-- The caret blinks on unscaled time; the text scrolls to keep it in view.
--
-- Owner API (via `Scene.GetScript(id, "input_field")`):
--   text                                         -- the current text (read it)
--   on_value_changed = function(id, text) … end  -- every edit
--   on_submit = function(id, text) … end         -- Enter (SingleLine)
--   on_end_edit = function(id, text) … end       -- focus left it
--   set_text(s) / set_text_without_notify(s)

local Field = {
    fields = {
        text = { type = "text", default = "", tooltip = "The starting text" },
        char_limit = { type = "number", default = 0, tooltip = "Most characters allowed (0 = no limit)" },
        content_type = { type = "text", default = "Standard", tooltip = "Standard, Integer, Decimal or Password" },
        line_type = { type = "text", default = "SingleLine", tooltip = "SingleLine or MultiLine" },
        caret_blink_rate = { type = "number", default = 0.85, tooltip = "Caret blinks per second (0 = steady)" },
    },
}

local SELECTION = { 0.659, 0.808, 1.0, 0.753 }
local self_id, area, text_id, hint, caret, marks
local chars = {}               -- the text, one UTF-8 character per entry
local pos, anchor = 0, 0       -- caret and selection anchor, in characters
local focused, original, blink = false, "", 0
local scroll = { 0, 0 }
local highlights = {}

local multiline = function() return Field.line_type == "MultiLine" end

local function split(s)
    local out = {}
    for _, c in utf8.codes(s) do out[#out + 1] = utf8.char(c) end
    return out
end

local function shown(i, j)
    local out = {}
    for k = i, j do
        local c = chars[k]
        out[#out + 1] = (Field.content_type == "Password" and c ~= "\n") and "*" or c
    end
    return table.concat(out)
end

-- (line, column) of character position `p` (0-based; line 0-based).
local function locate(p)
    local line, start = 0, 0
    for k = 1, p do
        if chars[k] == "\n" then line, start = line + 1, k end
    end
    return line, p - start, start
end

local function line_start(p) local _, _, s = locate(p); return s end
local function line_end(p)
    local k = p
    while k < #chars and chars[k + 1] ~= "\n" do k = k + 1 end
    return k
end

local function width(i, j) return (Text.MeasureString(text_id, shown(i, j))) end
local function line_h() local _, h = Text.MeasureString(text_id, "A"); return h end
local function pitch() local _, h = Text.MeasureString(text_id, "A\nA"); return h - line_h() end

-- Where the caret at `p` sits, text-local reference units (x right, line down).
local function caret_xy(p)
    local line, _, s = locate(p)
    return width(s + 1, p), line
end

local function commit(notify)
    Field.text = table.concat(chars)
    blink = 0
    if notify and Field.on_value_changed then Field.on_value_changed(self_id, Field.text) end
end

local function set(s, notify)
    chars = split(s or "")
    pos, anchor = #chars, #chars
    commit(notify)
end

function Field.set_text(s) set(s, true) end
function Field.set_text_without_notify(s) set(s, false) end

local function delete_selection()
    local a, b = math.min(pos, anchor), math.max(pos, anchor)
    for _ = a + 1, b do table.remove(chars, a + 1) end
    pos, anchor = a, a
    return b > a
end

-- Whether `c` may go in at `pos` (selection already removed).
local function accepts(c)
    if Field.char_limit > 0 and #chars >= Field.char_limit then return false end
    local t = Field.content_type
    if t == "Integer" or t == "Decimal" then
        if c == "-" then return pos == 0 and chars[1] ~= "-" end
        if c == "." and t == "Decimal" then return not table.concat(chars):find(".", 1, true) end
        return c:match("^%d$") ~= nil
    end
    return c == "\n" or c:byte() >= 32
end

local function type_text(s)
    local changed = false
    for _, code in utf8.codes(s) do
        local c = utf8.char(code)
        if c == "\b" then
            if not delete_selection() and pos > 0 then
                table.remove(chars, pos)
                pos, anchor = pos - 1, pos - 1
            end
            changed = true
        elseif c ~= "\t" and (c ~= "\n" or multiline()) then
            changed = delete_selection() or changed
            if accepts(c) then
                table.insert(chars, pos + 1, c)
                pos, anchor = pos + 1, pos + 1
                changed = true
            end
        end
    end
    return changed
end

-- Paste `s` over the selection, one character at a time through `accepts`, so the
-- content type and char limit hold and SingleLine drops line breaks.
local function paste(s)
    local changed = delete_selection()
    for _, code in utf8.codes(s) do
        local c = utf8.char(code)
        if (c ~= "\n" or multiline()) and accepts(c) then
            table.insert(chars, pos + 1, c)
            pos, anchor = pos + 1, pos + 1
            changed = true
        end
    end
    return changed
end

-- Ctrl/Cmd + C, X or V. Returns whether the text changed.
local function clipboard_keys()
    local a, b = math.min(pos, anchor), math.max(pos, anchor)
    local cut = Input.GetKeyDown("X")
    if (cut or Input.GetKeyDown("C")) and b > a and Field.content_type ~= "Password" then
        Input.SetClipboard(table.concat(chars, "", a + 1, b))
        if cut then return delete_selection() end
    end
    return Input.GetKeyDown("V") and paste(Input.GetClipboard())
end

local function shift() return Input.IsKeyDown("LEFTSHIFT") or Input.IsKeyDown("RIGHTSHIFT") end
local function ctrl()
    return Input.IsKeyDown("LEFTCONTROL") or Input.IsKeyDown("RIGHTCONTROL")
        or Input.IsKeyDown("LEFTSUPER") or Input.IsKeyDown("RIGHTSUPER")
end

-- Move the caret to `p`, extending the selection when `extend`.
local function move_to(p, extend)
    pos = math.max(0, math.min(#chars, p))
    if not extend then anchor = pos end
    blink = 0
end

-- The character position nearest a pointer event.
local function index_at(event)
    local r = UI.GetRect(text_id)
    local at = event.canvas_position
    if not r or not at then return pos end
    local x = at.x - r.x
    local line = 0
    if multiline() then
        line = math.max(0, math.floor(((r.y + r.height) - at.y) / pitch()))
    end
    local p, l = 0, 0
    while l < line and p < #chars do
        if chars[p + 1] == "\n" then l = l + 1 end
        p = p + 1
    end
    local s, e = p, line_end(p)
    local best, best_d = s, math.huge
    for k = s, e do
        local d = math.abs(width(s + 1, k) - x)
        if d < best_d then best, best_d = k, d end
    end
    return best
end

-- Keep the caret in view by scrolling the text inside the text area.
local function follow()
    local a = UI.GetRect(area)
    if not a then return end
    local x, line = caret_xy(pos)
    if x - scroll[1] > a.width - 3 then scroll[1] = x - a.width + 3 end
    if x - scroll[1] < 0 then scroll[1] = x end
    local total = 0
    for k = 1, #chars do
        if chars[k] == "\n" then total = math.max(total, width(line_start(k - 1) + 1, k - 1)) end
    end
    total = math.max(total, width(line_start(#chars) + 1, #chars))
    scroll[1] = math.max(0, math.min(scroll[1], math.max(0, total - a.width + 3)))
    if multiline() then
        local top, bottom = line * pitch(), line * pitch() + line_h()
        if bottom - scroll[2] > a.height then scroll[2] = bottom - a.height end
        if top - scroll[2] < 0 then scroll[2] = top end
    end
end

-- One highlight rect per selected line segment (a pool under `Selection`).
local function draw_selection()
    local a, b = math.min(pos, anchor), math.max(pos, anchor)
    local n = 0
    if focused and b > a then
        local p = a
        while p < b do
            local e = math.min(line_end(p), b)
            local x0, line = caret_xy(p)
            local x1 = caret_xy(e)
            n = n + 1
            local h = highlights[n]
            if not h then
                h = Scene.CreateEntity("Highlight")
                Scene.AddComponent(h, "Image")
                Image.SetColor(h, SELECTION[1], SELECTION[2], SELECTION[3], SELECTION[4])
                Image.SetRaycastTarget(h, false)
                Scene.SetParent(h, marks)
                highlights[n] = h
            end
            Scene.SetActive(h, true)
            local top = multiline() and 1 or 0.5
            RectTransform.SetAnchorMin(h, 0, top)
            RectTransform.SetAnchorMax(h, 0, top)
            RectTransform.SetPivot(h, 0, top)
            RectTransform.SetSizeDelta(h, math.max(x1 - x0, 6), line_h())
            RectTransform.SetAnchoredPosition(h, x0 - scroll[1], -line * pitch() + scroll[2])
            p = e + 1
        end
    end
    for k = n + 1, #highlights do Scene.SetActive(highlights[k], false) end
end

local function refresh()
    Text.SetText(text_id, shown(1, #chars))
    if hint then Scene.SetActive(hint, #chars == 0) end
    if focused then follow() end
    RectTransform.SetAnchoredPosition(text_id, -scroll[1], scroll[2])
    local x, line = caret_xy(pos)
    RectTransform.SetAnchoredPosition(caret, x - scroll[1], -line * pitch() + scroll[2])
    RectTransform.SetSizeDelta(caret, 3, line_h())
    local rate = Field.caret_blink_rate
    local lit = focused and pos == anchor and (rate <= 0 or (blink * rate) % 1 < 0.5)
    local r, g, b = Image.GetColor(caret)
    Image.SetColor(caret, r, g, b, lit and 1 or 0)
    draw_selection()
end

function Field.Awake(id)
    self_id = id
    area = Scene.FindChild(id, "Text Area")
    text_id = area and Scene.FindChild(area, "Text")
    hint = area and Scene.FindChild(area, "Placeholder")
    caret = area and Scene.FindChild(area, "Caret")
    marks = area and Scene.FindChild(area, "Selection")
    if not (text_id and caret and marks) then text_id = nil; return end
    Text.SetRichText(text_id, false)
    Text.SetWrap(text_id, false)
    local top = multiline() and 1 or 0.5
    local align = multiline() and "TopLeft" or "MiddleLeft"
    Text.SetAlignment(text_id, align)
    if hint then Text.SetAlignment(hint, align) end
    RectTransform.SetPivot(text_id, 0, top)
    RectTransform.SetAnchorMin(caret, 0, top)
    RectTransform.SetAnchorMax(caret, 0, top)
    RectTransform.SetPivot(caret, 0, top)
    set(Field.text, false)
    refresh()
end

function Field.OnSelect(id)
    if not text_id then return end
    -- Select all; a click's OnPointerDown (which follows) then places the caret.
    focused, original = true, Field.text
    pos, anchor = #chars, 0
    refresh()
end

function Field.OnDeselect(id)
    if not text_id then return end
    focused = false
    anchor = pos
    refresh()
    if Field.on_end_edit then Field.on_end_edit(self_id, Field.text) end
end

function Field.OnPointerDown(id, event)
    if not text_id or event.button ~= "Left" then return end
    move_to(index_at(event), shift() and focused)
    refresh()
end

function Field.OnDrag(id, event)
    if not text_id or event.button ~= "Left" then return end
    move_to(index_at(event), true)
    refresh()
end

function Field.OnMove(id, event)
    if not text_id then return end
    local d, ext = event.direction, shift()
    if (d == "Left" or d == "Right") and pos ~= anchor and not ext then
        local a, b = math.min(pos, anchor), math.max(pos, anchor)
        move_to(d == "Left" and a or b, false)
    elseif d == "Left" then move_to(pos - 1, ext)
    elseif d == "Right" then move_to(pos + 1, ext)
    elseif not multiline() then move_to(d == "Up" and 0 or #chars, ext)
    else
        local line, col = locate(pos)
        local target = d == "Up" and line - 1 or line + 1
        local p, l = 0, 0
        while l < target and p < #chars do
            if chars[p + 1] == "\n" then l = l + 1 end
            p = p + 1
        end
        if target < 0 then p = 0 elseif l < target then p = #chars
        else p = math.min(p + col, line_end(p)) end
        move_to(p, ext)
    end
    refresh()
end

function Field.OnSubmit(id)
    if text_id and not multiline() and Field.on_submit then Field.on_submit(self_id, Field.text) end
end

function Field.OnCancel(id)
    if not text_id then return end
    if Field.text ~= original then set(original, true) end
    UI.SetSelected(nil)
end

function Field.Update(id)
    if not text_id or not focused then return end
    blink = blink + Time.unscaledDeltaTime()
    -- With Ctrl / Cmd held the keys are shortcuts, not text.
    local changed = not ctrl() and type_text(Input.GetTextInput())
    if Input.GetKeyDown("DELETE") then
        if not delete_selection() and pos < #chars then table.remove(chars, pos + 1) end
        changed = true
    end
    if Input.GetKeyDown("HOME") then move_to(line_start(pos), shift()) end
    if Input.GetKeyDown("END") then move_to(line_end(pos), shift()) end
    if ctrl() and Input.GetKeyDown("A") then pos, anchor = #chars, 0 end
    if ctrl() and clipboard_keys() then changed = true end
    if changed then commit(true) end
    refresh()
end

return Field
