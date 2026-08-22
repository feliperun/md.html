---
title: Meridian API specification
summary: A synthetic technical development spec for the Meridian scheduling service.
lang: en
theme: spec.theme.css
toc:
  depth: 2
sections:
  goals:
    component: cards
  milestones:
    component: timeline
  ownership:
    component: kv
---

# Meridian API

This document specifies the public contract for the Meridian scheduling service. It is the single source of truth for clients, operators, and the platform team.

## Goals

### In scope

- One scheduling API for shifts, coverage, and availability
- Idempotent writes keyed by the `Idempotency-Key` header
- Stable cursor pagination for every list endpoint

### Out of scope

- Payroll export, which keeps its existing contract
- The legacy queue migration, tracked as a separate initiative

## Contract notes

::: kv
- **API version**: 2.1
- **Auth**: bearer token
- **Rate limit**: 600 requests per minute
:::

::: note
The service is read-heavy: expect a 20:1 read-to-write ratio at peak.
:::

::: decision
Dates and times are always UTC; clients convert to local time.
:::

## Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/shifts` | List shifts in a range |
| POST | `/shifts` | Create a shift |
| GET | `/shifts/{id}` | Read one shift |
| PATCH | `/shifts/{id}` | Update a shift |

## Errors

Every error uses a stable envelope:

```json
{ "code": "string", "message": "string", "requestId": "string" }
```

The `code` field is stable and documented; clients must not parse `message`.

## Milestones

1. Freeze the API contract for review
2. Complete the load-test pass
3. Enable the beta cohort
4. Cut over the remaining traffic

## Rollout

::: steps
1. Deploy the API behind the feature flag.
2. Route ten percent of read traffic.
3. Watch the error budget for one week.
4. Lift the flag for all regions.
:::

## Ownership

- **API owner**: Priya Raman
- **Client library**: Tom Okonkwo
- **Operations**: Ana Souza

## References

- [Service architecture](https://example.test/meridian/architecture)
- [Error code catalog](https://example.test/meridian/errors)
