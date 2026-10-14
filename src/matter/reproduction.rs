//! External lineage and counterfactual measurements; no reproduction action is added to the laws.
use std::{collections::{BTreeMap,BTreeSet,VecDeque},fs,path::Path};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use crate::{chemistry::seeded_value,experiment::RunOptions,law_system::SimulationError,
    persistence::{io_error,json_error,read_json,write_new_json},universe::{Universe,UniverseSpec}};
use super::{MatterConfig,MatterState,EventKind,lab,memory::evolve,
    cluster_memory::{components,erase,distance,stats,cut_links,rearrange}};

#[derive(Clone,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct ReproductionProtocol {
    pub warmup:u64,pub stability:u64,pub horizon:u64,pub persistence:u64,
    pub replicas:usize,pub candidates:usize,pub min_size:usize,pub max_size:usize,
    pub archive_limit_mib:u64,
    pub allow_bonded_feedstock:bool,
}
impl Default for ReproductionProtocol {
    fn default()->Self {Self {warmup:1000,stability:20,horizon:256,persistence:20,
        replicas:4,candidates:1,min_size:3,max_size:6,archive_limit_mib:256,allow_bonded_feedstock:false}}
}
impl ReproductionProtocol {
    fn validate(&self)->Result<(),SimulationError> {
        if [self.warmup,self.stability,self.horizon].iter().any(|&n|n==0 || n>10000)
            || self.persistence<2 || self.persistence>self.horizon || !(4..=32).contains(&self.replicas)
            || !(1..=4).contains(&self.candidates) || self.min_size<2 || self.max_size<self.min_size
            || self.max_size>8 || !(64..=4096).contains(&self.archive_limit_mib) {
            return Err(SimulationError::new("invalid reproduction protocol"));
        } Ok(())
    }
}
fn state(world:&Universe)->Result<&MatterState,SimulationError> {
    world.material_state().ok_or_else(||SimulationError::new("reproduction study requires Matter"))
}
fn edges(state:&MatterState,ids:&[u64])->Result<Vec<(u64,u64)>,SimulationError> {
    let mut result=Vec::new();
    for &id in ids {let p=state.particle(id).ok_or_else(||SimulationError::new("missing tracked particle"))?;
        result.extend(p.bonds.into_iter().filter(|&other|other!=0).map(|other|(id.min(other),id.max(other))));}
    result.sort_unstable();result.dedup();Ok(result)
}
fn signature(state:&MatterState,ids:&[u64])->Result<Option<Vec<u8>>,SimulationError> {
    let endpoints:Vec<_>=ids.iter().copied().filter(|&id|state.particle(id).is_some_and(|p|p.degree()==1)).collect();
    if endpoints.len()!=2 || ids.iter().any(|&id|!state.particle(id).is_some_and(|p|p.degree()<=2)){return Ok(None);}
    let mut previous=0;let mut current=endpoints[0];let mut sequence=Vec::new();let mut visited=BTreeSet::new();
    loop {if !visited.insert(current){return Err(SimulationError::new("cycle in selected open chain"));}
        let p=state.particle(current).ok_or_else(||SimulationError::new("missing chain member"))?;
        sequence.push(p.material);
        let next=p.bonds.into_iter().find(|&other|other!=0 && other!=previous);
        if let Some(next)=next {previous=current;current=next;}else{break;}
    }
    if sequence.len()!=ids.len(){return Err(SimulationError::new("chain membership mismatch"));}
    let reverse:Vec<_>=sequence.iter().rev().copied().collect();Ok(Some(sequence.min(reverse)))
}
fn active_ids(world:&Universe,ids:&[u64])->Result<Vec<u64>,SimulationError> {
    let s=state(world)?;Ok(ids.iter().copied().filter(|&id|s.particle(id).is_some_and(|p|p.conformation==1)).collect())
}
fn near(s:&MatterState,position:u32,ids:&[u64],radius:u32)->bool {
    ids.iter().any(|&id|s.particle(id).is_some_and(|p|distance(s,p.position,position)<=radius))
}
fn controls(world:&Universe,parent:&[u64])->Result<Option<(Universe,Universe,Vec<u64>)>,SimulationError> {
    let s=state(world)?;let active=active_ids(world,parent)?;if active.is_empty(){return Ok(None);}
    let mut used=BTreeSet::new();let mut remote=Vec::new();
    for &id in &active {let p=s.particle(id).ok_or_else(||SimulationError::new("missing catalyst"))?;
        let target=(1..=s.count() as u64).find(|other|!used.contains(other) && !parent.contains(other)
            && s.particle(*other).is_some_and(|q|q.material==p.material && q.conformation==1 && !near(s,q.position,parent,6)));
        let Some(target)=target else{return Ok(None);};used.insert(target);remote.push(target);
    }
    let local=erase(world,&active.iter().map(|&id|(id,0)).collect::<Vec<_>>())?;
    let remote_world=erase(world,&remote.iter().map(|&id|(id,0)).collect::<Vec<_>>())?;
    let a=state(&local)?;let b=state(&remote_world)?;
    if a.energy_input!=b.energy_input || a.energy_output!=b.energy_output {
        return Err(SimulationError::new("local and remote erasure bath work differs"));
    }
    Ok(Some((local,remote_world,remote)))
}

