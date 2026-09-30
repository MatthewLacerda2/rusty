## `LayoutElement`

Read and tune an entity's `LayoutElementComponent` (#421) — Unity's
`LayoutElement` and `ContentSizeFitter` in one component. The size overrides
replace what the element's content reports to its parent `LayoutGroup`, per axis:
**min** (never smaller), **preferred** (what it asks for) and **flexible** (its
weight when spare space is shared). Each is a `(width, height)` pair where `nil`
(or a negative number) means "use the content's size". The **fit** sizes the
element's *own* rect to its min or preferred size around its pivot — a text box
that grows with its string, a list that grows with its items; it applies when no
parent group arranges the element (under a group, the fitted size is the size the
group keeps for an uncontrolled axis). Adding one also adds a `RectTransform`.
Getters return a neutral default without one; setters are then no-ops.

| Function | Signature | Returns |
|---|---|---|
| `LayoutElement.GetIgnoreLayout` / `SetIgnoreLayout` | `(id)` / `(id, bool)` | whether the parent group skips it (it keeps its own anchors) |
| `LayoutElement.GetMinSize` / `SetMinSize` | `(id)` / `(id, w, h)` | min size overrides, `nil` where unset |
| `LayoutElement.GetPreferredSize` / `SetPreferredSize` | `(id)` / `(id, w, h)` | preferred size overrides, `nil` where unset |
| `LayoutElement.GetFlexibleSize` / `SetFlexibleSize` | `(id)` / `(id, w, h)` | flexible weights, `nil` where unset |
| `LayoutElement.GetFit` / `SetFit` | `(id)` / `(id, horizontal, vertical)` | the content fitter per axis: `"Unconstrained"`, `"MinSize"` or `"PreferredSize"` (an unknown name leaves both unchanged) |
