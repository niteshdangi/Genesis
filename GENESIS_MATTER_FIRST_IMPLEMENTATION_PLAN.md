# Genesis matter first implementation plan

13 October 2026; operating model and integrated v0.7.0 outcome updated 14 October 2026.

## Objective and scope

Implement one complete experimental substrate in which initially nonliving matter can form persistent structures, exchange and dissipate energy, store information, and influence further interactions. Investigate whether self-maintenance, causal reproduction, inherited adaptation, and eventually functional information processing arise within that substrate.

The scientific hypothesis is that fixed local laws can support evolving structures that acquire new functional capabilities without adding higher-level behavior during a run. Engineering can deliver the substrate and instruments. It cannot guarantee the emergence of life, cognition, humans, or civilization.

This plan replaces the scripted-agent route as the main Genesis direction. Existing chemistry, polymer, and agent worlds remain comparison models. No program, build, installation, benchmark, simulation, or test is authorized by this planning document.

## Technology decisions

| Layer | Choice | Reason and boundary |
| --- | --- | --- |
| Causal runtime | Rust 2024, stable toolchain pinned when implementation begins | Reuse the current core and keep ownership, resource budgets, and state transitions explicit. |
| CPU reference | Single-thread deterministic implementation, then Rayon | Establish specified semantics before parallel execution; Rayon evaluates independent proposals, not competing writes. |
| GPU backend | Native Rust wgpu with WGSL kernels | Portable compute across supported adapters. Accelerate eligible kernels after profiling and comparison with the reference semantics. |
| Hot state | Packed arrays for particle properties, bounded adjacency, and spatial chunks | Predictable access and fewer allocations than object-per-particle or map-per-interaction designs. |
| Snapshot storage | Versioned binary DTOs using Postcard, compressed using zstd | Preserve complete restart state without JSON expansion in the simulation path. Schema migration remains explicit. |
| Integrity | Existing SHA-256 support over canonical logical state | State identity must not depend on compression output, memory layout, chunk allocation order, or worker count. |
| Experiment catalog | SQLite through rusqlite with bundled SQLite | Local experiment metadata, checkpoint references, and derived observations. No database transaction per physical interaction. |
| Event archive | Bounded binary event batches with compression, indexed by catalog | Record material and energy provenance with configurable retention. State remains authoritative. |
| Configuration | serde JSON with strict validation, canonical exported specifications | Human-readable laws and initial conditions, separate from hot state and binary snapshots. |
| Inspector | Existing local browser interface; TypeScript and WebGPU when richer rendering is implemented | Preserve the current UI initially. Receive immutable viewport observations, never own the world state. |
| Offline analysis | Python with NumPy and Polars, optional notebooks | Analyze recorded runs outside the execution loop. Add dependencies only when implementing their consumers. |

These are architecture selections, not a request to install packages. Exact compatible dependency versions will be selected together and locked during implementation. Hardware capability detection and CPU fallback are required; no particular GPU is assumed.

wgpu supports graphics and compute on native backends including Vulkan, Metal, and DirectX 12. That portability does not establish equivalent simulation outcomes across adapters. WGSL integer capabilities and limits must shape kernel design. Reference state uses checked integers; initial GPU kernels use bounded u32 operations with explicit overflow handling or documented multiword representations where required. Floating-point values may be used for visualization and analysis, not silently introduced into causal arithmetic.

