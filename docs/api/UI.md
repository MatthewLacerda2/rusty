## `UI`

EventSystem-level UI verbs: layout reads (#417), focus, the gameplay-vs-UI guards
and the agent's pointer verbs (#420). Points are **UI screen pixels**: bottom-left
origin, y-up — the frame of `GetRect(id).screen`. (`Input.GetMousePosition` is
top-left: UI `y` = screen height − mouse `y`.)

| Function | Signature | Returns |
|---|---|---|
| `UI.GetRect` | `(id)` | rect table, or `nil` when the entity is not laid out under a canvas |
| `UI.GetScreenSize` | `()` | `width, height` in pixels — the screen the UI lays out on |
| `UI.SetSelected` | `(id)` | — focus `id` (`nil` clears); `OnDeselect` / `OnSelect` fire at the head of the next tick's script phase |
| `UI.GetSelected` | `()` | the focused entity's id, or `nil` |
| `UI.IsPointerOverUI` | `()` | `true` when the pointer is over a raycast target this tick |
| `UI.IsPointerConsumed` | `()` | `true` when gameplay should leave the pointer alone: it is over the UI, or a press that began over the UI is still held |
| `UI.Raycast` | `(x, y)` | the top-most raycast target under the point, or `nil` |
| `UI.Click` | `(id)` | `true` if the click will land on `id` (or its descendant) — see below |
| `UI.List` | `()` | array of every visible `Selectable`, in draw order — see below |

**Gameplay vs UI.** A click on a menu must not also fire the weapon. Guard
gameplay pointer input with `UI.IsPointerConsumed()` — UI callbacks run before
`Update`, so it is already settled for the tick:

```lua
function Weapon.Update(id, dt)
  if Input.IsKeyDown("Mouse0") and not UI.IsPointerConsumed() then fire(id) end
end
```

**`UI.Click(id)`** is a real click through the real input path: it moves the
pointer to the centre of `id`'s final quad and presses and releases `Mouse0`, so
on the next tick the pointer enters, presses, releases and clicks exactly as a
player's would — a modal covering the button eats it, a locked cursor misses the
UI. It returns whether the top-most hit at that point is `id` or one of its
descendants (and the cursor is not locked). **`UI.Raycast(x, y)`** and
**`UI.List()`** compute the layout on demand from the live scene, in edit mode
too. Each `List` entry is `{ id, name, state, interactable, selected, rect }` —
`state` as `Selectable.GetState`, `rect = { x, y, width, height }` in screen
pixels — so a bot can find "Start Game" by name and click it:

```lua
for _, b in ipairs(UI.List()) do
  if b.name == "Start Game" then UI.Click(b.id) end
end
```

The rect table: `x, y, width, height` — the axis-aligned bounds of the element's
final quad (after rotation / scale) in reference units; `screen = { x, y, width,
height }` — the same in screen pixels (bottom-left origin, y-up); `corners` — the
exact quad in reference units, `{ {x, y}, … }` bottom-left, top-left, top-right,
bottom-right; `canvas` — the root canvas id; `scale_factor`. `GetRect` computes on
demand from the live scene, so it reflects a change made earlier in the same
script, in edit mode as well as in Play. The screen size is the game view's pixel
size in the windowed app and the `Video` resolution headless.
