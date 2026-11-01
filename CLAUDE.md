# DimmerBut

Windows tray app in Rust (`dimmerbut-rs/`): software dimming, monitor brightness, night mode.

## graphify

This project has a graphify knowledge graph at `graphify-out/` (`GRAPH_REPORT.md`, `graph.json`, `graph.html`).

- Before answering questions about the codebase or architecture, read `graphify-out/GRAPH_REPORT.md`, and use `graphify query "<question>"` / `graphify path "A" "B"` / `graphify explain "X"` for specifics.
- **After every change to code, run `graphify update .`** (AST only, no LLM or API key needed) so the graph stays current, and include the updated `graphify-out/` files in the same commit.
- For changes to docs or images, run `/graphify . --update` instead.

## Git

After finishing a change, commit it automatically (including the refreshed `graphify-out/` files) and push to `origin`. Don't wait to be asked.
