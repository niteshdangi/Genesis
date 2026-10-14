# Genesis review and consolidated roadmap

Updated 14 October 2026. The initial review was written on 13 October. Genesis is now v0.7.0 with versioned Matter laws and recorded laboratory campaigns. Historical version numbers remain historical records; milestones describe demonstrated outcomes rather than release numbers.

## Assessment

Genesis's end goal is a material universe in which fixed local laws can support increasing complexity: persistent structures, self-maintenance, reproduction, heritable evolution, adaptive information processing, and potentially intelligence and civilization. Higher-level behavior must arise from material interactions, not organism commands or scripted civilization rewards. Building the substrate is feasible; emergence at the full intended scale remains an open research question.

Matter is the main universe. Earlier chemistry, polymers, cellular automata and scripted agents are historical comparison models, not alternative success criteria. In particular, civilization in the scripted agent world would not establish matter-first emergence. The engine/law separation, transactional generations and conservation checks remain the engineering foundation.

The continuous designed-source diagnostic shows that existing structures can produce matching material descendants which catalyze further descendants without later interventions. Independently confirmed unseeded causal reproduction and inherited differences in success under competition remain unresolved. Larger populations, richer rendering, and additional abstractions do not establish those outcomes.

Later opt-in v3 research extends the local law to permit one mismatched recruitment endpoint at one-sixteenth the matching catalytic bonus. The [variation capability campaign](research/variation/REPORT.md) completed and replayed 12 trajectories. Four active runs produced 677 dimers, with 312 changed-signature births and 279 faithful copies made by altered-signature parents; four inactive controls produced none, while four v2 controls copied without signature changes. This establishes seeded within-lineage variation and copying of altered signatures in favorable artificial chemistry. It does not establish selection, adaptation, spontaneous origin or open-ended evolution. Default laws remain v2, and the v0.7.0 evidence below remains a historical record of the earlier delivery.

## Evidence used

Subsequent [selection and unseeded-origin research](research/selection-origin/REPORT.md) completed 40 replayed trajectories. Four worlds initialized unbonded and inactive developed 333 event-attributed catalytic births through three to five generations; mechanism-disabled controls had no catalytic births. This supports unseeded artificial material autocatalysis, not spontaneous life. Shared-resource competition showed directional trait reversals across four seeds, but its strict full gate failed because a disadvantaged lineage did not reach the required depth in two trajectories. Adaptive evolution, open-ended complexity, intelligence and civilizations remain unresolved. These later results are separate from the historical v0.7.0 delivery below.

The original private project knowledge dump informed the initial plan and is not included in this public repository.

The original document reported chemistry screenshots through generation 4; these remain historical observations rather than the current evidence baseline.

The 14 October review inspected relevant runtime and persistence source and archived campaign results, and completed an offline, locked Cargo check of all targets with the optional GPU feature. Archived experiment outcomes below were not all independently rerun during that review.

