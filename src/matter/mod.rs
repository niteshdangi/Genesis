mod config;
mod state;
mod transitions;
mod observe;
pub mod lab;
pub mod memory;
pub mod cluster_memory;
pub mod reproduction;

pub use config::{MatterConfig, Material, Space3, Execution, Reservoir};
pub(crate) use config::LegacyMatterConfig;
pub use state::{MatterState, MatterBaseline, MatterEvent, EventKind, Chunk, ParticlePage, Particle};
pub use observe::MatterObservation;

use crate::{law_system::{LawSystem, SimulationError}, engine::Engine};
use serde_json::{Value, json};

pub const SEMANTICS: &str = "genesis-local-matter-3d-v1";
pub const CONTACT_SEMANTICS: &str = "genesis-local-matter-3d-v2";
pub const VARIATION_SEMANTICS: &str = "genesis-local-matter-3d-v3";

pub struct MatterLaw {
    pub(crate) config: MatterConfig,
    pub(crate) gpu: Option<crate::backend::GpuFields>,
}

impl MatterLaw {
    pub fn new(config: MatterConfig) -> Result<Self, SimulationError> {
        config.validate()?;
        let gpu = if config.execution.gpu_fields {
            match crate::backend::GpuFields::new() {
                Ok(gpu) => Some(gpu),
                Err(_) if config.execution.cpu_fallback => None,
                Err(error) => return Err(error),
            }
        } else { None };
        Ok(Self { config, gpu })
    }
    pub fn initial(&self) -> Result<MatterState, SimulationError> { MatterState::initial(&self.config) }
    pub fn config(&self) -> &MatterConfig { &self.config }
    pub fn backend(&self) -> &'static str {
        if self.gpu.is_some() { "wgpu-fields-cpu-interactions" }
        else if self.config.execution.parallel { "rayon-proposals" } else { "cpu-reference" }
    }
    pub fn metrics(&self, state: &MatterState, observation: &MatterObservation) -> Value {
        json!({"particles":state.count(),"bonds":observation.bonds,"components":observation.components,
            "largest_component":observation.largest_component,"size_distribution":observation.sizes,
            "active_conformations":observation.active_conformations,"heat":observation.heat.to_string(),
            "excitation":observation.excitation.to_string(),"structural_energy":observation.structural_energy.to_string(),
            "total_energy":observation.total_energy.to_string(),"energy_input":state.energy_input.to_string(),
            "energy_output":state.energy_output.to_string(),"retained_events":state.events.len(),
            "events_truncated":state.events_truncated,"chunks":state.chunks.len(),"backend":self.backend(),
            "semantics":self.config.semantics(),"law_version":self.config.law_version,
            "interpretation":"components are material structures; no organism or cognition classification"})
    }
}

impl LawSystem for MatterLaw {
    type State = MatterState;
    type Workspace = ();
    type Baseline = MatterBaseline;
    type Observation = MatterObservation;
    fn prepare_state(&self,state:&mut MatterState)->Result<(),SimulationError> {
        state.validate_structure(&self.config)?;state.rebuild_field_index();state.refresh_events()?;Ok(())
    }
    fn prepare_quiescent_skip(&self,state:&mut MatterState)->Result<bool,SimulationError> {
        // Every reaction/motion needs excitation; charging needs heat > 1 and diffusion a gradient >= 2.
        if self.config.reservoir.enabled || state.pages.iter().any(|p|p.excitation.iter().any(|&n|n!=0))
            || state.chunks.values().any(|c|c.heat.iter().any(|&n|n>1)) {return Ok(false);}
        state.events.clear();state.events_json.clear();state.events_truncated=false;
        state.heat_active=Default::default();state.heat_dirty=Default::default();Ok(true)
    }
    fn stages(&self) -> &'static [&'static str] { &["reservoirs", "heat transport", "local motion", "material interactions"] }
    fn workspace(&self, _: &Self::State) {}
    fn baseline(&self, state: &Self::State) -> Result<MatterBaseline, SimulationError> {
        state.validate_structure(&self.config)?;
        Ok(MatterBaseline { materials: state.material_counts(), energy: state.total_energy(&self.config)? })
    }
    fn advance(&self, stage: usize, current: &Self::State, next: &mut Self::State, _: &mut (), generation: u64) -> Result<(), SimulationError> {
        let tick = generation.checked_add(1).ok_or_else(|| SimulationError::new("matter tick overflow"))?;
        match stage {
            0 => transitions::reservoirs(self, current, next, tick),
            1 => transitions::heat(self, current, next, tick),
            2 => transitions::motion(self, current, next, tick),
            3 => transitions::interact(self, current, next, tick),
            _ => Err(SimulationError::new("unknown matter stage")),
        }
    }
    fn validate(&self, state: &Self::State, baseline: &MatterBaseline) -> Result<(), SimulationError> {
        state.validate_structure(&self.config)?;
        if state.material_counts() != baseline.materials { return Err(SimulationError::new("elemental material conservation failed")); }
        let accounted = state.total_energy(&self.config)?.checked_add(state.energy_output)
            .ok_or_else(|| SimulationError::new("matter energy accounting overflow"))?;
        if Some(accounted) != baseline.energy.checked_add(state.energy_input) { return Err(SimulationError::new("matter energy ledger mismatch")); }
        Ok(())
    }
    fn observe(&self, state: &Self::State) -> Result<MatterObservation, SimulationError> { observe::measure(state, &self.config) }
}

pub fn digest(engine: &Engine<MatterLaw>, config: &MatterConfig) -> Result<String, SimulationError> {
    use sha2::{Digest, Sha256};
    let mut state = engine.world().clone();
    state.events.clear(); state.events_json.clear(); state.events_truncated = false;
    state.chunks.retain(|_, chunk| !chunk.empty());
    let parts = engine.binary_parts_with(&state)?;
    let mut logical = config.clone(); logical.execution = Execution::default();
    let bytes = if logical.law_version == 1 {
        postcard::to_allocvec(&(SEMANTICS, logical.legacy(), parts))
    } else {
        postcard::to_allocvec(&(logical.semantics(), logical, parts))
    }.map_err(|e| SimulationError::new(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
