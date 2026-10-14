//! Cluster-specific causal readout experiments, kept outside the material laws.
use std::{collections::BTreeSet,fs,path::Path};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use crate::{chemistry::seeded_value,experiment::RunOptions,law_system::SimulationError,
    persistence::{io_error,json_error,read_json,write_new_json},universe::{Universe,UniverseSpec}};
use super::{MatterConfig,MatterState,MatterEvent,EventKind,lab,memory::{self,evolve,pulse,thermal_match,verify_thermal_match}};

#[derive(Clone,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct ClusterProtocol {
    pub warmup:u64,pub stability:u64,pub delay:u64,pub response_steps:u64,
    pub replicas:usize,pub candidates:usize,pub min_size:usize,pub max_size:usize,
    pub conditioning_heat:u32,pub probe_heat:u32,pub archive_limit_mib:u64,
    pub cold_write:bool,pub open_chains_only:bool,pub stability_graph_only:bool,pub receiver_endpoints_only:bool,
    pub require_intact_channel:bool,
    pub latency_readout:bool,
}
impl Default for ClusterProtocol {
    fn default()->Self {Self {warmup:1000,stability:20,delay:200,response_steps:32,replicas:16,candidates:2,
        min_size:2,max_size:8,conditioning_heat:64,probe_heat:64,archive_limit_mib:256,
        cold_write:false,open_chains_only:false,stability_graph_only:false,receiver_endpoints_only:false,require_intact_channel:false,latency_readout:false}}
}
impl ClusterProtocol {
    fn validate(&self)->Result<(),SimulationError> {
        if [self.warmup,self.stability,self.delay,self.response_steps].iter().any(|&n|n==0 || n>10_000)
            || !(4..=64).contains(&self.replicas) || !(1..=8).contains(&self.candidates)
            || self.min_size<2 || self.max_size<self.min_size || self.max_size>64
            || self.conditioning_heat==0 || self.conditioning_heat>1_000_000 || self.probe_heat==0 || self.probe_heat>1_000_000
            || !(64..=4096).contains(&self.archive_limit_mib) {return Err(SimulationError::new("invalid cluster-memory protocol"));}
        Ok(())
    }
}
fn state(universe:&Universe)->Result<&MatterState,SimulationError> {
    universe.material_state().ok_or_else(||SimulationError::new("cluster experiment requires material state"))
}
pub(crate) fn components(state:&MatterState)->Result<Vec<Vec<u64>>,SimulationError> {
    let mut visited=BTreeSet::new();let mut result=Vec::new();
    for id in 1..=state.count() as u64 {
        if visited.contains(&id){continue;}
        let mut pending=vec![id];let mut group=Vec::new();
        while let Some(id)=pending.pop() {
            if !visited.insert(id){continue;}
            let p=state.particle(id).ok_or_else(||SimulationError::new("missing cluster particle"))?;
            group.push(id);pending.extend(p.bonds.into_iter().filter(|&n|n!=0));
        }
        group.sort_unstable();result.push(group);
    }
    Ok(result)
}
pub(crate) fn geometry_equal(a:&MatterState,b:&MatterState,ids:&[u64])->bool {
    ids.iter().all(|&id|match (a.particle(id),b.particle(id)) {
        (Some(p),Some(q))=>p.position==q.position && p.bonds==q.bonds && p.orientation==q.orientation,
        _=>false,
    })
}
fn select(base:&Universe,stable:&Universe,protocol:&ClusterProtocol)->Result<Vec<Vec<u64>>,SimulationError> {
    let a=state(base)?;let b=state(stable)?;
    let mut groups:Vec<_>=components(a)?.into_iter().filter(|g| {
        if g.len()<protocol.min_size || g.len()>protocol.max_size{return false;}
        if protocol.open_chains_only {
            let degrees:Vec<_>=g.iter().filter_map(|&id|a.particle(id).map(|p|p.degree())).collect();
            if degrees.iter().filter(|&&n|n==1).count()!=2 || degrees.iter().any(|&n|n>2){return false;}
        }
        if !protocol.stability_graph_only{return geometry_equal(a,b,g);}
        g.iter().all(|&id|match (a.particle(id),b.particle(id)) {
            (Some(p),Some(q))=>{
                let mut left:Vec<_>=p.bonds.into_iter().filter(|&n|n!=0).collect();let mut right:Vec<_>=q.bonds.into_iter().filter(|&n|n!=0).collect();
                left.sort_unstable();right.sort_unstable();left==right
            },_=>false,
        })
    }).collect();
    groups.sort_by(|a,b|b.len().cmp(&a.len()).then_with(||a[0].cmp(&b[0])));
    groups.truncate(protocol.candidates);
    if protocol.open_chains_only {for group in &mut groups {
        let input=group.iter().copied().filter(|&id|a.particle(id).is_some_and(|p|p.degree()==1)).min()
            .ok_or_else(||SimulationError::new("selected chain lacks an endpoint"))?;
        let index=group.iter().position(|&id|id==input).ok_or_else(||SimulationError::new("chain input absent"))?;group.swap(0,index);
    }}
    Ok(groups)
}
fn receiver_ids(cohort:&[u64],initial:&Universe,protocol:&ClusterProtocol)->Result<BTreeSet<u64>,SimulationError> {
    let root=*cohort.first().ok_or_else(||SimulationError::new("empty receiver cohort"))?;let state=state(initial)?;
    let ids:BTreeSet<_>=cohort.iter().copied().filter(|&id|id!=root && (!protocol.receiver_endpoints_only
        || state.particle(id).is_some_and(|p|p.degree()==1))).collect();
    if ids.is_empty(){return Err(SimulationError::new("no selected receivers"));}Ok(ids)
}
pub(crate) fn erase(universe:&Universe,changes:&[(u64,u8)])->Result<Universe,SimulationError> {
    universe.edit_material_state(|state,config,tick| {
        let mut seen=BTreeSet::new();
        for &(id,after) in changes {
            if after>1 || !seen.insert(id){return Err(SimulationError::new("invalid or duplicate local erasure"));}
            let mut p=state.particle(id).ok_or_else(||SimulationError::new("missing erasure particle"))?;
            let before=p.conformation;if before==after{continue;}
            let energy=config.materials[p.material as usize].conformation_energy;
            memory::ledger(state,u32::from(before)*energy,u32::from(after)*energy)?;
            state.event(MatterEvent {tick,kind:EventKind::Intervention,particles:[id,0],positions:[p.position,p.position],
                before:[u32::from(before),2],after:[u32::from(after),2],energy},config)?;
            p.conformation=after;state.write_particle(p)?;
        }
        Ok(())
    })
}
fn quench(universe:&Universe)->Result<Universe,SimulationError> {
    universe.edit_material_state(|state,config,tick| {
        for position in 0..state.space.len() as u32 {
            let before=state.heat(position);
            memory::thermal_event(state,config,tick,0,position,before,0,0)?;
            state.set_heat(position,0);
        }
        for id in 1..=state.count() as u64 {
            let mut p=state.particle(id).ok_or_else(||SimulationError::new("missing quench particle"))?;
            memory::thermal_event(state,config,tick,id,p.position,p.excitation,0,1)?;
            p.excitation=0;state.write_particle(p)?;
        }
        Ok(())
    })
}
pub fn distance(state:&MatterState,a:u32,b:u32)->u32 {
    let a=state.space.xyz(a);let b=state.space.xyz(b);
    a.into_iter().zip(b).zip([state.space.width,state.space.height,state.space.depth])
        .map(|((x,y),size)|{let d=x.abs_diff(y);d.min(size-d)}).sum()
}
fn remote_erasures(trained:&Universe,reference:&Universe,cohort:&[u64],changes:&[(u64,u8)])
    ->Result<Option<Vec<(u64,u8)>>,SimulationError> {
    let a=state(trained)?;let b=state(reference)?;
    let mut connected=BTreeSet::new();
    for group in components(a)? {if group.iter().any(|id|cohort.contains(id)){connected.extend(group);}}
    let mut used=BTreeSet::new();let mut result=Vec::new();
    for &(source,after) in changes {
        let original=a.particle(source).ok_or_else(||SimulationError::new("missing marker source"))?;
        let candidate=(1..=a.count() as u64).find(|id| {
            if connected.contains(id) || used.contains(id){return false;}
            let Some(p)=a.particle(*id) else{return false;};let Some(q)=b.particle(*id) else{return false;};
            p.material==original.material && p.conformation==original.conformation && q.conformation==after
                && cohort.iter().all(|&other|a.particle(other).is_some_and(|q|distance(a,p.position,q.position)>=6))
        });
        let Some(id)=candidate else{return Ok(None);};used.insert(id);result.push((id,after));
    }
    Ok(Some(result))
}
pub(crate) fn stats(values:&[f64])->Value {
    let n=values.len();let mean=values.iter().sum::<f64>()/n as f64;
    let variance=values.iter().map(|x|(x-mean).powi(2)).sum::<f64>()/(n-1) as f64;
    let half=2.2*(variance/n as f64).sqrt();
    json!({"n":n,"mean":mean,"approximate_interval":[mean-half,mean+half],
        "interval_excludes_zero":mean-half>0.0 || mean+half<0.0,
        "nonzero_pairs":values.iter().filter(|&&x|x!=0.0).count(),"paired_values":values})
}
fn probe(universe:Universe,path:&Path,receivers:&BTreeSet<u64>,protocol:&ClusterProtocol)->Result<u64,SimulationError> {
    let start=universe.generation();let mut trace=Vec::new();let mut integral=0u64;let mut first_excitation=None;
    let options=RunOptions {steps:protocol.response_steps,sample_every:protocol.response_steps,checkpoint_every:protocol.response_steps,
        label:"cluster-probe".into(),archive_limit_mib:protocol.archive_limit_mib,..RunOptions::default()};
    let outcome=lab::run_observed(universe,path,options,|universe| {
        let state=state(universe)?;let mut excitation=0u64;let mut active=0usize;
        for &id in receivers {
            let p=state.particle(id).ok_or_else(||SimulationError::new("missing receiver"))?;
            excitation+=u64::from(p.excitation);active+=usize::from(p.conformation==1);
        }
        let elapsed=universe.generation()-start;
        if elapsed>0 {integral=integral.checked_add(excitation).ok_or_else(||SimulationError::new("receiver integral overflow"))?;}
        if elapsed>0 && excitation>0 && first_excitation.is_none(){first_excitation=Some(elapsed);}
        trace.push(json!({"elapsed":elapsed,"receiver_excitation":excitation,"receiver_active":active}));Ok(())
    })?;
    if !outcome.completed{return Err(SimulationError::new(format!("cluster probe incomplete: {}",outcome.error.unwrap_or_default())));}
    let restricted_latency=first_excitation.unwrap_or(protocol.response_steps+1);
    write_new_json(&path.join("receiver-trace.json"),&json!({"excitation_integral":integral,"first_excitation":first_excitation,
        "restricted_latency":restricted_latency,"censored":first_excitation.is_none(),"trace":trace}))?;
    Ok(if protocol.latency_readout {restricted_latency}else{integral})
}

