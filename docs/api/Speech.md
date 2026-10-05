## `Speech` — **dev builds only**

A line of text becomes a spoken **WAV** the engine plays (#386). Voice is the one
sound `Sound.*` can never make — zimmer synthesises oscillators and noise, not
phonemes — so enemy barks and dialogue come from ElevenLabs text-to-speech.
`Speech` is named for the capability, not the vendor, and stays separate from
`Sound`, which is free and offline: this namespace is the one that spends money.

Registered only under the `dev` Cargo feature (a shipped game has no `Speech` table)
and **refused during Play**: generation is authoring, never gameplay. Every call goes
through the provider boundary in [`docs/providers.md`](../providers.md): the key from
`ELEVENLABS_API_KEY` (environment, then `.env`), the project's budget, estimates
never bills.

| Function | Signature | Returns |
|---|---|---|
| `Speech.Generate` | `(brief)` | the generated `.wav`'s path; or `nil, why` when the brief is not finished |

```lua
local wav = Speech.Generate({
  text  = "Contact, second floor.",
  voice = "<voice id>",           -- required: there is no default voice
  model = "eleven_flash_v2_5",    -- optional; this is the default
})
Audio.PlayAt(wav, 0, 1.6, 4)
```

### The brief

| Field | | Meaning |
|---|---|---|
| `text` | required | The words. At most 40,000 characters. |
| `voice` | required | An ElevenLabs voice id. **No default exists**: every vendor default voice expires on 2026-12-31. |
| `model` | optional | `eleven_flash_v2_5` (**default**, 5¢ / 1000 chars), `eleven_multilingual_v2` (10¢), `eleven_v3` (10¢, most expressive). |
| `language` | optional | ISO 639-1 code pinning the reading. Refused on `eleven_multilingual_v2`, which accepts it and silently ignores it. |
| `seed` | optional | Integer. **Best-effort, not guaranteed**: the one place in the engine where "same input, same bytes" is a hope rather than a promise. |

### What a call does

1. **Refused during Play** — raises.
2. **Local refusals** — empty `text`, more than 40,000 characters, no `voice`, an
   unknown `model`, `language` on `eleven_multilingual_v2`. Each returns
   `nil, why` naming the field, and **does not raise**, so a loop over twenty lines
   loses none to one unfinished brief:
   ```lua
   for _, line in ipairs(barks) do
     local wav, why = Speech.Generate(line)
     if not wav then Debug.Warn(why) end
   end
   ```
3. **The cache** — the file is addressed by the hash of the brief
   (`speech-<sha256>.wav` in the generated-assets folder, see `docs/providers.md`
   § Generated assets). A brief already generated returns its path and **calls and
   spends nothing**. That is also what makes a line reproducible despite the
   best-effort seed: ask again, get the same file.
4. **The boundary** — a missing key or a call that would cross the budget raises,
   with the fix in the message. Nothing is spent.
5. **One call** — the vendor speaks the line and the audio comes back on the same
   request (MP3, `mp3_44100_128`, the one format every tier gets). A vendor refusal
   (a bad key, a key missing a permission, a voice withdrawn, no credits) raises with
   the vendor's own sentence. Its estimate (characters × the model's dated rate,
   rounded up) is charged to the budget the moment the vendor answers.
6. **MP3 → WAV on arrival** (#385): decoded once, encoder delay and padding trimmed,
   stored as 16-bit PCM WAV with a `.json` sidecar beside it. The result plays,
   loops gaplessly, and works with `Sound.Level` like any other WAV.

No GUI: generation is a verb an agent calls (over MCP it is one `eval`); the WAV it
leaves behind is an ordinary audio asset with its own component and inspector.
