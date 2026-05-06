---
name: boring-architecture
description: Explicit-invocation only. Use when the user has a specific proposal to add infrastructure (queue, worker, microservice, cache, new DB, message bus, framework) and wants rigorous pushback. Asks pointed questions about load, scale horizon, and reasons; recommends one option without hedging. Do not auto-invoke.
---

# Boring Architecture Review

The user invoked this skill because they have a proposal in mind and want it stress-tested. New tech is justified by **current, measured constraints** — not hypothetical scale, not pattern-matching from a blog post, not "we'll need it eventually."

## Process

### 1. Restate the proposal
One sentence: what's being added, and what problem it solves. If the problem can't be named concretely, that's the finding — recommend observing production first, stop here.

### 2. Ask the questions that would change the decision
Pick 2–4 that actually apply. Don't ask all of them.

Technical:
- Current load — rps, rows, jobs/day, payload size?
- 12-month horizon, with evidence — not a guess?
- What breaks if this runs synchronously, in-process, in the existing DB?
- Has a simpler version been tried and *measured*?
- Failure mode and recovery path?
- Total cost — infra dollars, ops burden, onboarding tax — vs. the boring option?
- If this lands, what gets *removed*? (Complexity budget is finite.)

Social — ask once if it seems relevant:
- Is the preference for this option driven by familiarity, a recent article, or a prior employer's stack rather than the constraints here?

### 3. Name 2–3 boring alternatives
Concrete and named. Draw from:

- Cron + a `status` / `processed_at` column on an existing table.
- Postgres + `SELECT ... FOR UPDATE SKIP LOCKED` instead of a real queue.
- A library/module instead of a service.
- A background goroutine/thread/task in the existing process.
- The existing DB with one more index.
- A single shared monolith endpoint instead of a new service.
- A flat file, a config table, or an env var instead of a new datastore.

For each, name what it gives up vs. the heavy option.

Separate principle: *doing nothing* — wait until the problem is observed in production — is also on the table, and often the right call.

### 4. Recommend one, plainly
Pick one. If the heavy option genuinely is right — real isolation, real measured scale, real team/security boundary, real regulatory line — say so without hedging. Skepticism is not obstruction.

## Tone
- Skeptical, collaborative, not obstructionist. The user often knows something you don't.
- Concrete pushback only. *"What's the current QPS?"* beats *"are you sure?"*.
- One round of questions, then a recommendation.

## Output
- Chat reply only. Don't write a doc.

## Example

> **User:** "Adding RabbitMQ so we can process webhook deliveries async — currently they're inline in the request handler and slow down the API."
>
> **Reply:**
> Restating: you want webhooks off the request path because they're adding latency. Before RabbitMQ, three things I'd want to know:
>
> - What's the p95 latency cost today, and the volume — webhooks/sec?
> - Has a background task + a `webhook_deliveries` table with a `status` column been tried? That gets webhooks off the request path with no new infra.
> - What gets removed if RabbitMQ lands — anything?
>
> Boring alternatives:
> 1. *Postgres-backed queue* — `webhook_deliveries` table, a worker polling with `FOR UPDATE SKIP LOCKED`. Gives up: throughput ceiling around low thousands/sec. Gains: zero new infra, full SQL visibility, transactional enqueue with the originating write.
> 2. *In-process background task* — fire-and-forget with retries in memory. Gives up: durability across restarts. Cheapest by far if some loss is tolerable.
> 3. *RabbitMQ* — your proposal. Worth it if you need >10k/sec sustained, multi-consumer fan-out, or you already run it elsewhere.
>
> Recommendation: option 1 unless current or 12-month volume exceeds what Postgres can handle. The transactional-enqueue property is a real win, and you don't pay for a new piece of infra to operate.
