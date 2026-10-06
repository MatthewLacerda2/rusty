# Providers: bringing a key, and the budget

Some authoring verbs ask a third-party vendor to generate an asset (ElevenLabs for
voice lines, for example). Those calls live behind one dev-only boundary,
`src/dev/providers/` (#383), and this page is the part a person does by hand.

A shipped game never makes a provider call. It plays the files that were generated
while authoring, the same way it uses baked lighting.

## Bringing a key

rusty never ships or shares an account. You bring your own key:

1. Copy `.env.example` to `.env` at your game project's root, or any folder above
   it (the engine checkout, when the project is its local `./project`). Keep `.env`
   out of git: the engine's `.gitignore` covers it, and a game repo should ignore it
   too.
2. Fill in the variable, e.g. `ELEVENLABS_API_KEY=sk_...`.

An exported environment variable wins over `.env`. The file only fills gaps, so
a key exported in your shell is the one used. When no key is found, the refusal
names the variable and every place that was checked.

## The budget

Each project keeps a budget and a running total of what it has spent, in
`provider_budget.json` at the project root (#829), created on the first call. It is
the project's own setting, not regenerable output, so it sits at the root rather
than under `cache/`:

```json
{
  "budget_cents": 500,
  "spent_cents": 0
}
```

A project with no file starts at **500 cents (US$5)**. A call that would take
`spent_cents` past `budget_cents` is refused before it goes out, and the refusal
names the estimate, what was already spent, the budget and how far over it would
go. To go on, raise `budget_cents`. Nothing else can override it, because it exists
for agents that run unattended.

Every figure is an **estimate**: rusty's arithmetic over a dated price table copied
from the vendor's pricing page. Vendors don't report what a call cost, so check
your vendor dashboard for the real bill.

## The rules

- **Dev builds only.** The module and its HTTP client compile only with the `dev`
  feature.
- **Edit mode only.** Provider verbs are refused during Play. Generation is
  authoring, and a network call inside the sim would break the determinism the
  harness depends on.
- **No test makes a real call.** Every provider sits behind a trait, and tests use
  mocks.

## Generated assets

What a provider makes lands in `assets/generated/`, named for **the hash
of the brief that made it** (#384):

```
assets/generated/<kind>-<sha256 of the brief>.<ext>
assets/generated/<kind>-<sha256 of the brief>.<ext>.json
```

A *brief* is everything a generation asks for: the prompt, the voice, the model,
the duration, the seed. It lives in the document that uses the asset, so it is
versioned with the project. Any file in the folder can be made again from its brief
(at a cost). Whether a game's own repo versions the folder or ignores it is still
open (#866): today nothing ignores it.

- **Nothing is paid for twice.** Asking for a brief whose file is already there
  returns that file and makes no call. Editing a prompt back to an earlier wording
  finds the earlier file again.
- **The hash is of the brief, not the output.** The brief is serialised
  canonically before it is hashed: keys sorted, and absent and `null` fields
  left out. A re-saved brief, or one whose type gained an optional field since,
  keeps its address.
- **The sidecar** (`<file>.json`) records what made the file: the brief in that
  canonical form, its kind, the UTC day it was generated, and the estimated cost in
  cents. It never records a key or an account. Hashing its `kind` and `brief`
  gives back the digest in the file name.

Every generated asset is in one of three states:

| State | Meaning |
|---|---|
| `sketch` | A brief with nothing generated yet. It plays silence and is not an error: it's the normal state of a line someone is still writing. |
| `generated` | The file for this exact brief exists. |
| `stale` | A file exists for an earlier version of the brief, but not for this one. It is **not** regenerated on its own. The agent decides when to spend. |

## The verbs

| Verb | Vendor | Makes | Priced by |
|---|---|---|---|
| [`Speech.Generate`](api/Speech.md) (#386) | ElevenLabs text-to-speech | `speech-<hash>.wav` | characters × the model's rate (Flash 5¢ / 1000, multilingual v2 and v3 10¢), rounded up |
| [`Sfx.Generate`](api/Sfx.md) (#388) | ElevenLabs sound generation | `sfx-<hash>.wav` | requested seconds × 0.4¢, rounded up (rusty's own rate, unverified and deliberately high: see the page) |

Rates are scorsese's dated tables (`scorsese_providers::prices`), read off the
vendor's page on the date they carry; moving the scorsese pin is how they change.
The one exception is `Sfx`: scorsese has no sound-effects table, so its dated rate
lives in `src/dev/providers/sfx/price.rs`.
