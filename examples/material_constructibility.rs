use std::{fs, path::{Path, PathBuf}};
use genesis::{
    engine::Engine, experiment::RunOptions,
    matter::{MatterConfig, MatterLaw, MatterState, Space3},
    persistence::write_new_json, storage::checkpoint, universe::Universe,
};
use serde_json::{json, Value};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn configuration(seed: u64, version: u32) -> Result<MatterConfig> {
    let mut specification = serde_json::to_value(MatterConfig::default())?;
    specification["law_version"] = json!(version);
    let mut config: MatterConfig = serde_json::from_value(specification)?;
    config.seed = seed;
    config.space = Space3 { width: 8, height: 8, depth: 8 };
    config.particles = 12;
    config.initial_heat = 0;
    config.initial_excitation = 16;
    config.binding_rate = 0;
    config.breaking_rate = 0;
    config.motion_rate = 0;
    config.exchange_rate = 0;
    config.reservoir.enabled = false;
    config.execution.event_limit = 128;
    for material in &mut config.materials {
        material.ports = 63;
        material.valence = 1;
        material.affinities = [100; 4];
        material.conformation_energy = 1;
        material.switching_rate = 0;
        material.coupling = 0;
        material.catalytic_strength = 1;
    }
    Ok(config)
}

fn ids(variant: u64) -> [u64; 6] {
    let offset = variant * 2;
    [1 + offset, 2 + offset, 5 + offset, 6 + offset, 9 + offset, 10 + offset]
}

fn relocate(state: &mut MatterState, changes: &[(u64, [u32; 3])]) -> Result<()> {
    for &(id, _) in changes {
        let particle = state.particle(id).ok_or("missing relocation particle")?;
        state.set_occupant(particle.position, 0);
    }
    for &(id, xyz) in changes {
        let mut particle = state.particle(id).ok_or("missing relocation particle")?;
        particle.position = state.space.position(xyz);
        state.write_particle(particle)?;
        state.set_occupant(particle.position, id);
    }
    for &(id, _) in changes {
        let mut particle = state.particle(id).ok_or("missing relocated particle")?;
        let neighbors: Vec<_> = particle.bonds.into_iter().filter(|&other| other != 0).collect();
        particle.bonds = [0; 6];
        for other in neighbors {
            let neighbor = state.particle(other).ok_or("missing bond endpoint")?;
            let direction = state.space.direction(particle.position, neighbor.position)
                .ok_or("relocation broke bond geometry")?;
            particle.bonds[direction] = other;
        }
        state.write_particle(particle)?;
    }
    Ok(())
}

fn initial(config: &MatterConfig, variant: u64, condition: &str) -> Result<Engine<MatterLaw>> {
    let mut state = MatterState::initial(config)?;
    for id in 1..=12 {
        let particle = state.particle(id).ok_or("missing initial particle")?;
        state.set_occupant(particle.position, 0);
    }
    for id in 1..=12 {
        let mut particle = state.particle(id).ok_or("missing initial particle")?;
        particle.position = state.space.position([(id % 8) as u32, (id / 8) as u32, 6]);
        particle.orientation = 0;
        particle.conformation = 1;
        if condition == "starved" { particle.excitation = 0; }
        state.write_particle(particle)?;
        state.set_occupant(particle.position, id);
    }
    let [a, b, p, q, _, _] = ids(variant);
    let x = if condition == "wrapped" { 7 } else { 2 };
    let right = (x + 1) % 8;
    let (left_target, right_target) = if condition == "mismatch" { (q, p) } else { (p, q) };
    relocate(&mut state, &[(a, [x, 2, 2]), (b, [right, 2, 2]),
        (left_target, [x, 3, 2]), (right_target, [right, 3, 2])])?;
    let mut first = state.particle(a).ok_or("missing source")?;
    let mut second = state.particle(b).ok_or("missing source")?;
    first.bonds[0] = b;
    second.bonds[1] = a;
    if condition == "inactive" {
        first.conformation = 0;
        second.conformation = 0;
        state.add_heat(first.position, 1)?;
        state.add_heat(second.position, 1)?;
    }
    state.write_particle(first)?;
    state.write_particle(second)?;
    Ok(Engine::new(state, MatterLaw::new(config.clone())?)?)
}