fn eligible_feedstock(s:&MatterState,parent:&[u64],forbidden:&[u64],allow_bonded:bool)
    ->Result<BTreeSet<u64>,SimulationError> {
    let mut ids=BTreeSet::new();
    for id in 1..=s.count() as u64 {
        let p=s.particle(id).ok_or_else(||SimulationError::new("missing feedstock particle"))?;
        if !parent.contains(&id) && !forbidden.contains(&id) && (allow_bonded || p.degree()==0) {ids.insert(id);}
    }
    Ok(ids)
}

struct Pending { first:u64,sequence:Vec<u8> }
fn observe_episode(world:Universe,path:&Path,parent:&[u64],forbidden:&[u64],protocol:&ReproductionProtocol,
    save_product:bool)->Result<Value,SimulationError> {
    let initial=state(&world)?;let start=world.generation();
    let target=signature(initial,parent)?.ok_or_else(||SimulationError::new("tracked parent is not an open chain"))?;
    let original_edges=edges(initial,parent)?;
    let free=eligible_feedstock(initial,parent,forbidden,false)?;
    let mut origins=BTreeMap::new();
    for group in components(initial)? {for &id in &group {origins.insert(id,group[0]);}}
    let eligible=if protocol.allow_bonded_feedstock {
        eligible_feedstock(initial,parent,forbidden,true)?
    }else{free.clone()};
    let mut used=BTreeSet::new();let mut contact=BTreeSet::new();let mut contaminated=BTreeSet::new();
    let mut pending:BTreeMap<(Vec<u64>,Vec<(u64,u64)>),Pending>=BTreeMap::new();
    let mut products=Vec::new();let mut all_products=0;let mut matching=0;let mut matching_global=0;
    let mut intact_ticks=0;let mut active_exposure=0;let mut charges=0;let mut dissipations=0;
    let mut catalytic_bindings=0;let mut catalytic_contacts=0;
    let mut independent=true;let mut product_saved=false;let mut trace=Vec::new();
    let options=RunOptions {steps:protocol.horizon,sample_every:32,checkpoint_every:protocol.horizon,
        archive_limit_mib:protocol.archive_limit_mib,label:"autonomous-assembly".into(),..RunOptions::default()};
    let outcome=lab::run_observed(world,path,options,|world| {
        let tick=world.generation();let s=state(world)?;if tick==start{return Ok(());}
        let group=components(s)?.into_iter().find(|g|g.contains(&parent[0])).unwrap_or_default();
        let graph_intact=group==parent && edges(s,parent)?==original_edges;
        intact_ticks+=u64::from(graph_intact);
        active_exposure+=active_ids(world,parent)?.len() as u64;
        if parent.iter().any(|&id|s.particle(id).is_some_and(|p|near(s,p.position,forbidden,5))){independent=false;}
        let catalytic_targets:BTreeSet<_>=s.events.iter().filter(|event|
            matches!(event.kind,EventKind::ContactCatalysis) && parent.contains(&event.particles[0]))
            .map(|event|event.particles[1]).collect();
        for event in &s.events {
            if parent.contains(&event.particles[0]) {
                charges+=u64::from(matches!(event.kind,EventKind::Charge));
                dissipations+=u64::from(matches!(event.kind,EventKind::Dissipate));
                catalytic_contacts+=u64::from(matches!(event.kind,EventKind::ContactCatalysis));
            }
            if matches!(event.kind,EventKind::Bind) {
                catalytic_bindings+=u64::from(event.particles.iter().all(|id|catalytic_targets.contains(id)));
                for (&id,&position) in event.particles.iter().zip(&event.positions) {
                    if eligible.contains(&id) {
                        if near(s,position,parent,1){contact.insert(id);}
                        if near(s,position,forbidden,1){contaminated.insert(id);}
                    }
                }
            }
        }
        let mut present=BTreeSet::new();
        for ids in components(s)? {
            if ids.len()!=parent.len() || ids.iter().any(|id|!eligible.contains(id) || used.contains(id)){continue;}
            let initial_components:BTreeSet<_>=ids.iter().filter_map(|id|origins.get(id).copied()).collect();
            if protocol.allow_bonded_feedstock && initial_components.len()<2 {continue;}
            let Some(sequence)=signature(s,&ids)? else{continue;};
            let key=(ids.clone(),edges(s,&ids)?);present.insert(key.clone());
            let candidate=pending.entry(key).or_insert(Pending {first:tick,sequence});
            if tick-candidate.first+1<protocol.persistence {continue;}
            used.extend(ids.iter().copied());
            let related=ids.iter().any(|id|contact.contains(id)) && !ids.iter().any(|id|contaminated.contains(id));
            let same=candidate.sequence==target;matching_global+=usize::from(same);
            if related {all_products+=1;matching+=usize::from(same);}
            let mut checkpoint=None;
            if save_product && !product_saved && related && same && !active_ids(world,&ids)?.is_empty() {
                world.save_checkpoint(&path.join("first-product.gns"))?;product_saved=true;checkpoint=Some("first-product.gns");
            }
            products.push(json!({"ids":ids,"initial_components":initial_components,"signature":candidate.sequence,"first_generation":candidate.first,
                "qualified_generation":tick,"parent_contact":related,"matches_parent":same,"checkpoint":checkpoint}));
        }
        pending.retain(|key,_|present.contains(key) && !key.0.iter().any(|id|used.contains(id)));
        if tick%32==0 || tick-start==protocol.horizon {trace.push(json!({"elapsed":tick-start,"graph_intact":graph_intact,
            "active_parent_particles":active_ids(world,parent)?.len(),"matching_products":matching,"all_contact_products":all_products}));}
        Ok(())
    })?;
    if !outcome.completed{return Err(SimulationError::new("autonomous assembly episode stopped; see its outcome.json"));}
    let result=json!({"parent":parent,"signature":target,"initial_free_feedstock":free.len(),"initial_eligible_feedstock":eligible.len(),
        "allow_bonded_feedstock":protocol.allow_bonded_feedstock,"matching_contact_products":matching,
        "all_contact_products":all_products,"matching_global_products":matching_global,"products":products,
        "intact_fraction":intact_ticks as f64/protocol.horizon as f64,"active_exposure":active_exposure,
        "parent_catalyzed_bindings":catalytic_bindings,"parent_catalytic_contact_events":catalytic_contacts,
        "parent_charge_events":charges,"parent_dissipation_events":dissipations,"original_parent_remained_distant":independent,
        "matching_fraction":if all_products==0 {0.0}else{matching as f64/all_products as f64},"trace":trace});
    write_new_json(&path.join("assembly.json"),&result)?;Ok(result)
}