pub fn run(config:MatterConfig,seeds:&[u64],protocol:ClusterProtocol,output:&Path)->Result<Value,SimulationError> {
    run_mode(config,seeds,protocol,output,false)
}
pub fn run_mode(config:MatterConfig,seeds:&[u64],protocol:ClusterProtocol,output:&Path,prepare_only:bool)->Result<Value,SimulationError> {
    config.validate()?;protocol.validate()?;
    if !config.execution.full_provenance || seeds.is_empty() || seeds.len()>16 || seeds.iter().collect::<BTreeSet<_>>().len()!=seeds.len() {
        return Err(SimulationError::new("cluster memory requires complete provenance and distinct seeds"));
    }
    if let Some(parent)=output.parent().filter(|p|!p.as_os_str().is_empty()){fs::create_dir_all(parent).map_err(io_error)?;}
    fs::create_dir(output).map_err(io_error)?;
    write_new_json(&output.join("protocol.json"),&json!({"schema":1,"config":config,"seeds":seeds,"protocol":protocol,"prepare_only":prepare_only,
        "selection":if protocol.open_chains_only {"largest open chains within size bounds; smallest endpoint ID is the input; minimum member ID breaks size ties"}
            else {"largest bonded components within size bounds; lowest ID breaks ties"},
        "stability":if protocol.stability_graph_only {"bond endpoint IDs agree at both stability endpoints; positions may fluctuate"}
            else {"positions, orientations and bond slots agree at both stability endpoints"},
        "selection_limits":"stability checked at endpoints, not every intervening generation; components selected before conditioning, not by probe outcomes",
        "primary_response":if protocol.latency_readout {"restricted time to first receiver excitation; unobserved signals censored at response_steps + 1"}
            else {"time integral of receiver excitation"},
        "receiver_roles":if protocol.receiver_endpoints_only {"other initial chain endpoint"}else{"all cohort particles except input"},
        "primary_contrast":"local-probe minus equal-energy remote-probe, then conditioned versus local conformation erasure",
        "controls":"unconditioned, locally erased and remote-erased histories; same material types, flip directions, erasure work, and future random stream",
        "quench":"all heat and excitation set to zero through explicit bath ledger entries before probes; structural energy retained",
        "random_streams":"replicas fork the same stored histories with paired fresh future seeds; they are not independent new worlds",
        "candidate_rule":"nonzero retained marker; cohort geometry agrees across training histories; paired approximate interval for local-erasure effect excludes zero; remote erasure has exactly zero paired effects",
        "limits":["selected clusters remain inside the full material environment","quench and erasure are external interventions",
            "approximate intervals are exploratory, uncorrected for multiple candidates","does not establish adaptive value, self-maintenance, reproduction or life"]}))?;
    let mut results=Vec::new();
    for &seed in seeds {
        let seed_path=output.join(format!("seed-{seed}"));fs::create_dir(&seed_path).map_err(io_error)?;
        let mut seeded=config.clone();seeded.seed=seed;
        let warm=evolve(Universe::from_spec(UniverseSpec::Matter(seeded.clone()))?,&seed_path.join("warmup"),protocol.warmup,protocol.archive_limit_mib,"cluster-warmup")?;
        let stable=evolve(warm.fork_current()?,&seed_path.join("stability"),protocol.stability,protocol.archive_limit_mib,"cluster-stability")?;
        let cohorts=select(&warm,&stable,&protocol)?;
        write_new_json(&seed_path.join("selected-clusters.json"),&cohorts)?;
        seeded.reservoir.enabled=false;let base=stable.fork_with(UniverseSpec::Matter(seeded))?;
        let base=if protocol.cold_write {quench(&base)?}else{base};
        base.save_checkpoint(&seed_path.join("base.gns"))?;
        if cohorts.is_empty(){results.push(json!({"seed":seed,"status":"no-eligible-stable-cluster"}));}
        for cohort in cohorts {
            let root=cohort[0];let cohort_path=seed_path.join(format!("cluster-{root}"));fs::create_dir(&cohort_path).map_err(io_error)?;
            let input_sites:BTreeSet<_>=cohort.iter().map(|&id|state(&base).and_then(|s|s.particle(id).map(|p|p.position)
                .ok_or_else(||SimulationError::new("missing conditioning member")))).collect::<Result<_,_>>()?;
            let space=state(&base)?.space;
            let remote_sites:BTreeSet<_>=input_sites.iter().map(|&site|space.shift(site,[(space.width/2) as i32,(space.height/2) as i32,0])).collect();
            if !input_sites.is_disjoint(&remote_sites){return Err(SimulationError::new("conditioning regions overlap"));}
            let trained=evolve(pulse(&base,&input_sites,protocol.conditioning_heat)?,&cohort_path.join("conditioned"),protocol.delay,protocol.archive_limit_mib,"cluster-conditioned")?;
            let reference=evolve(pulse(&base,&remote_sites,protocol.conditioning_heat)?,&cohort_path.join("unconditioned"),protocol.delay,protocol.archive_limit_mib,"cluster-unconditioned")?;
            let a=state(&trained)?;let b=state(&reference)?;
            let changes:Vec<_>=cohort.iter().filter_map(|&id|a.particle(id).zip(b.particle(id)).filter(|(p,q)|p.conformation!=q.conformation).map(|(_,q)|(id,q.conformation))).collect();
            let geometry=geometry_equal(a,b,&cohort);
            let marker=json!({"seed":seed,"root":root,"cohort":cohort,"marker_changes":changes,"cohort_geometry_agrees":geometry});
            write_new_json(&cohort_path.join("marker.json"),&marker)?;
            if prepare_only {results.push(json!({"marker":marker,"status":"conditioning-recorded"}));continue;}
            if changes.is_empty(){results.push(json!({"marker":marker,"status":"no-retained-conformation-marker"}));continue;}
            let Some(remote_changes)=remote_erasures(&trained,&reference,&cohort,&changes)? else {
                results.push(json!({"marker":marker,"status":"no-matched-remote-erasure"}));continue;
            };
            let local_erased=erase(&trained,&changes)?;let remote_erased=erase(&trained,&remote_changes)?;
            let l=state(&local_erased)?;let r=state(&remote_erased)?;
            if l.energy_input!=r.energy_input || l.energy_output!=r.energy_output {return Err(SimulationError::new("local and remote erasure work mismatch"));}
            local_erased.save_checkpoint(&cohort_path.join("local-erased.gns"))?;
            remote_erased.save_checkpoint(&cohort_path.join("remote-erased.gns"))?;
            write_new_json(&cohort_path.join("erasures.json"),&json!({"local":changes,"remote":remote_changes,"work_matched":true}))?;
            let cold=quench(&trained)?;
            let histories=[("unconditioned",thermal_match(&reference,&cold)?),("conditioned",thermal_match(&trained,&cold)?),
                ("locally-erased",thermal_match(&local_erased,&cold)?),("remotely-erased",thermal_match(&remote_erased,&cold)?)];
            for (name,history) in &histories {verify_thermal_match(history,&cold)?;history.save_checkpoint(&cohort_path.join(format!("{name}-cold.gns")))?;}
            let receivers=receiver_ids(&cohort,&base,&protocol)?;
            let mut trials=Vec::new();let mut history_effects=Vec::new();let mut local_effects=Vec::new();let mut remote_effects=Vec::new();
            let mut future_seeds=BTreeSet::new();
            for replica in 0..protocol.replicas {
                let future_seed=seeded_value(seed,0,99_000,replica as u64,root);let mut contrasts=[0f64;4];
                if !future_seeds.insert(future_seed){return Err(SimulationError::new("duplicate future random stream"));}
                let replica_path=cohort_path.join(format!("replica-{replica:03}"));fs::create_dir(&replica_path).map_err(io_error)?;
                for (index,(name,history)) in histories.iter().enumerate() {
                    let mut future=match history.spec(){UniverseSpec::Matter(config)=>config.clone(),_=>unreachable!()};future.seed=future_seed;
                    let history=history.fork_with(UniverseSpec::Matter(future))?;
                    let anchor=state(&history)?.particle(root).ok_or_else(||SimulationError::new("missing probe receiver"))?.position;
                    let local=BTreeSet::from([anchor]);let remote=BTreeSet::from([space.shift(anchor,[(space.width/2) as i32,(space.height/2) as i32,0])]);
                    let mut scores=[0u64;2];
                    for (probe_index,(label,sites)) in [("local",&local),("remote",&remote)].into_iter().enumerate() {
                        scores[probe_index]=probe(pulse(&history,sites,protocol.probe_heat)?,&replica_path.join(format!("{name}-{label}")),&receivers,&protocol)?;
                    }
                    contrasts[index]=scores[0] as f64-scores[1] as f64;
                    trials.push(json!({"replica":replica,"future_seed":future_seed.to_string(),"history":name,
                        "local_excitation_integral":scores[0],"remote_excitation_integral":scores[1],"stimulus_contrast":contrasts[index]}));
                }
                history_effects.push(contrasts[1]-contrasts[0]);local_effects.push(contrasts[1]-contrasts[2]);remote_effects.push(contrasts[1]-contrasts[3]);
            }
            let local_stats=stats(&local_effects);let remote_stats=stats(&remote_effects);
            let candidate=geometry && local_stats["interval_excludes_zero"]==true && remote_effects.iter().all(|&n|n==0.0);
            let result=json!({"marker":marker,"status":"probes-completed","receivers":receivers,"trials":trials,
                "history_effect":stats(&history_effects),"local_erasure_effect":local_stats,"remote_erasure_effect":remote_stats,
                "exploratory_erasure_dependent_candidate":candidate});
            write_new_json(&cohort_path.join("result.json"),&result)?;results.push(result);
        }
    }
    let candidate_seeds:BTreeSet<_>=results.iter().filter(|r|r["exploratory_erasure_dependent_candidate"]==true)
        .filter_map(|r|r["marker"]["seed"].as_u64()).collect();
    let report=json!({"schema":1,"completed":true,"prepare_only":prepare_only,"results":results,"candidate_world_seeds":candidate_seeds,
        "interpretation":"exploratory local erasure dependence across paired future streams; fresh-world confirmation required"});
    write_new_json(&output.join("cluster-memory.json"),&report)?;Ok(report)
}

