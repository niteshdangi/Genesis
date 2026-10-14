use std::{collections::BTreeMap,fs,path::{Path,PathBuf}};
use genesis::{
    engine::Engine,experiment::RunOptions,law_system::SimulationError,
    matter::{MatterConfig,MatterLaw,MatterState,Space3,EventKind,lab,cluster_memory::distance},
    persistence::write_new_json,storage::checkpoint,universe::Universe,
};
use serde_json::{json,Value};

type Result<T> = std::result::Result<T,Box<dyn std::error::Error>>;

fn pair(a:u64,b:u64)->(u64,u64) {(a.min(b),a.max(b))}

fn initial(seed:u64,variant:u64,condition:&str)->Result<(Universe,[u64;2])> {
    let mut config=MatterConfig::default();
    config.law_version=if condition=="v2" {2} else {3};
    config.space=Space3 {width:8,height:8,depth:8};
    config.seed=seed;config.particles=350;config.initial_excitation=16;
    config.binding_rate=0;config.breaking_rate=0;config.motion_rate=100_000;
    config.execution.event_limit=8192;
    for material in &mut config.materials {
        material.ports=63;material.valence=1;material.affinities=[100;4];
        material.conformation_energy=1;material.switching_rate=0;
        material.coupling=0;material.catalytic_strength=1;
    }
    let mut state=MatterState::initial(&config)?;
    for id in 1..=state.count() as u64 {
        let mut p=state.particle(id).ok_or("missing initial particle")?;
        p.conformation=1;p.orientation=0;state.write_particle(p)?;
    }
    let source=[1+2*variant,2+2*variant];
    let mut a=state.particle(source[0]).ok_or("missing source")?;
    let mut b=state.particle(source[1]).ok_or("missing source")?;
    let target=state.space.neighbor(a.position,0);
    let other=state.occupant(target);
    if other!=b.id {
        state.set_occupant(b.position,0);state.set_occupant(target,0);
        if other!=0 {
            let mut displaced=state.particle(other).ok_or("missing displaced particle")?;
            displaced.position=b.position;state.write_particle(displaced)?;
            state.set_occupant(b.position,other);
        }
        b.position=target;state.write_particle(b)?;state.set_occupant(target,b.id);
    }
    a.bonds[0]=b.id;b.bonds[1]=a.id;
    if condition=="inactive" {
        a.conformation=0;b.conformation=0;
        state.add_heat(a.position,1)?;state.add_heat(b.position,1)?;
    }
    state.write_particle(a)?;state.write_particle(b)?;
    let engine=Engine::new(state,MatterLaw::new(config.clone())?)?;
    Ok((Universe::restore_bundle(&checkpoint::bundle(&engine,&config)?)?,source))
}