fn repair_episode(world:Universe,path:&Path,parent:&[u64],protocol:&ReproductionProtocol)->Result<Value,SimulationError> {
    let expected=edges(state(&world)?,parent)?;let cut=*expected.first().ok_or_else(||SimulationError::new("repair parent lacks a bond"))?;
    let damaged=cut_links(&world,&[cut])?;let start=damaged.generation();let mut consecutive=0;let mut recovered=None;let mut charges=0;
    let outcome=lab::run_observed(damaged,path,RunOptions {steps:protocol.horizon,sample_every:32,
        checkpoint_every:protocol.horizon,archive_limit_mib:protocol.archive_limit_mib,label:"damage-recovery".into(),..RunOptions::default()},|world| {
        if world.generation()==start{return Ok(());}
        let s=state(world)?;
        if edges(s,parent)?==expected {consecutive+=1;}else{consecutive=0;}
        if consecutive>=protocol.persistence && recovered.is_none(){recovered=Some(world.generation()-start);}
        charges+=s.events.iter().filter(|e|matches!(e.kind,EventKind::Charge) && parent.contains(&e.particles[0])).count();Ok(())
    })?;
    if !outcome.completed{return Err(SimulationError::new("damage recovery episode incomplete"));}
    let result=json!({"cut_bond":cut,"recovered_at":recovered,"recovered":recovered.is_some(),"parent_charge_events":charges});
    write_new_json(&path.join("repair.json"),&result)?;Ok(result)
}

