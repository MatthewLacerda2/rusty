## `Storage`

A namespaced, JSON-backed key-value store that survives across runs — the engine's
PlayerPrefs analog, backed by `project/storage.json` (human-readable so an agent can
diff a save in a PR). A value may be a scalar **or** a structured table.

| Function | Signature | Returns |
|---|---|---|
| `Storage.Set` | `(namespace, key, value)` | — (value: number / string / bool / table) |
| `Storage.Get` | `(namespace, key)` | the value, or `nil` |
| `Storage.Has` | `(namespace, key)` | `bool` |
| `Storage.Delete` | `(namespace, key)` | `bool` (whether something was removed) |
| `Storage.GetTable` | `(namespace)` | the whole namespace as a table, or `nil` |
| `Storage.SetTable` | `(namespace, table)` | — (replace a whole namespace at once) |

**Determinism.** Reads resolve from an in-memory snapshot loaded once at a boundary
(startup); writes mutate that snapshot and are persisted only at boundaries (Stop /
quit), never inside `FixedUpdate`. The headless harness runs the store **pathless**,
so it never reads a developer's real save and replays stay reproducible.

### Keybindings — rebindable controls

The windowed app reads a **physical→logical key remap** from `Storage` at startup
and applies it at the input source, *before* the simulation reads any key. Gameplay
and the `Input` API only ever see **logical** keys, so the remap is invisible to
scripts — `Input.IsKeyDown("W")` still asks about the logical `"W"`. To offer
rebindable controls, persist the chosen bindings as a flat `{ physical: logical }`
table under the `keybindings` namespace, key `bindings`; only entries that differ
from identity need to be stored, and the mapping loads on next launch:

```lua
-- Make the arrow keys drive the same logical keys as WASD.
Storage.SetTable("keybindings", { bindings = { UP = "W", DOWN = "S", LEFT = "A", RIGHT = "D" } })
```

The remap is a platform-layer (windowed) concern: the headless harness and
bot-players inject **logical** keys directly via `Input.Press`/`Input.Release`, so
they bypass the keymap and stay deterministic.
