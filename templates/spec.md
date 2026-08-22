---
title: Widget API specification
summary: A specification template demonstrating the canonical mdhtml structure.
lang: en
theme: technical
toc:
  depth: 2
---

# Widget API

This document specifies the public contract for the Widget service.

## Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/widgets` | List widgets |
| POST | `/widgets` | Create a widget |
| GET | `/widgets/{id}` | Read one widget |

## Errors

All errors use a stable envelope:

```json
{ "code": "string", "message": "string" }
```

## Versioning

- Breaking changes bump the major version
- Additive changes bump the minor version
- The `Accept` header selects the requested version

> This contract is frozen for the 1.0 release.