| Area | Evidence | Current conclusion |
| --- | --- | --- |
| Material substrate and reproducibility | [Initial execution validation](runs/validation-20261014-035753/VALIDATION_REPORT.md), [packed-index validation](runs/packed-index-validation-20261014-110428/verification.json) | 3D matter, energy accounting, checkpoint restore, replay, sampled CPU/Rayon/GPU-field agreement and atomic failure preservation exercised |
| Controlled substrate dynamics | [Historical substrate campaign](runs/substrate-20261014-040312/REPORT.md), [v2 baseline](runs/material-v2-baseline-20261014/outcome.json), [v2 replay](runs/material-v2-baseline-replay-20261014.json), [restored continuation](runs/material-v2-restored-baseline-20261014/outcome.json) | Unseeded 750-particle v2 world completed 10,000 ticks with full provenance; all 101 replay checks and the continuation restored at tick 5,000 have the same final digest |
| Local assembly constructibility | [Designed v2 diagnostic](runs/material-v2-delivery-diagnostics-20261014/result.json) | 144 archived episodes across two material variants establish selective local catalysis, product-mediated further assembly under disclosed interventions, ordinary product motion, controls and CPU/Rayon parity; not spontaneous reproduction |
| Continuous seeded copying | [Continuous diagnostic](runs/material-v2-autonomous-catalysis-20261014/result.json), [verification](runs/material-v2-continuous-verification-20261014.json) | Eight intact trajectories produced 79-82 matching products each and 6-9 catalytic generations; 202 descendant-mediated births occurred beyond five toroidal steps of the original source. All 16 inactive/disabled controls produced none; all 24 trajectories replayed. Designed initial chemistry, not spontaneous life |
| Physical memory | [Material memory findings](analysis/MATERIAL_MEMORY_FINDINGS.md) | Four passing cohorts among seven eligible audits; fresh-world support in one of three confirmation worlds, under exploratory spatial-state criteria; not learning or adaptive benefit |
| Reproduction | [Historical chain verdict](runs/autonomous-reproduction-20261014-102534/workflow-result.json), [dimer report](runs/dimer-reproduction-20261014-104138/REPORT.md), [gradient verdict](runs/gradient-reproduction-20261014-111902/workflow-result.json), [resource correction](runs/material-v2-resource-correction-20261014/reproduction.json), [remaining corrected worlds](runs/material-v2-resource-correction-confirmation-20261014/reproduction.json), [natural v2 dimers](runs/material-v2-natural-dimers-20261014/workflow-result.json) | Original v2 chain assembly was resource-ineligible in all five selected-parent worlds; the sixth lacked a parent. Fresh natural dimers still show no supported repeated production; heredity, mutation and selection remain unestablished |
| Capacity | [Million-particle outcome](runs/million-capacity-20261014-103109/outcome.json) | One CPU-reference tick completed in approximately 31 seconds; this is short-horizon capacity evidence, not sustained large-world throughput |
| Sustained active-world execution | [1,000-tick profile](runs/material-v2-profile-20261014/profile.json), [memory-profile execution](runs/material-v2-memory-profile-20261014/core/profile.json), [process memory](runs/material-v2-memory-profile-20261014/memory.json) | 750 particles: mean core step 24.203 ms and 17.589 ms in two concurrent-workload measurements; peak working set 15.2 MiB in the second run; no full historical event archive in these profiles |
| Backend parity | [Active Rayon continuation](runs/material-v2-rayon-parity-20261014/outcome.json), [GPU run](runs/material-v2-gpu-parity-20261014/outcome.json), [GPU state comparison](runs/material-v2-gpu-comparison-20261014.json) | CPU/Rayon agree after another 1,000 active ticks; actual GPU fields agree with CPU at tick 1,000 in canonical digest, causal state and all 1,000 compressed event batches; GPU acceleration is not established |

## Integrated v0.7.0 delivery and its limits

The material-v2 law is a generic local contact interaction: a matching bonded, active material pair can increase the probability of one adjacent ordinary bond forming. Existing port, valence, excitation and energy constraints still apply. There is no organism identity, whole-structure clone, reproduction instruction, fitness reward or hidden resource source. The exact rule and conflict footprint are specified in the [implementation plan](GENESIS_MATTER_FIRST_IMPLEMENTATION_PLAN.md).

New worlds use v2. JSON specifications without `law_version` and historical native checkpoints retain v1. Historical replay was exercised against a frozen executable: 13 checks matched, two-tick continuations produced identical digests, and that executable successfully read a v1 checkpoint newly saved by v0.7.0. Browser Restore accepted both old and new standalone bundles and displayed their actual law versions. Unsupported version 3 returns HTTP 400 without replacing the current world.

The designed diagnostic starts with externally arranged active bonds and feedstock. Its product assay relocates the original source and positions fresh feedstock. These interventions establish constructibility and reusable material catalysis, not autonomous ecological reproduction or spontaneous origin. Additional executable checks verify atomic failure at one- and two-event full-provenance limits, explicitly disclosed truncation when full provenance is disabled, zero-affinity rejection, and absence of false catalytic attribution for ordinary binding.

The separate unseeded baseline used seed 101, a 16-cubed torus and 750 initially unbonded particles. It completed in 233.585 seconds including observation and archive work. Its final energy is 255,138: initial 17,884 plus reservoir input 2,797,254 minus output 2,560,000. Recorded events include 5,020 bindings, 4,142 unbindings and 66 accepted contact-catalysis records, corresponding to 33 catalytically enabled bindings. Six connected structures remained, the largest containing 665 particles; aggregation is not evidence of life. Full replay and restored continuation finish at digest `7db6ab650f822332905b08a6d79bf9cf3b21acde4e0106d622f62cea0426cd53`.

