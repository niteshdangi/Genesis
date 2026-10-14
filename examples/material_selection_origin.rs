use std::{collections::BTreeMap,fs,path::{Path,PathBuf}};
use genesis::{engine::Engine,experiment::RunOptions,law_system::SimulationError,
    matter::{MatterConfig,MatterLaw,MatterState,Space3,EventKind,lab},
    persistence::write_new_json,storage::checkpoint,universe::Universe};
use serde_json::{json,Value};
type Result<T> = std::result::Result<T,Box<dyn std::error::Error>>;
fn pair(a:u64,b:u64)->(u64,u64){(a.min(b),a.max(b))}
fn signature(s:&MatterState,a:u64,b:u64)->std::result::Result<(u64,u64),SimulationError>{
    Ok(pair(s.particle(a).ok_or_else(||SimulationError::new("missing atom"))?.material.into(),
        s.particle(b).ok_or_else(||SimulationError::new("missing atom"))?.material.into()))
}
fn initial(seed:u64,swap:bool,condition:&str,origin:bool)->Result<Universe>{
    let mut c=MatterConfig::default();c.space=Space3{width:8,height:8,depth:8};c.seed=seed;
    c.particles=350;c.initial_excitation=16;c.motion_rate=100_000;c.execution.event_limit=8192;
    c.law_version=if origin && condition=="disabled"{1}else if origin{3}else{2};
    c.binding_rate=if origin{250}else{0};c.breaking_rate=if origin{100}else{0};
    for m in &mut c.materials{m.ports=63;m.valence=1;m.affinities=[100;4];m.conformation_energy=1;
        m.switching_rate=if origin{1000}else{0};m.coupling=0;m.catalytic_strength=1;}
    if !origin{c.materials[0].catalytic_strength=0;
        if condition=="favor-01"{c.materials[1].catalytic_strength=3;}
        if condition=="favor-03"{c.materials[3].catalytic_strength=3;}}
    let mut s=MatterState::initial(&c)?;
    if !origin{
        for id in 1..=s.count() as u64{let mut p=s.particle(id).ok_or("missing particle")?;
            p.conformation=1;p.orientation=0;s.write_particle(p)?;}
        let ids=[1,2,5,4];
        for id in ids{let p=s.particle(id).ok_or("missing source")?;s.set_occupant(p.position,0);}
        let mut destinations=if swap{[[5,5,4],[6,5,4],[1,1,2],[2,1,2]]}
            else{[[1,1,2],[2,1,2],[5,5,4],[6,5,4]]};
        // Initial placement displaces feedstock without deleting it or changing its energy.
        for (id,xyz) in ids.into_iter().zip(destinations.iter_mut()){
            let target=s.space.position(*xyz);let occupant=s.occupant(target);
            if occupant!=0{let vacant=(0..s.space.len() as u32).find(|&p|s.occupant(p)==0
                && !destinations_copy(s.space,swap).contains(&p)).ok_or("no vacant displacement site")?;
                let mut p=s.particle(occupant).ok_or("missing displaced particle")?;
                s.set_occupant(target,0);p.position=vacant;s.write_particle(p)?;s.set_occupant(vacant,occupant);}
            let mut p=s.particle(id).ok_or("missing source atom")?;p.position=target;
            if condition=="inactive"{p.conformation=0;s.add_heat(target,1)?;}
            s.write_particle(p)?;s.set_occupant(target,id);
        }
        for (a,b) in [(1,2),(5,4)]{let mut p=s.particle(a).ok_or("missing template")?;
            let mut q=s.particle(b).ok_or("missing template")?;p.bonds[0]=b;q.bonds[1]=a;
            s.write_particle(p)?;s.write_particle(q)?;}
    }
    let e=Engine::new(s,MatterLaw::new(c.clone())?)?;
    Ok(Universe::restore_bundle(&checkpoint::bundle(&e,&c)?)?)
}
fn destinations_copy(space:Space3,swap:bool)->[u32;4]{
    let xyz=if swap{[[5,5,4],[6,5,4],[1,1,2],[2,1,2]]}else{[[1,1,2],[2,1,2],[5,5,4],[6,5,4]]};
    xyz.map(|p|space.position(p))
}
fn episode(path:&Path,seed:u64,swap:bool,condition:&str,origin:bool)->Result<Value>{
    let world=initial(seed,swap,condition,origin)?;
    let initial_energy=world.metrics()["total_energy"].clone();
    let mut ancestry=BTreeMap::<(u64,u64),(u32,u8)>::new();
    if !origin{ancestry.insert(pair(1,2),(0,0));ancestry.insert(pair(5,4),(0,1));}
    let mut births=Vec::new();let mut roots=0usize;let mut changes=0usize;
    let mut faithful_changed=0usize;let mut depths=[0u32;2];let mut counts=[0usize;2];
    let mut max_depth=0;let mut live_lineage=BTreeMap::new();
    let horizon=if origin{2048}else{768};
    let result=lab::run_observed(world,path,RunOptions{steps:horizon,sample_every:128,
        checkpoint_every:horizon,archive_limit_mib:256,label:if origin{"unseeded-material-origin"}else{"shared-resource-selection"}.into(),
        ..Default::default()},|world|{
        let s=world.material_state().ok_or_else(||SimulationError::new("missing state"))?;
        let events=&s.events;if s.events_truncated{return Err(SimulationError::new("truncated provenance"));}
        for e in events.iter().filter(|e|matches!(e.kind,EventKind::Unbind)){ancestry.remove(&pair(e.particles[0],e.particles[1]));}
        let contacts:BTreeMap<_,_>=events.iter().filter(|e|matches!(e.kind,EventKind::ContactCatalysis))
            .map(|e|(e.particles[1],e.particles[0])).collect();
        let previous=ancestry.clone();
        for e in events.iter().filter(|e|matches!(e.kind,EventKind::Bind)){
            let [a,b]=e.particles;let child=pair(a,b);let child_sig=signature(s,a,b)?;
            if ancestry.contains_key(&child){return Err(SimulationError::new("duplicate extant bond birth"));}
            if let (Some(&p),Some(&q))=(contacts.get(&a),contacts.get(&b)){
                let parent=pair(p,q);let &(depth,lineage)=previous.get(&parent)
                    .ok_or_else(||SimulationError::new("untracked catalytic source"))?;
                let parent_sig=signature(s,p,q)?;let changed=child_sig!=parent_sig;
                let copied_changed=depth>0 && child_sig==parent_sig;
                changes+=usize::from(changed);faithful_changed+=usize::from(copied_changed);
                if !origin{let expected=if lineage==0{(0,1)}else{(0,3)};
                    if child_sig!=expected{return Err(SimulationError::new("v2 lineage fidelity failed"));}
                    counts[lineage as usize]+=1;depths[lineage as usize]=depths[lineage as usize].max(depth+1);}
                max_depth=max_depth.max(depth+1);ancestry.insert(child,(depth+1,lineage));
                births.push(json!({"tick":world.generation(),"parent":parent,"child":child,
                    "parent_signature":parent_sig,"child_signature":child_sig,"depth":depth+1,"lineage":lineage,"changed":changed}));
            }else if origin{roots+=1;ancestry.insert(child,(0,0));}
            else{return Err(SimulationError::new("noncatalytic birth in zero-ordinary-binding selection condition"));}
        }
        live_lineage.clear();for (&ids,&(depth,lineage)) in &ancestry{
            if !s.particle(ids.0).is_some_and(|p|p.bonds.contains(&ids.1)){return Err(SimulationError::new("stale lineage bond"));}
            if depth>0{*live_lineage.entry(lineage).or_insert(0usize)+=1;}}
        Ok(())
    })?;
    if !result.completed{return Err(format!("episode stopped: {:?}",result.error).into());}
    let replay=lab::replay(path)?;
    write_new_json(&path.join("births.json"),&json!({"births":births,"interventions_after_initialization":[],
        "roots_are_ordinary_nucleation":origin,"claim_limit":"material autocatalysis, not life"}))?;
    Ok(json!({"seed":seed,"swap":swap,"condition":condition,"origin":origin,"counts":counts,
        "depths":depths,"catalytic_births":births.len(),"ordinary_nucleations":roots,"signature_changes":changes,
        "faithful_descendant_signature_births":faithful_changed,"maximum_depth":max_depth,"live_lineage":live_lineage,
        "initial_energy":initial_energy,"metrics":result.final_metrics,"replay":replay}))
}
fn main()->Result<()>{
    let out=PathBuf::from(std::env::args().nth(1).ok_or("new output directory required")?);fs::create_dir(&out)?;
    write_new_json(&out.join("protocol.json"),&json!({"selection_seeds":[61,62,63,64],"swaps":[false,true],
        "conditions":["favor-01","favor-03","neutral","inactive"],"selection_ticks":768,
        "selection_gate":"both lineages depth >=3 in each active run; each seed's swapped mean favors 01 under favor-01 and 03 under favor-03; reversal changes mean 01-minus-03 by at least 4; inactive controls zero; neutral means descriptive",
        "origin_seeds":[81,82,83,84],"origin_conditions":["active","disabled"],"origin_ticks":2048,
        "origin_gate":"at least three catalytic generations and ten catalytic births in every active seed, with more catalytic births than paired disabled control",
        "claim_limits":["seeded selection among supplied traits, not evolved fitness","unseeded material autocatalysis, not spontaneous life","finite dimer repertoire, not open-ended evolution","all outcomes retained; no threshold relaxation"]}))?;
    let mut selection=Vec::new();
    for seed in 61..=64{for swap in [false,true]{for condition in ["favor-01","favor-03","neutral","inactive"]{
        println!("selection seed {seed}, swap {swap}, {condition}");
        selection.push(episode(&out.join(format!("selection-{seed}-{swap}-{condition}")),seed,swap,condition,false)?);
    }}}
    let mut contrasts=Vec::new();let mut selection_supported=true;
    for seed in 61..=64{let mean=|condition:&str|->f64{selection.iter().filter(|r|r["seed"]==seed && r["condition"]==condition)
        .map(|r|r["counts"][0].as_u64().unwrap_or(0) as f64-r["counts"][1].as_u64().unwrap_or(0) as f64).sum::<f64>()/2.0};
        let a=mean("favor-01");let b=mean("favor-03");let pass=a>0.0 && b<0.0 && a-b>=4.0;
        selection_supported&=pass;contrasts.push(json!({"seed":seed,"favor01":a,"favor03":b,"neutral":mean("neutral"),"pass":pass}));}
    for row in &selection{selection_supported&=if row["condition"]=="inactive"{row["catalytic_births"]==0}
        else{row["depths"].as_array().is_some_and(|d|d.iter().all(|v|v.as_u64().is_some_and(|n|n>=3)))};}
    write_new_json(&out.join("selection-result.json"),&json!({"completed":true,"supported":selection_supported,"rows":selection,"contrasts":contrasts,
        "adaptation_demonstrated":false,"open_ended_evolution_demonstrated":false}))?;
    let mut origins=Vec::new();for seed in 81..=84{for condition in ["active","disabled"]{
        println!("unseeded origin seed {seed}, {condition}");
        origins.push(episode(&out.join(format!("origin-{seed}-{condition}")),seed,false,condition,true)?);
    }}
    let origin_supported=origins.iter().filter(|r|r["condition"]=="active").all(|r|
        r["maximum_depth"].as_u64().is_some_and(|n|n>=3) && r["catalytic_births"].as_u64().is_some_and(|n|n>=10)
        && origins.iter().any(|control|control["seed"]==r["seed"] && control["condition"]=="disabled" && control["catalytic_births"]==0));
    write_new_json(&out.join("origin-result.json"),&json!({"completed":true,"supported":origin_supported,"rows":origins,
        "spontaneous_life_demonstrated":false,"intelligence_demonstrated":false}))?;
    println!("Campaign complete: selection support {selection_supported}; unseeded autocatalysis support {origin_supported}. Negative outcomes retained.");
    Ok(())
}
