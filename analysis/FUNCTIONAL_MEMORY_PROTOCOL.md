# Local material response experiment

This is a specialised laboratory diagnostic, not a main universe mode or the active research priority. The primary experience is Matter; the current research question is causal, repeatable reproduction. Commands below use the consolidated `genesis lab` entry point. Historical top-level aliases and archived workflows remain supported.

Run from the repository root:

```powershell
.\target\validation\release\genesis.exe lab memory --spec experiments\matter-memory-spec.json --protocol experiments\matter-memory-protocol.json --seeds 1,2,3 --out runs\my-memory-campaign
```

The output directory must be new. This command executes and archives simulations.

Each seed starts with unbonded matter and warms up under the configured laws. All later phases use the same laws with the continuous reservoirs disabled. A cube receives a conditioning heat pulse; the matched unconditioned branch receives exactly the same amount of heat in a remote cube. Both evolve through a delay.

A third history copies conformations from the delayed unconditioned branch into the conditioned branch, matching by immutable particle ID. It retains the conditioned positions, orientations and bonds. This is specifically a conformation-erasure control, not complete removal of physical history.

All three histories then have their heat fields and particle excitation reset to the shared preconditioning checkpoint. These bath operations explicitly record energy input and output. Heat is matched by site; excitation is matched by particle ID. Positions, occupancy and bonds can differ, so excitation density at a site can still differ. Structural energy can also differ. Each intervention preserves the original baseline and passes the engine's conservation and structure checks.

Each matched history is forked into two probe trials: local heat pulse versus equal-energy remote heat pulse. Their start generation, pulse magnitude, response duration, random seed and laws agree. Native checkpoints capture the intervened initial states, and full event batches capture subsequent dynamics. Each trial supports `genesis lab replay --run DIRECTORY`. The protocol records how those initial states were constructed; replay of one trial verifies its evolution, not the whole conditioning protocol.

The predefined primary response is the number of conformation switches whose event position lies in the fixed local cube during the response window. Bond events touching that cube and regional endpoint measurements are secondary readouts. The regions remain fixed in space; the measurement does not follow moving particles.

For each seed:

```
unconditioned contrast = unconditioned local-probe switches - unconditioned remote-probe switches
conditioned contrast   = conditioned local-probe switches   - conditioned remote-probe switches
erased contrast        = erased local-probe switches        - erased remote-probe switches

history effect = conditioned contrast - unconditioned contrast
history effect after conformation erasure = erased contrast - unconditioned contrast
conformation erasure effect = conditioned contrast - erased contrast
```

All counts and per-generation switch traces appear in `memory.json`; every seed also has `result.json` and `history-differences.json`. `protocol.json` fixes the measurements before execution. Failed or truncated segments are rejected; their existing run artifacts remain available, and no completed campaign verdict is written.

These are exploratory paired contrasts, not statistical significance results. Remote stimulation can reach the local region during the probe window. Conditioning can alter geometry, and conformation erasure does not undo those changes. Nonzero contrasts may motivate further experiments; they do not establish adaptive benefit, intelligence, learning, reproduction or life. A zero contrast does not rule out memory in other variables, regions, durations or conditions.

## Responsiveness screen

```powershell
.\target\validation\release\genesis.exe lab response --spec experiments\matter-memory-spec.json --protocol experiments\matter-memory-protocol.json --seeds 1,2,3 --heats 12,64 --out runs\my-response-screen
```

The screen compares warm, warm with reduced motion, cold, and cold with reduced motion. Reduced motion is a configured probability of 100,000 per million. Cold starts have zero heat, zero excitation and no continuous reservoirs; other material rules and randomly scattered matter remain the same. The configured warmup runs separately for each arm and seed. Reservoirs are disabled consistently after warmup.

For each arm, seed and pulse magnitude, equal-energy local and remote pulses evolve for the protocol delay. Checkpoints and event archives are divided at elapsed generations 1, 5, 20, 100 and the final delay, omitting times beyond that delay. Regional charging, motion and switching counts describe each interval; global switching and paired state differences distinguish autonomous activity from pulse-dependent activity.

Both final states are thermally matched to their common pre-pulse checkpoint. The retained difference counts changed positions, orientations, bonds and conformations; it excludes excitation and heat. This measures physical state retention, not useful memory.

The selection rule is recorded before screening: prefer the lowest pulse at which every screen seed retains a conformation difference at the final delay. Conditions selected from these data must use fresh seeds for later memory evaluation. Screening is exploratory and does not establish an optimized configuration or improved physical realism. Results, trajectories and retained-state comparisons are in `screen.json` and per-pair `response.json` files. Every individual evolution segment can be replayed with the standard replay command.

## Cluster-specific repeated probes

