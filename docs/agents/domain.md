# Domain Docs

How engineering skills should consume domain documentation in this repo.

## Layout

This repo is configured as **multi-context**:

- Root `CONTEXT-MAP.md` points to context-level `CONTEXT.md` files
- `docs/adr/` contains system-wide ADRs
- `src/<context>/docs/adr/` contains context-specific ADRs

## Consumption rules

- Before exploring or changing design-sensitive code, read relevant context docs and ADRs first
- If these files are missing, proceed silently
- Use glossary vocabulary from relevant `CONTEXT.md` files
- If a proposed change conflicts with an ADR, surface the conflict explicitly
