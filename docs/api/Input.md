## `Input`

The full keyboard and mouse (#416): held keys and per-tick edges, the pointer, raw
mouse motion, the wheel, typed text and the cursor request — plus a writable half
that injects all of it, so a script, bot-player or the harness can play as the user.

**Everything is a named key.** Keys are case-insensitive strings (`"W"`, `"space"`);
mouse buttons are keys too — `"Mouse0"` left, `"Mouse1"` right, `"Mouse2"` middle,
`"Mouse3"` back, `"Mouse4"` forward — so `IsKeyDown` / `GetKeyDown` / `Press` work on
them unchanged. Continuous inputs are **axes** (`GetMouseDelta`, `GetScrollDelta`).
Gamepads (#471) slot into the same model: pad buttons become more key names, sticks
become more axes.

**Edges are per sim tick.** Everything written between two ticks — OS events or
injection — is published at the start of the next tick and holds for exactly that
tick: `GetKeyDown`/`GetKeyUp` are `true` on it only, and `GetMouseDelta`,
`GetScrollDelta`, `GetTextInput` report what accumulated before it (zero / `""` on the
tick after). A key pressed and released between two ticks reports **both** edges on
the next tick (and `IsKeyDown` is already `false`), so no tap is lost however ticks
and frames interleave. Held state (`IsKeyDown`) and `MoveMouse` take effect at once.
A `Press` from inside a script's `Update` is therefore seen by `GetKeyDown` on the
*next* tick. Because a tick sees a pure function of what was written before it,
injected input replays identically in the headless harness.

**Mouse position** is in **game-view pixels**, origin top-left: the pixel grid the
game renders and lays its UI out at. In the player that is the window; in the editor
it is the Game-view panel (the window pointer is mapped into the panel's rect and
render size). **Mouse delta** is raw device motion (not pointer pixels), the input
for FPS mouse-look — it keeps flowing while the cursor is locked. **Scroll** is
vertical wheel lines, positive = away from the user; touchpad pixels are scaled
(20 px = 1 line).

**Typed text** follows Unity's `inputString`: printable characters, plus `"\b"`
(backspace), `"\n"` (Enter) and `"\t"` (Tab); other control keys add nothing.

**Cursor.** Entering Play resets the request to locked + hidden (mouse-look), before
any `Start` runs; Stop resets it to free + visible. `SetCursorLocked` /
`SetCursorVisible` override it (a pause menu unlocks and shows the cursor). The sim
only *records* the request; the platform applies it to the OS cursor whenever the
game has input (a lock uses OS pointer-lock where supported, else confines the
pointer to the window).

**Who hears input.** The player: the game always does. The editor: only in Play
with the **Game view focused** — entering Play switches to the Game tab and focuses
it, a click inside the Game view focuses it, a click elsewhere unfocuses it (except
while the cursor is locked, when the pointer is captured; ESC stops Play). Losing
focus releases held keys, with their `GetKeyUp` edges.

| Function | Signature | Returns |
|---|---|---|
| `Input.IsKeyDown` | `(key)` | `bool` — held right now |
| `Input.GetKeyDown` | `(key)` | `bool` — went down since the previous tick (this tick only) |
| `Input.GetKeyUp` | `(key)` | `bool` — went up since the previous tick (this tick only) |
| `Input.GetMousePosition` | `()` | `x, y` in game-view pixels, origin top-left |
| `Input.GetMouseDelta` | `()` | `dx, dy` raw motion accumulated before this tick |
| `Input.GetScrollDelta` | `()` | wheel lines scrolled before this tick (positive = up) |
| `Input.GetTextInput` | `()` | the characters typed before this tick, in order |
| `Input.SetCursorLocked` | `(locked)` | — request a locked (captured) or free cursor |
| `Input.SetCursorVisible` | `(visible)` | — request a shown or hidden cursor |
| `Input.IsCursorLocked` | `()` | `bool` — the current request |
| `Input.IsCursorVisible` | `()` | `bool` — the current request |
| `Input.Press` | `(key)` | — hold a key or mouse button down |
| `Input.Release` | `(key)` | — let it go |
| `Input.MoveMouse` | `(x, y)` | — move the pointer (game-view pixels) |
| `Input.AddMouseDelta` | `(dx, dy)` | — inject raw motion for the next tick |
| `Input.Scroll` | `(dy)` | — inject wheel lines for the next tick |
| `Input.TypeText` | `(text)` | — inject typed characters for the next tick |

Injection is **logical**: it bypasses the keybinding remap (below), exactly as the
harness and bot-players always have.

```lua
-- Mouse-look + fire, and a pause menu that frees the cursor.
local Shooter = {}
local yaw = 0

function Shooter.Update(id, dt)
  local dx, _ = Input.GetMouseDelta()
  yaw = yaw + dx * 0.1
  Transform.SetRotation(id, 0, yaw, 0)
  if Input.GetKeyDown("Mouse0") then
    local ox, oy, oz = Camera.GetPosition()
    local fx, fy, fz = Camera.GetForward()
    local hit, target = Physics.Raycast(ox, oy, oz, fx, fy, fz, id)
  end
  if Input.GetKeyDown("Escape") then
    Input.SetCursorLocked(false)
    Input.SetCursorVisible(true)
  end
end

return Shooter
```

### Key names

Letters and digits are bare (`"A"`…`"Z"`, `"0"`…`"9"`); the rest follow Unity's
`KeyCode` names, uppercased. The table lives in `src/shell/input/keys.rs`.

| Group | Names |
|---|---|
| Function | `F1` … `F24` |
| Arrows | `UP`, `DOWN`, `LEFT`, `RIGHT` |
| Editing | `SPACE`, `ESCAPE`, `TAB`, `ENTER`, `BACKSPACE`, `INSERT`, `DELETE`, `HOME`, `END`, `PAGEUP`, `PAGEDOWN` |
| Modifiers | `LEFTSHIFT`, `RIGHTSHIFT`, `LEFTCONTROL`, `RIGHTCONTROL`, `LEFTALT`, `RIGHTALT`, `LEFTSUPER`, `RIGHTSUPER` (Cmd / Windows key) |
| Locks & system | `CAPSLOCK`, `NUMLOCK`, `SCROLLLOCK`, `PRINT`, `PAUSE`, `MENU` |
| Punctuation | `MINUS`, `EQUALS`, `LEFTBRACKET`, `RIGHTBRACKET`, `BACKSLASH`, `SEMICOLON`, `QUOTE`, `BACKQUOTE`, `COMMA`, `PERIOD`, `SLASH` |
| Keypad | `KEYPAD0` … `KEYPAD9`, `KEYPADPLUS`, `KEYPADMINUS`, `KEYPADMULTIPLY`, `KEYPADDIVIDE`, `KEYPADPERIOD`, `KEYPADENTER` |
| Mouse | `MOUSE0` (left), `MOUSE1` (right), `MOUSE2` (middle), `MOUSE3` (back), `MOUSE4` (forward), `MOUSE<n>` (other buttons) |

Keys outside the table (media keys, IME/language keys) are not forwarded. Physical
names are what the keybinding remap below maps *from*; mouse buttons are remappable
the same way.
