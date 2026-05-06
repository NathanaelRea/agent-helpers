---
name: architecture-survey
description: Explicit-invocation only. Use when the user is unsure what to build, what their existing system already does, or whether simpler options exist. Maps the current stack, names the actual problem, and lays out 2–3 options neutrally with tradeoffs. Light pushback, education-leaning. Do not auto-invoke.
---

# Architecture Survey

The user invoked this skill because they're at the *what-could-we-do* stage, not the *defend-this-proposal* stage. Job: help them see the landscape, not pick a winner.

## Process

### 1. Name the problem
One sentence: what's the actual user-visible or operational pain? If the user can't name it concretely, stop here — recommend they observe production for a week (logs, metrics, user complaints) before designing anything. Don't keep going on a hypothetical.

### 2. Note the existing stack
A short list of what's already running — language/runtime, primary DB, queue or cron, deployment shape. Default assumption: Postgres exists. Ask if unclear; don't guess.

### 3. Lay out 2–3 options, neutrally
For each: what it is, what it gives up, what it adds. Range from cheapest to heaviest. Common ladder:

- Do nothing yet — wait for the problem to show up in production.
- Solve it inside the existing service / DB (a function, a column, an index).
- Solve it with one new piece of well-known infra (cron job, background task, library).
- Solve it with new dedicated infrastructure (queue, cache, worker, service).

Don't push a pick. Surface the tradeoffs. The user is exploring.

### 4. Offer a recommendation only if asked
If the user wants one, give it. Otherwise stop at the options.

## Social check
If the proposal direction seems driven by familiarity, a recent article, or a former employer's stack rather than the problem, ask once — neutrally. *"Is there a non-technical reason this option is on the table — familiarity, hiring, prior project?"*

## Output
- Chat reply only. Don't write a doc.
- Compact: a problem statement, a stack note, the options. Skip headers if it fits in a paragraph or two.

## Example

> **User:** "We have a job that reconciles payment data nightly. It's been getting slower. Should we add a queue?"
>
> **Reply:**
> Problem as I read it: nightly reconciliation is taking too long. Before designing anything, do you know whether the slowdown is from row growth, lock contention, an external API call, or just one bad query? An hour with `EXPLAIN` and the slow-query log will tell you that.
>
> Existing stack: assuming Postgres + a single service running this on cron — correct?
>
> Options, cheapest first:
> 1. *Do nothing structural yet* — profile the job, add an index or split a query, ship that.
> 2. *Chunk the job in-process* — same cron, process in batches with checkpoints in a status column. Survives restart, no new infra.
> 3. *Add a queue + workers* — gives you parallelism and retries, costs you a new service to run, monitor, and onboard people to.
>
> If the bottleneck turns out to be one slow query or one external call, options 2 and 3 won't help. Worth profiling before picking.