fn episode(engine: &Engine<MatterLaw>, config: &MatterConfig, path: &Path, steps: u64) -> Result<Universe> {
    let universe = Universe::restore_bundle(&checkpoint::bundle(engine, config)?)?;
    let outcome = genesis::matter::lab::run(universe, path, RunOptions {
        steps, sample_every: 1, checkpoint_every: steps, archive_limit_mib: 64,
        label: "designed-constructibility-not-spontaneous-origin".into(), ..Default::default()
    })?;
    if !outcome.completed { return Err(format!("episode stopped: {:?}", outcome.error).into()); }
    genesis::matter::lab::replay(path)?;
    Ok(Universe::load_checkpoint(&path.join("final.gns"))?)
}

fn bonded(universe: &Universe, a: u64, b: u64) -> Result<bool> {
    Ok(universe.material_state().ok_or("missing material state")?
        .particle(a).ok_or("missing tracked particle")?.bonds.contains(&b))
}

fn boundary_checks() -> Result<Value> {
    for limit in [1, 2] {
        let mut config = configuration(1, 2)?;
        config.execution.event_limit = limit;
        let mut engine = initial(&config, 0, "intact")?;
        let mut rejected = false;
        for _ in 0..32 {
            let generation = engine.generation();
            let committed = checkpoint::bundle(&engine, &config)?;
            if let Err(error) = engine.step() {
                if !error.to_string().contains("event budget exceeded")
                    || engine.generation() != generation
                    || checkpoint::bundle(&engine, &config)? != committed {
                    return Err("event-budget failure changed committed state or reported the wrong error".into());
                }
                rejected = true;
                break;
            }
        }
        if !rejected || engine.world().particle(5).ok_or("missing atomic target")?.bonds.contains(&6) {
            return Err("catalytic event-budget boundary was not exercised atomically".into());
        }
    }
    let mut config = configuration(1, 2)?;
    config.execution.event_limit = 2;
    config.execution.full_provenance = false;
    let mut engine = initial(&config, 0, "intact")?;
    for _ in 0..32 {
        engine.step()?;
        if engine.world().particle(5).ok_or("missing boundary target")?.bonds.contains(&6) {
            if !engine.world().events_truncated || engine.world().events.len() != 2 {
                return Err("bounded non-full provenance did not disclose truncation".into());
            }
            break;
        }
    }
    if !engine.world().particle(5).ok_or("missing boundary target")?.bonds.contains(&6) {
        return Err("bounded non-full provenance prevented material assembly".into());
    }
    let mut config = configuration(1, 2)?;
    config.materials[0].affinities[1] = 0;
    config.materials[1].affinities[0] = 0;
    let mut engine = initial(&config, 0, "intact")?;
    for _ in 0..32 { engine.step()?; }
    if engine.world().particle(5).ok_or("missing zero-affinity target")?.bonds.contains(&6) {
        return Err("contact catalysis bypassed zero material affinity".into());
    }
    let mut config = configuration(1, 2)?;
    config.binding_rate = 1_000_000;
    let mut engine = initial(&config, 0, "intact")?;
    engine.step()?;
    if !engine.world().particle(5).ok_or("missing ordinary target")?.bonds.contains(&6)
        || engine.world().events.iter().any(|event|
            matches!(event.kind, genesis::matter::EventKind::ContactCatalysis)) {
        return Err("ordinary binding acquired false catalytic provenance".into());
    }
    Ok(json!({"full_provenance_limits": [1,2], "atomic_failure":true,
        "bounded_provenance_discloses_truncation":true, "zero_affinity_preserved":true,
        "ordinary_binding_has_no_false_catalytic_provenance":true}))
}

