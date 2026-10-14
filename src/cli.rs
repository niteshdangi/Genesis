use std::{collections::BTreeMap, path::{Path, PathBuf}};
use crate::{experiment::{self, RunOptions}, law_system::SimulationError,
    persistence::{json_error, read_json, write_new_json}, universe::{Universe, UniverseSpec}};

fn number<T: std::str::FromStr>(text: &str, name: &str) -> Result<T, SimulationError> {
    text.parse().map_err(|_| SimulationError::new(format!("invalid {name}: {text}")))
}

fn arguments(args: impl Iterator<Item = String>) -> Result<BTreeMap<String, String>, SimulationError> {
    let mut args = args;
    let mut options = BTreeMap::new();
    while let Some(key) = args.next() {
        if !key.starts_with("--") { return Err(SimulationError::new(format!("expected an option, got {key}"))); }
        let value = args.next().ok_or_else(|| SimulationError::new(format!("{key} needs a value")))?;
        if value.starts_with("--") || options.insert(key.clone(), value).is_some() { return Err(SimulationError::new(format!("missing or repeated option {key}"))); }
    }
    Ok(options)
}

fn required(options: &mut BTreeMap<String, String>, key: &str) -> Result<String, SimulationError> {
    options.remove(key).ok_or_else(|| SimulationError::new(format!("missing {key}")))
}

fn no_remaining(options: &BTreeMap<String, String>) -> Result<(), SimulationError> {
    if options.is_empty() { Ok(()) } else { Err(SimulationError::new(format!("unknown options: {}", options.keys().cloned().collect::<Vec<_>>().join(", ")))) }
}

fn run_options(options: &mut BTreeMap<String, String>) -> Result<RunOptions, SimulationError> {
    let mut run: RunOptions = if let Some(path) = options.remove("--options") { read_json(Path::new(&path))? } else { RunOptions::default() };
    if let Some(value) = options.remove("--steps") { run.steps = number(&value, "steps")?; }
    if let Some(value) = options.remove("--sample-every") { run.sample_every = number(&value, "sample interval")?; }
    if let Some(value) = options.remove("--checkpoint-every") { run.checkpoint_every = number(&value, "checkpoint interval")?; }
    if let Some(value) = options.remove("--revision") { run.revision = value; }
    if let Some(value) = options.remove("--label") { run.label = value; }
    if let Some(value) = options.remove("--archive-limit-mib") {run.archive_limit_mib=number(&value,"archive budget")?;}
    Ok(run)
}

fn universe(options: &mut BTreeMap<String, String>) -> Result<Universe, SimulationError> {
    if let Some(path) = options.remove("--resume") {
        if ["--spec", "--law", "--seed", "--scenario"].iter().any(|key| options.contains_key(*key)) {
            return Err(SimulationError::new("resume cannot be combined with an initial law, spec, seed or scenario"));
        }
        return Universe::load_checkpoint(Path::new(&path));
    }
    if let Some(scenario) = options.remove("--scenario") {
        if options.contains_key("--spec") || options.contains_key("--law") {
            return Err(SimulationError::new("a chemistry scenario supplies its own law and initial state"));
        }
        let seed = options.remove("--seed").map(|text| number(&text, "seed")).transpose()?.unwrap_or(1);
        return crate::scenarios::chemistry(&scenario, seed);
    }
    let mut spec = if let Some(path) = options.remove("--spec") {
        if options.contains_key("--law") { return Err(SimulationError::new("choose --spec or --law")); }
        read_json(Path::new(&path))?
    } else { UniverseSpec::named(&options.remove("--law").unwrap_or_else(|| "matter".into()))? };
    if let Some(text) = options.remove("--seed") {
        let seed = number(&text, "seed")?;
        match &mut spec {
            UniverseSpec::Chemistry(c) => c.seed = seed, UniverseSpec::Polymers(c) => c.seed = seed,
            UniverseSpec::Agents(c) => c.seed = seed,
            UniverseSpec::Matter(c) => c.seed = seed,
            UniverseSpec::Cellular(_) => return Err(SimulationError::new("cellular worlds do not use a random seed")),
        }
    }
    Universe::from_spec(spec)
}

