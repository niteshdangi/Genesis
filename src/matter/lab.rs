use std::{collections::{BTreeMap,BTreeSet},fs::{self,File,OpenOptions},io::{BufRead,BufReader,BufWriter,Write},path::Path,time::{Instant,SystemTime,UNIX_EPOCH}};
use serde_json::{Value,json};
use crate::{experiment::{RunManifest,RunOptions,RunRecord,RunOutcome},law_system::SimulationError,
    persistence::{io_error,json_error,read_json,write_new_json},storage::{catalog::Catalog,journal},universe::{Universe,UniverseSpec}};
use super::MatterConfig;

fn record(writer:&mut BufWriter<File>,row:&RunRecord)->Result<(),SimulationError> {
    serde_json::to_writer(&mut *writer,row).map_err(json_error)?; writer.write_all(b"\n").map_err(io_error)?;
    writer.flush().map_err(io_error)?; writer.get_ref().sync_all().map_err(io_error)
}
fn make_directory(path:&Path)->Result<(),SimulationError> {
    if let Some(parent)=path.parent().filter(|p|!p.as_os_str().is_empty()) {fs::create_dir_all(parent).map_err(io_error)?;}
    fs::create_dir(path).map_err(io_error)
}
fn reserve_checkpoint(config:&MatterConfig)->u64 {
    (config.space.chunk_count() as u64*4096*16 + config.particles as u64*96
        + config.execution.event_limit as u64*96 + config.space.chunk_count() as u64*160 + 4096).saturating_mul(2)
}
pub fn run(universe:Universe,output:&Path,options:RunOptions)->Result<RunOutcome,SimulationError> {
    run_observed(universe,output,options,|_|Ok(()))
}
pub fn profile(mut universe:Universe,output:&Path,steps:u64)->Result<Value,SimulationError> {
    if steps==0 || steps>10000 || universe.generation().checked_add(steps).is_none() {
        return Err(SimulationError::new("profile requires 1..10000 steps without generation overflow"));
    }
    let UniverseSpec::Matter(config)=universe.spec() else{return Err(SimulationError::new("profile requires Matter"));};
    if !config.execution.full_provenance{return Err(SimulationError::new("profile requires untruncated event generation"));}
    make_directory(output)?;
    universe.save_checkpoint(&output.join("initial.gns"))?;
    let start_generation=universe.generation();let initial_digest=universe.digest()?;
    let mut rows=Vec::new();let mut total=0u128;let mut failure=None;
    use sha2::{Digest,Sha256};
    for _ in 0..steps {
        let started=Instant::now();
        if let Err(error)=universe.step(){failure=Some(error.to_string());break;}
        let elapsed=started.elapsed().as_micros();total+=elapsed;
        let (events,truncated)=universe.material_events().ok_or_else(||SimulationError::new("missing profile events"))?;
        if truncated{return Err(SimulationError::new("profile events truncated"));}
        let bytes=postcard::to_allocvec(events).map_err(|e|SimulationError::new(e.to_string()))?;
        rows.push(json!({"generation":universe.generation().to_string(),"step_microseconds":elapsed.to_string(),
            "event_count":events.len(),"event_digest":format!("{:x}",Sha256::digest(bytes))}));
    }
    universe.save_checkpoint(&output.join("final.gns"))?;
    let completed_steps=universe.generation()-start_generation;
    let result=json!({"completed":failure.is_none(),"error":failure,"initial_generation":start_generation.to_string(),"final_generation":universe.generation().to_string(),
        "initial_digest":initial_digest,"final_digest":universe.digest()?,"steps":steps,"core_step_microseconds":total.to_string(),
        "completed_steps":completed_steps,"mean_step_microseconds":(total/u128::from(completed_steps.max(1))).to_string(),"metrics":universe.metrics(),"rows":rows,
        "full_event_archive":false,"scope":"Core step timings include invariant validation and observation; exclude loading, event hashing, checkpoint I/O and per-tick laboratory journals. Initial/final checkpoints and event fingerprints are retained."});
    write_new_json(&output.join("profile.json"),&result)?;Ok(result)
}
pub fn run_observed<F>(mut universe:Universe,output:&Path,options:RunOptions,mut observer:F)->Result<RunOutcome,SimulationError>
where F:FnMut(&Universe)->Result<(),SimulationError> {
    let initial_generation=universe.generation(); options.validate(initial_generation)?;
    for intervention in &options.interventions {if intervention.spec.id()!="matter" {return Err(SimulationError::new("material interventions must retain the Matter world"));}}
    make_directory(output)?; fs::create_dir(output.join("events")).map_err(io_error)?;
    let initial_digest=universe.digest()?;
    let initial_bytes=universe.save_checkpoint(&output.join("initial.gns"))?;
    let archive_limit=options.archive_limit_mib*1024*1024;
    let UniverseSpec::Matter(config)=universe.spec() else{return Err(SimulationError::new("material run requires a Matter specification"));};
    let checkpoint_reserve=options.interventions.iter().filter_map(|intervention|match &intervention.spec {
        UniverseSpec::Matter(config)=>Some(reserve_checkpoint(config)),_=>None,
    }).chain(std::iter::once(reserve_checkpoint(config))).max().unwrap_or(0);
    // Reserve room for a final checkpoint even when a campaign reaches its archive budget.
    if initial_bytes.saturating_add(checkpoint_reserve)>archive_limit{return Err(SimulationError::new("archive budget cannot hold initial and conservatively reserved final material checkpoints"));}
    let mut archive_bytes=initial_bytes;
    let manifest=RunManifest {schema:2,package_version:env!("CARGO_PKG_VERSION").into(),semantics:universe.semantics().into(),
        created_unix_seconds:SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|SimulationError::new(e.to_string()))?.as_secs(),
        os:std::env::consts::OS.into(),architecture:std::env::consts::ARCH.into(),initial_digest:initial_digest.clone(),
        initial_generation,spec:universe.spec().clone(),options:options.clone()};
    write_new_json(&output.join("manifest.json"),&manifest)?;
    let catalog=Catalog::open(&output.join("catalog.sqlite"))?;
    let mut metadata=serde_json::to_value(&manifest).map_err(json_error)?;
    use sha2::{Digest,Sha256};
    metadata["dependency_lock_digest"]=json!(format!("{:x}",Sha256::digest(include_bytes!("../../Cargo.lock"))));
    metadata["artifact_layout"]=json!("immutable compressed chunk objects and binary checkpoint manifests");
    write_new_json(&output.join("provenance.json"),&metadata)?;
    let run_id=catalog.begin(&options.label,"matter",&metadata)?;
    catalog.checkpoint(run_id,initial_generation,&initial_digest,"initial.gns",initial_bytes)?;
    let file=OpenOptions::new().write(true).create_new(true).open(output.join("history.ndjson")).map_err(io_error)?;
    let mut writer=BufWriter::new(file);
    record(&mut writer,&RunRecord {generation:initial_generation,digest:Some(initial_digest),metrics:Some(universe.metrics()),events:Vec::new()})?;
    catalog.sample(run_id,initial_generation,&universe.metrics())?;
    observer(&universe)?;
    let started=Instant::now(); let target=initial_generation+options.steps;
    let mut index=0; let mut error=None;
    while universe.generation()<target {
        let tick=universe.generation()+1;
        let mut interventions=Vec::new();
        let intervention=options.interventions.get(index).filter(|i|i.generation==tick);
        let candidate=if let Some(intervention)=intervention {
            interventions.push(json!({"kind":"intervention","generation":tick.to_string(),"spec":intervention.spec}));
            universe.fork_with(intervention.spec.clone())
        } else {universe.fork_current()};
        let mut candidate=match candidate {Ok(candidate)=>candidate,Err(failure)=>{error=Some(failure.to_string());break;}};
        if let Err(failure)=candidate.step() {error=Some(failure.to_string());break;}
        let sampled=tick%options.sample_every==0 || tick==target;
        let checkpointed=tick%options.checkpoint_every==0;
        let digest=if sampled || checkpointed {match candidate.digest(){Ok(digest)=>Some(digest),Err(e)=>{error=Some(e.to_string());break;}}}else{None};
        let events_path=format!("events/{tick:020}.zst");
        let (events,truncated)=candidate.material_events().ok_or_else(||SimulationError::new("missing material event source"))?;
        let event_count=events.len();
        let event_upper_bound=(event_count as u64).saturating_mul(128).saturating_add(4096);
        let scheduled_reserve=if checkpointed {checkpoint_reserve}else{0};
        if archive_bytes.saturating_add(event_upper_bound).saturating_add(scheduled_reserve).saturating_add(checkpoint_reserve)>archive_limit {
            error=Some("material event/checkpoint archive budget reached; increase archive_limit_mib for a longer campaign".into());break;
        }
        let event_digest=match journal::save(&output.join(&events_path),tick,events,truncated) {Ok(digest)=>digest,Err(e)=>{error=Some(e.to_string());break;}};
        let checkpoint_path=format!("checkpoint-{tick:020}.gns");
        let checkpoint_bytes=if checkpointed {match candidate.save_checkpoint(&output.join(&checkpoint_path)){Ok(bytes)=>Some(bytes),Err(e)=>{error=Some(e.to_string());break;}}}else{None};
        let event_bytes=fs::metadata(output.join(&events_path)).map_err(io_error)?.len();
        archive_bytes=archive_bytes.saturating_add(event_bytes).saturating_add(checkpoint_bytes.unwrap_or(0));
        if archive_bytes.saturating_add(checkpoint_reserve)>archive_limit {error=Some("material archive budget reached before run-cursor publication".into());break;}
        interventions.push(json!({"kind":"material_batch","path":events_path,"digest":event_digest,"retained":event_count,"truncated":truncated}));
        let row=RunRecord {generation:tick,digest:digest.clone(),metrics:sampled.then(||candidate.metrics()),events:interventions};
        if let Err(e)=record(&mut writer,&row) {error=Some(e.to_string());break;}
        // Artifact and journal publication precede the in-memory run cursor.
        universe=candidate; if intervention.is_some(){index+=1;}
        let catalog_result=(||{
            catalog.events(run_id,tick,&events_path,&event_digest,event_count,truncated)?;
            if let Some(metrics)=&row.metrics {catalog.sample(run_id,tick,metrics)?;}
            if let Some(bytes)=checkpoint_bytes {catalog.checkpoint(run_id,tick,digest.as_deref().ok_or_else(||SimulationError::new("missing checkpoint digest"))?,&checkpoint_path,bytes)?;}
            Ok::<(),SimulationError>(())
        })();
        if let Err(failure)=catalog_result {error=Some(format!("catalog update failed after journal publication: {failure}"));break;}
        if let Err(failure)=observer(&universe) {error=Some(format!("observer failed after journal publication: {failure}"));break;}
    }
    let final_bytes=universe.save_checkpoint(&output.join("final.gns"))?;
    let final_digest=universe.digest()?;
    let outcome=RunOutcome {completed:error.is_none() && universe.generation()==target,error,initial_generation,
        final_generation:universe.generation(),elapsed_milliseconds:started.elapsed().as_millis().to_string(),final_digest,
        final_metrics:universe.metrics(),checkpoint_bytes:final_bytes};
    write_new_json(&output.join("outcome.json"),&outcome)?;
    catalog.checkpoint(run_id,outcome.final_generation,&outcome.final_digest,"final.gns",final_bytes)?;
    catalog.finish(run_id,outcome.completed)?;
    Ok(outcome)
}