pub(crate) fn cut_links(universe:&Universe,links:&[(u64,u64)])->Result<Universe,SimulationError> {
    universe.edit_material_state(|state,config,tick| {
        for &(a,b) in links {
            let mut p=state.particle(a).ok_or_else(||SimulationError::new("missing link source"))?;
            let mut q=state.particle(b).ok_or_else(||SimulationError::new("missing link target"))?;
            let d=p.bonds.iter().position(|&n|n==b).ok_or_else(||SimulationError::new("ablation link is absent"))?;
            if q.bonds[d^1]!=a{return Err(SimulationError::new("ablation link is not reciprocal"));}
            p.bonds[d]=0;q.bonds[d^1]=0;state.write_particle(p)?;state.write_particle(q)?;
            // Removing the bond releases its stored energy before the common thermal quench.
            state.add_heat(p.position,config.bond_energy)?;
            state.event(MatterEvent {tick,kind:EventKind::Intervention,particles:[a,b],positions:[p.position,q.position],
                before:[1,3],after:[0,3],energy:config.bond_energy},config)?;
        }
        Ok(())
    })
}
fn remote_links(reference:&Universe,cohort:&[u64],links:&[(u64,u64)])->Result<Option<Vec<(u64,u64)>>,SimulationError> {
    let state=state(reference)?;let mut connected=BTreeSet::new();
    for group in components(state)? {if group.iter().any(|id|cohort.contains(id)){connected.extend(group);}}
    let mut used=BTreeSet::new();let mut result=Vec::new();
    for &(a,b) in links {
        let mut kinds=[state.particle(a).ok_or_else(||SimulationError::new("missing ablation source"))?.material,
            state.particle(b).ok_or_else(||SimulationError::new("missing ablation target"))?.material];kinds.sort_unstable();
        let mut found=None;
        'search: for id in 1..=state.count() as u64 {
            let p=state.particle(id).ok_or_else(||SimulationError::new("missing remote link source"))?;
            if connected.contains(&id) || used.contains(&id){continue;}
            for other in p.bonds.into_iter().filter(|&n|n>id) {
                if connected.contains(&other) || used.contains(&other){continue;}
                let q=state.particle(other).ok_or_else(||SimulationError::new("missing remote link target"))?;
                let mut pair=[p.material,q.material];pair.sort_unstable();
                if pair==kinds && cohort.iter().all(|&member|state.particle(member).is_some_and(|r|
                    distance(state,p.position,r.position)>=6 && distance(state,q.position,r.position)>=6)) {
                    found=Some((id,other));break 'search;
                }
            }
        }
        let Some(pair)=found else{return Ok(None);};used.insert(pair.0);used.insert(pair.1);result.push(pair);
    }
    Ok(Some(result))
}

