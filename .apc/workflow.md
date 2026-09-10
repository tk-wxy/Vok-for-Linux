# Development Protocol

Use this file as guidance, not ceremony. Real code and verified
results override stale notes.

## Trust

| File | Trust and use |
|---|---|
| `manifest.md` | High after user confirmation; project anchor |
| `rules.md` | High only while evidence, scope, and status match |
| `decisions.md` | High within the recorded scope |
| `memory.md §0` | Low; use as a lead and verify |
| `history.md` | Forensics only |

## 0. Cold start

On first contact, model/tool switch, or return after a long gap:

1. Read `manifest.md`.
2. Read `memory.md`.
3. Scan the `rules.md` index and dead ends; read high-risk or relevant
   rules in full.
4. Scan the `decisions.md` table of contents; open relevant decisions
   only.
5. Read this workflow.

You should now know the project, hard constraints, current stage, and
next step.

## 1. Session start

1. Read `memory.md §0` and verify any detail you will rely on.
2. Use the rules index for relevant modules.
3. Open decisions only for needed root causes; search history only for
   forensics.

## 2. Execute a task

1. Derive intent, boundaries, and acceptance criteria from the request
   and repository. Ask only if material ambiguity remains.
2. Identify affected modules and manifest high-risk zones.
3. Check relevant dead ends and decisions before choosing an approach.
4. Diagnose before editing. In high-risk zones, change one variable at
   a time. Keep tunable values named.
5. Verify with tests, build, or execution. Label unavailable paths
   `unverified`.
6. If repeated attempts fail at the same point, stop, roll back when
   appropriate, and reassess.

## 3. Task variants

- **Feature**: preserve invariants; implement in small verifiable steps.
- **Bug**: reproduce and trace the causal chain to the actual mechanism
  (not just the nearest upstream event). Fix the mechanism, not the
  visible symptom.

  Reject fixes that suppress an effect without addressing its cause —
  e.g. hardcoding visual properties, swallowing errors, adding delays
  without understanding the timing issue, or gating around broken logic.
  If the fix doesn't explain *why* the bug occurred, it's not done.

  Verify: (1) the original mechanism no longer triggers, not just that
  it's no longer visible, and (2) the change hasn't relocated or
  disguised the failure elsewhere.
- **Review**: check relevant rules and high-risk changes; report before
  mutating unless the user requested fixes.
- **Refactor**: require adequate tests; separate behavior changes; verify
  after each step.

## 4. Task end

First classify the result:

- **No reusable knowledge or material state change**: no-op; do not
  modify `.apc/`.
- **Material change**: update only the applicable files below.

Updates:

1. `rules.md`: add only reproduced/evidenced pitfalls or dead ends.
   Include scope, evidence, verification date, status, index, and
   symptom lookup.
2. `decisions.md`: add only adopted or user-confirmed decisions.
   Include scope, evidence, rejected alternatives, date, and status.
3. `memory.md §0`: overwrite only when current state, next step, or
   known issues materially changed.
4. `memory.md §0A`: keep at most three sessions; move the oldest whole
   entry to `history.md`.

Each fact has one home: identity in manifest, scoped constraints in
rules, root causes in decisions, current state in memory, archives in
history. Use pointers elsewhere. Never invent an entry to satisfy this
protocol.

## 5. Working principles

- Evidence before authority.
- Source and verified results beat notes.
- A verified simple approach beats an unverified clever one.
- Keep knowledge concise; store conclusions and pointers, not logs or
  code dumps.
- Inspect before asking; ask when ambiguity materially changes the
  result.
- Stay aligned with the manifest.