fn main() -> Result<()> {
    let mut arguments = std::env::args().skip(1);
    let output = PathBuf::from(arguments.next().ok_or("new output directory required")?);
    if arguments.next().is_some() { return Err("unexpected argument".into()); }
    fs::create_dir(&output)?;
    write_new_json(&output.join("protocol.json"), &json!({
        "initialization": "designed bonded active source, active unbonded feedstock; not spontaneous origin",
        "seeds": [1,2,3,4,5,6,7,8], "variants": [[0,1],[2,3]], "horizon": 32,
        "conditions": ["intact","inactive","mismatch","starved","disabled","wrapped"],
        "interventions": "source relocation and fresh-feedstock positioning before product assay; zero positional work in this model",
        "claim_limit": "local material constructibility and product-mediated assembly; not unseeded heredity, selection, metabolism or life"
    }))?;
    let boundaries = boundary_checks()?;
    let mut rows: Vec<Value> = Vec::new();
    for variant in 0..2 {
        for seed in 1..=8 {
            let [a, b, p, q, r, s] = ids(variant);
            for condition in ["intact", "inactive", "mismatch", "starved", "disabled", "wrapped"] {
                let config = configuration(seed, if condition == "disabled" { 1 } else { 2 })?;
                let engine = initial(&config, variant, condition)?;
                let path = output.join(format!("variant-{variant}-seed-{seed}-{condition}"));
                let final_world = episode(&engine, &config, &path, 32)?;
                let formed = bonded(&final_world, p, q)?;
                let expected = matches!(condition, "intact" | "wrapped");
                if formed != expected { return Err(format!("constructibility gate failed: {path:?}").into()); }
                let mut row = json!({"variant":variant,"seed":seed,"condition":condition,"matching_bond_formed":formed,
                    "digest":final_world.digest()?,"replay":true});
                if condition == "intact" {
                    let mut parallel = config.clone();
                    parallel.execution.parallel = true;
                    let parallel_engine = engine.fork_with(MatterLaw::new(parallel.clone())?)?;
                    let parallel_path = output.join(format!("variant-{variant}-seed-{seed}-parallel"));
                    let parallel_world = episode(&parallel_engine, &parallel, &parallel_path, 32)?;
                    if parallel_world.digest()? != final_world.digest()? {
                        return Err("CPU/Rayon constructibility state mismatch".into());
                    }
                    for tick in 1..=32 {
                        let filename = format!("events/{tick:020}.zst");
                        if fs::read(path.join(&filename))? != fs::read(parallel_path.join(&filename))? {
                            return Err("CPU/Rayon constructibility event mismatch".into());
                        }
                    }
                    row["cpu_rayon_state_and_events_match"] = json!(true);
                    let (engine, config, _) = checkpoint::load(&path.join("final.gns"))?;
                    let mut state = engine.world().clone();
                    relocate(&mut state, &[(a, [2, 7, 2]), (b, [3, 7, 2]),
                        (r, [2, 4, 2]), (s, [3, 4, 2])])?;
                    state.events.clear();
                    state.events_truncated = false;
                    let parts = engine.binary_parts_with(&state)?;
                    let product_engine = Engine::restore_parts(MatterLaw::new(config.clone())?, parts, state)?;
                    let product_path = output.join(format!("variant-{variant}-seed-{seed}-product"));
                    let product_world = episode(&product_engine, &config, &product_path, 32)?;
                    if !bonded(&product_world, r, s)? {
                        return Err("product failed to mediate assembly with original source relocated".into());
                    }
                    let (engine, mut moving, _) = checkpoint::load(&product_path.join("final.gns"))?;
                    moving.motion_rate = 1_000_000;
                    let motion = engine.fork_with(MatterLaw::new(moving.clone())?)?;
                    let motion_path = output.join(format!("variant-{variant}-seed-{seed}-motion"));
                    let motion_world = episode(&motion, &moving, &motion_path, 8)?;
                    if !bonded(&motion_world, p, q)? || !bonded(&motion_world, r, s)? {
                        return Err("ordinary motion destroyed a product bond".into());
                    }
                    if !motion_world.material_events().ok_or("missing motion provenance")?.0.iter().any(|event|
                        matches!(event.kind, genesis::matter::EventKind::Move) && [p,q].contains(&event.particles[0])) {
                        return Err("ordinary product motion was not observed".into());
                    }
                    row["product_mediated_further_assembly"] = json!(true);
                    row["ordinary_bond_preserving_motion"] = json!(true);
                }
                rows.push(row);
            }
        }
    }
    write_new_json(&output.join("result.json"), &json!({"completed":true,"episodes":144,"rows":rows,"boundary_checks":boundaries,
        "interpretation":"Designed local catalysis and product-mediated further assembly, not spontaneous reproduction or evolution."}))?;
    println!("Completed 144 archived constructibility, product, motion and parallel episodes with exact replay.");
    Ok(())
}