pub fn link_audit(source:&Path,output:&Path,replicas:usize)->Result<Value,SimulationError> {
    link_audit_mode(source,output,replicas,false)
}
pub fn link_audit_mode(source:&Path,output:&Path,replicas:usize,added:bool)->Result<Value,SimulationError> {
    if !(16..=64).contains(&replicas){return Err(SimulationError::new("link audit requires 16..64 future streams"));}
    let metadata:Value=read_json(&source.join("protocol.json"))?;
    let protocol:ClusterProtocol=serde_json::from_value(metadata["protocol"].clone()).map_err(json_error)?;protocol.validate()?;
    let seeds:Vec<u64>=serde_json::from_value(metadata["seeds"].clone()).map_err(json_error)?;
    if let Some(parent)=output.parent().filter(|p|!p.as_os_str().is_empty()){fs::create_dir_all(parent).map_err(io_error)?;}
    fs::create_dir(output).map_err(io_error)?;
    write_new_json(&output.join("protocol.json"),&json!({"schema":1,"source":source.to_string_lossy(),"replicas":replicas,"added_links":added,
        "selection":if added {"all selected cohorts with incident bonds present only after local conditioning, absent initially and in the remote-conditioned reference"}
            else {"all selected cohorts with initial bonds retained in the remote-conditioned reference but absent after local conditioning"},
        "selection_limits":"no filtering on observed probe responses",
        "hypothesis":if added {"conditioning-created attachments store a persistent change in later energy transmission"}
            else {"conditioning-induced bond loss stores a persistent change in later energy transmission"},
        "controls":"remove target bonds from their carrier history versus matched remote bond removal; preserve all positions, orientations and conformations",
        "quench":"bond energy is released as heat, then all histories receive the same zero-heat/zero-excitation bath",
        "readout":"receiver excitation integral, local versus equal-energy remote probe",
        "random_stream_namespace":99001,"limits":"approximate intervals are exploratory; lost-bond ablation is not restoration of the entire conditioned state"}))?;
    let mut results=Vec::new();
    for seed in seeds {
        let seed_path=source.join(format!("seed-{seed}"));
        let cohorts:Vec<Vec<u64>>=read_json(&seed_path.join("selected-clusters.json"))?;
        let baseline=Universe::load_checkpoint(&seed_path.join("base.gns"))?;
        if cohorts.is_empty(){results.push(json!({"seed":seed,"status":"no-eligible-stable-cluster"}));}
        for cohort in cohorts {
            let root=*cohort.first().ok_or_else(||SimulationError::new("empty audited cohort"))?;
            if cohort.iter().collect::<BTreeSet<_>>().len()!=cohort.len(){return Err(SimulationError::new("duplicate audited member"));}
            let original=seed_path.join(format!("cluster-{root}"));
            let reference=Universe::load_checkpoint(&original.join("unconditioned/final.gns"))?;
            let trained=Universe::load_checkpoint(&original.join("conditioned/final.gns"))?;
            let a=state(&reference)?;let b=state(&trained)?;let mut targets=BTreeSet::new();
            for &id in &cohort {
                let p=a.particle(id).ok_or_else(||SimulationError::new("missing audited member"))?;
                let q=b.particle(id).ok_or_else(||SimulationError::new("missing conditioned member"))?;
                let initial=state(&baseline)?.particle(id).ok_or_else(||SimulationError::new("missing original cohort member"))?;
                for other in (if added {q.bonds}else{p.bonds}).into_iter().filter(|&n|n!=0) {
                    let eligible=if added {!p.bonds.contains(&other) && !initial.bonds.contains(&other)}
                        else {!q.bonds.contains(&other) && initial.bonds.contains(&other)};
                    if eligible{targets.insert((id.min(other),id.max(other)));}
                }
            }
            let targets:Vec<_>=targets.into_iter().collect();
            let audit_path=output.join(format!("seed-{seed}-cluster-{root}"));fs::create_dir(&audit_path).map_err(io_error)?;
            if targets.is_empty(){results.push(json!({"seed":seed,"root":root,"status":if added {"no-conditioning-created-links"}else{"no-original-bond-loss"}}));continue;}
            let owner=if added {&trained}else{&reference};
            let Some(remote)=remote_links(owner,&cohort,&targets)? else {results.push(json!({"seed":seed,"root":root,"status":"no-matched-remote-links"}));continue;};
            let cut=cut_links(owner,&targets)?;let remote_cut=cut_links(owner,&remote)?;
            let cold=quench(&reference)?;
            let histories=[("unconditioned",thermal_match(&reference,&cold)?),("conditioned",thermal_match(&trained,&cold)?),
                ("local-link-cut",thermal_match(&cut,&cold)?),("remote-link-cut",thermal_match(&remote_cut,&cold)?)];
            if state(&histories[2].1)?.energy_output!=state(&histories[3].1)?.energy_output
                || state(&histories[2].1)?.energy_input!=state(&histories[3].1)?.energy_input {return Err(SimulationError::new("bond-ablation bath work mismatch"));}
            for (name,history) in &histories {verify_thermal_match(history,&cold)?;history.save_checkpoint(&audit_path.join(format!("{name}.gns")))?;}
            write_new_json(&audit_path.join("links.json"),&json!({"target_links":targets,"added_links":added,"matched_remote":remote,"cohort":cohort,
                "unconditioned_source_digest":reference.digest()?,"conditioned_source_digest":trained.digest()?}))?;
            let receivers=receiver_ids(&cohort,&baseline,&protocol)?;
            let space=state(&reference)?.space;let mut trials=Vec::new();let mut history_effect=Vec::new();let mut local_effect=Vec::new();let mut remote_effect=Vec::new();
            let mut future_seeds:BTreeSet<_>=(0..protocol.replicas).map(|i|seeded_value(seed,0,99000,i as u64,root)).collect();
            for replica in 0..replicas {
                let future_seed=seeded_value(seed,0,99001,replica as u64,root);
                if !future_seeds.insert(future_seed){return Err(SimulationError::new("audit future stream overlaps earlier probes"));}
                let replica_path=audit_path.join(format!("replica-{replica:03}"));fs::create_dir(&replica_path).map_err(io_error)?;
                let mut contrasts=[0f64;4];
                for (index,(name,history)) in histories.iter().enumerate() {
                    let mut config=match history.spec(){UniverseSpec::Matter(c)=>c.clone(),_=>unreachable!()};config.seed=future_seed;
                    let history=history.fork_with(UniverseSpec::Matter(config))?;
                    let anchor=state(&history)?.particle(root).ok_or_else(||SimulationError::new("missing link-audit input"))?.position;
                    let local=BTreeSet::from([anchor]);let remote=BTreeSet::from([space.shift(anchor,[(space.width/2) as i32,(space.height/2) as i32,0])]);
                    let mut scores=[0u64;2];
                    for (i,(label,sites)) in [("local",&local),("remote",&remote)].into_iter().enumerate() {
                        scores[i]=probe(pulse(&history,sites,protocol.probe_heat)?,&replica_path.join(format!("{name}-{label}")),&receivers,&protocol)?;
                    }
                    contrasts[index]=scores[0] as f64-scores[1] as f64;
                    trials.push(json!({"replica":replica,"future_seed":future_seed.to_string(),"history":name,"scores":scores,"contrast":contrasts[index]}));
                }
                let owner_index=usize::from(added);
                history_effect.push(contrasts[1]-contrasts[0]);local_effect.push(contrasts[2]-contrasts[owner_index]);remote_effect.push(contrasts[3]-contrasts[owner_index]);
            }
            let hs=stats(&history_effect);let ls=stats(&local_effect);let rs=stats(&remote_effect);
            let supported=hs["interval_excludes_zero"]==true && ls["interval_excludes_zero"]==true
                && (hs["mean"].as_f64().unwrap_or(0.0)*ls["mean"].as_f64().unwrap_or(0.0))*if added {-1.0}else{1.0}>0.0
                && remote_effect.iter().all(|&n|n==0.0);
            let result=json!({"seed":seed,"root":root,"status":"audit-completed","target_links":targets,"added_links":added,"trials":trials,
                "history_effect":hs,"local_link_ablation_effect":ls,"remote_link_ablation_effect":rs,"exploratory_bond_history_support":supported});
            write_new_json(&audit_path.join("result.json"),&result)?;results.push(result);
        }
    }
    let supported_seeds:BTreeSet<_>=results.iter().filter(|r|r["exploratory_bond_history_support"]==true).filter_map(|r|r["seed"].as_u64()).collect();
    let report=json!({"schema":1,"completed":true,"added_links":added,"results":results,"supported_world_seeds":supported_seeds,
        "interpretation":"controlled bond-history mediation audit; exploratory, not adaptive memory or a novel-science claim"});
    write_new_json(&output.join("link-audit.json"),&report)?;Ok(report)
}

