---
title: Felipe R. Broering — Lead Engineer, AI Engineering
summary: Engineering leader and full-stack builder specializing in AI engineering — coding agents on rails, agentic delivery harnesses, and production-grade systems.
lang: en
theme: felipe-cv.theme.css
toc:
  depth: 2
sections:
  open-source:
    component: cards
---

# Felipe R. Broering

Lead Engineer · AI Engineering — Florianópolis, SC, Brazil

::: kv
- **Email**: hi@felipe.run
- **LinkedIn**: [linkedin.com/in/felipebroering](https://www.linkedin.com/in/felipebroering/)
- **GitHub**: [github.com/feliperun](https://github.com/feliperun)
:::

## Profile

Engineer and engineering leader with nearly two decades building complex digital products end to end: discovery, domain modeling, architecture, implementation, and production. Over the last few years I have been rebuilding *how that work gets done* with AI. I design the harnesses, guardrails, and agent workflows that let a team ship with coding agents at production quality: fast, reviewable, and accountable, not a pile of AI-generated code nobody trusts.

I work best where product and engineering can't be separated: complex workflows, operational systems, regulated domains, cloud-native platforms, and automation that has to stay reliable in production. My background spans full-stack engineering, cloud architecture, product management, and hardware integration, across roles as software engineer, tech lead, CTO/founder, engineering manager, and head of engineering.

I've led platforms in critical operations, including Coreum at Micromed, featured by Google Cloud. I also build open-source, terminal-first AI tooling: agents on rails, local-first systems, and the engineering harnesses that keep agent-assisted changes production-ready.

## AI engineering

I don't just use AI tools, I engineer the system around them. The principle behind everything I ship: **rules first, AI second.** Put the model on rails so velocity never costs you architecture, correctness, or accountability.

::: kv
- **Agentic delivery harness**: Per-repo `AGENTS.md` playbooks, ADRs for every structural decision, TDD, structural quality gates, and pre-commit + CI, so agent-written diffs stay small, scoped, and reviewable. Never `--no-verify`.
- **Reusable skills**: A library of skills — Figma-to-code, code review, release validation, production investigation, doc sync, technical planning, TDD enforcement — so recurring work becomes one repeatable command.
- **Autonomous CI / merge loops**: Pipelines that open a PR, watch CI, fix failures, apply review-bot suggestions, and merge on green.
- **Multi-agent orchestration**: Fan-out/verify workflows and specialized subagents, with adversarial verification before any finding is trusted.
- **Model-neutral by design**: LLM-agnostic tooling across Claude, Gemini, and local models, wired through MCP servers.
- **Terminal-first**: Claude Code and Cursor CLI, structured prompts, local harnesses, and MCP integrations plugged straight into the dev and production feedback loop.
:::

## Featured case study

**Coreum on Google Cloud: scaling a cloud-native platform.** Led Coreum from zero into Micromed's cloud-native platform: scalable workflows, AI features, ERP integration, and a production foundation built for fast, safe releases — it powers cardiology diagnostics. [Published by Google Cloud.](https://cloud.google.com/customers/micromed)

::: stats
| Metric | Result |
| --- | --- |
| Infrastructure cost | -50%+ |
| Exam load time | 2s → 200ms |
| Migration | 20TB, 3h downtime |
| Releases | Multiple daily, zero downtime |
| Daily workloads | 10,000+ |
:::

## How I build

I treat product engineering as a full-cycle discipline. My preferred loop:

> Problem → Domain model → Product behavior → Architecture → Implementation → Production → Observability → Iteration.

I start close to the real problem — users, workflows, business constraints, operational risks, edge cases, success criteria — then model the domain, design the system, build it, ship it, watch how it behaves in production, and feed what I learn back in. AI makes each pass faster; the discipline is what keeps it production-grade.

## Open source

### phai — AI on rails for personal finance

Founder and maintainer of a rules-first, LLM-neutral personal-finance agent. Rules decide; the model assists, never the other way around. Ingests open-finance data (Pluggy), normalizes into SQLite or BigQuery, with MCP integration and a local web app embedded in a single Rust binary.

Rust, SQLite, BigQuery, Pluggy, MCP · [github.com/feliperun/phai](https://github.com/feliperun/phai)

### cueme — local-first second brain with a live conversation copilot

A file-first personal knowledge product for macOS. Every note is a normal folder on disk; SQLite, FTS5, and `sqlite-vec` embeddings are rebuildable indexes, never the source of truth. A real-time conversation copilot captures both sides of a call natively, transcribes and translates on-device, and streams live guidance grounded in local semantic retrieval.

Swift 6, Claude Code CLI, ScreenCaptureKit, sqlite-vec · [github.com/feliperun/cueme](https://github.com/feliperun/cueme)

### ford — autonomous agent, hardened cloud deploy

One command that provisions a production-grade, self-hosted AI agent to the cloud: a Claude Sonnet gateway with persistent memory, a headless browser, and a WhatsApp channel, on Terraform infrastructure with defense-in-depth — no external IP, egress firewall, secrets never on disk, read-only hardened containers.

OpenTofu, Google Cloud, Docker, Mem0/Qdrant · [github.com/feliperun/create-openclaw-agent](https://github.com/feliperun/create-openclaw-agent)

### eai — natural language to safe shell commands

Rust CLI that turns natural language into safe shell commands through an inspect-confirm-run flow, multi-provider across Ollama, Gemini, Groq, and OpenAI.

Rust, multi-provider LLM · [github.com/feliperun/eai](https://github.com/feliperun/eai)

### dsync — Markdown as the source of truth

Rust CLI that keeps Markdown as the local source of truth while syncing documents with Google Docs and Linear Docs.

Rust · [github.com/feliperun/dsync](https://github.com/feliperun/dsync)

## Experience

### Senior Product Engineer — Micromed Health

Florianópolis, Brazil — Jan 2026 to Present

After three and a half years leading engineering, I chose to return to deep hands-on work: the AI-native way of building I'd been driving as a leader is now what I do all day.

- Designing and operating the team's AI engineering harness: coding agents on rails, reusable skills, ADRs, TDD, quality gates, code review, and autonomous CI/merge loops.
- Building reusable engineering skills so recurring work runs as repeatable, inspectable commands.
- Leading product engineering initiatives from Figma through implementation, validation, release, and production iteration.
- Building web experiences with React, Node.js, and TypeScript; Python for ECG signal processing; Rust as the native integration layer between medical hardware, local PCs, and cloud.

### Head of Engineering — Micromed Health

Florianópolis, Brazil — May 2022 to Dec 2025

Led engineering for Coreum, Micromed's cloud-native platform for cardiology diagnostics, exam workflows, AI-assisted analysis, ERP integration, connected devices, and production operations.

- Led Coreum from early product development into a Google Cloud customer case study platform.
- Drove the modernization program behind the Google Cloud case study results: infrastructure cost, exam loading latency, the 20TB migration, and the move to multiple daily zero-downtime releases.
- Structured engineering process and culture to support team growth, new projects, faster delivery, and more reliable production operations.
- Led and supported multidisciplinary teams across backend, frontend, UX, cloud, firmware, electronics, QA, integrations, and product delivery.

### Engineering Manager — Micromed Health

Florianópolis, Brazil — Nov 2020 to Jun 2022

Joined Micromed to lead engineering from scratch for an inpatient monitoring solution combining ECG acquisition, cloud processing, machine learning algorithms, and cardiac risk prediction.

- Built and led an internal team of seven, responsible for platform core, back ends, front ends, design system, and cloud infrastructure.
- Coordinated ten third-party engineers across AI microservices, firmware, and electronics.
- Owned system design and led end-to-end development across software, cloud, ML integration, hardware integration, and product execution.

### Engineering Manager — Animati

Florianópolis, Brazil — Jun 2019 to Oct 2020

Led development of S.I.M., an integrated diagnostic medicine system and cloud-native SaaS designed to fill RIS/PACS workflow gaps.

- Managed the full product development cycle from problem discovery and software architecture to customer delivery.
- Built a team from scratch working with React, TypeScript, Python, Django REST, Node.js, Firebase, and serverless architecture.
- Introduced design-system thinking, detailed UX modeling, micro-frontends, and cost-efficient cloud patterns.

### Product Manager: Healthcare, then Construction — Softplan

Florianópolis, Brazil — Mar 2018 to May 2019

- Led product discovery for a new healthcare business unit, evaluating build/buy/partner paths and presenting strategic options to the CEO.
- Managed CRM and commercial modules for Sienge, a construction ERP used across Brazil.

### Chief Technology Officer & Founder — Healfies

Florianópolis, Brazil — Jan 2015 to Mar 2018

Founded and led technology for a healthcare network where people and organizations could securely organize and share information.

- Owned platform system design, product roadmap, technical strategy, and team building from scratch, leading seven people.
- Designed and operated AWS and Google Cloud infrastructure: microservices, storage, functions, Kubernetes, cost management.
- Lived the full startup cycle, helping raise R$2.2M, connect 23 diagnostic centers, deliver 2M records, and register 10K users.

### Tech Lead — Chaordic

Florianópolis, Brazil — Jan 2014 to May 2015

Led delivery and integration of a personalization platform for major Brazilian e-commerce brands.

- Acted as PM and technical lead for eight engineers across JavaScript, UX/frontend, and QA.
- Introduced OKRs and a technical account management function, cutting integration time by 50% and reducing churn among top accounts.

### Product Manager — Pixeon Medical Systems

Florianópolis / São Paulo, Brazil — Oct 2010 to Jan 2014

- Owned the LIS/RIS/PACS product portfolio, translating market and sales needs into engineering roadmaps.
- Led the full cycle for six new products, from problem definition through regulatory registration and go-to-market.

### Full-stack Engineer — Pixeon Medical Systems

Brazil — Jan 2007 to Oct 2010

- Built a desktop DICOM viewer for CT, MRI, ultrasound, and CR imaging with C++, Qt, and Java, under Scrum and CI.

## Expertise

AI Engineering, Agentic Workflows & Multi-Agent Orchestration, LLM Tooling & MCP, Prompt & Skill Design, Product Engineering, Full-Stack Engineering, System Design, Cloud-Native Architecture, Developer Tools & CLIs, Local-First Systems, Release Engineering, Zero-downtime Deployments, Engineering Leadership, Google Cloud, AWS, Interoperability (DICOM / HL7 / FHIR).

## Education & languages

::: kv
- **Education**: BS in Computer Engineering, Universidade do Vale do Itajaí (2001–2005)
- **Portuguese**: Native or bilingual proficiency
- **English**: Professional working proficiency
- **Spanish**: Basic to intermediate proficiency
:::