The original fresh reproduction campaign used discovery seeds 101-103 and confirmation seeds 104-106. Its immutable archives contain 60 completed assembly episodes with zero matching products. **The follow-up feasibility audit corrects their interpretation:** all five selected-parent worlds lacked the required free material types, so those zeros do not evaluate the proposed assembly mechanism. Seed 104 had no eligible natural parent. For example, seed 101's six-particle signature needs four material-0 atoms, one material-1 and one material-2; its eligible pool contained just one material-0 and one material-3 atom. Seeds 105 and 106 had enough total free atoms but not the required composition.

The observer now shares feedstock eligibility with its resource preflight and reports `insufficient-eligible-feedstock` explicitly. Recycled-feedstock protocols additionally need two initial components containing usable material. Reports separate assayed and ineligible world seeds. Re-executed seeds 101-106 confirm the corrected classification; a resource-sufficient seed-107 dimer still completes its assay with all five measured effect summaries unchanged. Original archives and the earlier verification receipt are preserved, not rewritten.

The separate prospective natural-dimer campaign used seeds 107-112, the existing two-particle protocol and unchanged support thresholds. Its 72 assembly episodes produced 61 global matching observations, but zero parent-contact matching products and zero parent-catalyzed bindings. All six selected dimers met structural retention plus energy-exchange criteria, which is not a metabolism claim. The existing sparse natural operating condition still does not establish reproduction.

### Continuous seeded autocatalysis

The [executable diagnostic](examples/material_autocatalysis.rs) addresses the earlier constructibility limitation directly: after one initial source bond is arranged, feedstock remains scattered and no particles are relocated, supplied or manipulated during the trajectory. The existing v2 local law is unchanged. Ordinary movement, excitation expenditure and reservoir exchange drive encounters and separation. Birth records infer ancestry from accepted source-target contact events, not authoritative parent fields or copying commands.

The prospectively fixed matrix is two material-pair variants, four seeds, three conditions and 2,048 ticks per trajectory in an 8-cubed, 350-particle world. Binding without catalysis, breaking and conformation switching are disabled for this designed kinetic condition. All particles start with designed active chemistry. Every intact run exceeds the predeclared thresholds of eight products, three catalytic generations and one descendant-mediated birth beyond five toroidal steps from the original source. Inactive-source and v1-disabled controls each produce zero products.

Across the eight positive runs there are 647 event-attributed products and 202 qualifying distant descendant births. All product bonds persist, both material-pair variants copy faithfully, every run replays exactly, and all initial energies are 8,000 with valid final input/output ledgers. Separation is measured at the accepted birth event; it is not a claim that the original source remains absent throughout a separate descendant horizon. The frozen diagnostic executable, source and dependency lock are preserved with the run.

This demonstrates autonomous **seeded material autocatalysis**, closing the need for repeated external feedstock positioning in the earlier diagnostic. It does not demonstrate spontaneous origin, mutation, competition, selection, metabolism or intelligence. The variants use different resources, so their separate runs cannot establish competitive fitness.

### Dense unseeded comparison

Fresh worlds 113-118 used the same dense geometry with no initial bonds or active conformations, rare ordinary nucleation at 250 per million and switching at 1,000 per million. This prospective operating condition differs from the designed diagnostic and is not pooled with it. All six worlds passed resource preflight with 204-234 free feedstock particles. Accepted parent-catalyzed bindings and matching contact product observations each total six in first-generation trials; seed 115 also supplied a candidate for the separate descendant assay.

The production-effect intervals in seeds 115 and 116 are [-0.3, 0.8], including zero. All causal and stronger repeated-production gates remain false in the [recorded campaign](runs/material-v2-dense-unseeded-20261014/workflow-result.json). These isolated observations do not establish unseeded causal reproduction. No thresholds were relaxed and no designed source was silently inserted into these worlds.

All six selected templates initially had only one active atom, while the v2 contact mechanism requires both source atoms active. Ordinary switching can subsequently make them competent, so this is not the same impossibility as missing feedstock; it limits exposure and power. Any subsequent competent-template selection must be declared prospectively on new worlds, not retroactively applied to retain only favorable outcomes.

Measurements used Windows x86-64, Rust/Cargo 1.95.0 and an Intel Core i7-11800H with 8 cores/16 logical processors. Concurrent validation workloads make these timings indicative, not isolated performance promises. The memory measurement samples private bytes every 100 ms and records the process's peak working set; neither it nor the short million-particle observation establishes sustained million-particle throughput.