pub fn entry() -> Result<(), SimulationError> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "serve".into());
    if command == "help" || command == "--help" || command == "-h" {
        println!("Genesis - one material universe, with a laboratory behind it.\n\
Start: genesis serve [--resume CHECKPOINT] [--port N] [--tick-ms N]\n\
The default world is Matter and starts paused. The browser offers Play/Pause, Step, Save, Restore and observations.\n\
Research: genesis lab run --out NEW_DIRECTORY [--spec FILE|--resume CHECKPOINT] [--seed N] [--steps N]\n\
Verify: genesis lab replay --run DIRECTORY\n\
Inspect: genesis lab analyze --run DIRECTORY --out NEW_JSON_FILE\n\
Use genesis lab help for controls, reproduction assays and historical comparison tools.\n\
Historical top-level commands remain compatible. Existing experiments and checkpoints are preserved.");
        return Ok(());
    }
    let command = if command == "lab" {
        let name = args.next().unwrap_or_else(|| "help".into());
        match name.as_str() {
            "help" | "--help" | "-h" => "help",
            "serve" => "serve",
            "run" => "run",
            "replay" => "replay",
            "analyze" => "analyze",
            "controls" => "matter-suite",
            "reproduction" => "matter-reproduction",
            "profile" => "matter-profile",
            "study" => "study",
            "catalog" => "catalog",
            "export-spec" => "export-spec",
            "fork" => "fork",
            "perturb" => "perturb",
            "probe" => "matter-probe",
            "memory" => "matter-memory",
            "response" => "matter-response",
            "cluster-memory" => "matter-cluster-memory",
            "cluster-prepare" => "matter-cluster-prepare",
            "link-audit" => "matter-link-audit",
            "geometry-audit" => "matter-geometry-audit",
            "product-audit" => "matter-product-audit",
            "compare" => "matter-compare",
            "skip" => "matter-skip",
            "polymer-controls" => "suite",
            _ => return Err(SimulationError::new(format!("unknown laboratory command: {name}; use genesis lab help"))),
        }.to_owned()
    } else { command };
    if command == "help" || command == "--help" || command == "-h" {
        println!("Genesis Laboratory - prefix every command below with genesis lab.\n\
Recorded-run workflow (Matter by default):\n\
  run --out NEW_DIRECTORY [--spec FILE|--resume FILE|--law NAME|--scenario NAME] [--seed N] [--steps N] [--sample-every N] [--checkpoint-every N] [--options FILE] [--revision TEXT]\n\
  replay --run DIRECTORY [--out NEW_JSON_FILE]\n\
  analyze --run DIRECTORY --out NEW_JSON_FILE\n\
Active research question: causal, repeatable reproduction under local material laws.\n\
  reproduction --spec FILE --protocol FILE --seeds LIST --out NEW_DIRECTORY\n\
  controls --out NEW_DIRECTORY --seeds LIST [--spec MATTER_SPEC] [--steps N] [--options FILE]\n\
Runtime diagnostics:\n\
  profile --checkpoint FILE --steps N --out NEW_DIRECTORY\n\
  study --plan FILE --out NEW_DIRECTORY\n\
  catalog --database FILE\n\
  export-spec --law NAME --out NEW_JSON_FILE\n\
  fork --checkpoint FILE --spec FILE --out NEW_CHECKPOINT_FILE\n\
  compare --reference CHECKPOINT --branch CHECKPOINT --out NEW_JSON_FILE\n\
  skip --checkpoint FILE --steps N --out NEW_CHECKPOINT (provably inactive worlds only)\n\
Historical and specialised diagnostics, not additional universe modes:\n\
  perturb --checkpoint FILE --particle ID --conformation 0|1 --out NEW_CHECKPOINT_FILE\n\
  probe --checkpoint FILE --particle ID --conformation 0|1 --out NEW_DIRECTORY [--steps N]\n\
  memory --out NEW_DIRECTORY --seeds LIST [--spec MATTER_SPEC] [--protocol FILE]\n\
  response --out NEW_DIRECTORY --seeds LIST [--spec MATTER_SPEC] [--protocol FILE] [--heats 12,64]\n\
  cluster-memory --out NEW_DIRECTORY --seeds LIST [--spec MATTER_SPEC] [--protocol FILE]\n\
  cluster-prepare --out NEW_DIRECTORY --seeds LIST [--spec MATTER_SPEC] [--protocol FILE]\n\
  link-audit --source CLUSTER_CAMPAIGN --out NEW_DIRECTORY [--replicas 32] [--mode lost|added]\n\
  geometry-audit --source CLUSTER_CAMPAIGN --out NEW_DIRECTORY [--replicas 32]\n\
  product-audit --source COMPLETED_CAMPAIGN --out NEW_DIRECTORY\n\
  polymer-controls --out NEW_DIRECTORY --seeds LIST [--spec POLYMER_SPEC] [--steps N] [--options FILE]\n\
  serve [--law matter|chemistry|polymers|agents|conway|cyclic|transport] [--spec FILE|--resume FILE] [--seed N] [--port N] [--tick-ms N]\n\
Chemistry scenarios: exchange, toroidal-group, overlapping-footprint, competing-groups.\n\
Archives are immutable. Experiments execute only when invoked. Historical top-level aliases remain supported.");
        return Ok(());
    }
    let mut options = arguments(args)?;
    match command.as_str() {
        "serve" => {
            let port: u16 = options.remove("--port").map(|v| number(&v, "port")).transpose()?.unwrap_or(8080);
            if port == 0 { return Err(SimulationError::new("port must be nonzero")); }
            let tick: u64 = options.remove("--tick-ms").map(|v| number(&v, "tick milliseconds")).transpose()?.unwrap_or(100);
            if tick == 0 || tick > 60_000 { return Err(SimulationError::new("tick must be 1 to 60000 milliseconds")); }
            let universe = universe(&mut options)?;
            no_remaining(&options)?;
            crate::server::serve(universe, port, tick)
        }
        "run" => {
            let output = PathBuf::from(required(&mut options, "--out")?);
            let run = run_options(&mut options)?;
            let universe = universe(&mut options)?;
            no_remaining(&options)?;
            let outcome = experiment::run(universe, &output, run)?;
            println!("{}", serde_json::to_string_pretty(&outcome).map_err(json_error)?);
            if !outcome.completed { return Err(SimulationError::new(outcome.error.unwrap_or_else(|| "run did not complete".into()))); }
            Ok(())
        }
        "suite" => {
            let output = PathBuf::from(required(&mut options, "--out")?);
            let seeds: Vec<u64> = required(&mut options, "--seeds")?.split(',').map(|s| number(s.trim(), "seed")).collect::<Result<_, _>>()?;
            let run = run_options(&mut options)?;
            let config = if let Some(path) = options.remove("--spec") {
                match read_json::<UniverseSpec>(Path::new(&path))? {
                    UniverseSpec::Polymers(config) => config,
                    _ => return Err(SimulationError::new("control suite requires a polymer world")),
                }
            } else { crate::polymers::PolymerConfig::default() };
            no_remaining(&options)?;
            let report = experiment::control_suite(config, &seeds, &output, run)?;
            println!("{}", serde_json::to_string_pretty(&report).map_err(json_error)?);
            if report["completed"].as_bool() != Some(true) { return Err(SimulationError::new("control runs stopped early; see suite.json")); }
            Ok(())
        }
        "study" => {
            let plan = PathBuf::from(required(&mut options, "--plan")?);
            let output = PathBuf::from(required(&mut options, "--out")?);
            no_remaining(&options)?;
            let report = experiment::study(read_json(&plan)?, &output)?;
            println!("{}", serde_json::to_string_pretty(&report).map_err(json_error)?);
            if report["completed"].as_bool() != Some(true) { return Err(SimulationError::new("study runs stopped early; see study.json")); }
            Ok(())
        }
        "matter-suite"=>{
            let output=PathBuf::from(required(&mut options,"--out")?);
            let seeds:Vec<u64>=required(&mut options,"--seeds")?.split(',').map(|s|number(s.trim(),"seed")).collect::<Result<_,_>>()?;
            let run=run_options(&mut options)?;
            let config=if let Some(path)=options.remove("--spec") {match read_json::<UniverseSpec>(Path::new(&path))? {
                UniverseSpec::Matter(config)=>config,_=>return Err(SimulationError::new("material suite requires a Matter specification")),
            }}else{crate::matter::MatterConfig::default()};
            no_remaining(&options)?;
            let report=crate::matter::lab::suite(config,&seeds,&output,run)?;
            println!("{}",serde_json::to_string_pretty(&report).map_err(json_error)?);
            if report["completed"].as_bool()!=Some(true){return Err(SimulationError::new("material controls stopped early; see suite.json"));}Ok(())
        }
        "catalog"=>{
            let database=PathBuf::from(required(&mut options,"--database")?);no_remaining(&options)?;
            if !database.is_file(){return Err(SimulationError::new("catalog file does not exist"));}
            let catalog=crate::storage::catalog::Catalog::open(&database)?;
            println!("{}",serde_json::to_string_pretty(&catalog.list()?).map_err(json_error)?);Ok(())
        }
        "perturb"=>{
            let input=PathBuf::from(required(&mut options,"--checkpoint")?);
            let output=PathBuf::from(required(&mut options,"--out")?);
            let id=number(&required(&mut options,"--particle")?,"particle ID")?;
            let conformation=number(&required(&mut options,"--conformation")?,"conformation")?;
            no_remaining(&options)?;
            Universe::load_checkpoint(&input)?.perturb_conformation(id,conformation)?.save_checkpoint(&output).map(|_|())
        }
        "matter-probe"=>{
            let checkpoint=PathBuf::from(required(&mut options,"--checkpoint")?);
            let output=PathBuf::from(required(&mut options,"--out")?);
            let id=number(&required(&mut options,"--particle")?,"particle ID")?;
            let conformation=number(&required(&mut options,"--conformation")?,"conformation")?;
            let run=run_options(&mut options)?;no_remaining(&options)?;
            let result=crate::matter::lab::probe(Universe::load_checkpoint(&checkpoint)?,id,conformation,&output,run)?;
            println!("{}",serde_json::to_string_pretty(&result).map_err(json_error)?);
            if result["completed"].as_bool()!=Some(true){return Err(SimulationError::new("material probe stopped early; see probe.json"));}Ok(())
        }
        "matter-profile"=>{
            let checkpoint=PathBuf::from(required(&mut options,"--checkpoint")?);
            let output=PathBuf::from(required(&mut options,"--out")?);
            let steps=number(&required(&mut options,"--steps")?,"profile steps")?;no_remaining(&options)?;
            let result=crate::matter::lab::profile(Universe::load_checkpoint(&checkpoint)?,&output,steps)?;
            println!("{}",serde_json::to_string_pretty(&result).map_err(json_error)?);
            if result["completed"]!=true{return Err(SimulationError::new("profile stopped; last valid state retained in final.gns"));}Ok(())
        }
        "matter-skip"=>{
            let checkpoint=PathBuf::from(required(&mut options,"--checkpoint")?);let output=PathBuf::from(required(&mut options,"--out")?);
            let steps=number(&required(&mut options,"--steps")?,"skip steps")?;no_remaining(&options)?;
            let mut universe=Universe::load_checkpoint(&checkpoint)?;let initial=universe.generation();let source_digest=universe.digest()?;
            let started=std::time::Instant::now();
            if !universe.skip_quiescent(steps)? {return Err(SimulationError::new("world is active; exact quiescent skip rejected"));}
            let elapsed=started.elapsed().as_micros().to_string();universe.save_checkpoint(&output)?;
            println!("{}",serde_json::to_string_pretty(&serde_json::json!({"method":"exact-quiescent-v1","initial_generation":initial.to_string(),
                "final_generation":universe.generation().to_string(),"skipped_steps":steps.to_string(),"source_digest":source_digest,
                "final_digest":universe.digest()?,"skip_microseconds":elapsed,"metrics":universe.metrics()})).map_err(json_error)?);Ok(())
        }
        "matter-geometry-audit"=>{
            let source=PathBuf::from(required(&mut options,"--source")?);let output=PathBuf::from(required(&mut options,"--out")?);
            let replicas=options.remove("--replicas").map(|n|number(&n,"replicas")).transpose()?.unwrap_or(32);
            no_remaining(&options)?;let report=crate::matter::cluster_memory::geometry_audit(&source,&output,replicas)?;
            println!("{}",serde_json::to_string_pretty(&report).map_err(json_error)?);Ok(())
        }
        "matter-link-audit"=>{
            let source=PathBuf::from(required(&mut options,"--source")?);let output=PathBuf::from(required(&mut options,"--out")?);
            let replicas=options.remove("--replicas").map(|n|number(&n,"replicas")).transpose()?.unwrap_or(32);
            let added=match options.remove("--mode").as_deref().unwrap_or("lost") {"lost"=>false,"added"=>true,
                _=>return Err(SimulationError::new("link audit mode must be lost or added"))};
            no_remaining(&options)?;let report=crate::matter::cluster_memory::link_audit_mode(&source,&output,replicas,added)?;
            println!("{}",serde_json::to_string_pretty(&report).map_err(json_error)?);Ok(())
        }
        "matter-product-audit"=>{
            let source=PathBuf::from(required(&mut options,"--source")?);
            let output=PathBuf::from(required(&mut options,"--out")?);no_remaining(&options)?;
            let result=crate::matter::reproduction::audit_product(&source,&output)?;
            println!("{}",serde_json::to_string_pretty(&result).map_err(json_error)?);Ok(())
        }
        "matter-reproduction"=>{
            let output=PathBuf::from(required(&mut options,"--out")?);
            let seeds:Vec<u64>=required(&mut options,"--seeds")?.split(',').map(|s|number(s.trim(),"seed")).collect::<Result<_,_>>()?;
            let config=match read_json::<UniverseSpec>(Path::new(&required(&mut options,"--spec")?))? {
                UniverseSpec::Matter(config)=>config,_=>return Err(SimulationError::new("reproduction requires a Matter specification")),
            };
            let protocol=read_json(Path::new(&required(&mut options,"--protocol")?))?;no_remaining(&options)?;
            let result=crate::matter::reproduction::run(config,&seeds,protocol,&output)?;
            println!("{}",serde_json::to_string_pretty(&result).map_err(json_error)?);Ok(())
        }
        "matter-cluster-memory"|"matter-cluster-prepare"=>{
            let output=PathBuf::from(required(&mut options,"--out")?);
            let seeds:Vec<u64>=required(&mut options,"--seeds")?.split(',').map(|s|number(s.trim(),"seed")).collect::<Result<_,_>>()?;
            let config=if let Some(path)=options.remove("--spec") {match read_json::<UniverseSpec>(Path::new(&path))? {
                UniverseSpec::Matter(config)=>config,_=>return Err(SimulationError::new("cluster memory requires a Matter specification")),
            }}else{crate::matter::MatterConfig::default()};
            let protocol=if let Some(path)=options.remove("--protocol") {read_json(Path::new(&path))?}
                else{crate::matter::cluster_memory::ClusterProtocol::default()};
            no_remaining(&options)?;
            let report=crate::matter::cluster_memory::run_mode(config,&seeds,protocol,&output,command=="matter-cluster-prepare")?;
            println!("{}",serde_json::to_string_pretty(&report).map_err(json_error)?);Ok(())
        }
        "matter-memory"|"matter-response"=>{
            let output=PathBuf::from(required(&mut options,"--out")?);
            let seeds:Vec<u64>=required(&mut options,"--seeds")?.split(',').map(|s|number(s.trim(),"seed")).collect::<Result<_,_>>()?;
            let config=if let Some(path)=options.remove("--spec") {match read_json::<UniverseSpec>(Path::new(&path))? {
                UniverseSpec::Matter(config)=>config,_=>return Err(SimulationError::new("memory experiment requires a Matter specification")),
            }}else{crate::matter::MatterConfig::default()};
            let protocol=if let Some(path)=options.remove("--protocol") {read_json(Path::new(&path))?}else{crate::matter::memory::MemoryProtocol::default()};
            let heat_levels:Vec<u32>=if command=="matter-response" {options.remove("--heats").unwrap_or_else(||"12,64".into()).split(',')
                .map(|s|number(s.trim(),"pulse heat")).collect::<Result<_,_>>()?}else{Vec::new()};
            no_remaining(&options)?;
            let report=if command=="matter-response" {crate::matter::memory::screen(config,&seeds,protocol,&heat_levels,&output)?}
                else{crate::matter::memory::run(config,&seeds,protocol,&output)?};
            println!("{}",serde_json::to_string_pretty(&report).map_err(json_error)?);Ok(())
        }
        "matter-compare"=>{
            let reference=PathBuf::from(required(&mut options,"--reference")?);
            let branch=PathBuf::from(required(&mut options,"--branch")?);
            let output=PathBuf::from(required(&mut options,"--out")?);no_remaining(&options)?;
            let result=crate::matter::lab::compare(&Universe::load_checkpoint(&reference)?,&Universe::load_checkpoint(&branch)?)?;
            write_new_json(&output,&result)
        }
        "replay" => {
            let directory = PathBuf::from(required(&mut options, "--run")?);
            let output = options.remove("--out").map(PathBuf::from);
            no_remaining(&options)?;
            let result = experiment::replay(&directory)?;
            if let Some(output) = output { write_new_json(&output, &result)?; }
            println!("{}", serde_json::to_string_pretty(&result).map_err(json_error)?);
            Ok(())
        }
        "analyze" => {
            let directory = PathBuf::from(required(&mut options, "--run")?);
            let output = PathBuf::from(required(&mut options, "--out")?);
            no_remaining(&options)?;
            write_new_json(&output, &experiment::analyze(&directory)?)
        }
        "export-spec" => {
            let law = required(&mut options, "--law")?;
            let output = PathBuf::from(required(&mut options, "--out")?);
            no_remaining(&options)?;
            write_new_json(&output, &UniverseSpec::named(&law)?)
        }
        "fork" => {
            let input = PathBuf::from(required(&mut options, "--checkpoint")?);
            let spec = PathBuf::from(required(&mut options, "--spec")?);
            let output = PathBuf::from(required(&mut options, "--out")?);
            no_remaining(&options)?;
            let universe = Universe::load_checkpoint(&input)?;
            universe.fork_with(read_json(&spec)?)?.save_checkpoint(&output).map(|_|())
        }
        _ => Err(SimulationError::new("unknown command; use genesis help")),
    }
}