pub fn replay(directory:&Path)->Result<Value,SimulationError> {
    let manifest:RunManifest=read_json(&directory.join("manifest.json"))?;
    if manifest.schema!=2 {return Err(SimulationError::new("unsupported material experiment manifest"));}
    let mut universe=Universe::load_checkpoint(&directory.join("initial.gns"))?;
    if manifest.semantics!=universe.semantics() {return Err(SimulationError::new("material experiment semantics mismatch"));}
    if universe.digest()?!=manifest.initial_digest {return Err(SimulationError::new("initial material digest mismatch"));}
    let outcome:RunOutcome=read_json(&directory.join("outcome.json"))?;
    manifest.options.validate(universe.generation())?;
    let reader=BufReader::new(File::open(directory.join("history.ndjson")).map_err(io_error)?);
    let mut previous=None; let mut index=0; let mut matched=0u64;
    for line in reader.lines() {
        if previous==Some(outcome.final_generation){break;}
        let row:RunRecord=serde_json::from_str(&line.map_err(io_error)?).map_err(json_error)?;
        if previous.is_some_and(|tick|row.generation!=tick+1) || (previous.is_none() && row.generation!=universe.generation()) {return Err(SimulationError::new("material journal is not contiguous"));}
        previous=Some(row.generation);
        if universe.generation()<row.generation {
            if let Some(intervention)=manifest.options.interventions.get(index).filter(|i|i.generation==row.generation) {
                universe=universe.fork_with(intervention.spec.clone())?;index+=1;
            }
            universe.step()?;
        }
        if let Some(expected)=row.digest {if universe.digest()?!=expected {return Err(SimulationError::new(format!("material replay diverged at {}",row.generation)));}matched+=1;}
        for event in &row.events {if event["kind"]=="material_batch" {
            let path=batch_path(directory,event)?;
            let expected=event["digest"].as_str().ok_or_else(||SimulationError::new("missing event batch digest"))?;
            let batch=journal::load(&path,expected)?;
            let (events,truncated)=universe.material_events().ok_or_else(||SimulationError::new("missing replay event stream"))?;
            if batch.generation!=row.generation || batch.truncated!=truncated
                || postcard::to_allocvec(&batch.events).map_err(|e|SimulationError::new(e.to_string()))?
                    !=postcard::to_allocvec(events).map_err(|e|SimulationError::new(e.to_string()))? {
                return Err(SimulationError::new(format!("material event replay diverged at {}",row.generation)));
            }
        }}
    }
    if universe.generation()!=outcome.final_generation || universe.digest()?!=outcome.final_digest {return Err(SimulationError::new("material final replay differs"));}
    Ok(json!({"matching_checkpoints":matched,"generation":universe.generation().to_string(),"digest":outcome.final_digest}))
}
pub(crate) fn batch_path(directory:&Path,event:&Value)->Result<std::path::PathBuf,SimulationError> {
    let relative=event["path"].as_str().ok_or_else(||SimulationError::new("missing event path"))?;
    let path=Path::new(relative);
    let components=path.components().collect::<Vec<_>>();
    if components.len()!=2 || components.iter().any(|part|!matches!(part,std::path::Component::Normal(_)))
        || components[0].as_os_str()!="events" || path.extension().and_then(|e|e.to_str())!=Some("zst") {
        return Err(SimulationError::new("invalid material event archive path"));
    }
    Ok(directory.join(path))
}

