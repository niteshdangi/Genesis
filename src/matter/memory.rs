//! Observer interventions and measurements; these are not autonomous material laws.
use std::{collections::BTreeSet, fs::{self,File}, io::{BufRead,BufReader}, path::Path};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use crate::{experiment::{RunOptions,RunOutcome,RunRecord},law_system::SimulationError,
    persistence::{io_error,json_error,read_json,write_new_json},storage::journal,universe::{Universe,UniverseSpec}};
use super::{MatterConfig,MatterState,MatterEvent,EventKind,Space3,lab};

#[derive(Clone,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct MemoryProtocol {
    pub warmup:u64,
    pub delay:u64,
    pub response_steps:u64,
    pub center:Option<[u32;3]>,
    pub radius:u32,
    pub conditioning_heat_per_site:u32,
    pub probe_heat_per_site:u32,
    pub archive_limit_mib:u64,
}
impl Default for MemoryProtocol {
    fn default()->Self {Self {warmup:1000,delay:200,response_steps:100,center:None,radius:1,
        conditioning_heat_per_site:12,probe_heat_per_site:8,archive_limit_mib:256}}
}
impl MemoryProtocol {
    fn regions(&self,space:Space3)->Result<(BTreeSet<u32>,BTreeSet<u32>),SimulationError> {
        if [self.warmup,self.delay,self.response_steps].iter().any(|&n|n==0 || n>10_000)
            || self.radius>8 || self.conditioning_heat_per_site==0 || self.conditioning_heat_per_site>1_000_000
            || self.probe_heat_per_site==0 || self.probe_heat_per_site>1_000_000
            || !(64..=4096).contains(&self.archive_limit_mib) {
            return Err(SimulationError::new("invalid memory protocol duration, pulse or archive budget"));
        }
        let center=self.center.unwrap_or([space.width/2,space.height/2,space.depth/2]);
        if center.iter().zip([space.width,space.height,space.depth]).any(|(&p,n)|p>=n)
            || [space.width,space.height,space.depth].iter().any(|&n|2*(2*self.radius+1)>=n) {
            return Err(SimulationError::new("memory regions must be in bounds and separated on the torus"));
        }
        let remote=[(center[0]+space.width/2)%space.width,(center[1]+space.height/2)%space.height,center[2]];
        let region=|center| {
            let mut sites=BTreeSet::new();let origin=space.position(center);let r=self.radius as i32;
            for z in -r..=r {for y in -r..=r {for x in -r..=r {sites.insert(space.shift(origin,[x,y,z]));}}}
            sites
        };
        let local=region(center);let remote=region(remote);
        if !local.is_disjoint(&remote) {return Err(SimulationError::new("local and remote memory regions overlap"));}
        Ok((local,remote))
    }
}

pub(crate) fn ledger(state:&mut MatterState,before:u32,after:u32)->Result<(),SimulationError> {
    let counter=if after>=before {&mut state.energy_input}else{&mut state.energy_output};
    *counter=counter.checked_add(u128::from(before.abs_diff(after)))
        .ok_or_else(||SimulationError::new("external bath ledger overflow"))?;
    Ok(())
}

pub(crate) fn thermal_event(state:&mut MatterState,config:&MatterConfig,tick:u64,id:u64,position:u32,
    before:u32,after:u32,channel:u32)->Result<(),SimulationError> {
    if before==after {return Ok(());}
    ledger(state,before,after)?;
    state.event(MatterEvent {tick,kind:EventKind::Intervention,particles:[id,0],positions:[position,position],
        before:[before,channel],after:[after,channel],energy:before.abs_diff(after)},config)
}

pub(crate) fn pulse(universe:&Universe,sites:&BTreeSet<u32>,amount:u32)->Result<Universe,SimulationError> {
    universe.edit_material_state(|state,config,tick| {
        for &position in sites {
            let before=state.heat(position);state.add_heat(position,amount)?;
            let after=state.heat(position);ledger(state,before,after)?;
            state.event(MatterEvent {tick,kind:EventKind::Input,particles:[0,0],positions:[position,position],
                before:[before,0],after:[after,0],energy:amount},config)?;
        }
        Ok(())
    })
}

