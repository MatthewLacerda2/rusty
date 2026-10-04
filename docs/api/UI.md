## `UI`

EventSystem-level UI verbs: layout reads (#417), focus, the gameplay-vs-UI guards
and the agent's pointer verbs (#420). Points are **UI screen pixels**: bottom-left
origin, y-up — the frame of `GetRect(id).screen`. (`Input.GetMousePosition` is
top-left: UI `y` = screen height − mouse `y`.) Every verb looks through the active
camera (#429): markers sit where they draw, and a point also casts the camera ray
through it, so world and camera canvases are hit, listed and clicked too.

| Function | Signature | Returns |
|---|---|---|
| `UI.Create` | `(kind, [parentId])` | the new widget's root `id` — the Create ▸ UI menu's API face; see below |
| `UI.GetRect` | `(id)` | rect table, or `nil` when the entity is not laid out under a canvas |
| `UI.GetScreenSize` | `()` | `width, height` in pixels — the screen the UI lays out on |
| `UI.SetSelected` | `(id)` | — focus `id` (`nil` clears); `OnDeselect` / `OnSelect` fire at the head of the next tick's script phase |
| `UI.GetSelected` | `()` | the focused entity's id, or `nil` |
| `UI.IsPointerOverUI` | `()` | `true` when the pointer is over a raycast target this tick |
| `UI.IsPointerConsumed` | `()` | `true` when gameplay should leave the pointer alone: it is over the UI, or a press that began over the UI is still held |
| `UI.Raycast` | `(x, y)` | the top-most raycast target under the point — screen canvases first, then world canvases along the camera ray, nearest first — or `nil` |
| `UI.FindSelectable` | `(id, direction)` | where keyboard navigation from `id` goes in `direction` (`"Up"`, `"Down"`, `"Left"`, `"Right"`), or `nil` — Unity's `FindSelectableOn*`, for an `OnMove` handler that navigates itself |
| `UI.Click` | `(id)` | `true` if the click will land on `id` (or its descendant) — see below |
| `UI.List` | `()` | array of every visible `Selectable`, in draw order — see below |
| `UI.SetShader` | `(id, name)` | — draw `id`'s graphics (its `Image`, `Shape` and `Text`) with the baked ui shader `name` (#427); `nil` or `""` restores the standard shader. Naming a different shader drops the old one's param values. Errors when `id` has none of them. See below |
| `UI.GetShader` | `(id)` | the ui shader `id`'s graphics draw with (its first graphic's in draw order: Image, Shape, Text), or `nil` |
| `UI.SetShaderParam` | `(id, name, value)` | — set a **runtime** param of the shader on every graphic of `id` that names one, e.g. `"dissolve.amount"`; `value` is a number or an array (one number broadcasts). Strict: an unknown or baked param, or a wrong count, errors listing the runtime params; so does calling it before `SetShader` |
| `UI.GetShaderParam` | `(id, name)` | the value it draws with: the one set, else the baked default (a number, or an array for a vector param) |

**Custom shaders** (`UI.SetShader` & co., #427) are Unity's `Graphic.material`: bake
one with `Shader.Bake` and `pass = "ui"` (see [`Shader.md`](Shader.md#the-ui-pass)),
name it on a graphic, and drive its runtime params the way
`Material.SetShaderParam` drives a material's — same names (`"block.param"`, or
`"block.index.param"` when the block repeats), same values, no re-bake:

```lua
UI.SetShader(panel, "hud_dissolve")
UI.SetShaderParam(panel, "dissolve.amount", 0.5)   -- half burnt away
-- or ease it over time on the UI's unscaled clock (#661, see Tween.md):
Tween.To(panel, "UI.dissolve.amount", 1, 0.6, { unscaled = true })
```

The model — what a ui shader changes, the unscaled clock, fallback — is in
[*Custom shaders* in `docs/ui.md`](../ui.md#custom-shaders).

**`UI.Create(kind, [parentId])`** builds what the editor's GameObject ▸ UI menu
builds (#422), through the same code: `kind` is `"Canvas"`, `"Panel"`, `"Image"`,
`"Text"`, `"Button"`, `"Toggle"`, `"Toggle Group"`, `"Slider"`, `"Scrollbar"`,
`"Scroll View"`, `"Dropdown"` or `"Input Field"` (case and spaces ignored; an
unknown kind errors, listing them). The widget goes under `parentId` when that is
inside a canvas, else under the scene's first root canvas, else under a new
`Canvas`; a `Canvas` itself is made at the root. During play the widget's script
loads at the head of the next tick, like any spawn. The widgets and their script
APIs are in [Widgets in `docs/ui.md`](../ui.md#widgets).

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
descendants. An element on a world canvas is clicked at its projected centre; while
the cursor is locked the click goes through the screen centre instead (look at the
terminal, click), landing only if `id` is what the crosshair is on. **`UI.Raycast(x, y)`** and
**`UI.List()`** compute the layout on demand from the live scene, in edit mode
too. Each `List` entry is `{ id, name, state, interactable, selected, rect }` —
`state` as `Selectable.GetState`, `rect = { x, y, width, height }` in screen
pixels (projected for a world canvas; absent when it is behind the camera) — so a bot can find "Start Game" by name and click it:

```lua
for _, b in ipairs(UI.List()) do
  if b.name == "Start Game" then UI.Click(b.id) end
end
```

The rect table: `x, y, width, height` — the axis-aligned bounds of the element's
final quad (after rotation / scale) in reference units; `screen = { x, y, width,
height }` — the same in screen pixels (bottom-left origin, y-up); `corners` — the
exact quad in reference units, `{ {x, y}, … }` bottom-left, top-left, top-right,
bottom-right; `canvas` — the root canvas id; `scale_factor`; `world` — `true` on a
`WorldSpace` / `ScreenSpaceCamera` canvas, whose `screen` box is its projected quad
(absent while any corner is behind the camera). `GetRect` computes on
demand from the live scene, so it reflects a change made earlier in the same
script, in edit mode as well as in Play. The screen size is the game view's pixel
size in the windowed app and the `Video` resolution headless.