Default and GPU-enabled release builds and the all-target GPU-feature check passed. The GPU run explicitly disabled CPU fallback and reported `wgpu-fields-cpu-interactions`; its tick-1,000 digest is `446000da38f8ec29b9b84a4a371a4fdd7aeb3e92ebf4396105ef215afd92a4fe`, equal to the CPU baseline at that tick. It took 663.541 seconds, so it is compatibility evidence, not a speedup claim. Loading its strict GPU checkpoint with a CPU-only verifier correctly failed; the GPU-capable verifier then completed the state comparison. The default remains the CPU reference.

Recorded run manifests honestly retain `revision: "unrecorded"` where no committed revision existed at execution time. They are not retroactively rewritten. The local source baseline and dependency lock make subsequent revision-attributed runs possible; archived outputs remain outside source control.

**Engineering outcome:** an integrated, replayable versioned material research platform with corrected resource eligibility and continuous event-inferred lineages. **Scientific outcome:** seeded autonomous material copying and fidelity of two material-pair variants are demonstrated; statistically supported unseeded repeated reproduction, mutation, competitive selection, adaptive evolution, intelligence and civilization are not. Its final scientific goal is not delivered.

## One universe, with a laboratory behind it

The primary browser experience is the Matter world with Play/Pause, Step, Save, Restore and read-only observations. The Laboratory is closed initially and contains configuration, batching, reset, recording and historical comparison selection. Restoring an older world remains supported and visibly identifies it as a comparison model. Switching worlds is explicit and starts a new paused state; no existing world is silently converted.

The normal starting command is `genesis serve`; Matter is already the default. A native manifest checkpoint with sidecar chunks can be resumed with `genesis serve --resume CHECKPOINT`. Browser Save downloads a standalone bundle suitable for browser Restore.

Recorded experiments share one entry point and artifact workflow:

```powershell
genesis lab run --out runs\my-run --steps 1000
genesis lab replay --run runs\my-run
genesis lab analyze --run runs\my-run --out runs\my-analysis.json
genesis lab help
```

`genesis lab controls`, `genesis lab reproduction` and `genesis lab profile` reuse the existing material implementations. Historical top-level commands remain compatible with old scripts. Existing presets, native checkpoint formats, archived campaigns and frozen source/executable snapshots are preserved; they are not rewritten to match the new interface.

There is one active research question: can local material laws support causal, repeatable reproduction? Memory assays remain diagnostic evidence, not additional universe modes or an indefinitely expanding program. Add an experiment only when its possible outcomes would change a specific substrate or execution decision. Retain negative and ineligible cases. Do not add a configuration option without a demonstrated need.

## Improvements to the idea and document

### Define one near-term research question

Replace the broad near-term promise of a simulated civilization with: can generic local interactions support resource-limited, heritable reproduction, and which interactions are necessary?

Keep civilization as the long-term matter-first aspiration, not a reason to inject agents into the material world. Define generations as simulation steps. A thousand simulated years requires a domain-specific time model; it has no established meaning in the current substrate.

### Distinguish continuing state from durable state

Stable particle identity and bonds persist between generations in memory. Durable checkpoints need to preserve the complete causal state required to resume the same future: law configuration, seed, generation, particles, bonds, energy, and any workspace or counters that affect transitions. A UI snapshot is not automatically a restart format.

Matter now has versioned binary checkpoints, canonical logical digests, a recorded 10,000-tick baseline and restored continuation. Crash-recovery exercises and broader capacities remain separate acceptance questions. Record source revision and execution environment; scope replay guarantees to the specified implementation and environment. Frozen campaign inputs help provenance, but do not replace a committed source-control baseline.

### Specify the missing copying mechanism

The earlier chemistry's formation, dissociation and exchange vocabulary did not establish copying. V1 conformation-dependent catalysis did not establish sequence-selective reproduction. V2 matching contact catalysis now has designed constructibility evidence, but its fresh natural-chain campaign still found no supported repeated reproduction. Before extending the laws again, specify the remaining information carrier, autonomous alignment, product detachment and repeatability mechanism.