fn assay(world:&Universe,parent:&[u64],forbidden:&[u64],output:&Path,protocol:&ReproductionProtocol,seed:u64,namespace:u64)
    ->Result<Value,SimulationError> {
    fs::create_dir(output).map_err(io_error)?;
    let initial=state(world)?;
    let target=signature(initial,parent)?.ok_or_else(||SimulationError::new("tracked parent is not an open chain"))?;
    let eligible=eligible_feedstock(initial,parent,forbidden,protocol.allow_bonded_feedstock)?;
    let mut required=[0usize;4];let mut available=[0usize;4];
    for material in target {required[material as usize]+=1;}
    for &id in &eligible {
        let p=initial.particle(id).ok_or_else(||SimulationError::new("missing eligible feedstock"))?;
        available[p.material as usize]+=1;
    }
    let component_count=components(initial)?.iter().filter(|group|group.iter().any(|id|eligible.contains(id)
        && initial.particle(*id).is_some_and(|p|required[p.material as usize]>0))).count();
    if required.iter().zip(available).any(|(&need,have)|have<need)
        || (protocol.allow_bonded_feedstock && component_count<2) {
        let result=json!({"status":"insufficient-eligible-feedstock","parent":parent,
            "required_material_counts":required,"available_material_counts":available,
            "eligible_feedstock_particles":eligible.len(),"eligible_initial_components":component_count,
            "allow_bonded_feedstock":protocol.allow_bonded_feedstock,
            "interpretation":"matching assembly is impossible under the declared product rule; not an evaluated negative effect"});
        write_new_json(&output.join("result.json"),&result)?;return Ok(result);
    }
    let Some((local,remote,remote_ids))=controls(world,parent)? else {
        let result=json!({"status":"no-matched-active-erasure-control"});
        write_new_json(&output.join("result.json"),&result)?;return Ok(result);
    };
    write_new_json(&output.join("interventions.json"),&json!({"parent":parent,"local_erased":active_ids(world,parent)?,
        "remote_erased":remote_ids,"matched_bath_work":true,"forbidden_original_parent":forbidden}))?;
    let mut effects=Vec::new();let mut remote_effects=Vec::new();let mut specificity=Vec::new();let mut repair_effects=Vec::new();
    let mut exposure_effects=Vec::new();let mut trials=Vec::new();let mut first_product=None;
    for replica in 0..protocol.replicas {
        let future=seeded_value(seed,0,namespace,replica as u64,parent[0]);
        let replica_path=output.join(format!("replica-{replica:03}"));fs::create_dir(&replica_path).map_err(io_error)?;
        let mut results=Vec::new();
        for (label,history) in [("intact",world),("local-erased",&local),("remote-erased",&remote)] {
            let mut config=match history.spec(){UniverseSpec::Matter(c)=>c.clone(),_=>unreachable!()};config.seed=future;
            let branch=history.fork_with(UniverseSpec::Matter(config))?;
            let path=replica_path.join(label);let r=observe_episode(branch,&path,parent,forbidden,protocol,label=="intact" && first_product.is_none())?;
            if label=="intact" && first_product.is_none() {
                if let Some(product)=r["products"].as_array().and_then(|p|p.iter().find(|p|p["checkpoint"].is_string())) {
                    first_product=Some(json!({"path":path.join("first-product.gns").to_string_lossy(),"ids":product["ids"]}));
                }
            }
            results.push(r);
        }
        let score=|r:&Value,key:&str|r[key].as_f64().unwrap_or(0.0);
        effects.push(score(&results[0],"matching_contact_products")-score(&results[1],"matching_contact_products"));
        remote_effects.push(score(&results[0],"matching_contact_products")-score(&results[2],"matching_contact_products"));
        specificity.push(score(&results[0],"matching_fraction")-score(&results[1],"matching_fraction"));
        exposure_effects.push(score(&results[0],"active_exposure")-score(&results[1],"active_exposure"));
        let mut c=match world.spec(){UniverseSpec::Matter(c)=>c.clone(),_=>unreachable!()};c.seed=future;
        let driven=repair_episode(world.fork_with(UniverseSpec::Matter(c.clone()))?,&replica_path.join("repair-driven"),parent,protocol)?;
        c.reservoir.enabled=false;
        let closed=repair_episode(world.fork_with(UniverseSpec::Matter(c))?,&replica_path.join("repair-closed"),parent,protocol)?;
        repair_effects.push(u8::from(driven["recovered"]==true) as f64-u8::from(closed["recovered"]==true) as f64);
        trials.push(json!({"replica":replica,"future_seed":future.to_string(),"arms":results,"repair_driven":driven,"repair_closed":closed}));
    }
    let production=stats(&effects);let remote=stats(&remote_effects);let enrichment=stats(&specificity);let exposure=stats(&exposure_effects);let repair=stats(&repair_effects);
    let mean=|v:&Value|v["mean"].as_f64().unwrap_or(0.0);
    let causal=production["interval_excludes_zero"]==true && mean(&production)>0.0
        && remote["approximate_interval"][0].as_f64().unwrap_or(-1.0)>=-0.25
        && remote["approximate_interval"][1].as_f64().unwrap_or(1.0)<=0.25
        && exposure["interval_excludes_zero"]==true && mean(&exposure)>0.0;
    let maintains=trials.iter().all(|t|t["arms"][0]["intact_fraction"].as_f64().unwrap_or(0.0)>=0.9
        && t["arms"][0]["parent_charge_events"].as_u64().unwrap_or(0)>0
        && t["arms"][0]["original_parent_remained_distant"]==true);
    let result=json!({"status":"assay-completed","parent":parent,"signature":signature(state(world)?,parent)?,"trials":trials,
        "production_effect":production,"remote_effect":remote,"matching_fraction_effect":enrichment,"active_exposure_effect":exposure,
        "damage_recovery_effect":repair,"causal_matching_assembly":causal,"maintained_with_energy_exchange":maintains,
        "matching_enrichment_support":enrichment["interval_excludes_zero"]==true && mean(&enrichment)>0.0,
        "driven_recovery_support":repair["interval_excludes_zero"]==true && mean(&repair)>0.0,"first_product":first_product});
    write_new_json(&output.join("result.json"),&result)?;Ok(result)
}

