# Graph Report - DimmerBut  (2026-11-13)

## Corpus Check
- 20 files · ~18,604 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 4 file(s) not represented in the graph (top: (none) 2, .toml 1, .ico 1)

## Summary
- 570 nodes · 1068 edges · 20 communities (16 shown, 4 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 50 edges (avg confidence: 0.73)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `477bc6e4`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- window.rs
- monitors.rs
- ui.rs
- Flyout
- settings_ui.rs
- app.rs
- Dlg
- steps
- flyout.rs
- AppSettings
- settings.rs
- steps
- Surface
- geocode.rs
- DimmerBut
- Kind
- DimmerBut
- Fmts
- dimmerbut

## God Nodes (most connected - your core abstractions)
1. `Dlg` - 48 edges
2. `Flyout` - 42 edges
3. `AppSettings` - 24 edges
4. `App` - 23 edges
5. `Kind` - 20 edges
6. `AppCore` - 19 edges
7. `Surface` - 19 edges
8. `flyout_proc()` - 15 edges
9. `dlg_proc()` - 15 edges
10. `R4` - 15 edges

## Surprising Connections (you probably didn't know these)
- `show()` --calls--> `backdrop_supported()`  [INFERRED]
  dimmerbut-rs/src/settings_ui.rs → dimmerbut-rs/src/ui.rs
- `flyout_proc()` --calls--> `apply_fx()`  [INFERRED]
  dimmerbut-rs/src/flyout.rs → dimmerbut-rs/src/window.rs
- `flyout_proc()` --calls--> `with_app()`  [INFERRED]
  dimmerbut-rs/src/flyout.rs → dimmerbut-rs/src/window.rs
- `set_text()` --calls--> `wide()`  [INFERRED]
  dimmerbut-rs/src/settings_ui.rs → dimmerbut-rs/src/main.rs
- `FIELD_BG` --references--> `Rgb`  [INFERRED]
  dimmerbut-rs/src/settings_ui.rs → dimmerbut-rs/src/ui.rs

## Import Cycles
- 2-file cycle: `dimmerbut-rs/src/app.rs -> dimmerbut-rs/src/flyout.rs -> dimmerbut-rs/src/app.rs`

## Communities (20 total, 4 thin omitted)

### Community 0 - "window.rs"
Cohesion: 0.06
Nodes (28): is_enabled(), open(), refresh_path(), RUN_KEY, set(), VALUE, log_line(), main() (+20 more)

### Community 1 - "monitors.rs"
Cohesion: 0.07
Nodes (15): add_from_hmonitor(), Cmd, Brightness, Power, current_brightness(), enum_proc(), enumerate(), init_com() (+7 more)

### Community 2 - "ui.rs"
Cohesion: 0.07
Nodes (17): DARK, LIT, moon_icon(), RING, sample(), colorref(), FIELD_BG, ACCENT (+9 more)

### Community 3 - "Flyout"
Cohesion: 0.07
Nodes (25): AppCore, Confirm, Flyout, flyout_proc(), Fx, Hit, ConfirmCheck, ConfirmNo (+17 more)

### Community 4 - "settings_ui.rs"
Cohesion: 0.04
Nodes (41): CLIENT_H, CLIENT_W, CW, CX, FOOTER_Y, GeoSlot, ID_EDIT, N_KBED (+33 more)

### Community 5 - "app.rs"
Cohesion: 0.07
Nodes (12): App, CMD_SETTINGS, CMD_SHOW_FLYOUT, HK_DECREASE, HK_INCREASE, MOD_NOREPEAT, TIMER_APPLY, TIMER_POLL (+4 more)

### Community 6 - "Dlg"
Cohesion: 0.11
Nodes (9): Dlg, dlg_proc(), EditRef, Item, make_font(), set_text(), show(), text_of() (+1 more)

### Community 7 - "steps"
Cohesion: 0.10
Nodes (20): GH_TOKEN, jobs, release, name, on, workflow_dispatch, permissions, contents (+12 more)

### Community 8 - "flyout.rs"
Cohesion: 0.08
Nodes (12): CLASS, CONFIRM_H, Fmts, G_BRIGHTNESS, G_MONITOR, G_MOON, G_POWER, G_SETTINGS (+4 more)

### Community 9 - "AppSettings"
Cohesion: 0.08
Nodes (18): build_matrix(), DimController, ease(), get_gains(), get_kelvin(), kelvin_to_rgb(), minutes_of(), norm() (+10 more)

### Community 10 - "settings.rs"
Cohesion: 0.18
Nodes (8): format_hotkey(), key_name(), MAX_KELVIN, MIN_KELVIN, MOD_ALT, MOD_CONTROL, MOD_SHIFT, STEP

### Community 11 - "steps"
Cohesion: 0.11
Nodes (18): jobs, screenshots, name, on, workflow_dispatch, permissions, contents, runs-on (+10 more)

### Community 13 - "geocode.rs"
Cohesion: 0.21
Nodes (4): agent(), find(), GeoResult, matches()

### Community 14 - "DimmerBut"
Cohesion: 0.17
Nodes (11): Building, Command-line flags, DimmerBut, Features, Files, Install, Knowledge graph, Releases (+3 more)

### Community 15 - "Kind"
Cohesion: 0.08
Nodes (30): Act, Cancel, ClearKey, Find, Save, Fmt, Num, Time (+22 more)

### Community 16 - "DimmerBut"
Cohesion: 0.50
Nodes (3): DimmerBut, Git, graphify

## Knowledge Gaps
- **24 isolated node(s):** `dimmerbut`, `DimSlider`, `Night`, `Settings`, `ConfirmCheck` (+19 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 249 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **4 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AppSettings` connect `AppSettings` to `window.rs`, `Flyout`, `settings_ui.rs`, `app.rs`, `Dlg`, `settings.rs`?**
  _High betweenness centrality (0.114) - this node is a cross-community bridge._
- **What connects `dimmerbut`, `DimSlider`, `Night` to the rest of the system?**
  _24 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `window.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.05547785547785548 - nodes in this community are weakly interconnected._
- **Why does `Dlg` connect `Dlg` to `settings_ui.rs`, `AppSettings`, `Surface`, `Kind`, `Fmts`?**
  _High betweenness centrality (0.096) - this node is a cross-community bridge._
- **Should `monitors.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07030527289546716 - nodes in this community are weakly interconnected._
- **Why does `Flyout` connect `Flyout` to `monitors.rs`, `ui.rs`, `app.rs`, `flyout.rs`, `Surface`?**
  _High betweenness centrality (0.094) - this node is a cross-community bridge._
- **Should `ui.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.06538461538461539 - nodes in this community are weakly interconnected._