```powershell
.\target\validation\release\genesis.exe lab cluster-memory --spec experiments\matter-memory-responsive-spec.json --protocol experiments\matter-cluster-memory-protocol.json --seeds 7,8,9 --out runs\my-cluster-memory
```

This experiment selects bonded components before conditioning. Their positions, orientations and bonds must agree at both endpoints of a stability interval. Endpoint agreement does not prove uninterrupted stability. Size and ID determine selection; future probe outcomes do not.

Conditioning deposits heat at the selected cluster's occupied sites, or at an equal-size remote set. After the delay, the conformation differences among selected particles are recorded. Candidates with no conformation difference, or without a suitable remote erasure control, are retained as explicit negative/ineligible results.

Localized erasure copies only the cluster's differing conformations from the unconditioned branch. Remote erasure copies the same number of differences elsewhere, matching material type and flip direction. Remote targets must be at least six lattice steps from every cohort particle and outside the cohort's connected component. Local and remote erasure input/output work is checked exactly. Both preserve geometry and bonds.

All histories are quenched to zero heat and zero excitation through the energy ledger. Conformation and bond potential energy remain; this is not an equalization of structural energy. Probes then apply the same heat pulse to the cluster's input particle or to a remote site. The primary readout is the time integral of excitation on the other tracked particles; their identities remain fixed even if they move.

Four histories are evaluated: unconditioned, conditioned, locally erased and remotely erased. Replicas restore the same stored histories but use distinct, paired future random seeds. They quantify random-stream variation conditional on a selected stored state, not independent new-world variation. Every probe has an immutable initial/final checkpoint, full event archive and a per-generation receiver trace.

The exploratory candidate rule requires a retained marker, identical cohort geometry across conditioning histories, an approximate paired interval for local erasure that excludes zero, and exactly zero paired remote-erasure effects. The interval is mean plus/minus 2.2 standard errors; it is an exploratory heuristic, not an exact confidence guarantee or multiplicity-corrected result. Candidate discovery needs fresh-world confirmation.

Clusters remain in the full simulated environment; they are not physically excised. Conditioning, erasure, quenching and observational input/output designation are laboratory operations. A readout effect establishes neither adaptive usefulness nor self-maintaining biological memory, and it does not demonstrate a novel scientific breakthrough.

## Complete spatial-memory workflow

```powershell
./analysis/run_material_memory.ps1 -Out runs/my-material-memory
```

The runner performs conditioning and geometry audits for discovery worlds 22–24 and independent confirmation worlds 25–27. Its current preset selects naturally formed three-particle open chains by size and particle ID before conditioning. Bond identities must agree across the stability interval; pose need not agree. All initial matter is scattered, with no seeded chain. The preset uses reduced motion, divalent materials and a reduced bond-breaking probability, explicitly recorded in the configuration.

After conditioning, both branches retain only shared internal bonds, lose cohort-external bonds and are cooled through the energy ledger. A cohort must remain connected. Local restoration copies the unconditioned cohort positions and orientations into the conditioned branch while preserving conditioned conformations and shared bond identities. The matched remote operation applies the same displacements and orientation changes to distant, unbonded particles of the same materials. Occupied restoration destinations and unavailable matched controls are explicit ineligible cases.

The current primary readout is restricted first-excitation latency at the opposite initial endpoint over 32 ticks. No observed excitation is encoded as 33 and marked censored in each `receiver-trace.json`; this value is not an observed arrival time. Excitation integrals and full receiver traces remain available. Earlier protocols used excitation integrals, so their estimates cannot be pooled with latency estimates.

Each eligible cohort receives 64 paired future random streams across four histories and local/remote probes. These are repeated probes of the same stored histories, not 64 independent worlds. Support requires opposite-signed history and local-restoration effects, approximate intervals excluding zero, agreement of both replica halves, and exactly zero remote-restoration effects. All negative and ineligible cases remain in the report. These exploratory intervals are not multiplicity-corrected. Spatial restoration has zero work only under this artificial model, which has no position-dependent potential energy.

## Exact inactive-state fast-forward

```powershell
.\target\validation\release\genesis.exe lab skip --checkpoint stored.gns --steps 1000000000000 --out later.gns
```

The engine permits this operation only for Matter with reservoirs disabled, zero particle excitation, and heat at most one in every cell. Under these laws no motion, binding, switching, unbinding, energy transfer, charging or heat diffusion can occur. The engine checks the generation counter, validates the candidate and energy ledger, clears transient event records, and publishes a new immutable checkpoint. Other models and active worlds are rejected. Skipping does not fabricate event journals for unexecuted ticks.

This preserves inactive physical memory with work independent of the skipped duration. It provides no acceleration of active chemistry or civilizations, and ticks have no calibrated conversion to terrestrial years. Native checkpoint loading and continued normal stepping remain available.
