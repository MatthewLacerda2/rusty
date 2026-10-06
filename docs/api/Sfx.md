## `Sfx` — **dev builds only**

A sentence becomes a one-shot sound effect **WAV** the engine plays (#388): ElevenLabs'
sound-generation endpoint. It covers the recorded-world textures that `Sound.*`'s
synthesis cannot reach: a door with a latch, a hinge and a room, glass, gravel,
cloth, a shell casing on concrete, a magazine seating.

**Reach for the recipe first.** [`Sound.Bake`](Sound.md) is free, offline and
byte-reproducible forever. `Sfx.Generate` costs money every time the words change.
The prompt is the more tempting of the two and the more expensive one, so choose on
purpose:

| | `Sound.Bake` (zimmer) | `Sfx.Generate` |
|---|---|---|
| costs | nothing | money |
| needs | nothing | a key, a network |
| reproducible | byte-identically, forever | best-effort (the cache makes it stick) |
| good at | tonal, synthetic, stylised: UI, energy weapons, sci-fi, music | recorded-world textures: doors, glass, gravel, cloth, mechanisms |
| iteration | change a number, re-bake | rewrite the sentence, pay again |

`Sfx` has its own namespace, separate from `Sound`, because every `Sound` verb is
free. The boundary between the two namespaces is the warning. It is registered only
under the `dev` Cargo feature (a shipped game has no `Sfx` table) and is **refused
during Play**, because generation is authoring. Every call goes through the provider
boundary in [`docs/providers.md`](../providers.md): the key comes from
`ELEVENLABS_API_KEY` (the environment, then `.env`), it is checked against the
project's budget, and every cost is an estimate, never a bill.

| Function | Signature | Returns |
|---|---|---|
| `Sfx.Generate` | `(brief)` | the generated `.wav`'s path; or `nil, why` when the brief is not finished |

```lua
local wav = Sfx.Generate({
  text    = "heavy metal door slamming shut in a concrete stairwell",
  seconds = 2.0,                  -- required: the length is part of the brief
})
Audio.PlayAt(wav, 0, 1.6, 4)
```

There is no output-folder argument. The file's address (`sfx-<sha256>.wav` in the
generated-assets folder, see `docs/providers.md` § Generated assets) *is* the cache:
a file written to a folder the caller picked is one the cache could never find
again. `Speech.Generate` works the same way.

### The brief

| Field | | Meaning |
|---|---|---|
| `text` | required | The sentence describing the sound. |
| `seconds` | required | The length, **0.5 to 30**, the endpoint's own bounds. There is no "let the model choose": a one-shot that has to fit a reload animation has a length, and the model's own guess is neither reproducible nor reliably priced. |
| `prompt_influence` | optional | **0 to 1**, default **0.3** (the vendor's). Higher follows the words more closely and varies less. |
| `loop` | optional | `true` for an effect that loops seamlessly (ambience, a running engine). Default `false`. |

There is **no seed**, because the endpoint takes none. The same brief asked twice
returns the same file because of the cache, not because the vendor repeats itself.
The model is pinned to `eleven_text_to_sound_v2`, the only one the vendor lists, and
it is part of the brief's hash.

### What a call does

1. **Refused during Play**: raises.
2. **Local refusals**: empty `text`, no `seconds`, `seconds` outside 0.5 to 30, or
   `prompt_influence` outside 0 to 1. Each returns `nil, why` naming the field and
   **does not raise**, so a loop over a weapon's twenty sounds loses none to one
   unfinished brief:
   ```lua
   for _, fx in ipairs(rifle_sounds) do
     local wav, why = Sfx.Generate(fx)
     if not wav then Debug.Warn(why) end
   end
   ```
3. **The cache**: a brief already generated returns its path and **calls and spends
   nothing**.
4. **The boundary**: a missing key, or a call that would cross the budget, raises
   with the fix in the message. Nothing is spent.
5. **One call**: the vendor makes the effect and the audio comes back on the same
   request (MP3, `mp3_44100_128`, the one format every tier gets). A vendor refusal
   (a bad key, a missing permission, a plan that doesn't include sound effects, no
   credits) raises with the vendor's own sentence. The estimate is charged to the
   budget the moment the vendor answers.
6. **MP3 → WAV on arrival** (#385): decoded once, encoder delay and padding trimmed,
   and stored as 16-bit PCM WAV with a `.json` sidecar beside it. It is a
   spatialisable one-shot exactly like a bake.

### What it costs

Billed on the **length asked for**, not the words: **0.4¢ a second, rounded up per
call** (a 2 s door slam is 1¢, the 30 s maximum is 12¢). This rate is **unverified
and deliberately high**. On 2026-10-06 the vendor's pricing page couldn't be reached,
and the sources that could be reached disagree (11 credits a second on the API, 40
on the website). rusty uses the highest figure, so the estimate can only run over
and a guess can never cross the budget. The rate and its date live in
`src/dev/providers/sfx/price.rs`, and #388 records the sources.