fn common_graph(reference:&Universe,trained:&Universe,cohort:&[u64])->Result<(Universe,Universe),SimulationError> {
    let a=state(reference)?;let b=state(trained)?;let mut common=BTreeSet::new();
    for &id in cohort {
        let p=a.particle(id).ok_or_else(||SimulationError::new("missing common-graph member"))?;
        let q=b.particle(id).ok_or_else(||SimulationError::new("missing conditioned graph member"))?;
        for other in p.bonds.into_iter().filter(|&n|n>id && cohort.contains(&n)) {if q.bonds.contains(&other){common.insert((id,other));}}
    }
    let trim=|universe:&Universe|->Result<Universe,SimulationError> {
        let state=state(universe)?;let mut remove=BTreeSet::new();
        for &id in cohort {
            let p=state.particle(id).ok_or_else(||SimulationError::new("missing isolation member"))?;
            for other in p.bonds.into_iter().filter(|&n|n!=0) {
                let pair=(id.min(other),id.max(other));if !common.contains(&pair){remove.insert(pair);}
            }
        }
        cut_links(universe,&remove.into_iter().collect::<Vec<_>>())
    };
    Ok((trim(reference)?,trim(trained)?))
}
pub(crate) fn rearrange(universe:&Universe,targets:&[(u64,u32,u8)])->Result<Universe,SimulationError> {
    universe.edit_material_state(|state,config,tick| {
        let ids:BTreeSet<_>=targets.iter().map(|&(id,_,_)|id).collect();
        let sites:BTreeSet<_>=targets.iter().map(|&(_,site,_)|site).collect();
        if ids.len()!=targets.len() || sites.len()!=targets.len(){return Err(SimulationError::new("duplicate rearrangement member or site"));}
        let originals:Vec<_>=targets.iter().map(|&(id,site,orientation)| {
            if site as usize>=state.space.len() || orientation>2 {return Err(SimulationError::new("invalid rearrangement target"));}
            let occupant=state.occupant(site);
            if occupant!=0 && !ids.contains(&occupant){return Err(SimulationError::new("rearrangement destination occupied by unrelated matter"));}
            let p=state.particle(id).ok_or_else(||SimulationError::new("missing rearrangement member"))?;
            if p.bonds.iter().any(|&n|n!=0 && !ids.contains(&n)){return Err(SimulationError::new("rearrangement cannot break an external bond"));}
            Ok(p)
        }).collect::<Result<_,SimulationError>>()?;
        for p in &originals{state.set_occupant(p.position,0);}
        for &(id,position,orientation) in targets {
            let mut p=state.particle(id).ok_or_else(||SimulationError::new("missing relocated member"))?;
            p.position=position;p.orientation=orientation;p.bonds=[0;6];state.write_particle(p)?;state.set_occupant(position,id);
        }
        for old in originals {
            let mut p=state.particle(old.id).ok_or_else(||SimulationError::new("missing relocated graph member"))?;
            for other in old.bonds.into_iter().filter(|&n|n!=0) {
                let q=state.particle(other).ok_or_else(||SimulationError::new("missing relocated neighbor"))?;
                let direction=state.space.direction(p.position,q.position).ok_or_else(||SimulationError::new("rearrangement breaks internal geometry"))?;
                p.bonds[direction]=other;
            }
            state.write_particle(p)?;
            if p.position!=old.position || p.orientation!=old.orientation {
                state.event(MatterEvent {tick,kind:EventKind::Intervention,particles:[p.id,0],positions:[old.position,p.position],
                    before:[old.position,u32::from(old.orientation)],after:[p.position,u32::from(p.orientation)],energy:0},config)?;
            }
        }
        Ok(())
    })
}
fn remote_rearrangement(universe:&Universe,cohort:&[u64],targets:&[(u64,u32,u8)])->Result<Option<Vec<(u64,u32,u8)>>,SimulationError> {
    let state=state(universe)?;let space=state.space;let mut used=BTreeSet::new();let mut destinations=BTreeSet::new();let mut result=Vec::new();
    for &(source,target,orientation) in targets {
        let p=state.particle(source).ok_or_else(||SimulationError::new("missing displacement source"))?;
        if p.position==target && p.orientation==orientation{continue;}
        let from=space.xyz(p.position);let to=space.xyz(target);
        let delta=[to[0] as i32-from[0] as i32,to[1] as i32-from[1] as i32,to[2] as i32-from[2] as i32];
        let rotation=(orientation+3-p.orientation)%3;let mut found=None;
        for id in 1..=state.count() as u64 {
            let q=state.particle(id).ok_or_else(||SimulationError::new("missing remote displacement candidate"))?;
            if q.degree()!=0 || q.material!=p.material || cohort.contains(&id) || used.contains(&id){continue;}
            let destination=space.shift(q.position,delta);let occupant=state.occupant(destination);
            if (occupant!=0 && occupant!=id) || destinations.contains(&destination){continue;}
            if !cohort.iter().all(|&member|state.particle(member).is_some_and(|r|
                distance(state,q.position,r.position)>=6 && distance(state,destination,r.position)>=6)){continue;}
            found=Some((id,destination,(q.orientation+rotation)%3));break;
        }
        let Some(target)=found else{return Ok(None);};used.insert(target.0);destinations.insert(target.1);result.push(target);
    }
    Ok(Some(result))
}

