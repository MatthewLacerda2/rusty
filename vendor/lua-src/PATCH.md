# lua-src, patched

`mlua`'s `vendored` build compiles Lua from the `lua-src` crate. This is that crate
(version `551.0.2`, its Lua 5.4.9 sources only: rusty builds `lua54`), patched so
that `pairs` and `next` walk a table in an order that is a function of what the VM
ran, as the determinism rule asks of the sim. The root `Cargo.toml`'s
`[patch.crates-io]` points `lua-src` here; the workspace excludes this folder.

Stock Lua places a hash-part key by a hash that changes from run to run:

1. **A string key** hashes with a per-state seed made from the clock and three
   addresses (`luai_makeseed`, `lstate.c`).
2. **A table, function, userdata or coroutine key** hashes by its address, which
   moves with the allocator and ASLR whatever the seed.
3. **A light C function key** (`print`, `math.floor`) hashes by its code address,
   which moves with ASLR.

The patch, each change marked `rusty` in the source:

- `lstate.c`: `luai_makeseed` is a constant. The stock seed guards a server against
  keys chosen to collide; an offline game has no such caller.
- `lobject.h`, `lstate.h`: `Table`, both closures, `Udata`/`Udata0` and `lua_State`
  carry a `keyid`, the object's creation number in its VM
  (`global_State::nextkeyid`, assigned by `luaE_keyid` where each is made:
  `luaH_new`, `luaF_newCclosure`, `luaF_newLclosure`, `luaS_newudata`,
  `lua_newthread`, `lua_newstate`). Closures and userdata keep their size (the field
  fills padding); a table and a coroutine grow by 8 bytes.
- `ltable.c`: `mainposition` hashes an object key by `keyidof`, its `keyid`, and
  every light C function to one slot, so their walk is the same on every run (the collision chain's order, not
  insertion order).

Light userdata still hashes by its pointer: its value is whatever the engine put
there, and `mlua` keys its registry by such pointers on a hot path.

What still differs per run: `tostring` of an object prints its address, and `%p`
in `string.format` too. A script that keys a table by that string walks by address.

**Upgrading `mlua`**: if its `lua-src` version moves, copy the new crate's
`Cargo.toml.orig` (as `Cargo.toml`, without its `[workspace]` and
`[dev-dependencies]`), `src/`, `LICENSE`, `README.md` and the `lua-5.4.*` folder
here, re-apply each `rusty` change, and run
`scripting::tests::walk_order`, which launches a second process and fails when the
two walk a table differently.
