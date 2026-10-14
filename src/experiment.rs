use std::{collections::{BTreeMap, BTreeSet}, fs::{self, File, OpenOptions}, io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf}, time::{Instant, SystemTime, UNIX_EPOCH}};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use crate::{law_system::SimulationError, persistence::{Checkpoint, SEMANTICS, io_error, json_error, read_json, write_new_json},
    universe::{Universe, UniverseSpec}};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intervention { pub generation: u64, pub spec: UniverseSpec }

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RunOptions {
    pub steps: u64,
    pub checkpoint_every: u64,
    pub sample_every: u64,
    pub revision: String,
    pub label: String,
    pub interventions: Vec<Intervention>,
    pub archive_limit_mib: u64,
}

impl Default for RunOptions {
    fn default() -> Self { Self { steps: 10_000, checkpoint_every: 1_000, sample_every: 100,
        revision: option_env!("GENESIS_REVISION").unwrap_or("unrecorded").into(), label: "baseline".into(), interventions: Vec::new(),archive_limit_mib:4096 } }
}

impl RunOptions {
    pub fn validate(&self, initial: u64) -> Result<(), SimulationError> {
        let end = initial.checked_add(self.steps).ok_or_else(|| SimulationError::new("run generation overflow"))?;
        if self.sample_every == 0 || self.checkpoint_every == 0 || self.label.len() > 128 || self.revision.len() > 256 || self.archive_limit_mib<16 || self.archive_limit_mib>1_048_576 {
            return Err(SimulationError::new("invalid experiment intervals or metadata"));
        }
        let mut previous = None;
        for intervention in &self.interventions {
            intervention.spec.validate()?;
            if intervention.generation <= initial || intervention.generation > end || previous.is_some_and(|g| g >= intervention.generation) {
                return Err(SimulationError::new("interventions must be strictly ordered within the future run interval"));
            }
            previous = Some(intervention.generation);
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct RunManifest {
    pub schema: u32,
    pub package_version: String,
    pub semantics: String,
    pub created_unix_seconds: u64,
    pub os: String,
    pub architecture: String,
    pub initial_digest: String,
    pub initial_generation: u64,
    pub spec: UniverseSpec,
    pub options: RunOptions,
}

#[derive(Serialize, Deserialize)]
pub struct RunRecord {
    pub generation: u64,
    pub digest: Option<String>,
    pub metrics: Option<Value>,
    pub events: Vec<Value>,
}

#[derive(Serialize, Deserialize)]
pub struct RunOutcome {
    pub completed: bool,
    pub error: Option<String>,
    pub initial_generation: u64,
    pub final_generation: u64,
    pub elapsed_milliseconds: String,
    pub final_digest: String,
    pub final_metrics: Value,
    pub checkpoint_bytes: u64,
}

fn journal(writer: &mut BufWriter<File>, record: &RunRecord) -> Result<(), SimulationError> {
    serde_json::to_writer(&mut *writer, record).map_err(json_error)?;
    writer.write_all(b"\n").map_err(io_error)
}

pub fn run(mut universe: Universe, output: &Path, options: RunOptions) -> Result<RunOutcome, SimulationError> {
    if universe.id()=="matter" {return crate::matter::lab::run(universe,output,options);}
    let initial_generation = universe.generation();
    options.validate(initial_generation)?;
    for intervention in &options.interventions {
        if intervention.spec.id() != universe.id() { return Err(SimulationError::new("intervention changes universe family")); }
    }
    // A run owns a fresh directory, preserving all earlier evidence.
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) { fs::create_dir_all(parent).map_err(io_error)?; }
    fs::create_dir(output).map_err(io_error)?;
    let initial = universe.checkpoint()?;
    initial.save(&output.join("initial.json"))?;
    let manifest = RunManifest { schema: 1, package_version: env!("CARGO_PKG_VERSION").into(), semantics: SEMANTICS.into(),
        created_unix_seconds: SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| SimulationError::new(e.to_string()))?.as_secs(),
        os: std::env::consts::OS.into(), architecture: std::env::consts::ARCH.into(),
        initial_digest: initial.digest.clone(), initial_generation, spec: universe.spec().clone(), options: options.clone() };
    write_new_json(&output.join("manifest.json"), &manifest)?;
    let file = OpenOptions::new().write(true).create_new(true).open(output.join("history.ndjson")).map_err(io_error)?;
    let mut writer = BufWriter::new(file);
    journal(&mut writer, &RunRecord { generation: initial_generation, digest: Some(initial.digest), metrics: Some(universe.metrics()), events: Vec::new() })?;
    let started = Instant::now();
    let mut error = None;
    let mut intervention_index = 0;
    let target = initial_generation + options.steps;
    while universe.generation() < target {
        let next_generation = universe.generation() + 1;
        let mut events = Vec::new();
        let mut candidate = None;
        if let Some(intervention) = options.interventions.get(intervention_index).filter(|i| i.generation == next_generation) {
            match universe.fork_with(intervention.spec.clone()) {
                Ok(replacement) => {
                    candidate = Some(replacement);
                    events.push(json!({"kind":"intervention", "generation":next_generation.to_string(), "spec":intervention.spec}));
                    intervention_index += 1;
                }
                Err(failure) => { error = Some(failure.to_string()); break; }
            }
        }
        if let Err(failure) = candidate.as_mut().unwrap_or(&mut universe).step() { error = Some(failure.to_string()); break; }
        if let Some(replacement) = candidate { universe = replacement; }
        let generation = universe.generation();
        events.extend_from_slice(universe.events());
        let sampled = generation % options.sample_every == 0 || generation == target;
        let checkpointed = generation % options.checkpoint_every == 0;
        let checkpoint = if sampled || checkpointed { Some(universe.checkpoint()?) } else { None };
        journal(&mut writer, &RunRecord { generation, digest: checkpoint.as_ref().map(|c| c.digest.clone()),
            metrics: sampled.then(|| universe.metrics()), events })?;
        if checkpointed {
            checkpoint.as_ref().ok_or_else(|| SimulationError::new("missing scheduled checkpoint"))?
                .save(&output.join(format!("checkpoint-{generation:020}.json")))?;
            writer.flush().map_err(io_error)?;
            writer.get_ref().sync_all().map_err(io_error)?;
        }
    }
    writer.flush().map_err(io_error)?;
    writer.get_ref().sync_all().map_err(io_error)?;
    let final_checkpoint = universe.checkpoint()?;
    let final_path = output.join("final.json");
    final_checkpoint.save(&final_path)?;
    let outcome = RunOutcome { completed: error.is_none() && universe.generation() == target, error,
        initial_generation, final_generation: universe.generation(), elapsed_milliseconds: started.elapsed().as_millis().to_string(),
        final_digest: final_checkpoint.digest, final_metrics: universe.metrics(), checkpoint_bytes: fs::metadata(final_path).map_err(io_error)?.len() };
    write_new_json(&output.join("outcome.json"), &outcome)?;
    Ok(outcome)
}

/// Compare a recorded history with a deterministic rerun; invoked only by an explicit CLI command.
pub fn replay(directory: &Path) -> Result<Value, SimulationError> {
    if directory.join("initial.gns").exists() {return crate::matter::lab::replay(directory);}
    let manifest: RunManifest = read_json(&directory.join("manifest.json"))?;
    if manifest.schema != 1 || manifest.semantics != SEMANTICS { return Err(SimulationError::new("unsupported run format")); }
    let initial = Checkpoint::load(&directory.join("initial.json"))?;
    if initial.digest != manifest.initial_digest { return Err(SimulationError::new("manifest and initial checkpoint disagree")); }
    manifest.options.validate(initial.payload.engine.generation)?;
    let mut universe = Universe::restore(initial)?;
    let mut checkpoints = 0u64;
    let mut intervention_index = 0;
    let reader = BufReader::new(File::open(directory.join("history.ndjson")).map_err(io_error)?);
    let mut previous = None;
    for line in reader.lines() {
        let line = line.map_err(io_error)?;
        let record: RunRecord = serde_json::from_str(&line).map_err(json_error)?;
        if previous.is_some_and(|generation| record.generation != generation + 1)
            || (previous.is_none() && record.generation != universe.generation()) {
            return Err(SimulationError::new("journal is not a contiguous generation history"));
        }
        previous = Some(record.generation);
        if universe.generation() < record.generation {
            let generation = universe.generation() + 1;
            if let Some(intervention) = manifest.options.interventions.get(intervention_index).filter(|i| i.generation == generation) {
                universe = universe.fork_with(intervention.spec.clone())?;
                intervention_index += 1;
            }
            universe.step()?;
        }
        if let Some(expected) = record.digest {
            let actual = universe.checkpoint()?.digest;
            if actual != expected { return Err(SimulationError::new(format!("replay digest diverged at generation {}", record.generation))); }
            add_count(&mut checkpoints)?;
        }
    }
    let outcome: RunOutcome = read_json(&directory.join("outcome.json"))?;
    if universe.generation() != outcome.final_generation || universe.checkpoint()?.digest != outcome.final_digest {
        return Err(SimulationError::new("replayed final state differs from recorded outcome"));
    }
    Ok(json!({"matching_checkpoints":checkpoints, "generation":universe.generation().to_string(), "digest":outcome.final_digest}))
}

fn add_count(value: &mut u64) -> Result<(), SimulationError> { crate::spatial::add(value, 1) }

#[derive(Default, Serialize)]
pub struct LineageSummary {
    pub sequence: String,
    pub parent: Option<String>,
    pub released: u64,
    pub died: Option<u64>,
    pub descendants: u64,
    pub depth: u64,
    pub matches: u64,
    pub residues: u64,
}

pub fn analyze(directory: &Path) -> Result<Value, SimulationError> {
    if directory.join("initial.gns").exists() {return crate::matter::lab::analyze(directory);}
    let reader = BufReader::new(File::open(directory.join("history.ndjson")).map_err(io_error)?);
    let mut sequences = BTreeSet::new();
    let mut birth_counts = BTreeMap::<String, u64>::new();
    let mut lineages = BTreeMap::<String, LineageSummary>::new();
    let initial = Checkpoint::load(&directory.join("initial.json"))?;
    if let Some(chains) = initial.payload.engine.state["chains"].as_object() {
        for (id, chain) in chains {
            if !chain["attached_to"].is_null() { continue; }
            let sequence: String = chain["residues"].as_array().into_iter().flatten()
                .filter_map(|m| m["kind"].as_u64().map(|kind| char::from(b'0' + kind as u8))).collect();
            sequences.insert(sequence.clone());
            lineages.insert(id.clone(), LineageSummary { sequence, parent: chain["parent"].as_u64().map(|id| id.to_string()),
                released: initial.payload.engine.generation, depth: chain["depth"].as_u64().unwrap_or(0), ..LineageSummary::default() });
        }
    }
    let mut samples = Vec::new();
    let mut deaths = BTreeMap::new();
    let mut innovations_since_sample = 0u64;
    for line in reader.lines() {
        let record: RunRecord = serde_json::from_str(&line.map_err(io_error)?).map_err(json_error)?;
        for event in &record.events {
            match event["kind"].as_str() {
                Some("release") | Some("nucleation") => {
                    let Some(id) = event["chain"].as_str() else { continue; };
                    let sequence = event["sequence"].as_str().unwrap_or_default().to_owned();
                    if sequences.insert(sequence.clone()) { add_count(&mut innovations_since_sample)?; }
                    add_count(birth_counts.entry(sequence.clone()).or_default())?;
                    lineages.insert(id.to_owned(), LineageSummary { sequence, parent: event["parent"].as_str().map(str::to_owned),
                        released: record.generation, died: None, descendants: 0,
                        depth: event["depth"].as_str().and_then(|x| x.parse().ok()).unwrap_or(0),
                        matches: event["matching_residues"].as_u64().unwrap_or(0), residues: event["residues"].as_u64().unwrap_or(0) });
                }
                Some("decay") => { if let Some(id) = event["chain"].as_str() { deaths.insert(id.to_owned(), record.generation); } }
                _ => {}
            }
        }
        if let Some(metrics) = record.metrics {
            samples.push(json!({"generation":record.generation.to_string(), "metrics":metrics,
                "cumulative_distinct_sequences":sequences.len(), "new_sequences_since_sample":innovations_since_sample}));
            innovations_since_sample = 0;
        }
    }
    let parents: Vec<_> = lineages.values().filter_map(|l| l.parent.clone()).collect();
    for parent in parents { if let Some(lineage) = lineages.get_mut(&parent) { add_count(&mut lineage.descendants)?; } }
    for (id, generation) in deaths { if let Some(lineage) = lineages.get_mut(&id) { lineage.died = Some(generation); } }
    Ok(json!({"interpretation":"sequence novelty is structural; functional adaptation and open-endedness require controls",
        "distinct_released_sequences":sequences.len(), "releases_by_sequence":birth_counts, "lineages":lineages, "samples":samples}))
}

pub fn control_suite(config: crate::polymers::PolymerConfig, seeds: &[u64], parent: &Path, options: RunOptions) -> Result<Value, SimulationError> {
    config.validate()?;
    if seeds.is_empty() || seeds.len() > 128 || seeds.iter().collect::<BTreeSet<_>>().len() != seeds.len() {
        return Err(SimulationError::new("supply 1 to 128 distinct experiment seeds"));
    }
    if !options.interventions.is_empty() { return Err(SimulationError::new("control suites require an unchanged law during each arm")); }
    if let Some(directory) = parent.parent().filter(|p| !p.as_os_str().is_empty()) { fs::create_dir_all(directory).map_err(io_error)?; }
    fs::create_dir(parent).map_err(io_error)?;
    let mut outcomes = Vec::new();
    for &seed in seeds {
        for arm in ["template", "copying-disabled", "template-absent", "neutral-sequence", "closed-energy"] {
            let mut config = config.clone();
            config.seed = seed;
            config.copying_enabled = arm != "copying-disabled";
            config.seeded_templates = arm != "template-absent";
            config.neutral_copying = arm == "neutral-sequence";
            if arm == "closed-energy" { config.energy_input = 0; }
            let mut options = options.clone();
            options.label = format!("{arm}-{seed}");
            let path: PathBuf = parent.join(&options.label);
            let outcome = run(Universe::from_spec(UniverseSpec::Polymers(config))?, &path, options)?;
            write_new_json(&path.join("analysis.json"), &analyze(&path)?)?;
            outcomes.push(json!({"arm":arm,"seed":seed.to_string(),"outcome":outcome}));
        }
    }
    let completed = outcomes.iter().all(|outcome| outcome["outcome"]["completed"].as_bool() == Some(true));
    let report = json!({"completed":completed,"arms":outcomes,"claim":"controlled observations only; no automatic emergence verdict"});
    write_new_json(&parent.join("suite.json"), &report)?;
    Ok(report)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyRun {
    pub name: String,
    pub spec: UniverseSpec,
    #[serde(default)]
    pub options: RunOptions,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyPlan { pub runs: Vec<StudyRun> }

pub fn study(plan: StudyPlan, output: &Path) -> Result<Value, SimulationError> {
    if plan.runs.is_empty() || plan.runs.len() > 1_024 { return Err(SimulationError::new("study needs 1 to 1024 runs")); }
    let mut names = BTreeSet::new();
    for entry in &plan.runs {
        if entry.name.is_empty() || entry.name.len() > 80 || !entry.name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !names.insert(entry.name.clone()) { return Err(SimulationError::new("study run names must be unique ASCII identifiers")); }
        entry.spec.validate()?;
        entry.options.validate(0)?;
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) { fs::create_dir_all(parent).map_err(io_error)?; }
    fs::create_dir(output).map_err(io_error)?;
    write_new_json(&output.join("plan.json"), &plan)?;
    let mut reports = Vec::new();
    for entry in plan.runs {
        let path = output.join(&entry.name);
        let universe = Universe::from_spec(entry.spec.clone())?;
        let outcome = run(universe, &path, entry.options)?;
        write_new_json(&path.join("analysis.json"), &analyze(&path)?)?;
        reports.push(json!({"name":entry.name,"spec":entry.spec,"outcome":outcome}));
    }
    let completed = reports.iter().all(|report| report["outcome"]["completed"].as_bool() == Some(true));
    let report = json!({"completed":completed,"runs":reports,"interpretation":"compare costs and outcomes across recorded configurations; no automatic open-endedness claim"});
    write_new_json(&output.join("study.json"), &report)?;
    Ok(report)
}
