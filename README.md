# Genesis Universe Engine

Genesis is a Rust research engine for persistent, programmable material worlds. It defines local interactions, conserves matter and energy, records causal events, saves checkpoints, and replays experiments. The default world is an artificial chemistry, not a simulation of Earth's physics or biology.

**Research status:** Designed material structures copy and pass changed signatures to descendants. In four small worlds started with unbonded, inactive particles, local chemistry generated catalytic lineages. A shared-resource competition campaign showed trait-dependent differences, but its full predeclared gate failed. Genesis has **not** demonstrated living organisms, open-ended evolution, emergent intelligence, technology, or civilizations.

## Run it

Install a current stable Rust toolchain, then from the repository root:

```powershell
cargo run --locked --release -- serve
```

Open the local address printed by the server. The world starts paused. For recorded Matter experiments:

```powershell
cargo run --locked --release -- lab run --spec experiments/matter-v2.json --steps 100 --out runs/my-first-run
cargo run --locked --release -- lab replay --run runs/my-first-run
```

The `examples/` directory contains the designed copying, variation, selection, and origin investigations. For example, `cargo run --locked --release --example material_variation -- runs/my-variation-run` recreates its fixed twelve-trajectory protocol. These research runs are more expensive than the short example above. `cargo run --locked --release -- help` lists the CLI.

## Evidence and limits

- [Variation protocol](analysis/MATERIAL_VARIATION_PROTOCOL.md) and [published summary](research/variation/REPORT.md)
- [Selection and unseeded origin protocol](analysis/SELECTION_AND_ORIGIN_PROTOCOL.md) and [published summary](research/selection-origin/REPORT.md)
- [Consolidated research assessment](GENESIS_REVIEW_AND_ROADMAP.md)
- [Implementation plan and material laws](GENESIS_MATTER_FIRST_IMPLEMENTATION_PLAN.md)

Compact result JSON is committed under `research/`. Full event archives and checkpoints are generated under `runs/` and remain local because of their size. Older research documents contain local `runs/` paths; those refer to archived experiments, not files included in this public repository. Their protocols and source are included so the work can be inspected or repeated. The original private project knowledge dump is not published.

The material laws are versioned. The default is v2; v3 is an opt-in variation experiment. Earlier v1/v2 checkpoints and replay semantics are retained. Results from designed starting structures and initially unstructured matter are interpreted separately.

