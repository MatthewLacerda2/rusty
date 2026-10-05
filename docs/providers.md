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