pub fn geometry_audit(source:&Path,output:&Path,replicas:usize)->Result<Value,SimulationError> {
    if !(32..=64).contains(&replicas){return Err(SimulationError::new("geometry audit requires 32..64 replicas"));}
    let metadata:Value=read_json(&source.join("protocol.json"))?;
    let protocol:ClusterProtocol=serde_json::from_value(metadata["protocol"].clone()).map_err(json_error)?;protocol.validate()?;
    let seeds:Vec<u64>=serde_json::from_value(metadata["seeds"].clone()).map_err(json_error)?;
    if let Some(parent)=output.parent().filter(|p|!p.as_os_str().is_empty()){fs::create_dir_all(parent).map_err(io_error)?;}
    fs::create_dir(output).map_err(io_error)?;
    write_new_json(&output.join("protocol.json"),&json!({"schema":1,"source":source.to_string_lossy(),"replicas":replicas,
        "hypothesis":"conditioning changes spatial arrangement, which persists and changes later energy transmission",
        "receiver_endpoints_only":protocol.receiver_endpoints_only,"source_protocol":protocol,
        "primary_readout":if protocol.latency_readout {"restricted first-excitation latency"}else{"excitation integral"},
        "selection":"all preselected cohorts with different positions/orientations and vacant restoration destinations; no filtering by readout",
        "graph_control":"remove all cohort-external links and all nonshared internal links in both conditioning histories, then quench",
        "local_control":"restore only cohort positions/orientations to reference; preserve conditioned conformations and shared graph",
        "remote_control":"same material types, displacements and relative orientation changes on distant unbonded particles",
        "energy_control":"all tested local and remote rearrangements preserve exact total energy, bath counters and heat/excitation fields",
        "candidate_rule":"history and local-restoration intervals exclude zero with opposite signs; local-restoration effects agree in both replica halves; every remote-control effect is zero",
        "random_stream_namespace":99002,"limits":["restoration is an external state intervention","the model has no position-dependent potential energy",
            "intervals are exploratory and uncorrected","cohort movement changes its exposure to surrounding material","not a claim of adaptive memory, life or novel physics"]}))?;
    let mut results=Vec::new();
    for seed in seeds {
        let seed_path=source.join(format!("seed-{seed}"));let cohorts:Vec<Vec<u64>>=read_json(&seed_path.join("selected-clusters.json"))?;
        let initial=Universe::load_checkpoint(&seed_path.join("base.gns"))?;
        if cohorts.is_empty(){results.push(json!({"seed":seed,"status":"no-eligible-stable-cluster"}));}
        for cohort in cohorts {
            let root=*cohort.first().ok_or_else(||SimulationError::new("empty geometry cohort"))?;
            let path=seed_path.join(format!("cluster-{root}"));
            let reference=Universe::load_checkpoint(&path.join("unconditioned/final.gns"))?;
            let trained=Universe::load_checkpoint(&path.join("conditioned/final.gns"))?;
            let audit_path=output.join(format!("seed-{seed}-cluster-{root}"));fs::create_dir(&audit_path).map_err(io_error)?;
            let targets:Vec<_>=cohort.iter().map(|&id|state(&reference).and_then(|s|s.particle(id).map(|p|(id,p.position,p.orientation))
                .ok_or_else(||SimulationError::new("missing reference geometry")))).collect::<Result<_,_>>()?;
            let ids:BTreeSet<_>=cohort.iter().copied().collect();let trained_state=state(&trained)?;
            if geometry_equal(state(&reference)?,trained_state,&cohort)
                || targets.iter().all(|&(id,position,orientation)|trained_state.particle(id).is_some_and(|p|p.position==position && p.orientation==orientation)) {
                results.push(json!({"seed":seed,"root":root,"status":"no-spatial-history-marker"}));continue;
            }
            if targets.iter().any(|&(_,position,_)|{let id=trained_state.occupant(position);id!=0 && !ids.contains(&id)}) {
                results.push(json!({"seed":seed,"root":root,"status":"restoration-destination-occupied"}));continue;
            }
            let (reference,trained)=common_graph(&reference,&trained,&cohort)?;
            if protocol.require_intact_channel {
                let connected=components(state(&trained)?)?.into_iter().find(|g|g.contains(&root)).unwrap_or_default();
                if !cohort.iter().all(|id|connected.contains(id)) {
                    results.push(json!({"seed":seed,"root":root,"status":"channel-not-intact-after-conditioning"}));continue;
                }
            }
            let cold=quench(&reference)?;let reference=thermal_match(&reference,&cold)?;let trained=thermal_match(&trained,&cold)?;
            let restored=rearrange(&trained,&targets)?;
            let Some(remote_targets)=remote_rearrangement(&trained,&cohort,&targets)? else {
                results.push(json!({"seed":seed,"root":root,"status":"no-matched-remote-rearrangement"}));continue;
            };
            let remote=rearrange(&trained,&remote_targets)?;
            let config=match trained.spec(){UniverseSpec::Matter(config)=>config,_=>unreachable!()};
            let energy=state(&trained)?.total_energy(config)?;
            for other in [&restored,&remote] {
                if state(other)?.total_energy(config)?!=energy || state(other)?.energy_input!=state(&trained)?.energy_input
                    || state(other)?.energy_output!=state(&trained)?.energy_output {return Err(SimulationError::new("geometry intervention changed energy or bath work"));}
                verify_thermal_match(other,&trained)?;
            }
            let histories=[("unconditioned",reference),("conditioned",trained),("local-restored",restored),("remote-restored",remote)];
            for (name,history) in &histories {history.save_checkpoint(&audit_path.join(format!("{name}.gns")))?;}
            write_new_json(&audit_path.join("interventions.json"),&json!({"cohort":cohort,"local_targets":targets,"remote_targets":remote_targets,"energy_preserved":true}))?;
            let receivers=receiver_ids(&cohort,&initial,&protocol)?;let space=state(&histories[0].1)?.space;
            let mut trials=Vec::new();let mut history_effect=Vec::new();let mut local_effect=Vec::new();let mut remote_effect=Vec::new();
            let mut streams:BTreeSet<_>=(0..64).flat_map(|i|[seeded_value(seed,0,99000,i,root),seeded_value(seed,0,99001,i,root)]).collect();
            for replica in 0..replicas {
                let future_seed=seeded_value(seed,0,99002,replica as u64,root);
                if !streams.insert(future_seed){return Err(SimulationError::new("geometry probe stream overlaps previous experiments"));}
                let replica_path=audit_path.join(format!("replica-{replica:03}"));fs::create_dir(&replica_path).map_err(io_error)?;let mut contrasts=[0f64;4];
                for (index,(name,history)) in histories.iter().enumerate() {
                    let mut config=match history.spec(){UniverseSpec::Matter(c)=>c.clone(),_=>unreachable!()};config.seed=future_seed;
                    let history=history.fork_with(UniverseSpec::Matter(config))?;
                    let anchor=state(&history)?.particle(root).ok_or_else(||SimulationError::new("missing geometric input"))?.position;
                    let local=BTreeSet::from([anchor]);let remote=BTreeSet::from([space.shift(anchor,[(space.width/2) as i32,(space.height/2) as i32,0])]);let mut scores=[0u64;2];
                    for (i,(label,sites)) in [("local",&local),("remote",&remote)].into_iter().enumerate() {
                        scores[i]=probe(pulse(&history,sites,protocol.probe_heat)?,&replica_path.join(format!("{name}-{label}")),&receivers,&protocol)?;
                    }
                    contrasts[index]=scores[0] as f64-scores[1] as f64;
                    trials.push(json!({"replica":replica,"future_seed":future_seed.to_string(),"history":name,"scores":scores,"contrast":contrasts[index]}));
                }
                history_effect.push(contrasts[1]-contrasts[0]);local_effect.push(contrasts[2]-contrasts[1]);remote_effect.push(contrasts[3]-contrasts[1]);
            }
            let hs=stats(&history_effect);let ls=stats(&local_effect);let rs=stats(&remote_effect);
            let halves=[stats(&local_effect[..replicas/2]),stats(&local_effect[replicas/2..])];
            let h=hs["mean"].as_f64().unwrap_or(0.0);let l=ls["mean"].as_f64().unwrap_or(0.0);
            let supported=hs["interval_excludes_zero"]==true && ls["interval_excludes_zero"]==true && h*l<0.0
                && halves.iter().all(|s|s["interval_excludes_zero"]==true && s["mean"].as_f64().unwrap_or(0.0)*l>0.0)
                && remote_effect.iter().all(|&n|n==0.0);
            let result=json!({"seed":seed,"root":root,"status":"geometry-audit-completed","trials":trials,
                "history_effect":hs,"local_restoration_effect":ls,"remote_restoration_effect":rs,"local_replica_halves":halves,
                "exploratory_geometric_memory_support":supported});
            write_new_json(&audit_path.join("result.json"),&result)?;results.push(result);
        }
    }
    let supported_seeds:BTreeSet<_>=results.iter().filter(|r|r["exploratory_geometric_memory_support"]==true).filter_map(|r|r["seed"].as_u64()).collect();
    let report=json!({"schema":1,"completed":true,"results":results,"supported_world_seeds":supported_seeds,
        "interpretation":"controlled spatial-state mediation, conditional on selected clusters; not adaptive memory, life, or a novel-science claim"});
    write_new_json(&output.join("geometry-audit.json"),&report)?;Ok(report)
}