pub fn analyze(directory:&Path)->Result<Value,SimulationError> {
    let outcome:RunOutcome=read_json(&directory.join("outcome.json"))?;
    let reader=BufReader::new(File::open(directory.join("history.ndjson")).map_err(io_error)?);
    let mut samples=Vec::new(); let mut counts=BTreeMap::<String,u64>::new();
    let mut last_switch=BTreeMap::<u64,(u64,u32)>::new();
    let initial=Universe::load_checkpoint(&directory.join("initial.gns"))?;
    if let Some(state)=initial.material_state() {for id in 1..=state.count() as u64 {
        if let Some(p)=state.particle(id){last_switch.insert(id,(initial.generation(),p.conformation as u32));}
    }}
    let mut dwell=BTreeMap::<u64,u64>::new(); let mut transitions=BTreeMap::<String,u64>::new();
    let mut bond_births=BTreeMap::<(u64,u64),u64>::new(); let mut bond_lifetimes=BTreeMap::<u64,u64>::new();
    let mut complete=true; let mut final_tick=0;
    for line in reader.lines() {
        if final_tick==outcome.final_generation && !samples.is_empty(){break;}
        let row:RunRecord=serde_json::from_str(&line.map_err(io_error)?).map_err(json_error)?;final_tick=row.generation;
        for reference in &row.events {if reference["kind"]=="material_batch" {
            let batch=journal::load(&batch_path(directory,reference)?,reference["digest"].as_str().ok_or_else(||SimulationError::new("missing batch digest"))?)?;
            if batch.generation!=row.generation {return Err(SimulationError::new("batch generation mismatch"));}
            complete&=!batch.truncated;
            for event in batch.events {
                let key=format!("{:?}",event.kind).to_lowercase();
                let count=counts.entry(key.clone()).or_default();*count=count.checked_add(1).ok_or_else(||SimulationError::new("analysis counter overflow"))?;
                match event.kind {
                    super::state::EventKind::Switch=>{
                        if let Some((previous,state))=last_switch.insert(event.particles[0],(row.generation,event.after[0])) {
                            *dwell.entry(row.generation-previous).or_default()+=1;
                            *transitions.entry(format!("{state}->{}",event.after[0])).or_default()+=1;
                        }
                    }
                    super::state::EventKind::Bind=>{bond_births.insert((event.particles[0],event.particles[1]),row.generation);}
                    super::state::EventKind::Unbind=>{if let Some(born)=bond_births.remove(&(event.particles[0],event.particles[1])){*bond_lifetimes.entry(row.generation-born).or_default()+=1;}}
                    _=>{}
                }
            }
        }}
        if let Some(metrics)=row.metrics {samples.push(json!({"generation":row.generation.to_string(),"metrics":metrics}));}
    }
    Ok(json!({"semantics":initial.semantics(),"full_retained_provenance":complete,"final_generation":final_tick.to_string(),
        "events":counts,"conformation_dwell_completed_intervals":dwell,"conformation_transitions":transitions,
        "bond_completed_lifetimes":bond_lifetimes,"right_censored_live_bonds":bond_births.len(),"samples":samples,
        "interpretation":"persistence and reaction history alone do not establish memory function, heredity, learning or life"}))
}