fn separate_original(world:&Universe,original:&[u64],child:&[u64])->Result<Option<(Universe,Vec<u64>)>,SimulationError> {
    let s=state(world)?;let mut moving=BTreeSet::new();
    for group in components(s)? {if group.iter().any(|id|original.contains(id)){moving.extend(group);}}
    if moving.len()>64 || child.iter().any(|id|moving.contains(id)){return Ok(None);}
    for dz in 0..s.space.depth {for dy in 0..s.space.height {for dx in 0..s.space.width {
        let targets:Vec<_>=moving.iter().filter_map(|&id|s.particle(id).map(|p|(id,s.space.shift(p.position,[dx as i32,dy as i32,dz as i32]),p.orientation))).collect();
        if targets.iter().all(|&(_,site,_)|(s.occupant(site)==0 || moving.contains(&s.occupant(site))) && !near(s,site,child,5)) {
            return Ok(Some((rearrange(world,&targets)?,moving.into_iter().collect())));
        }
    }}}Ok(None)
}

pub fn run(mut config:MatterConfig,seeds:&[u64],protocol:ReproductionProtocol,output:&Path)->Result<Value,SimulationError> {
    protocol.validate()?;config.validate()?;
    if seeds.is_empty() || seeds.len()>16 || seeds.iter().copied().collect::<BTreeSet<_>>().len()!=seeds.len()
        || !config.reservoir.enabled || !config.execution.full_provenance {
        return Err(SimulationError::new("reproduction campaign requires unique seeds, driven Matter and full provenance"));
    }
    if let Some(parent)=output.parent().filter(|p|!p.as_os_str().is_empty()){fs::create_dir_all(parent).map_err(io_error)?;}
    fs::create_dir(output).map_err(io_error)?;
    write_new_json(&output.join("protocol.json"),&json!({"schema":1,"protocol":protocol,"seeds":seeds,"spec":config,
        "hypothesis":"naturally formed open chains causally promote separate matching chains from initially unbonded feedstock",
        "selection":"stable internal bond identities within configured size bounds, active conformations; size then particle ID",
        "product_rule":if protocol.allow_bonded_feedstock {"same material sequence; disjoint non-parent atoms from at least two initial components; contact during accepted binding; persistent graph; each atom counted once"}
            else{"same reversal-invariant material sequence; initially unbonded disjoint atoms; contact during accepted binding; unchanged graph for persistence ticks; each atom counted once"},
        "controls":"initial local versus matched remote active-conformation erasure; identical heat and excitation; bath work matched; no repeated erasure",
        "descendant_rule":"first qualifying active product by replica and chronological order; original parent relocated beyond five lattice steps; reject loss of separation",
        "support_rule":"positive paired production and exposure effects; remote effect interval inside +/-0.25; matching enrichment and driven recovery required for stronger support",
        "limits":["counterfactual interventions are external","closed controls retain their initial thermal energy","initial erasure can be undone autonomously",
            "chain matching alone is not inherited reproduction","intervals are exploratory and uncorrected","no organism commands or new material laws"]}))?;
    let mut results=Vec::new();
    for &seed in seeds {
        config.seed=seed;let seed_path=output.join(format!("seed-{seed}"));fs::create_dir(&seed_path).map_err(io_error)?;
        println!("world {seed}: warmup");
        let before=evolve(Universe::from_spec(UniverseSpec::Matter(config.clone()))?,&seed_path.join("warmup"),protocol.warmup,protocol.archive_limit_mib,"reproduction-warmup")?;
        let base=evolve(before.fork_current()?,&seed_path.join("stability"),protocol.stability,protocol.archive_limit_mib,"reproduction-stability")?;
        let a=state(&before)?;let b=state(&base)?;
        let mut selected=Vec::new();
        for ids in components(b)? {
            if ids.len()>=protocol.min_size && ids.len()<=protocol.max_size && signature(b,&ids)?.is_some()
                && edges(a,&ids)?==edges(b,&ids)? && !active_ids(&base,&ids)?.is_empty(){selected.push(ids);}
        }
        selected.sort_by(|a,b|b.len().cmp(&a.len()).then_with(||a[0].cmp(&b[0])));selected.truncate(protocol.candidates);
        write_new_json(&seed_path.join("selected-parents.json"),&selected)?;
        if selected.is_empty(){results.push(json!({"seed":seed,"status":"no-eligible-natural-parent"}));}
        for parent in selected {
            println!("world {seed}: parent {}",parent[0]);
            let path=seed_path.join(format!("parent-{}",parent[0]));fs::create_dir(&path).map_err(io_error)?;
            let first=assay(&base,&parent,&[],&path.join("first-generation"),&protocol,seed,99100)?;
            let mut descendant=if first["status"]=="assay-completed" {json!({"status":"no-qualified-active-product"})}
                else {json!({"status":"first-generation-assay-ineligible"})};
            if first["first_product"].is_object() {
                let record=&first["first_product"];let child:Vec<u64>=serde_json::from_value(record["ids"].clone()).map_err(json_error)?;
                let world=Universe::load_checkpoint(Path::new(record["path"].as_str().ok_or_else(||SimulationError::new("missing product checkpoint"))?))?;
                if let Some((separated,original))=separate_original(&world,&parent,&child)? {
                    separated.save_checkpoint(&path.join("descendant-initial.gns"))?;
                    descendant=assay(&separated,&child,&original,&path.join("second-generation"),&protocol,seed,99101)?;
                }else{descendant=json!({"status":"original-parent-cannot-be-separated"});}
            }
            let supports=first["causal_matching_assembly"]==true && descendant["causal_matching_assembly"]==true
                && first["maintained_with_energy_exchange"]==true && descendant["maintained_with_energy_exchange"]==true
                && first["matching_enrichment_support"]==true && descendant["matching_enrichment_support"]==true
                && first["driven_recovery_support"]==true && descendant["driven_recovery_support"]==true;
            let result=json!({"seed":seed,"parent":parent,"first_generation":first,"second_generation":descendant,
                "exploratory_repeated_production_support":supports});write_new_json(&path.join("result.json"),&result)?;results.push(result);
        }
    }
    let supported:BTreeSet<_>=results.iter().filter(|r|r["exploratory_repeated_production_support"]==true).filter_map(|r|r["seed"].as_u64()).collect();
    let assayed:BTreeSet<_>=results.iter().filter(|r|r["first_generation"]["status"]=="assay-completed")
        .filter_map(|r|r["seed"].as_u64()).collect();
    let ineligible:BTreeSet<_>=seeds.iter().copied().filter(|seed|!assayed.contains(seed)).collect();
    let report=json!({"schema":1,"completed":true,"results":results,"supported_world_seeds":supported,
        "assayed_world_seeds":assayed,"ineligible_world_seeds":ineligible,
        "heritable_evolution_demonstrated":false,"interpretation":"matching assembly, maintenance and descendant counterfactuals; full heredity additionally needs variant-specific inheritance and selection"});
    write_new_json(&output.join("reproduction.json"),&report)?;Ok(report)
}

