# Material memory investigation

## What this delivery implements

The laboratory now runs a complete discovery and fresh-world confirmation workflow. It selects structures before observing their responses, archives conditioning and probe episodes, applies local and matched remote counterfactuals, and retains negative and ineligible cases. The current runner freezes and hashes its executable, physical configuration and protocol before either phase.

The engine also supports exact fast-forward of provably inactive Matter worlds. Reservoirs must be disabled, every particle must have zero excitation, and every heat cell must be at most one. These conditions prevent every available physical transition under the current laws. Active worlds, zero-length skips and counter overflow are rejected.

Validation used compilation, actual simulation episodes, checkpoint continuation, immutable artifact comparisons and deterministic replay.

## Scientific question and current protocol

Can a naturally assembled material structure retain a conditioning-dependent spatial arrangement that changes its later response to a heat pulse?

The current experiment uses 750 randomly scattered particles on a periodic 16³ lattice. Its explicitly configured operating condition has motion probability 100,000 per million, breaking probability 500 per million and material valence two. It does not seed a designed chain, organism or circuit.

After 1,000 warmup ticks and a 20-tick graph-stability interval, up to four naturally formed three-particle chains per world are selected by size and ID. Stability here means endpoint agreement of bond identities, not proof of uninterrupted stability. Matter is cooled before conditioning. A pulse deposits 64 heat units at each cohort site or an equal number of remote sites; the branch evolves for 200 ticks.

Before probing, both histories keep only their shared internal bonds, lose external cohort links and are cooled through the energy ledger. The channel must remain connected. Local restoration replaces the cohort's positions and orientations with those of the unconditioned history, preserving the conditioned conformations and shared graph. The matched remote intervention applies the same displacements and relative orientation changes to distant unbonded particles of the same materials. Restoration into occupied sites and missing remote controls are ineligible cases, not negative probe outcomes.

Four histories—unconditioned, conditioned, locally restored and remotely restored—each receive local and remote 256-unit heat probes. Each eligible cohort receives 64 paired future random streams. These replicate responses of a stored history; independent initial worlds are the discovery seeds 22–24 and confirmation seeds 25–27.

The primary readout is time to the opposite initial endpoint's first positive excitation over 32 ticks. Unobserved signals have a restricted value of 33 and are explicitly marked censored. This measures arrival under any available local physics, including heat diffusion; it does not isolate transmission through bonds. Each episode also stores the excitation integral and full trace.

Support requires history and local-restoration intervals excluding zero with opposite signs, local-restoration agreement in both replica halves, and exactly zero paired remote-restoration effects. Intervals are mean ± 2.2 standard errors, an exploratory heuristic without a multiple-comparison correction.

## Evidence boundaries

The complete campaign is archived at `runs/trimer-memory-workflow-20261014-093718`. Its original runner, source, configuration, protocol and executable are preserved under `reproducibility/`. Input and binary hashes were verified against those copies. The revised runner with frozen inputs completed a separate short four-stage pipeline exercise; that exercise is operational validation, not evidence of material memory.

The earlier longer-chain workflow at `runs/material-memory-workflow-20261014-053007` completed both phases with no supported world. Its records remain intact. Other earlier assays also produced negative or unconfirmed results. The trimer timing assay is a disclosed protocol revision, not a reinterpretation or pooling of those outcomes.

External heating, cooling, cohort selection and geometric restoration are laboratory interventions. They are not actions discovered by organisms. Zero-work geometric restoration holds only because this artificial model has no position-dependent potential energy. Cooling freezes activity; this is not biological maintenance of memory in a noisy environment.

History-dependent transmission can establish a primitive physical memory effect in this substrate. It does not establish learning, adaptive benefit, reproduction, life, intelligence, terrestrial chemistry or a novel scientific breakthrough. Conditioning can speed or slow the response.

## Completed outcome

Both phases completed: 21 preselected cohorts, seven eligible audits and 3,584 probe episodes. Four cohorts passed: two from discovery worlds 22 and 23, and two from fresh confirmation world 27. Three eligible cohorts from confirmation world 25 failed. Fourteen cohorts were ineligible: ten lacked a matched remote rearrangement, three lacked a spatial marker, and one lost its intact channel. Ineligible cases were not silently excluded from the campaign record.

The effect therefore replicated in one of three fresh initial worlds, with two passing structures in that world. This supports conditional spatial-state mediation under the declared exploratory criteria. It does not establish a reliable success rate across arbitrary worlds or material settings.

## Passing cases

| Phase | World / cohort root | Unconditioned local latency | Conditioned | Local restoration | Remote restoration |
| --- | --- | ---: | ---: | ---: | ---: |
| Discovery | 22 / 235 | 12.671875 | 8.281250 | 12.734375 | 8.281250 |
| Discovery | 23 / 383 | 10.796875 | 16.078125 | 10.796875 | 16.078125 |
| Fresh-world confirmation | 27 / 65 | 7.390625 | 12.687500 | 7.437500 | 12.687500 |
| Fresh-world confirmation | 27 / 143 | 11.093750 | 7.781250 | 10.968750 | 7.781250 |

Values are mean restricted latency in ticks across 64 paired streams. All local signals were observed in world 22 / 235 and both passing world-27 cohorts. In world 23 / 383, 62 conditioned and 63 unconditioned/local-restored signals were observed; the other observations are censored. Remote heat probes had no receiver signal within the horizon in these cases. The matched remote geometric restoration left every paired response unchanged.

For world 27 / 65, the history effect was +5.296875 ticks and local restoration effect −5.25 ticks. Its local-restoration halves were −6.3125 and −4.1875 ticks, with both approximate intervals excluding zero. For world 27 / 143, the corresponding effects were −3.3125 and +3.1875 ticks. Both cases reproduced the predeclared exploratory pattern, in opposite response directions. World 25's three eligible cohorts all failed the full rule, and world 26 had no eligible audit. This is selective replication under the stated operating condition, not a universally reliable memory device.

One complete replica's eight control/probe episodes replayed exactly for each of these four passing cases: 32 archived episodes in total. Arm completeness and campaign totals were checked against the recorded receiver traces. This is representative replay validation, not a claim that every archived episode was replayed.

## Runtime evidence

`runs/quiescent-validation-20261014-093740` contains an inactive checkpoint advanced through ten explicit ticks and through a ten-tick skip. Both produced the identical complete-state digest `872e5ed835d82eaff761d282d8237e106cd9b1c9a89a179b2ae244362ba406d6`. The explicit episode replayed exactly.

Skipping one trillion ticks took 1,931 microseconds for the engine operation in this single run, excluding checkpoint I/O. Reported physical metrics and energy counters were unchanged. The output checkpoint reloaded and continued normally. An active checkpoint was rejected with exit code one and no output checkpoint; zero steps and generation overflow were rejected too.

This path performs work independent of skipped duration, while still inspecting and validating the stored state. It does not accelerate active chemistry, assign years to ticks, or fabricate event journals for skipped ticks.

## Reproduce

```powershell
cargo build --release --target-dir target/validation
./analysis/run_material_memory.ps1 -Out runs/a-new-material-memory-campaign
```

Use a new output directory. The runner performs both phases without an intermediate approval step and preserves diagnostic artifacts if a stage fails. The full specification, controls and command descriptions are in `analysis/FUNCTIONAL_MEMORY_PROTOCOL.md`.