pub(crate) fn thermal_match(universe:&Universe,reference:&Universe)->Result<Universe,SimulationError> {
    let target=reference.material_state().ok_or_else(||SimulationError::new("missing thermal reference"))?;
    universe.edit_material_state(|state,config,tick| {
        if state.space!=target.space || state.count()!=target.count() {
            return Err(SimulationError::new("thermal match requires equal geometry and particle IDs"));
        }
        for position in 0..state.space.len() as u32 {
            let before=state.heat(position);let after=target.heat(position);
            thermal_event(state,config,tick,0,position,before,after,0)?;state.set_heat(position,after);
        }
        for id in 1..=state.count() as u64 {
            let mut p=state.particle(id).ok_or_else(||SimulationError::new("missing thermal particle"))?;
            let after=target.particle(id).ok_or_else(||SimulationError::new("missing thermal reference particle"))?.excitation;
            thermal_event(state,config,tick,id,p.position,p.excitation,after,1)?;
            p.excitation=after;state.write_particle(p)?;
        }
        Ok(())
    })
}

fn erase_conformations(universe:&Universe,reference:&Universe)->Result<Universe,SimulationError> {
    let target=reference.material_state().ok_or_else(||SimulationError::new("missing erasure reference"))?;
    universe.edit_material_state(|state,config,tick| {
        if state.space!=target.space || state.count()!=target.count() {return Err(SimulationError::new("incompatible erasure reference"));}
        for id in 1..=state.count() as u64 {
            let mut p=state.particle(id).ok_or_else(||SimulationError::new("missing erasure particle"))?;
            let after=target.particle(id).ok_or_else(||SimulationError::new("missing erasure target"))?.conformation;
            if p.conformation==after {continue;}
            let energy=config.materials[p.material as usize].conformation_energy;
            ledger(state,u32::from(p.conformation)*energy,u32::from(after)*energy)?;
            state.event(MatterEvent {tick,kind:EventKind::Intervention,particles:[id,0],positions:[p.position,p.position],
                before:[u32::from(p.conformation),2],after:[u32::from(after),2],energy},config)?;
            p.conformation=after;state.write_particle(p)?;
        }
        Ok(())
    })
}

pub(crate) fn evolve(universe:Universe,path:&Path,steps:u64,budget:u64,label:&str)->Result<Universe,SimulationError> {
    let options=RunOptions {steps,sample_every:50.min(steps),checkpoint_every:250.min(steps),archive_limit_mib:budget,
        label:label.into(),..RunOptions::default()};
    let outcome=lab::run(universe,path,options)?;
    if !outcome.completed {return Err(SimulationError::new(format!("{label} stopped at {}: {}",
        outcome.final_generation,outcome.error.unwrap_or_default())));}
    Universe::load_checkpoint(&path.join("final.gns"))
}

fn region_snapshot(universe:&Universe,sites:&BTreeSet<u32>)->Result<Value,SimulationError> {
    let state=universe.material_state().ok_or_else(||SimulationError::new("missing response material state"))?;
    let mut particles=0;let mut active=0;let mut excitation=0u128;let mut bond_ends=0;let mut heat=0u128;
    for &position in sites {
        heat+=u128::from(state.heat(position));
        if let Some(p)=state.particle(state.occupant(position)) {
            particles+=1;active+=usize::from(p.conformation==1);excitation+=u128::from(p.excitation);bond_ends+=p.degree();
        }
    }
    Ok(json!({"particles":particles,"active_conformations":active,"heat":heat.to_string(),
        "excitation":excitation.to_string(),"bond_endpoints":bond_ends}))
}