pub fn audit_product(source:&Path,output:&Path)->Result<Value,SimulationError> {
    let metadata:Value=read_json(&source.join("protocol.json"))?;
    let protocol:ReproductionProtocol=serde_json::from_value(metadata["protocol"].clone()).map_err(json_error)?;protocol.validate()?;
    let report:Value=read_json(&source.join("reproduction.json"))?;
    if report["completed"]!=true{return Err(SimulationError::new("product audit requires a completed campaign"));}
    if let Some(parent)=output.parent().filter(|p|!p.as_os_str().is_empty()){fs::create_dir_all(parent).map_err(io_error)?;}
    fs::create_dir(output).map_err(io_error)?;
    write_new_json(&output.join("protocol.json"),&json!({"source":source.to_string_lossy(),"protocol":protocol,
        "selection":"first active global matching product in recorded world, parent, replica and qualification order; at most 64 candidates",
        "claim":"post-campaign diagnostic of an independently formed product, not evidence of its parent causing reproduction",
        "support_limit":"this diagnostic cannot satisfy the campaign's parent-contact or heritable-reproduction claim"}))?;
    let mut attempts=Vec::new();let mut replayed=BTreeSet::new();let mut selected=None;
    'selection: for record in report["results"].as_array().ok_or_else(||SimulationError::new("missing campaign results"))? {
        let Some(parent)=record["parent"].as_array() else{continue;};
        let parent:Vec<u64>=serde_json::from_value(json!(parent)).map_err(json_error)?;
        let root=*parent.first().ok_or_else(||SimulationError::new("empty source parent"))?;
        let seed=record["seed"].as_u64().ok_or_else(||SimulationError::new("invalid source seed"))?;
        let Some(trials)=record["first_generation"]["trials"].as_array() else{continue;};
        for trial in trials {
            let replica=trial["replica"].as_u64().ok_or_else(||SimulationError::new("missing source replica"))?;
            if replica>=protocol.replicas as u64{return Err(SimulationError::new("source replica outside protocol"));}
            let episode=source.join(format!("seed-{seed}/parent-{root}/first-generation/replica-{replica:03}/intact"));
            let Some(products)=trial["arms"][0]["products"].as_array() else{continue;};
            for product in products.iter().filter(|p|p["matches_parent"]==true) {
                if attempts.len()>=64 {break 'selection;}
                if replayed.insert(episode.clone()){lab::replay(&episode)?;}
                let mut world=Universe::load_checkpoint(&episode.join("initial.gns"))?;
                let qualification=product["qualified_generation"].as_u64().ok_or_else(||SimulationError::new("invalid qualification generation"))?;
                let elapsed=qualification.checked_sub(world.generation()).ok_or_else(||SimulationError::new("qualification predates source"))?;
                if elapsed>protocol.horizon || elapsed<protocol.persistence{return Err(SimulationError::new("qualification outside recorded stability horizon"));}
                let expected=signature(state(&world)?,&parent)?.ok_or_else(||SimulationError::new("source parent is not an open chain"))?;
                let child:Vec<u64>=serde_json::from_value(product["ids"].clone()).map_err(json_error)?;
                let initial_state=state(&world)?;
                if child.len()!=parent.len() || child.iter().any(|id|parent.contains(id)
                    || !initial_state.particle(*id).is_some_and(|p|protocol.allow_bonded_feedstock || p.degree()==0)) {
                    return Err(SimulationError::new("diagnostic product is not disjoint initial feedstock"));
                }
                if protocol.allow_bonded_feedstock {
                    let origins:Vec<_>=components(initial_state)?.into_iter().filter(|g|g.iter().any(|id|child.contains(id))).collect();
                    if origins.len()<2 {return Err(SimulationError::new("diagnostic product is a fragment of one initial component"));}
                }
                let mut recent=VecDeque::new();
                for _ in 0..elapsed {
                    world.step()?;let s=state(&world)?;
                    let group=components(s)?.into_iter().find(|g|g.contains(&child[0])).unwrap_or_default();
                    recent.push_back(if group==child {Some(edges(s,&child)?)}else{None});
                    if recent.len()>protocol.persistence as usize {recent.pop_front();}
                }
                let group=components(state(&world)?)?.into_iter().find(|g|g.contains(&child[0])).unwrap_or_default();
                let final_edges=edges(state(&world)?,&child)?;
                if group!=child || signature(state(&world)?,&child)?!=Some(expected)
                    || recent.iter().any(|e|e.as_ref()!=Some(&final_edges)) {
                    return Err(SimulationError::new("reconstructed product disagrees with archived qualification"));
                }
                if active_ids(&world,&child)?.is_empty(){attempts.push(json!({"seed":seed,"replica":replica,"ids":child,"status":"product-inactive-at-qualification"}));continue;}
                let Some((separated,original))=separate_original(&world,&parent,&child)? else {
                    attempts.push(json!({"seed":seed,"replica":replica,"ids":child,"status":"original-parent-cannot-be-separated"}));continue;
                };
                separated.save_checkpoint(&output.join("product-initial.gns"))?;
                attempts.push(json!({"seed":seed,"replica":replica,"ids":child,"status":"selected-global-product",
                    "parent_contact":product["parent_contact"],"source_episode":episode.to_string_lossy(),"qualification":qualification}));
                selected=Some((separated,child,original,seed));break 'selection;
            }
        }
    }
    let result=if let Some((world,child,original,seed))=selected {
        let assay=assay(&world,&child,&original,&output.join("product-assay"),&protocol,seed,99102)?;
        json!({"completed":true,"status":"diagnostic-product-assay-completed","attempts":attempts,"assay":assay,
            "supports_original_reproduction_claim":false,"interpretation":"an independently assembled matching product was assayed; original causal lineage was not established"})
    }else{json!({"completed":true,"status":"no-eligible-active-global-product","attempts":attempts,"supports_original_reproduction_claim":false})};
    write_new_json(&output.join("product-audit.json"),&result)?;Ok(result)
}