pub fn compare(reference:&Universe,branch:&Universe)->Result<Value,SimulationError> {
    let a=reference.material_state().ok_or_else(||SimulationError::new("comparison requires a material reference"))?;
    let b=branch.material_state().ok_or_else(||SimulationError::new("comparison requires a material branch"))?;
    if a.space!=b.space || a.count()!=b.count() || reference.generation()!=branch.generation() {
        return Err(SimulationError::new("comparison requires equal geometry, material IDs and generation"));
    }
    let mut moved=0usize;let mut switched=0usize;let mut excitation_changed=0usize;let mut connectivity_changed=0usize;
    let mut orientation_changed=0usize;let mut changed_particles=0usize;let mut nonthermal_changed=0usize;
    let mut changed=Vec::new();
    for id in 1..=a.count() as u64 {
        let p=a.particle(id).ok_or_else(||SimulationError::new("missing reference particle"))?;
        let q=b.particle(id).ok_or_else(||SimulationError::new("missing branch particle"))?;
        moved+=usize::from(p.position!=q.position);switched+=usize::from(p.conformation!=q.conformation);
        excitation_changed+=usize::from(p.excitation!=q.excitation);connectivity_changed+=usize::from(p.bonds!=q.bonds);
        orientation_changed+=usize::from(p.orientation!=q.orientation);
        nonthermal_changed+=usize::from(p.position!=q.position || p.conformation!=q.conformation || p.bonds!=q.bonds || p.orientation!=q.orientation);
        if p.position!=q.position || p.conformation!=q.conformation || p.excitation!=q.excitation || p.bonds!=q.bonds || p.orientation!=q.orientation {
            changed_particles+=1;
            if changed.len()<1024{changed.push(id.to_string());}
        }
    }
    let mut heat_distance=0u128;let mut sites_changed=0usize;
    for position in 0..a.space.len() as u32 {let x=a.heat(position);let y=b.heat(position);heat_distance+=u128::from(x.abs_diff(y));sites_changed+=usize::from(x!=y);}
    Ok(json!({"generation":reference.generation().to_string(),"moved_particles":moved,"changed_conformations":switched,
        "changed_excitation":excitation_changed,"changed_connectivity":connectivity_changed,"changed_heat_sites":sites_changed,
        "changed_orientations":orientation_changed,"changed_particles":changed_particles,"nonthermal_changed_particles":nonthermal_changed,
        "heat_l1_distance":heat_distance.to_string(),"changed_particle_preview":changed,"preview_limit":1024,
        "reference_metrics":reference.metrics(),"branch_metrics":branch.metrics(),
        "interpretation":"causal state divergence is not by itself evidence of useful memory, adaptation or reproduction"}))
}