Sources: [wgpu](https://wgpu.rs/), [WGSL specification](https://www.w3.org/TR/WGSL/), [Rayon indexed parallel iteration](https://docs.rs/rayon/latest/rayon/iter/trait.IndexedParallelIterator.html), [Postcard](https://docs.rs/postcard/latest/postcard/), [zstd](https://docs.rs/zstd/latest/zstd/), [rusqlite](https://docs.rs/rusqlite/latest/rusqlite/).

SQLite is a local catalog with one writer. WAL may support concurrent local readers when configured appropriately; it does not turn SQLite into shared network storage. Sources: [SQLite WAL](https://www.sqlite.org/wal.html).

## Proposed material world

Use a configurable three-dimensional cubic lattice, divided into 16 by 16 by 16 chunks. Begin with a finite periodic boundary. Explicit reservoir connections provide driven experiments; closed experiments disable those transfers. This is an artificial material physics with precisely defined semantics, not a claim to reproduce terrestrial chemistry or quantum mechanics.

The 3D choice permits enclosing surfaces and richer spatial assemblies without reserving a special primitive for a cell membrane. Its additional computational cost is real. Initial development sizes should be modest; larger capacities are experiment parameters, not evidence of biological realism.

### Authoritative state

- Each occupied lattice site contains at most one particle with a stable ID, immutable elemental composition, material signature, bounded internal conformation, excitation state, orientation, and position.
- Neighbor connections are bonds with explicit endpoints, ports, and stored energy. The initial geometry permits nearest-neighbor bonds. Allowed local moves preserve or explicitly alter those bonds under the material rules.
- Field arrays hold quantized heat and other explicitly specified transferable quantities. Unaccounted scalar energy or hidden information channels are prohibited.
- Fuel and spent material are ordinary particle states with specified energy differences. Reservoir transfers, if enabled, record input/output matter and energy.
- Runtime state includes the simulation tick, law version, seed, ID allocator, and any scheduling state that affects future outcomes.

There are no authoritative organism, genome, brain, parent, fitness, goal, tool, or civilization fields. Observers may infer such descriptions; their labels must never select physical transitions.

### One local interaction vocabulary

1. Translation and reorientation proposals operate on particles or bounded local bonded neighborhoods, with exclusion and bond geometry constraints.
2. Compatible adjacent ports may bind or unbind. Every accepted change accounts for bond, excitation, and heat energy.
3. Neighbor interactions can change bounded internal conformations. Their rules depend on material state and local contacts, not discovered organism identities.
4. Local catalysts can alter the probability of an otherwise specified reaction. A catalyst cannot create matter, bypass energy accounting, or recognize and duplicate a whole structure.
5. Excitation and heat can transfer along defined local contacts. All transfer budgets come from a stage snapshot.
6. Fuel transitions provide dissipative activity and spent products. Recharging requires explicitly accounted environmental energy.

Rule parameters include affinities, barriers, port compatibility, conformation transitions, and local catalytic conditions. The first implementation uses typed, bounded local rule records. It does not introduce a general programming language or an unconstrained graph-rewrite interpreter.

The exact material signatures, transition probabilities, and energy changes require a written law table before coding the kernel. Every entry must state its neighborhood, preconditions, edited state, probability rule, conservation effects, and conflict footprint. Laws are fixed during a recorded baseline experiment; interventions create separately recorded branches.

### Memory and processing without individual processors

Memory is a persistent material configuration: for example, a metastable conformation or bond arrangement. Processing is a sequence of local interactions that changes subsequent responses. The engine supplies execution resources to regions; it does not allocate an artificial mind when an observer identifies an individual.

The substrate must allow both information retention and interaction-dependent switching. Unlimited stability would make information hard to update; immediate relaxation would erase it. Energy barriers and local coupling provide an experimental way to investigate that tradeoff.

Before spontaneous-origin campaigns, constructibility demonstrations should establish that the chosen laws permit persistent distinguishable configurations, input-dependent responses, reusable catalysis, and flexible assemblies. Such designed demonstrations remain separate from unseeded emergence experiments. Showing possibility does not show spontaneous appearance or evolutionary accessibility.

## Execution architecture

### Integrated material-v2 delivery

The v2 substrate adds one bounded contact-catalysis rule, not an organism or copying action. For adjacent, unbonded reactants P and Q, inspect the four parallel lattice edges one orthogonal step away. A candidate source edge A-B must already be bonded, both source conformations must be active, A's material must equal P's and B's must equal Q's. Source and target particles must be distinct. Existing target valence, ports and excitation requirements still apply.

Let affinity be the existing P-Q material affinity in percent. The ordinary binding rate remains `binding_rate * affinity / 100`. A matching source adds `125000 * min(4, strength(A) + strength(B)) * affinity / 100`, capped with the ordinary rate at one million. Zero affinity remains zero probability. Choose the strongest eligible source, then stable source IDs to break ties. The random draw remains keyed by the existing seed, tick, stage and reactant IDs.

Only when the draw requires the added catalytic rate does the proposal reserve the source particles and sites as well as the target footprint. It creates one ordinary P-Q bond, spends the existing `bond_energy + 1` excitation cost and transfers one unit to heat. Sources are neither consumed nor copied. Two accepted contact-provenance records identify the source-target pairs; they accompany the existing binding record and transfer zero energy. Products can act as sources only by satisfying the same material, bond and conformation conditions. Physical separation uses ordinary motion; no organism-level release primitive is introduced.

Compatibility is explicit: configurations without `law_version` retain v1, newly created default worlds use v2, and invalid versions are rejected. Canonical v1 digests and native schema-1 checkpoints retain their original binary configuration layout. V2 uses a schema-2 native configuration payload and a distinct semantics identifier. Old event-kind discriminants are preserved. Browser bundles, restore, replay, catalogs and analysis must identify the actual law version.

Engineering acceptance uses a labelled designed constructibility diagnostic with two material variants, inactive/mismatched/disabled/starved controls, product-mediated further assembly with the original source relocated away, ordinary product motion, toroidal geometry, CPU/Rayon parity, native restore and full event replay. Designed starting bonds and external relocation are laboratory interventions, not spontaneous origin or demonstrated evolution. Separate unseeded runs retain all outcomes. A 10,000-tick baseline establishes a bounded execution horizon, not biological sufficiency.

The complete delivery includes the law, compatibility, provenance, existing laboratory workflow, reproducible artifacts and updated scientific status. Spontaneous heredity, adaptive benefit, intelligence and civilization remain empirical goals; none may be inferred from a passing engineering diagnostic.

The v0.7.0 acceptance record is maintained in the [consolidated roadmap](GENESIS_REVIEW_AND_ROADMAP.md#integrated-v070-delivery-and-its-limits). It includes 144 archived designed episodes, event-budget boundary checks, historical v1 replay and old-reader compatibility, a full-provenance 10,000-tick v2 baseline, restored continuation, sustained core/memory measurements, active-world CPU/Rayon agreement, 1,000-tick CPU/GPU state/event agreement and browser bundle restore. GPU speedup is not established. The fresh six-world unseeded campaign completed without supported repeated reproduction; one world lacked an eligible natural parent. The designed local interaction is feasible, but the final scientific goal remains unmet.

### Autonomous resource-feasibility correction

The follow-up investigation found that a selected six-particle natural parent had only two initially eligible feedstock particles. Counting zero products in that condition cannot test the assembly mechanism: the product definition makes a matching product impossible. Correct the external assay, not the material laws or the reproduction thresholds.

1. In `src/matter/reproduction.rs`, share the feedstock-eligibility calculation between the observer and preflight. Count required and available atoms by immutable material. Recycled-feedstock protocols additionally need at least two eligible initial components. Record impossible conditions as `insufficient-eligible-feedstock`, with the counts, rather than a completed zero-effect assay. Do not promote an ineligible case to support.
2. Preserve existing archives. Re-execute the previously impossible seed-101 condition with the corrected observer and require an explicit ineligible result with no assembly episodes. Report assayed versus ineligible world seeds separately. Validate a resource-sufficient case through the existing natural dimer protocol.
3. Run the unchanged two-particle protocol under v2 on fresh discovery seeds 107-109 and confirmation seeds 110-112. The sole change relative to the historical dimer operating condition is the explicit law version and fresh seeds. No designed initial bonds, added atoms, copying commands, changed support thresholds or retroactive pooling are permitted.
4. Use offline, locked Cargo checks and executable simulation/replay workflows. Reconcile current status with both the retrospective feasibility audit and the prospective dimer outcome. A better assay still cannot guarantee autonomous heredity or intelligence.
5. If natural dimers still show no parent-catalyzed production, distinguish kinetic impossibility from poor natural operating conditions with one continuous designed-seed diagnostic, not another universe mode. Use two initial material-pair variants, seeds 1-4, 350 scattered particles in an 8-cubed world and 2,048 ticks; compare intact, inactive-source and v1-disabled arms. Keep the existing v2 law unchanged. Set ordinary binding/breaking and conformation switching to zero, leave ordinary motion, reservoir exchange and excitation spending enabled, and arrange only one initial source bond. No feedstock positioning, source relocation or other intervention is allowed after initialization. Require at least eight event-attributed products, three catalytic generations and a descendant-mediated birth more than five toroidal lattice steps from the original source in each intact run; negative arms must produce none. Archive and replay every trajectory. This establishes only seeded autonomous material autocatalysis, not spontaneous origin, metabolism, mutation, competitive selection or intelligence.

This follow-up is now executed: all 24 continuous trajectories passed and replayed; eight intact cases produced 647 products and reached 6-9 catalytic generations, including 202 distant descendant-mediated births. All 16 controls produced none. Corrected resource preflight rejects the five impossible original chain conditions and preserves a feasible dimer's measured effects. A separate dense unseeded campaign produced isolated catalytic events but no supported causal repetition. The full goal remains unresolved; see the [current evidence and limits](GENESIS_REVIEW_AND_ROADMAP.md#continuous-seeded-autocatalysis).

Retain the current engine/law separation. Add a Matter law system owning its state, rules, conservation baseline, and observation interface. The generic runtime must remain unaware of organisms or cognition.

Each stage follows the same specified procedure:

1. Read an immutable snapshot, including the full dependency halo for the maximum rule radius.
2. Evaluate local proposals into deterministic output buffers.
3. Assign each proposal a stable address and seeded priority independent of thread scheduling.
4. Resolve overlapping write/resource footprints with a globally specified total order.
5. Apply accepted changes to candidate chunks and validate their accounting and geometry.
6. Publish only after all stages and required validation succeed.

A complete tick has a fixed order: environmental transfers, field transport, motion, and chemical/conformation changes. This ordering is part of the model; changing it changes execution semantics. Random draws are keyed by seed, tick, stage, rule identity, and stable interaction identity. Hashing storage indices or using thread-local random streams is disallowed.

Parallelism is introduced first across independent experiments, then across proposal evaluation. Conflict resolution remains a deterministic reference algorithm until an equivalent parallel method exists. Integer reductions use sufficiently wide checked accumulators. Rayon by itself does not establish reproducibility.

Replace whole-world candidate clones with immutable chunk pages and copying of touched pages. Keep a simple reference path for comparison. An active-region index must account for background reaction probabilities, field gradients, incoming contacts, reservoir schedules, and dormant configurations. A seemingly motionless region cannot be skipped merely because its last frame was unchanged.

The first exact backend still processes all causally eligible activity. Sparse execution can save work in quiet worlds; it cannot promise sublinear cost for arbitrary active detail. Approximate coarse models, if added later, receive separate semantics identifiers and accuracy studies. Newly emergent processes must not be suppressed by a premature coarse model.

## Persistence and experiment records

Replace the JSON-only 64 MiB checkpoint ceiling for the Matter backend with a bounded, versioned binary manifest and chunk files. Existing JSON worlds retain their current import/export path until separately migrated.

Checkpoint payloads include configuration, tick, particles, bonds, fields, allocator/scheduler state, and conservation baselines. Occupancy or indexes may be rebuilt only when they are completely derivable under a specified deterministic algorithm.

Publish immutable chunk files and then a checkpoint manifest. Sync data before publishing references. Store catalog entries only after artifact publication; an interrupted catalog update leaves recoverable orphan artifacts rather than references to missing state. Resume loads and validates a complete checkpoint; UI observations are not restart files.

Hash canonical uncompressed logical state. Compression versions or levels must not change logical digests. Record source revision, dependency lock digest, semantics version, rule specification, initialization method, seed, backend, hardware, and interventions. Exact replay guarantees are scoped to explicitly established backend/environment combinations.

Event retention has two modes. Full-provenance campaigns preserve every relevant accepted edit needed for causal analysis. Exploration runs may retain sampled observations and bounded event windows; their records must state that limitation and cannot support stronger historical claims. Cataloging, rendering, and compression use bounded queues. Backpressure may slow wall-clock execution but must never change simulated outcomes or silently discard required provenance.

## Observation and scientific interpretation

Observation is read-only. Connected bond components are structural descriptors, not automatically organisms. Candidate maintained boundaries, persistent reaction networks, and compartment exchange require separate measurements.

The laboratory should support:

- Material and energy accounts, configuration lifetimes, reaction fluxes, connected structures, and spatial persistence.
- Recorded interventions and matched branches with comparable matter, energy, and environmental conditions.
- Causal influence analysis of whether an existing structure promotes construction of another structure from environmental material.
- Descendant tracking as an inferred, evidence-backed relation rather than a physical reproduction command.
- Heritable variation and differential persistence or reproduction across independent initializations.
- Information-retention measurements and controlled changes to prior material states to assess whether history changes future responses.

Information retention alone is not learning. A history-dependent response must have a demonstrated functional consequence before it is described as useful memory. Learning additionally requires an experience-dependent change in future behavior, distinguished from inherited fixed responses and simple relaxation dynamics.

The initial implementation was code-only; the user's subsequent instruction to run the engine authorizes compilation, execution, replay and controlled experiments.

Open-ended evolution remains a research question. The literature distinguishes properties of evolutionary mechanisms from properties of the substrate and its compositional possibilities. Sources: [Requirements for open-ended evolution](https://arxiv.org/abs/1507.07403), [Evolutionary innovations and substrate design](https://arxiv.org/abs/1806.01883).

## Three substantial implementation deliveries

| Delivery | Complete scope | Reviewable artifact |
| --- | --- | --- |
| A Material substrate | Full law specification, dynamic 3D space, particle/bond/conformation state, fuel and heat accounting, local interactions, deterministic reference execution | A complete Matter law module and published semantics with unseeded initial-condition configurations |
| B Durable scalable runtime | Chunk snapshots, restore/forks, replay records, resource limits, parallel experiments, deterministic parallel proposals, profiling instrumentation, optional eligible GPU kernels | A headless runtime that exposes its backend semantics and produces self-contained run artifacts |
| C Emergence laboratory | Read-only inspector, experiment catalog, provenance retention, controls, structural and functional analyses, parameter/capacity studies | One integrated workflow for investigating the hypothesis and recording both positive and negative results |

These deliveries are implementation milestones, not a ladder of claims that life and intelligence will appear. All three support the same complete hypothesis. GPU simulation is included in B only where the selected law operations and future authorized comparisons support it; a GPU renderer can be delivered independently.

The empirical research decision follows the complete implementation: do fixed laws generate causally reproductive, heritable, functionally adaptive structures from unseeded matter? If not, record the result and revise the substrate hypothesis in a new law version. Do not quietly inject designed organisms or rewrite the success criteria.

## Repository integration

Keep src/law_system.rs, the published-state contract in src/engine.rs, CLI entry points and the material renderer. The primary browser controls are Play/Pause, Step, Save and Restore. Put configuration, batching, reset, recording and comparison selection in a closed Laboratory disclosure; detailed metrics and events are inspectable without dominating the normal view. Refactor checkpoint DTOs and candidate storage rather than replacing the entire application.

Proposed additions are src/matter/{config,state,motion,bonds,conformation,fields,reactions,observe}.rs, src/runtime/{chunks,proposals,conflicts}.rs, src/storage/{checkpoint,catalog,journal}.rs, and an optional src/backend/wgpu.rs. Split into these files as responsibilities become concrete; a Cargo workspace is unnecessary initially.

src/agents.rs remains an explicitly scripted comparison world. src/polymers.rs remains an explicitly templated comparison world. Neither becomes the default explanation of emergent life. Preserve explicit comparison selection and historical data formats in the Laboratory; Matter is the main experience and uses its own versioned configuration and checkpoint schemas.

The CLI starts Matter with `genesis serve`. Research uses `genesis lab run`, `genesis lab replay` and `genesis lab analyze` with the existing immutable artifact pipeline. `genesis lab help` groups reproduction, controls, profiling and historical assays. Historical top-level aliases remain supported, and archived scripts and campaign records remain unchanged.

One active research question governs new work: can fixed local material laws support causal, repeatable reproduction? Memory findings remain bounded diagnostic evidence. Add an assay or configuration option only when its possible outcomes change a stated decision, rather than turning every protocol revision into a permanent public feature. See the [current roadmap](GENESIS_REVIEW_AND_ROADMAP.md) for evidence and outstanding acceptance gates.

No LLM, per-individual database, per-individual operating-system thread, automatic technology rewards, organism lifecycle API, or real-time wall-clock time acceleration is needed in the Matter law. Simulated time remains ticks of specified transitions, with wall-clock throughput recorded separately.

## Feasibility decisions and budget

Before kernel implementation, settle the material law table, interaction radius, occupancy/bond geometry, allowed local moves, probability mapping, energy ledger, and which configurations can retain and transmit information. Flexible assemblies and non-equilibrium driving are essential hypotheses to examine, not evidence already obtained.

Hardware sizing is based on measured bytes per occupied particle, bonds, field cells, active chunks, and candidate/proposal buffers. Set explicit memory, event-retention, and run-horizon budgets. Increasing capacity is an experimental intervention; the engine must not shrink observations, drop state, or alter laws to fit memory silently.

Do not buy hardware or choose an accelerator-specific architecture before profiling. CUDA is an optional later specialist backend if a confirmed NVIDIA-only deployment and measured bottleneck justify it. The initial portability choice remains Rust plus wgpu.

There is no calendar promise for spontaneous life or civilization. The useful deliverable is a faithful, observable computational physics and a defensible method for determining what it actually produces.