fn measure_response(path:&Path,sites:&BTreeSet<u32>)->Result<Value,SimulationError> {
    let outcome:RunOutcome=read_json(&path.join("outcome.json"))?;
    if !outcome.completed {return Err(SimulationError::new("incomplete response cannot be interpreted"));}
    let reader=BufReader::new(File::open(path.join("history.ndjson")).map_err(io_error)?);
    let mut expected=outcome.initial_generation;let mut switches=0u64;let mut binds=0u64;let mut unbinds=0u64;let mut trace=Vec::new();
    let mut global_switches=0u64;let mut charges=0u64;let mut motions=0u64;
    for line in reader.lines() {
        let row:RunRecord=serde_json::from_str(&line.map_err(io_error)?).map_err(json_error)?;
        if row.generation!=expected {return Err(SimulationError::new("response journal is not contiguous"));}
        let initial=row.generation==outcome.initial_generation;
        let references:Vec<_>=row.events.iter().filter(|e|e["kind"]=="material_batch").collect();
        if references.len()!=usize::from(!initial) {return Err(SimulationError::new("response journal is missing an event batch"));}
        let mut local_switches=0u64;
        for reference in references {
            let batch=journal::load(&lab::batch_path(path,reference)?,reference["digest"].as_str()
                .ok_or_else(||SimulationError::new("response batch digest missing"))?)?;
            if batch.truncated || batch.generation!=row.generation {return Err(SimulationError::new("response requires complete, correctly timed provenance"));}
            for event in batch.events {
                if matches!(event.kind,EventKind::Switch){global_switches+=1;}
                match event.kind {
                    EventKind::Switch if sites.contains(&event.positions[0])=>{switches+=1;local_switches+=1;}
                    EventKind::Bind if sites.contains(&event.positions[0]) || sites.contains(&event.positions[1])=>binds+=1,
                    EventKind::Unbind if sites.contains(&event.positions[0]) || sites.contains(&event.positions[1])=>unbinds+=1,
                    EventKind::Charge if sites.contains(&event.positions[0])=>charges+=1,
                    EventKind::Move | EventKind::Rotate if sites.contains(&event.positions[0]) || sites.contains(&event.positions[1])=>motions+=1,
                    _=>{}
                }
            }
        }
        trace.push(json!({"elapsed":row.generation-outcome.initial_generation,"local_switches":local_switches}));
        expected=expected.checked_add(1).ok_or_else(||SimulationError::new("response tick overflow"))?;
    }
    if expected!=outcome.final_generation+1 {return Err(SimulationError::new("response journal is incomplete"));}
    let final_universe=Universe::load_checkpoint(&path.join("final.gns"))?;
    Ok(json!({"local_switches":switches,"local_binds":binds,"local_unbinds":unbinds,"switch_trace":trace,
        "global_switches":global_switches,"local_charge_events":charges,"local_motion_events":motions,
        "final_region":region_snapshot(&final_universe,sites)?,"final_digest":outcome.final_digest,"full_provenance":true}))
}

pub(crate) fn verify_thermal_match(a:&Universe,b:&Universe)->Result<(),SimulationError> {
    let a=a.material_state().ok_or_else(||SimulationError::new("missing matched state"))?;
    let b=b.material_state().ok_or_else(||SimulationError::new("missing matched state"))?;
    if a.space!=b.space || a.count()!=b.count()
        || (0..a.space.len() as u32).any(|site|a.heat(site)!=b.heat(site))
        || (1..=a.count() as u64).any(|id|a.particle(id).map(|p|p.excitation)!=b.particle(id).map(|p|p.excitation)) {
        return Err(SimulationError::new("pre-probe heat and excitation are not exactly matched"));
    }
    Ok(())
}

