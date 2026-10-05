# Providers: bringing a key, and the budget

Some authoring verbs ask a third-party vendor to generate an asset (ElevenLabs for
voice lines, for example). Those calls live behind one dev-only boundary,
`src/dev/providers/` (#383), and this page is the part a person does by hand.

A shipped game never makes a provider call. It plays the files that were generated
while authoring, the same way it uses baked lighting.

## Bringing a key

rusty never ships or shares an account. You bring your own key:

1. Copy `.env.example` to `.env` at the workspace root (the folder holding
   `project/`). `.env` is gitignored.
2. Fill in the variable, e.g. `ELEVENLABS_API_KEY=sk_...`.

An exported environment variable wins over `.env`. The file only fills gaps, so
a key exported in your shell is the one used. When no key is found, the refusal
names the variable and every place that was checked.

## The budget

Each project keeps a budget and a running total of what it has spent, in
`project/provider_budget.json` (gitignored, created on the first call):

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

What a provider makes lands in `project/assets/generated/`, named for **the hash
of the brief that made it** (#384):

```
project/assets/generated/<kind>-<sha256 of the brief>.<ext>
project/assets/generated/<kind>-<sha256 of the brief>.<ext>.json
```

A *brief* is everything a generation asks for: the prompt, the voice, the model,
the duration, the seed. It lives in the document that uses the asset, so it is
versioned with the project. The folder itself is gitignored, because any file in it
can be made again from its brief (at a cost).

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
