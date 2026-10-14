# Autonomous maintenance and repeated production investigation

This is the active laboratory research question, not a reproduction mode granted to the universe. Use `genesis lab reproduction` for the existing assay and `genesis lab run`, `genesis lab replay` and `genesis lab analyze` for its shared artifact workflow. Historical commands and frozen campaign records remain unchanged.

The fixed-law hypothesis is that naturally formed material chains promote assembly of separate chains with the same material sequence from initially unbonded feedstock, and their products can promote further production. Energy exchange and damage recovery are evaluated separately. No organism, reproduction action, recipe, brain or fitness reward is added to the material laws.

The historical v1 chemistry allows conformation-dependent catalysis to lower nearby binding and unbinding barriers, but supplies no sequence-specific copying interaction. Material v2 additionally supplies matching local contact catalysis for one ordinary bond, not whole-chain copying. Specifications without `law_version` remain v1; the current runner explicitly selects v2. Autocatalytic activity and evolutionary heredity are distinct requirements; see [Self-Reproduction and Darwinian Evolution in Autocatalytic Chemical Reaction Systems](https://pmc.ncbi.nlm.nih.gov/articles/PMC8066523/) and [Universal motifs and the diversity of autocatalytic systems](https://pmc.ncbi.nlm.nih.gov/articles/PMC7568248/).

## Current integrated delivery campaign

The v0.7.0 runner defaults to [the unseeded v2 specification](../experiments/matter-v2.json), the existing chain protocol, discovery seeds 101-103 and independent confirmation seeds 104-106. The historical configurations and frozen campaigns below are not upgraded or pooled with this result.

```powershell
cargo build --offline --locked --release --bins --examples --target-dir target\delivery
.\analysis\run_reproduction.ps1 -Out runs\a-new-v2-reproduction-campaign
```

The [original completed fresh campaign](../runs/material-v2-reproduction-20261014/workflow-result.json) has no supported worlds, but its assembly interpretation needed correction. Seed 104 had no natural parent; all other worlds lacked the material types required by their product signatures. Their 60 archived zero-product episodes are resource-ineligible, not informative negative mechanism tests. The [corrected seed-101 report](../runs/material-v2-resource-correction-20261014/reproduction.json) and [remaining corrected worlds](../runs/material-v2-resource-correction-confirmation-20261014/reproduction.json) retain the necessary counts. No archived result is overwritten.

Before an assay, required material counts must fit within the eligible initial pool, excluding parent and forbidden original-parent atoms. Recycled-feedstock protocols also need at least two initial components containing usable material. Impossible conditions return `insufficient-eligible-feedstock` with counts. Reports distinguish `assayed_world_seeds` and `ineligible_world_seeds`; old executables that do not record eligibility are explicitly labelled as such by the runner.

The separate [natural v2 dimer campaign](../runs/material-v2-natural-dimers-20261014/workflow-result.json), seeds 107-112, completed 72 assembly episodes with 61 global matching observations and no matching parent-contact products or parent-catalyzed bindings. A [resource-sufficient seed-107 rerun](../runs/material-v2-resource-sufficient-validation-20261014/reproduction.json) preserves all five effect summaries under the corrected observer. Accepted source-target provenance is reported as `parent_catalytic_contact_events` and `parent_catalyzed_bindings`, without changing support criteria.

Separately, the [designed constructibility diagnostic](../runs/material-v2-delivery-diagnostics-20261014/result.json) passed 144 archived episodes and boundary checks. It deliberately arranges active source bonds and feedstock; product-mediated further assembly includes external source relocation and feedstock positioning. It is not autonomous origin, ecological reproduction, variant inheritance or selection.

```powershell
.\target\delivery\release\examples\material_constructibility.exe runs\a-new-designed-diagnostic
```

Its two material variants and disabled, inactive, mismatched and starved controls establish a local material capability, not completion of Genesis's scientific goal. The [delivery status](../GENESIS_REVIEW_AND_ROADMAP.md#integrated-v070-delivery-and-its-limits) records the separate 10,000-tick unseeded baseline and interpretation limits.

## Continuous copying without later interventions

The assayed-world fields describe completion of at least one first-generation assay in that world. Descendant eligibility remains a separate recorded status; an assayed world is not automatically a supported lineage.

The [continuous diagnostic](../runs/material-v2-autonomous-catalysis-20261014/result.json) complements, rather than replaces, the natural-world protocol. It starts with one designed source bond, scattered feedstock and explicitly active initial chemistry; no intervention occurs after initialization. Two variants, four seeds and intact/inactive/v1-disabled conditions run for 2,048 ticks each. The v2 law itself is not changed.

Eight intact runs produce 79-82 matching products each and reach 6-9 event-attributed catalytic generations. All 16 controls produce zero products. The 24 trajectories replay; all initial energies are 8,000 and the final reservoir ledgers agree. Descendant-mediated births beyond five toroidal steps of the original source total 202. This distance is checked at birth, not continuously over a separate descendant horizon. No mutation, competition, metabolic maintenance or intelligence claim follows from these counts.

```powershell
cargo build --offline --locked --release --examples --target-dir target\delivery
.\target\delivery\release\examples\material_autocatalysis.exe runs\a-new-continuous-diagnostic
```

The later [dense unseeded comparison](../runs/material-v2-dense-unseeded-20261014/workflow-result.json), seeds 113-118, uses no initial source bonds or active conformations. Its six first-generation parent-catalyzed bindings and six matching contact-product observations are isolated results: no world passes causal or stronger repeated-production gates. Keep these results separate from the seeded matrix and retain the original thresholds.

All six selected dense templates initially had one active atom rather than the two required for v2 contact catalysis. They can become competent through ordinary switching during the episode. A future readiness-based selection must be prospective and must retain ineligible and negative cases rather than reclassify this completed campaign as successful.

## Frozen experiment

Discovery worlds: 31–33. Independent confirmation worlds: 34–36. Configuration: the existing 16³, 750-particle divalent matter preset, with motion 100,000 and breaking 500 per million. All matter starts unbonded and unseeded. Warmup is 1,000 ticks; graph-stability interval is 20 ticks. Select one 3–6-particle open chain per world by decreasing size, then ID, with unchanged bond identities across that interval and at least one active conformation. Endpoint agreement does not establish uninterrupted stability.

Each selected parent receives four paired future streams, 256-tick episodes, and these controls:

- Intact parent.
- Initial erasure of its active conformations.
- Remote erasure of the same number of active conformations, matching material types and exact bath work, beyond six lattice steps.

Heat, excitation, geometry and bonds agree across these initial controls. Structural energy of the intact branch can differ; the local and remote erase branches remove the same amount. Erasure occurs only once and can be undone by subsequent ordinary dynamics. Active-exposure measurements explicitly check this limitation.

## Products and lineage

Only atoms that were individually unbonded at an episode's start are eligible feedstock. A product uses none of the parent atoms, is a separate open-chain component, and retains the exact same graph for 20 consecutive ticks. Every atom contributes to at most one counted product per episode, so fragmentation, repeated counting of the same object, and reuse of a parent fragment cannot satisfy this definition.

A matching product has the parent's material sequence, canonicalized against reversal. A contact product must contain an atom involved in an accepted binding event within one lattice step of the parent. This is a contact association, not by itself proof of catalytic causation. Other same-sized material sequences are counted to measure matching enrichment rather than generic assembly.

The first qualified matching product with active conformation is selected by replica order and chronological qualification, independently of response effect sizes. Its checkpoint becomes the descendant assay's source. The original parent's connected structures are translated to vacant sites beyond five lattice steps, preserving their bonds, material and energy. If this is impossible, the assay is explicitly ineligible. Descendant independence is rejected if the original parent subsequently comes within five steps, or if product assembly directly contacts it. Relocation is an external laboratory intervention with zero work only under this artificial model's lack of position-dependent potential.

## Maintenance and recovery

Measure original-parent graph retention, active-conformation exposure, charging and dissipation events. Retention plus energy exchange is not a complete definition of metabolism or self-maintenance.

Damage recovery removes the first canonical parent bond, releasing its stored energy into heat without adding external energy. Compare driven and reservoir-disabled episodes under each paired future stream. Recovery means the original graph returns and persists for 20 ticks. Closed controls retain their initial thermal energy; reservoir disabling is not immediate energy starvation. Random rebinding can produce recovery without adaptive repair.

## Criteria and limits

Causal matching assembly requires a positive paired intact-minus-local-erasure production interval, a positive active-exposure interval, and the remote-control interval entirely inside ±0.25 products. The remote criterion is an explicit equivalence margin, not an inference from a nonsignificant difference. Intervals use mean ±2.2 standard errors and remain exploratory, uncorrected for multiple comparisons.

Stronger repeated-production support additionally requires matching-fraction enrichment, at least 90% parent graph retention with nonzero charging in every intact trial, a positive driven-minus-closed damage-recovery interval, and the same criteria in the descendant assay. Every failed gate, missing product and ineligible control is retained. Absence of support over these seeds and horizons is not a proof of impossibility.

Even stronger support would not alone establish heritable evolution: that requires variant-specific inheritance and differential success under competition. The report does not automatically classify matching chains as organisms or descendants as biological offspring.

## Disclosed smaller-structure assay

The initial six-world campaign completed with no supported repeated production and no qualifying six-particle products. This is not a proof that reproduction is impossible; it also indicates a poorly responsive product assay at that size and horizon.

A separate prospective assay uses discovery seeds 43–45 and confirmation seeds 46–48, two-particle chains, valence one for every material, breaking probability 100 per million, and 500 warmup ticks. All other material tables and the 256-tick, four-stream control protocol remain recorded explicitly in `experiments/matter-dimer-reproduction-spec.json` and `experiments/matter-dimer-reproduction-protocol.json`. Nothing is seeded as a dimer: both components must assemble under ordinary laws from scattered atoms. The previous campaign remains intact and its results are not pooled with the revised assay.

This simpler operating condition increases observability of separate products and can exercise descendant controls. It does not provide a copying recipe or establish hereditary biology.

```powershell
./analysis/run_reproduction.ps1 -Out runs/a-new-dimer-campaign -Spec experiments/matter-dimer-reproduction-spec.json -Protocol experiments/matter-dimer-reproduction-protocol.json -DiscoverySeeds 43,44,45 -ConfirmationSeeds 46,47,48
```

## Original campaign command

```powershell
cargo build --release --target-dir target\delivery
.\analysis\run_reproduction.ps1 -Out runs\a-new-v1-reproduction-campaign -Spec runs\autonomous-reproduction-20261014-102534\inputs\spec.json -Protocol runs\autonomous-reproduction-20261014-102534\inputs\protocol.json -DiscoverySeeds 31,32,33 -ConfirmationSeeds 34,35,36
```

Both phases run without intermediate approvals. The runner freezes and hashes its executable and inputs. Every episode retains full event provenance, native checkpoints, a journal, `assembly.json` or `repair.json`, and a deterministic replay path.

## Energy-gradient and recycled-feedstock hypothesis

After the first two campaigns, two measurement and mechanism limits are explicit. First, initially unbonded feedstock was sparse after warmup; rejecting every atom from an existing molecule also excludes legitimate resource recycling. Second, catalysis only lowers activation thresholds in these laws. Where excitation already exceeds those thresholds, lowering them need not change binding eligibility.

The third prospective campaign uses fresh discovery worlds 56–58 and confirmation worlds 59–61. Matter starts with zero heat and excitation. A reservoir supplies four units per source site per tick, capped at six, with a four-unit sink. The hypothesis is that a driven low-energy gradient can leave some material close enough to an activation threshold for an active structure to affect assembly. Active conformations can also persist when local excitation becomes insufficient for switching; whether that helps repeated production remains to be measured.

This campaign keeps the dimer material tables, valence one and breaking rate 100 per million. It uses 1,000 warmup ticks, a 20-tick stability interval, eight paired future streams, and 512-tick episodes. Configuration and protocol are `matter-gradient-reproduction-spec.json` and `matter-gradient-reproduction-protocol.json`; no structures or conformations are seeded.

Bonded environmental matter is eligible feedstock in this declared observer mode. The observer records each atom's initial connected-component identity. A counted product must combine atoms from at least two initial components and remain disjoint from the parent and any excluded ancestor. Consequently, breaking one pre-existing molecule into pieces cannot qualify: at least one new bridge between initially separate components is required. Every atom is still counted at most once, and the contact, stable-graph, counterfactual, enrichment, maintenance and recovery criteria remain unchanged. This is a revised assay, not pooled confirmation of the earlier definition.

```powershell
./analysis/run_reproduction.ps1 -Out runs/a-new-gradient-campaign -Spec experiments/matter-gradient-reproduction-spec.json -Protocol experiments/matter-gradient-reproduction-protocol.json -DiscoverySeeds 56,57,58 -ConfirmationSeeds 59,60,61
```

The separate `matter-product-audit` command reconstructs a previously recorded global matching product, verifies its source episode and stable graph, relocates the original parent, and runs diagnostic controls. Since its formation need not be parent-associated, the diagnostic always reports that it does not establish the original reproduction claim.