pub fn run(config:MatterConfig,seeds:&[u64],protocol:MemoryProtocol,output:&Path)->Result<Value,SimulationError> {
    config.validate()?;
    if !config.execution.full_provenance || seeds.is_empty() || seeds.len()>16 || seeds.iter().collect::<BTreeSet<_>>().len()!=seeds.len() {
        return Err(SimulationError::new("memory experiment requires full provenance and 1..16 distinct seeds"));
    }
    let (local,remote)=protocol.regions(config.space)?;
    if let Some(parent)=output.parent().filter(|p|!p.as_os_str().is_empty()){fs::create_dir_all(parent).map_err(io_error)?;}
    fs::create_dir(output).map_err(io_error)?;
    write_new_json(&output.join("protocol.json"),&json!({"schema":1,"package_version":env!("CARGO_PKG_VERSION"),
        "config":config,"seeds":seeds.iter().map(u64::to_string).collect::<Vec<_>>(),"protocol":protocol,
        "local_sites":local,"remote_sites":remote,"primary_measure":"local conformation switches during the probe window",
        "contrast":"local-probe minus equal-energy remote-probe; conditioned contrast minus unconditioned contrast",
        "thermal_matching":"heat by fixed site and excitation by immutable particle ID, copied from the shared preconditioning checkpoint",
        "reservoir_policy":"configured during warmup, disabled for conditioning, delay and probing",
        "erasure_scope":"only conformations copied from the delayed unconditioned arm by particle ID; positions, orientation and bonds are preserved",
        "intervention_channels":{"0":"heat","1":"excitation","2":"conformation"},
        "limits":["remote pulses may reach the local region during the probe window","matching baths and erasure are external interventions",
            "structural energy may differ after thermal matching","erasure does not remove geometric or bond memory",
            "response contrast does not establish life, cognition or adaptive value"]}))?;
    let mut results=Vec::new();
    for &seed in seeds {
        let seed_path=output.join(format!("seed-{seed}"));fs::create_dir(&seed_path).map_err(io_error)?;
        let mut seeded=config.clone();seeded.seed=seed;
        let warm=evolve(Universe::from_spec(UniverseSpec::Matter(seeded.clone()))?,&seed_path.join("warmup"),
            protocol.warmup,protocol.archive_limit_mib,"memory-warmup")?;
        seeded.reservoir.enabled=false;
        let base=warm.fork_with(UniverseSpec::Matter(seeded))?;
        base.save_checkpoint(&seed_path.join("conditioning-base.gns"))?;
        let conditioned=evolve(pulse(&base,&local,protocol.conditioning_heat_per_site)?,&seed_path.join("conditioned-delay"),
            protocol.delay,protocol.archive_limit_mib,"conditioned-delay")?;
        let unconditioned=evolve(pulse(&base,&remote,protocol.conditioning_heat_per_site)?,&seed_path.join("unconditioned-delay"),
            protocol.delay,protocol.archive_limit_mib,"unconditioned-delay")?;
        let erased=erase_conformations(&conditioned,&unconditioned)?;
        erased.save_checkpoint(&seed_path.join("conformation-erased.gns"))?;
        write_new_json(&seed_path.join("history-differences.json"),&json!({
            "conditioned_vs_unconditioned":lab::compare(&unconditioned,&conditioned)?,
            "erased_vs_conditioned":lab::compare(&conditioned,&erased)?}))?;
        let histories=[("unconditioned",thermal_match(&unconditioned,&base)?),
            ("conditioned",thermal_match(&conditioned,&base)?),("conformation-erased",thermal_match(&erased,&base)?)];
        let mut trials=Vec::new();let mut contrasts=Vec::new();
        for (history,matched) in histories {
            verify_thermal_match(&matched,&base)?;
            matched.save_checkpoint(&seed_path.join(format!("{history}-matched.gns")))?;
            let mut counts=[0i64;2];
            for (index,(probe,sites)) in [("local",&local),("remote",&remote)].into_iter().enumerate() {
                let trial_path=seed_path.join(format!("{history}-{probe}-probe"));
                let trial=pulse(&matched,sites,protocol.probe_heat_per_site)?;
                let start=region_snapshot(&trial,&local)?;
                evolve(trial,&trial_path,protocol.response_steps,protocol.archive_limit_mib,&format!("{history}-{probe}"))?;
                let response=measure_response(&trial_path,&local)?;
                counts[index]=response["local_switches"].as_u64().and_then(|n|i64::try_from(n).ok())
                    .ok_or_else(||SimulationError::new("response count out of range"))?;
                trials.push(json!({"history":history,"probe":probe,"initial_region":start,"response":response,
                    "path":format!("seed-{seed}/{history}-{probe}-probe")}));
            }
            contrasts.push(counts[0]-counts[1]);
        }
        let result=json!({"seed":seed.to_string(),"thermal_match_verified":true,"trials":trials,
            "unconditioned_stimulus_contrast":contrasts[0],"conditioned_stimulus_contrast":contrasts[1],
            "erased_stimulus_contrast":contrasts[2],"history_effect":contrasts[1]-contrasts[0],
            "history_effect_after_conformation_erasure":contrasts[2]-contrasts[0],
            "conformation_erasure_effect":contrasts[1]-contrasts[2]});
        write_new_json(&seed_path.join("result.json"),&result)?;results.push(result);
    }
    let report=json!({"schema":1,"completed":true,"primary_measure":"local conformation switches", "results":results,
        "interpretation":"exploratory paired response contrasts; no automatic functional-memory or cognition verdict"});
    write_new_json(&output.join("memory.json"),&report)?;Ok(report)
}