fn episode(path:&Path,seed:u64,variant:u64,condition:&str)->Result<Value> {
    let (world,source)=initial(seed,variant,condition)?;
    let source_pair=pair(source[0],source[1]);
    let state=world.material_state().ok_or("missing initial state")?;
    let signature=pair(u64::from(state.particle(source[0]).ok_or("missing source")?.material),
        u64::from(state.particle(source[1]).ok_or("missing source")?.material));
    let initial_energy=world.metrics()["total_energy"].as_str().ok_or("missing initial energy")?.to_owned();
    let mut ancestry=BTreeMap::from([(source_pair,0u32)]);
    let mut births=Vec::new();let mut independent_births=0usize;let mut maximum_generation=0; let mut mutations=0usize;let mut faithful_mutant_births=0usize;
    let outcome=lab::run_observed(world,path,RunOptions {
        steps:2048,sample_every:128,checkpoint_every:1024,archive_limit_mib:256,
        label:"designed-source-material-variation".into(),..Default::default()
    },|world| {
        let state=world.material_state().ok_or_else(||SimulationError::new("missing observed state"))?;
        let (events,truncated)=world.material_events().ok_or_else(||SimulationError::new("missing event provenance"))?;
        if truncated {return Err(SimulationError::new("autocatalysis provenance truncated"));}
        let contacts:BTreeMap<_,_>=events.iter().filter(|e|matches!(e.kind,EventKind::ContactCatalysis))
            .map(|e|(e.particles[1],e.particles[0])).collect();
        for event in events.iter().filter(|e|matches!(e.kind,EventKind::Bind)) {
            let [a,b]=event.particles;
            let first=*contacts.get(&a).ok_or_else(||SimulationError::new("binding lacks source provenance"))?;
            let second=*contacts.get(&b).ok_or_else(||SimulationError::new("binding lacks source provenance"))?;
            let parent=pair(first,second);
            let generation=ancestry.get(&parent).ok_or_else(||SimulationError::new("unknown catalytic parent"))?+1;
            let p=state.particle(a).ok_or_else(||SimulationError::new("missing product"))?;
            let q=state.particle(b).ok_or_else(||SimulationError::new("missing product"))?;
            let child_signature=pair(u64::from(p.material),u64::from(q.material));
            let source_a=state.particle(first).ok_or_else(||SimulationError::new("missing parent"))?;
            let source_b=state.particle(second).ok_or_else(||SimulationError::new("missing parent"))?;
            let parent_signature=pair(u64::from(source_a.material),u64::from(source_b.material));
            let mismatches=usize::from(source_a.material!=p.material)+usize::from(source_b.material!=q.material);
            if mismatches>usize::from(condition!="v2") || ancestry.insert(pair(a,b),generation).is_some() {
                return Err(SimulationError::new("unsupported mismatch or duplicate product"));
            }
            mutations+=usize::from(child_signature!=parent_signature);
            let faithful_mutant=parent_signature!=signature && child_signature==parent_signature;
            faithful_mutant_births+=usize::from(faithful_mutant);
            let distant=[first,second,a,b].into_iter().all(|id|source.iter().all(|&original|
                state.particle(id).zip(state.particle(original))
                    .is_some_and(|(p,q)|distance(state,p.position,q.position)>5)));
            let independent=generation>=2 && distant;
            independent_births+=usize::from(independent);
            maximum_generation=maximum_generation.max(generation);
            births.push(json!({"tick":world.generation(),"parent":parent,"product":[a,b],
                "parent_signature":parent_signature,"child_signature":child_signature,"mismatched_endpoints":mismatches,"faithful_mutant_birth":faithful_mutant,"catalytic_generation":generation,"distant_from_original_source":independent}));
        }
        Ok(())
    })?;
    if !outcome.completed {return Err(format!("autocatalysis run stopped: {:?}",outcome.error).into());}
    lab::replay(path)?;
    let final_world=Universe::load_checkpoint(&path.join("final.gns"))?;
    let final_state=final_world.material_state().ok_or("missing final state")?;
    if ancestry.keys().any(|&(a,b)|!final_state.particle(a).is_some_and(|p|p.bonds.contains(&b))) {
        return Err("tracked product lost its bond".into());
    }
    write_new_json(&path.join("births.json"),&json!({"initial_source":source,"signature":signature,"births":births,
        "interventions_after_initialization":[],"claim":"event-attributed seeded material autocatalysis, not spontaneous life"}))?;
    Ok(json!({"seed":seed,"variant":variant,"condition":condition,"products":births.len(),
        "mutations":mutations,"faithful_mutant_births":faithful_mutant_births,"maximum_catalytic_generation":maximum_generation,"independent_descendant_births":independent_births,
        "all_product_bonds_persist":true,"initial_energy":initial_energy,"final_metrics":final_world.metrics(),
        "digest":final_world.digest()?,"replay":true}))
}

fn main()->Result<()> {
    let mut args=std::env::args().skip(1);
    let output=PathBuf::from(args.next().ok_or("new output directory required")?);
    if args.next().is_some() {return Err("unexpected argument".into());}
    fs::create_dir(&output)?;
    write_new_json(&output.join("protocol.json"),&json!({"seeds":[41,42],"variants":[[0,1],[2,3]],
        "conditions":["intact","inactive","v2"],"ticks":2048,"particles":350,"space":[8,8,8],
        "initialization":"scattered particles with designed active chemistry and one arranged source bond",
        "interventions_after_initialization":[],"law":"v3: one endpoint mismatch has 1/16 contact catalytic rate; two mismatches rejected", "minimum_mutations":1,"minimum_faithful_mutant_births":1,"minimum_products":8,"minimum_catalytic_generations":3,
        "minimum_independent_descendant_births":1,"separation":"all source and product atoms beyond five toroidal steps of both original-source atoms at birth",
        "claim_limit":"changed material pairs born through local catalysis and faithfully copied by changed descendants; not spontaneous origin, competitive selection, adaptation, metabolism or intelligence"}))?;
    let mut rows=Vec::new();
    for variant in 0..2 {
        for seed in 41..=42 {
            for condition in ["intact","inactive","v2"] {
                println!("variant {variant}, seed {seed}, {condition}");
                let path=output.join(format!("variant-{variant}-seed-{seed}-{condition}"));
                let row=episode(&path,seed,variant,condition)?;
                rows.push(row);
            }
        }
    }
    let passed=rows.iter().all(|row|if row["condition"]!="inactive" {
        (if row["condition"]=="intact" {row["mutations"].as_u64().is_some_and(|n|n>=1) && row["faithful_mutant_births"].as_u64().is_some_and(|n|n>=1)} else {row["mutations"]==0 && row["faithful_mutant_births"]==0}) && row["products"].as_u64().is_some_and(|n|n>=8)
            && row["maximum_catalytic_generation"].as_u64().is_some_and(|n|n>=3)
            && row["independent_descendant_births"].as_u64().is_some_and(|n|n>=1)
    }else {row["products"]==0});
    write_new_json(&output.join("result.json"),&json!({"completed":true,"support":passed,"episodes":12,"rows":rows,
        "interpretation":"predeclared seeded heritable variation capability diagnostic; no selection or adaptation claim"}))?;
    if !passed {return Err("predeclared autonomous autocatalysis diagnostic gates were not all met; evidence retained".into());}
    println!("All 12 continuous trajectories replayed and met the predeclared diagnostic gates.");
    Ok(())
}
