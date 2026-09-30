## `Animator`

Drive an entity's animation clips. Clips are imported from the entity's skinned
glTF mesh (its `animations`); the named `clip` selects one to play. A keyframe
sampler poses the skeleton each fixed step, so playback — including a looping
clip's wrap — is deterministic. A non-looping clip holds its last frame at the
end.

The `Set*` parameter setters write the animator's named, typed **graph
parameters** (Unity's Animator parameters): the variables the animation graph's
transition conditions read each fixed step to decide state changes (#312).
A parameter is created on first set; once a name holds a type, a set of a
*different* type is ignored (a console warning, never an error). Parameter
values are saved with the entity, and `Debug.Snapshot` reports them under the
animator's `parameters` key.

**Animation graphs (#316).** An animator may reference an **`AnimationGraph`**
asset (a `.animgraph` file, #315) — a state machine whose nodes each play one
clip and whose edges carry conditions over the parameters above. While a graph
is assigned and enabled, the engine evaluates it automatically each fixed step:
the **active node's** outgoing edges are checked in authored (priority) order,
and the **first** edge whose conditions *all* hold fires — a crossfade into the
target node's clip over the edge's `transition_duration` (0 is a hard cut),
adopting the target's `is_loop` flag and per-node speed (stacked on the
component's own speed multiplier). A `Trigger` condition on the fired edge is
consumed (auto-cleared) exactly once. An edge with no conditions never fires on
its own — jump it explicitly with `PlayNode`. On the first step after a graph
is assigned, the animator **binds** it: declared parameter defaults are seeded
(values a script already wrote win) and the entry node starts. Scripts just set
parameters; the character animates itself. `Debug.Snapshot` reports the binding
under the animator's `graph` / `graph_enabled` / `node` keys, and the graph
path + active state are saved with the entity.

| Function | Signature | Notes |
|---|---|---|
| `Animator.Play` | `(id, clip)` | Hard-cut to `clip` from its start (also releases a `Pause`). Raw clip-level control: it does not consult the graph — jump graph states with `PlayNode`. |
| `Animator.PlayAnimation` | `(id, clip)` | Like `Play`, but honest and queryable: returns `true` only when the entity's mesh actually carries a clip named `clip` (and an animator to play it); `false` otherwise, without side effects (`Play` silently no-ops on a missing clip). |
| `Animator.Crossfade` | `(id, clip, duration)` | Blend out of the current clip over `duration` seconds (a zero/negative duration, or a fade into the current clip, degrades to `Play`). |
| `Animator.Stop` | `(id)` | Halt playback (freezes the pose). |
| `Animator.Pause` | `(id)` | Hold the playhead where it is: the pose freezes but playback stays active (a hold, not a `Stop`). |
| `Animator.Resume` | `(id)` | Release a `Pause`, continuing from the held playhead. |
| `Animator.SetLooping` | `(id, loop)` | `loop = true` wraps the playhead at the current clip's end so it repeats seamlessly (idle/run/walk cycles); `false` (the default) holds the last frame. Persists across `Play`/`Crossfade`; entering a graph node overwrites it with the node's `is_loop`. |
| `Animator.SetBool` | `(id, name, value)` | Set the `Bool` parameter `name` (e.g. `Animator.SetBool(id, "isGrounded", true)`). |
| `Animator.SetFloat` | `(id, name, value)` | Set the `Float` parameter `name` (e.g. `Animator.SetFloat(id, "speed", 4.2)`). |
| `Animator.SetInt` | `(id, name, value)` | Set the `Int` parameter `name` (truncating: Lua numbers convert to a 32-bit integer). |
| `Animator.SetTrigger` | `(id, name)` | Latch the one-shot `Trigger` parameter `name`. The graph evaluator auto-clears it the moment it fires a transition; until then an unconsumed trigger stays latched — scripts never reset it by hand. |
| `Animator.SetGraph` | `(id, path)` | Assign the `.animgraph` asset at `path` to drive this animator (`""` clears it). Resets the active state, so evaluation re-binds — seeding declared defaults and entering the new graph's entry node — on the next fixed step. |
| `Animator.SetGraphEnabled` | `(id, enabled)` | Pause/resume graph auto-evaluation. Disabled, the graph is inert data and the animator stays under direct `Play`/`Crossfade` control; re-enabling resumes from the same active node. |
| `Animator.PlayNode` | `(id, node)` | Jump straight to the named graph node (state), bypassing all conditions: a hard cut into its clip, adopting its loop flag and speed. Returns `true` on success; `false` (with a console warning for a bad graph or unknown node) otherwise. |
| `Animator.GetCurrentNode` | `(id)` | The active graph node's name, or `nil` when there is no animator/graph or evaluation hasn't bound it yet. |