Start by investigating a small polymer template with distinguishable variants. This is a proposed experiment, not a claim that the current substrate can implement it. Change only the substrate features the mechanism needs. Generic local catalyst recognition or contact-dependent assembly can be legitimate laws; recognizing an entire organism and cloning it would predefine the phenomenon under investigation.

Degree limits of two constrain molecules to paths and cycles, but those structures can still encode sequences. Matter already permits material-specific valences; some campaigns deliberately restrict them. Earlier rigid-group movement and current occupancy/bond geometry have different constraints. Investigate alignment, growth and release under the actual material laws before adding more knobs or assuming branching solves copying.

### Give energy an operational role

The historical chemistry made movement depend on an energy threshold without spending it. Matter now separately accounts for heat, excitation, structural energy and reservoir transfers. These are specified artificial dynamics, not terrestrial thermodynamics; persistence with energy exchange does not establish metabolism.

Specify which future processes consume usable resources, how those resources are renewed, and why some structures gain a reproductive advantage. Retain closed-world experiments as accounting controls. For driven experiments, use an explicit ledger such as total energy change equals input minus output, with internal transfers conserving the accounted total. Merely adding energy does not establish metabolism.

### Measure causes and repeated reproduction

A related molecule appearing near a parent is insufficient. Compare a template-present condition with matched template-absent and copying-disabled conditions, preserving material quantities and comparable starting energy. Trace assembly events and material provenance; stable particle IDs alone do not identify parentage.

Require released descendants to produce further descendants. Measure copying fidelity, viable variation, and reproductive differences across independently seeded runs. Choose seed counts, run horizons, controls, and effect criteria before selecting attractive outcomes. Designed seeds can establish a mechanism; spontaneous origin is a separate, harder experiment.

### Bound the scalability and open-endedness claims

A fixed lattice with bounded state has finite capacity. Under deterministic, closed, finite-state execution, an indefinitely extended trajectory eventually revisits state; the current implementation also has finite counters. Long-run adaptive novelty is a measurable research target, while literal unbounded novelty is not a justified claim for this implementation.

Exact simulation of active local interactions generally requires processing their causal consequences. Sparse updates, batching, and coarse models may reduce work in particular regimes, but sublinear cost for arbitrary detailed worlds is not an established property. Benchmark cost against grid size, particle count, bond count, active regions, and observation frequency. Any approximation needs an explicit statement of which results it preserves and which it changes.

