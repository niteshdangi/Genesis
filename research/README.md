# Published research snapshots

These compact files capture the stated criteria and aggregate results from the final variation, competition, and unseeded autocatalysis campaigns. The matching Rust examples and research protocols are in `examples/` and `analysis/`.

One complete paired example from the unseeded-origin campaign is published as [origin-81-run-and-control.tar.gz](selection-origin/origin-81-run-and-control.tar.gz): the v3 active run and its v1 mechanism-disabled control, both at seed 81 for 2,048 ticks. The archive includes their manifests, causal event history, birth records, SQLite catalogs, compressed event chunks, and initial/final checkpoints. Its SHA-256 is `4e862ad0e2fa871059cb226b29d0447eacf0bab0f66546a3899212a170a607f8`.

From the repository root, create `runs/published-origin-81`, then extract with `tar -xzf research/selection-origin/origin-81-run-and-control.tar.gz -C runs/published-origin-81`. Inspect the two directories or replay them with `cargo run --locked --release -- lab replay --run runs/published-origin-81/origin-81-active` and the same command for `origin-81-disabled`. These are two illustrative trajectories, not an independent replication or the full 40-trajectory selection-and-origin campaign.

The other detailed per-tick events, native checkpoints, SQLite catalogs, and executable copies remain in local `runs/` directories ignored by Git. The published summaries are evidence of the completed local campaigns, not a substitute for their complete archives or independent reproduction. Run the examples to generate new archives. Original thresholds and negative outcomes are retained.

The earlier v0.7.0 roadmap refers to other local `runs/` paths that are not included here.
