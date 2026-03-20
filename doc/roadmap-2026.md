# Floresta Technical Strategy & Roadmap 2026

## Governance & Scope

This document reflects the current technical direction proposed by the Floresta core team. We value quality and transparency during our process and this will be the primary place to follow all updates regarding our roadmap.

- The roadmap is **public and open for feedback**
- Direction and prioritization are **curated and approved by the Floresta core team**
- External contributors are encouraged to:
  - Provide feedback
  - Propose ideas
  - Make contributions aligned with the roadmap and strategic themes

## Vision

Floresta is a lightweight and embeddable Bitcoin client designed for applications and users that require strong validation guarantees but cannot accommodate the operational overhead of traditional full nodes.

Rather than a monolithic design, Floresta is built as a **composable system**.

Applications integrate through well-defined interfaces instead of modifying or forking the codebase.

Modularity is validated through real integrations with ecosystem tools.

Floresta aims to be a **production-grade alternative to Bitcoin Core** for lightweight, application-focused use cases, while maintaining strong guarantees when it comes to consensus and security.

## Strategic Themes

All roadmap initiatives are guided by the following six strategic themes:

1. **Reliability & Security**
2. **Sync & Transaction Relay**
3. **Bitcoin Ecosystem Support**
4. **Modular Architecture**
5. **Testing & Validation**
6. **Community Adoption & User Experience**

## 2026 Roadmap

> This section reflects the current priorities identified for each strategic theme to be worked on throughout 2026, and can be refined as work progresses.
>
> Workstreams and quarterly efforts addressing these initiatives are detailed on the [Project Board](https://github.com/orgs/getfloresta/projects/2).

### Reliability & Security

- Ensure trustworthy runtime behavior
- Maintain production-quality execution
- Strengthen operational robustness
- Improve resilience to network-level attacks

### Sync & Transaction Relay

- Improve blockchain synchronization performance
- Enhance transaction and block propagation
- Strengthen mempool capabilities

### Bitcoin Ecosystem Support

- Maintain compatibility with widely used RPC interfaces
- Enable integration with external developer toolkits
- Validate behavior against common Bitcoin workflows

### Modular Architecture

- Enforce architectural separation through domain boundaries
- Organize development around modular components

### Testing & Validation

- Establish comprehensive testing strategies (unit, integration and fuzzing)
- Strengthen coverage across core components
- Improve CI reliability and automated validation pipelines

### Community Adoption & User Experience

- Improve documentation and onboarding materials
- Refine user-facing interfaces and developer tooling
- Strengthen presence in the Bitcoin ecosystem
- Grow and support the contributor base

## Notes

- The roadmap defines **direction, not guarantees**
- This is a living document and will be updated quarterly based on community feedback and project evolution
- Priorities and timelines may shift as we progress, based on:
  - Implementation complexity
  - Security considerations
  - Ecosystem needs