pub fn probe(initial:Universe,id:u64,conformation:u8,output:&Path,options:RunOptions)->Result<Value,SimulationError> {
    if !options.interventions.is_empty(){return Err(SimulationError::new("a local-state probe cannot combine scheduled law changes"));}
    let perturbed=initial.perturb_conformation(id,conformation)?;
    let initial_comparison=compare(&initial,&perturbed)?;
    make_directory(output)?;
    let reference_outcome=run(initial,&output.join("reference"),options.clone())?;
    let branch_outcome=run(perturbed,&output.join("perturbed"),options)?;
    let comparison=if reference_outcome.final_generation==branch_outcome.final_generation {
        Some(compare(&Universe::load_checkpoint(&output.join("reference/final.gns"))?,&Universe::load_checkpoint(&output.join("perturbed/final.gns"))?)?)
    }else{None};
    let result=json!({"completed":reference_outcome.completed && branch_outcome.completed,
        "intervention":{"particle":id.to_string(),"conformation":conformation,"energy_accounting":"local energy is transferred, not created"},
        "initial_difference":initial_comparison,"final_difference":comparison,"reference":reference_outcome,"perturbed":branch_outcome,
        "interpretation":"requires matched controls and functional outcomes before a memory or learning claim"});
    write_new_json(&output.join("probe.json"),&result)?;Ok(result)
}

