## `Timer`

Timers and coroutines — Unity's `Invoke` / `InvokeRepeating` and `StartCoroutine`
with `WaitForSeconds` & co. (#444), so a reload, a burst, a respawn delay or an
AI "wait, then peek" is one line instead of a hand-rolled `self.t = self.t - dt`
countdown. A namespace (not bare globals, as Unity's instance methods would
suggest) so it is one more entry in the one API surface: scripts, the console
REPL and bot-players reach it identically.

Every timer and coroutine is **owned by an entity** (`id`) and gets an integer
**handle**. Starting one on a missing or inactive entity is an error.

| Function | Signature | Returns |
|---|---|---|
| `Timer.Invoke` | `(id, fn_or_name, delay)` — call `fn(id)` once after `delay` seconds of **scaled** time. `fn_or_name` is a function, or the name of a function on the entity's scripts (looked up when it fires, first script index first — Unity's `Invoke("Reload", 1.5)`). | handle |
| `Timer.InvokeRepeating` | `(id, fn_or_name, delay, interval)` — like `Invoke`, then again every `interval` seconds (`> 0`, else an error) until cancelled. Deadlines are kept drift-free: time past one deadline counts toward the next. | handle |
| `Timer.CancelInvoke` | `(id [, name])` — cancel the entity's invokes: all of them, or only those started with `name`. Coroutines are untouched. | — |
| `Timer.IsInvoking` | `(id [, name])` | `true` while the entity has a pending invoke (started with `name`, if given) |
| `Timer.StartCoroutine` | `(id, fn, ...)` — run `fn(id, ...)` as a coroutine **right away, up to its first yield** (Unity's contract), then resume it whenever what it yielded is satisfied. | handle |
| `Timer.StopAllCoroutines` | `(id)` — stop the entity's coroutines. Invokes are untouched. | — |
| `Timer.Cancel` | `(handle)` — cancel one invoke or coroutine (Unity's `StopCoroutine`, for either kind). | `true` if it was pending |
| `Timer.IsPending` | `(handle)` | `true` while that invoke / coroutine has not finished, failed or been cancelled |
| `Timer.WaitForSeconds` | `(t)` — yield instruction: resume after `t` seconds of **scaled** time (`Time.SetTimeScale(0)` holds it). | instruction |
| `Timer.WaitForSecondsRealtime` | `(t)` — yield instruction: resume after `t` seconds of **unscaled** time (keeps running through a time-scale pause). | instruction |
| `Timer.WaitForFixedUpdate` | `()` — yield instruction: resume on the next fixed tick (same as a bare `coroutine.yield()`). | instruction |
| `Timer.WaitUntil` | `(fn)` — yield instruction: resume on the first tick `fn()` returns a truthy value (asked once per tick). | instruction |

A coroutine is a plain Lua function that yields an instruction — Unity's
`yield return new WaitForSeconds(0.5)`:

```lua
function Gun.Reload(id)
    Timer.StartCoroutine(id, function(id)
        Animator.Play(id, "reload")
        coroutine.yield(Timer.WaitForSeconds(1.5))   -- scaled: slow-mo slows the reload
        Gun.ammo = Gun.clip
        coroutine.yield()                            -- bare yield: next tick
        Gun.ready = true
    end)
end
```

**When it runs — deterministic, in the fixed step.** Timers and coroutines are
stepped once per `FixedUpdate` tick, in the script phase **after every `Update`
and before physics and `LateUpdate`**, in ascending `(owner entity id, handle)`
order (handles are handed out in start order). Their clock is the tick's own:
the scaled `dt` (`Time.deltaTime`) for `Invoke`/`WaitForSeconds`, the unscaled
fixed step for `WaitForSecondsRealtime` — **never the wall clock**, so a harness
replay fires every timer on the same tick. Work never runs in the tick it was
scheduled: a delay `d` started on tick `N` fires on tick `N + max(1, ceil(d / dt))`
(so `Invoke(id, f, 0)` and a bare yield both mean "next tick"). `WaitForFixedUpdate`
therefore resumes in the next tick's timer phase — rusty has one fixed tick, not
Unity's separate render frame and physics step.

**Lifetime.** Timers and coroutines stop — for good, not paused — when their
entity is **deactivated** or **destroyed** (Unity stops coroutines on
deactivate; rusty applies the same rule to invokes). Re-enabling the entity does
not revive them: restart them from `OnEnable`. A new VM (every Play) starts with
none, and they are **not scene data**: Stop restores the edit snapshot, which has
no pending timers.

**Errors.** An error inside an invoked function is logged like an `Update` error
(`[Lua Error] Invoke on entity N failed: …`); a repeating invoke keeps repeating.
An error inside a coroutine — or a yield of anything but `nil` or a `Timer.Wait*`
instruction — logs `[Lua Error] Coroutine on entity N failed: …` and ends that
coroutine. Nested `yield StartCoroutine(...)` (waiting on another coroutine) is
not supported; use `Timer.WaitUntil(function() return not Timer.IsPending(h) end)`.

> A coroutine resumes in a later script evaluation, where the namespaces are
> registered afresh — so look namespaces up by name after a yield (`Transform.…`),
> never through a local cached before it (`local T = Transform`).
