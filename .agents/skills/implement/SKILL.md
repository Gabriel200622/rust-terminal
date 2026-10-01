---
name: implement
description: "Implement a piece of work based on a spec or set of tickets."
disable-model-invocation: true
---

Implement the work described by the user in the spec or tickets.

Use /tdd where possible, at pre-agreed seams.

Follow the repository's verification policy: use affected package/target checks
and focused behavior tests. CI owns the full suite; run repo-wide checks locally
only when explicitly requested.

Once done, use /code-review to review the work.

Commit your work to the current branch.