pub fn suite(config:MatterConfig,seeds:&[u64],output:&Path,options:RunOptions)->Result<Value,SimulationError> {
    config.validate()?;
    if seeds.is_empty() || seeds.len()>128 || seeds.iter().collect::<BTreeSet<_>>().len()!=seeds.len() || !options.interventions.is_empty() {
        return Err(SimulationError::new("material controls need 1..128 distinct seeds and no scheduled interventions"));
    }
    make_directory(output)?;let mut runs=Vec::new();
    for &seed in seeds {for arm in ["driven","closed","no-catalysis","no-coupling","locked-conformations"] {
        let mut config=config.clone();config.seed=seed;
        match arm {
            "closed"=>config.reservoir.enabled=false,
            "no-catalysis"=>{for material in &mut config.materials {material.catalytic_strength=0;}},
            "no-coupling"=>{for material in &mut config.materials {material.coupling=0;}},
            "locked-conformations"=>{for material in &mut config.materials {material.switching_rate=0;material.coupling=0;}},
            _=>{},
        }
        let mut options=options.clone();options.label=format!("{arm}-{seed}");let path=output.join(&options.label);
        let outcome=run(Universe::from_spec(UniverseSpec::Matter(config))?,&path,options)?;
        write_new_json(&path.join("analysis.json"),&analyze(&path)?)?;
        runs.push(json!({"arm":arm,"seed":seed.to_string(),"outcome":outcome}));
    }}
    let completed=runs.iter().all(|r|r["outcome"]["completed"].as_bool()==Some(true));
    let report=json!({"completed":completed,"runs":runs,"claim":"material control experiments; no automatic emergence classification"});
    write_new_json(&output.join("suite.json"),&report)?;Ok(report)
}