Open-ended evolution has competing definitions and multiple proposed measurement approaches. Neutral controls and lineage persistence help distinguish adaptation from transient noise. See the [ISAL overview](https://alife.org/encyclopedia/introduction/open-ended-evolution/). [Flow-Lenia](https://arxiv.org/abs/2506.08569) provides a relevant precedent for embedding heritable model parameters within dynamics and measuring evolutionary activity; it does not validate Genesis outcomes.

### Make the document easier to maintain

Keep the historical v0.1-v0.4 material in a history section. Replace the future version ladder with the four milestones below. Maintain one current-status table with links to recorded experiment evidence rather than repeating status claims throughout the document.

Separate implementation facts, document-reported observations, independently reproduced results, and proposed changes. Replace the blanket statement that physics can be proven consistent with the more precise claim that invariants are checked for executed candidate states; a formal proof would require separate work.

## Four consolidated milestones

| Milestone | Combined scope | Completion evidence | Current status |
| --- | --- | --- | --- |
| M1 Reproducible material platform | Earlier chemistry foundations plus Matter, persistence and essential experiment facilities | Valid long runs, replay, restore, targeted scenarios, and an exportable experiment record | Integrated bounded delivery verified, including 10,000 ticks, replay, restore and v1 compatibility; larger capacities and crash recovery are not certified |
| M2 Heritable reproduction | Information carriers, local assembly, resource accounting, release, repeated copying, mutation and initial selection | Descendants reproduce; controls establish a causal parent effect; variants retain traits affecting reproductive success | Continuous seeded material copying and two-variant fidelity demonstrated through 6-9 catalytic generations; unseeded causal repetition, mutation and competitive selection remain unresolved |
| M3 Adaptive ecology | v0.6 evolution studies plus the analysis and experiment workflows needed to investigate them | Repeated resource-dependent adaptation, measured lineage dynamics, and bounded claims about sustained novelty | Research proposal |
| M4 Programmable universe runtime | Shared execution facilities informed by concrete worlds | Worlds share execution and experiment facilities without domain logic leaking into the engine | Multiple comparison backends exist; further generalization deferred until a concrete need |

Milestones contain several experiments and implementation tasks. Those tasks are work items inside the milestone, not separate public phases. Completion depends on evidence, not a calendar date.

### M1 Reproducible material platform

Deliver controlled scenarios for exchange, toroidal rigid translation, self-overlapping group footprints, competing destinations, and failed-stage handling. Run a documented baseline for at least 10,000 generations and report invariants, reaction totals, molecule distributions, timing, and memory. This horizon is an initial baseline, not a universal sufficiency threshold.

Add a headless experiment path, explicit configuration, canonical full-state digests, durable checkpoints, and save/restore. Hash all state that affects future transitions, rather than just populations or UI fields. Compare identical executions at recorded checkpoints and compare uninterrupted execution with restored execution. Preserve the selectable cellular laws through recorded behavioral scenarios.

Export an experiment manifest containing the code revision, law/configuration versions, seed, initial digest, checkpoints, interventions, outcomes, and environment. Capture failure context and retain the last valid state. Validation uses runtime invariants and executable experiment scenarios.

Exit when another run can reproduce the recorded baseline and restored continuation, the targeted scenarios have evidence, and results are inspectable outside the browser. The milestone establishes trustworthy experiments, not artificial life.

### M2 Heritable reproduction

Investigate one minimal mechanism first: a designed information-bearing template that promotes local assembly from environmental matter. Include the substrate changes needed for copying, detachment, and repeated reproduction within this milestone rather than creating separate energy, polymer, and catalyst releases.

Use matched controls, event provenance, multiple independent seeds, and at least two distinguishable candidate variants. Account for every resource consumed or exchanged. Establish that products are independently reproductive and that variant traits persist through descendants; then determine whether those traits influence reproductive success under resource competition.

Exit when controlled runs support causal reproduction, heredity, viable variation, and initial selection under a documented protocol. Do not require spontaneous appearance to pass the designed-mechanism experiment. Record spontaneous-origin searches separately, including unsuccessful runs.

If no candidate mechanism works within the predeclared search budget, record the negative result and revise the substrate hypothesis. Do not relabel continued bonding as replication.

### M3 Adaptive ecology

Study competing lineages under explicitly accounted resource renewal, depletion, and environmental change. Measure persistence, turnover, extinction, inherited traits, and functional reproductive advantage. Compare selected dynamics with copying-disabled or neutral conditions appropriate to the mechanism.

Vary seeds, resources, run lengths, and world capacities to examine whether a result survives beyond a single setting. Use several measures of novelty and function; entropy or molecule size alone cannot determine adaptation. Record plateaus and collapses as outcomes.

Exit when repeated controlled experiments demonstrate resource-dependent adaptation and an analysis workflow can explain lineage outcomes. Report sustained adaptive novelty only for the measured horizons and capacities. Open-endedness remains a research question even if this engineering milestone is complete.

### M4 Programmable universe runtime

Existing scripted agents and explicitly templated polymers already provide materially different comparison worlds. Use concrete requirements from these and Matter to determine which interfaces genuinely need generalization; do not promote them into additional main experiences or add another world merely to expand the menu.

Share replay, checkpointing, experiment manifests, failure handling, and law scheduling where execution semantics permit. Keep chemistry and agent behavior in their respective laws. Profile before adding parallel backends; evaluate determinism and approximation separately from throughput.

Exit when the selected worlds can be configured, run, paused, restored and compared through shared facilities, with explicit execution semantics and evidence that domain concepts remain outside the generic engine. This is a supporting engineering program, not the active research priority.

Civilization emergence, large-population scaling, multiscale approximations, and a language remain possible later programs. M4 does not silently promise millions of agents or unscripted technological history.

## Next decision

Do not expand the command menu or treat the integrated engineering delivery as a scientific finish. For M2, build on the demonstrated continuous seeded mechanism while retaining its limits: resource-aware natural origin, mutation and inherited differential success under shared-resource competition still require evidence. Any subsequent law change must be versioned, controlled and judged against unchanged scientific gates. Do not insert organisms or guarantee emergence.

The implementation version is v0.7.0. Continue using milestone IDs for the research roadmap so software delivery is never mistaken for a demonstrated scientific outcome. Bounded sustained active-world measurements are now recorded; increasing particle count or adding an agent civilization is not a substitute for M2.