pub fn screen(config:MatterConfig,seeds:&[u64],protocol:MemoryProtocol,heat_levels:&[u32],output:&Path)->Result<Value,SimulationError> {
    config.validate()?;
    if !config.execution.full_provenance || seeds.is_empty() || seeds.len()>16 || seeds.iter().collect::<BTreeSet<_>>().len()!=seeds.len()
        || heat_levels.is_empty() || heat_levels.len()>4 || heat_levels.iter().any(|&n|n==0 || n>1_000_000)
        || heat_levels.iter().collect::<BTreeSet<_>>().len()!=heat_levels.len() {
        return Err(SimulationError::new("response screen requires full provenance, distinct seeds and 1..4 distinct positive heat levels"));
    }
    let (local,remote)=protocol.regions(config.space)?;
    let mut times:Vec<_>=[1,5,20,100,protocol.delay].into_iter().filter(|&n|n<=protocol.delay).collect();
    times.sort_unstable();times.dedup();
    if let Some(parent)=output.parent().filter(|p|!p.as_os_str().is_empty()){fs::create_dir_all(parent).map_err(io_error)?;}
    fs::create_dir(output).map_err(io_error)?;
    write_new_json(&output.join("screen-protocol.json"),&json!({"schema":1,"config":config,"seeds":seeds,
        "protocol":protocol,"heat_levels":heat_levels,"sample_times":times,"local_sites":local,"remote_sites":remote,
        "arms":["warm","warm-reduced-motion","cold","cold-reduced-motion"],
        "reduced_motion_rate":100_000,"cold_initial_heat":0,"cold_initial_excitation":0,"cold_reservoir_enabled":false,
        "reservoir_policy":"configured during warmup; disabled in every pulse/delay trial",
        "comparison":"equal-energy local and remote conditioning pulses, paired by seed, arm and pulse magnitude",
        "retention_readout":"changed positions, orientations, bonds or conformations after thermal matching to the pre-pulse checkpoint",
        "selection_rule":"prefer the lowest pulse at which every seed retains a conformation difference at the final delay; exploratory selection requires fresh seeds for subsequent memory validation",
        "limits":["parameter arms are changed artificial-law configurations, not improved physics",
            "remote heat can propagate to the local region","particle divergence is not useful memory or adaptation"]}))?;
    let mut results=Vec::new();
    for arm in ["warm","warm-reduced-motion","cold","cold-reduced-motion"] {
        let mut arm_config=config.clone();
        if arm.contains("reduced-motion"){arm_config.motion_rate=100_000;}
        if arm.starts_with("cold"){arm_config.initial_heat=0;arm_config.initial_excitation=0;arm_config.reservoir.enabled=false;}
        arm_config.validate()?;
        let arm_path=output.join(arm);fs::create_dir(&arm_path).map_err(io_error)?;
        write_new_json(&arm_path.join("spec.json"),&UniverseSpec::Matter(arm_config.clone()))?;
        for &seed in seeds {
            let seed_path=arm_path.join(format!("seed-{seed}"));fs::create_dir(&seed_path).map_err(io_error)?;
            let mut seeded=arm_config.clone();seeded.seed=seed;
            let warm=evolve(Universe::from_spec(UniverseSpec::Matter(seeded.clone()))?,&seed_path.join("warmup"),
                protocol.warmup,protocol.archive_limit_mib,"response-warmup")?;
            seeded.reservoir.enabled=false;
            let base=warm.fork_with(UniverseSpec::Matter(seeded))?;
            base.save_checkpoint(&seed_path.join("base.gns"))?;
            for &heat in heat_levels {
                let trial_path=seed_path.join(format!("heat-{heat}"));fs::create_dir(&trial_path).map_err(io_error)?;
                let mut local_state=pulse(&base,&local,heat)?;let mut remote_state=pulse(&base,&remote,heat)?;
                local_state.save_checkpoint(&trial_path.join("local-pulse.gns"))?;
                remote_state.save_checkpoint(&trial_path.join("remote-pulse.gns"))?;
                let mut previous=0;let mut trajectory=Vec::new();
                for &elapsed in &times {
                    let local_path=trial_path.join(format!("local-{elapsed:05}"));let remote_path=trial_path.join(format!("remote-{elapsed:05}"));
                    local_state=evolve(local_state,&local_path,elapsed-previous,protocol.archive_limit_mib,"local-response")?;
                    remote_state=evolve(remote_state,&remote_path,elapsed-previous,protocol.archive_limit_mib,"remote-response")?;
                    trajectory.push(json!({"elapsed":elapsed,"comparison":lab::compare(&remote_state,&local_state)?,
                        "local_interval_response":measure_response(&local_path,&local)?,
                        "remote_interval_response":measure_response(&remote_path,&local)?}));
                    previous=elapsed;
                }
                let local_matched=thermal_match(&local_state,&base)?;let remote_matched=thermal_match(&remote_state,&base)?;
                verify_thermal_match(&local_matched,&remote_matched)?;
                local_matched.save_checkpoint(&trial_path.join("local-matched.gns"))?;
                remote_matched.save_checkpoint(&trial_path.join("remote-matched.gns"))?;
                let result=json!({"arm":arm,"seed":seed.to_string(),"heat_per_site":heat,"trajectory":trajectory,
                    "final_thermal_match_verified":true,"retained_difference":lab::compare(&remote_matched,&local_matched)?});
                write_new_json(&trial_path.join("response.json"),&result)?;results.push(result);
            }
        }
    }
    let report=json!({"schema":1,"completed":true,"results":results,
        "interpretation":"stimulus responsiveness and physical state retention; no functional-memory verdict"});
    write_new_json(&output.join("screen.json"),&report)?;Ok(report)
}
