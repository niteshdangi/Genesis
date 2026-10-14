use crate::law_system::{LawSystem, SimulationError};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineArchive {
    pub generation: u64,
    pub state: serde_json::Value,
    pub workspace: serde_json::Value,
    pub baseline: serde_json::Value,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct BinaryParts {
    pub generation: u64,
    pub state: Vec<u8>,
    pub workspace: Vec<u8>,
    pub baseline: Vec<u8>,
}

pub struct Engine<L: LawSystem> {
    current: L::State,
    working: L::State,
    next: L::State,
    laws: L,
    workspace: L::Workspace,
    baseline: L::Baseline,
    observation: L::Observation,
    generation: u64,
}

impl<L: LawSystem> Engine<L> {
    pub fn new(mut initial: L::State, laws: L) -> Result<Self, SimulationError> {
        laws.prepare_state(&mut initial)?;
        let baseline = laws.baseline(&initial)?;
        laws.validate(&initial, &baseline)?;
        let observation = laws.observe(&initial)?;
        let workspace = laws.workspace(&initial);
        laws.validate_workspace(&initial, &workspace)?;
        Ok(Self {
            working: initial.clone(),
            next: initial.clone(),
            current: initial,
            laws,
            workspace,
            baseline,
            observation,
            generation: 0,
        })
    }

    pub fn world(&self) -> &L::State {
        &self.current
    }

    pub fn observation(&self) -> &L::Observation {
        &self.observation
    }
    pub fn laws(&self)->&L { &self.laws }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn fork_with(&self,laws:L) -> Result<Self,SimulationError> {
        laws.validate(&self.current,&self.baseline)?;
        laws.validate_workspace(&self.current,&self.workspace)?;
        let observation=laws.observe(&self.current)?;
        Ok(Self {current:self.current.clone(),working:self.current.clone(),next:self.current.clone(),laws,
            workspace:self.workspace.clone(),baseline:self.baseline.clone(),observation,generation:self.generation})
    }

    pub fn archive(&self) -> Result<EngineArchive, SimulationError> {
        let encode = |error: serde_json::Error| SimulationError::new(error.to_string());
        Ok(EngineArchive {
            generation: self.generation,
            state: serde_json::to_value(&self.current).map_err(encode)?,
            workspace: serde_json::to_value(&self.workspace).map_err(encode)?,
            baseline: serde_json::to_value(&self.baseline).map_err(encode)?,
        })
    }

    pub fn binary_parts_with(&self, state: &L::State) -> Result<BinaryParts, SimulationError> {
        let encode = |error: postcard::Error| SimulationError::new(error.to_string());
        Ok(BinaryParts { generation: self.generation,
            state: postcard::to_allocvec(state).map_err(encode)?,
            workspace: postcard::to_allocvec(&self.workspace).map_err(encode)?,
            baseline: postcard::to_allocvec(&self.baseline).map_err(encode)? })
    }

    pub fn restore_parts(laws: L, parts: BinaryParts, mut state: L::State) -> Result<Self, SimulationError> {
        laws.prepare_state(&mut state)?;
        let workspace = postcard::from_bytes(&parts.workspace).map_err(|e| SimulationError::new(e.to_string()))?;
        let baseline = postcard::from_bytes(&parts.baseline).map_err(|e| SimulationError::new(e.to_string()))?;
        laws.validate(&state, &baseline)?;
        laws.validate_workspace(&state, &workspace)?;
        let observation = laws.observe(&state)?;
        Ok(Self { working: state.clone(), next: state.clone(), current: state, laws,
            workspace, baseline, observation, generation: parts.generation })
    }

    pub fn restore(laws: L, archive: EngineArchive) -> Result<Self, SimulationError> {
        let decode = |error: serde_json::Error| SimulationError::new(error.to_string());
        let mut current: L::State = serde_json::from_value(archive.state).map_err(decode)?;
        laws.prepare_state(&mut current)?;
        let workspace = serde_json::from_value(archive.workspace).map_err(decode)?;
        let baseline = serde_json::from_value(archive.baseline).map_err(decode)?;
        laws.validate(&current, &baseline)?;
        laws.validate_workspace(&current, &workspace)?;
        let observation = laws.observe(&current)?;
        Ok(Self {
            working: current.clone(), next: current.clone(), current, laws,
            workspace, baseline, observation, generation: archive.generation,
        })
    }

    pub fn skip_quiescent(&mut self,steps:u64)->Result<bool,SimulationError> {
        if steps==0{return Err(SimulationError::new("quiescent skip requires positive steps"));}
        let generation=self.generation.checked_add(steps).ok_or_else(||SimulationError::new("quiescent skip generation overflow"))?;
        let mut current=self.current.clone();
        if !self.laws.prepare_quiescent_skip(&mut current)? {return Ok(false);}
        self.laws.validate(&current,&self.baseline)?;
        self.laws.validate_workspace(&current,&self.workspace)?;
        let observation=self.laws.observe(&current)?;
        self.working=current.clone();self.next=current.clone();self.current=current;
        self.observation=observation;self.generation=generation;Ok(true)
    }

    pub fn step(&mut self) -> Result<(), SimulationError> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| SimulationError::new("generation overflow"))?;
        self.working.clone_from(&self.current);
        let mut workspace = self.workspace.clone();
        for (stage, name) in self.laws.stages().iter().enumerate() {
            self.next.clone_from(&self.working);
            let context = |error: SimulationError| {
                SimulationError::new(format!("generation {generation}, {name}: {error}"))
            };
            self.laws
                .advance(
                    stage,
                    &self.working,
                    &mut self.next,
                    &mut workspace,
                    self.generation,
                )
                .map_err(context)?;
            self.laws
                .validate(&self.next, &self.baseline)
                .map_err(context)?;
            self.laws.validate_workspace(&self.next, &workspace).map_err(context)?;
            std::mem::swap(&mut self.working, &mut self.next);
        }
        let observation = self.laws.observe(&self.working).map_err(|error| SimulationError::new(format!("generation {generation}, observation: {error}")))?;
        std::mem::swap(&mut self.current, &mut self.working);
        self.workspace = workspace;
        self.observation = observation;
        self.generation = generation;
        Ok(())
    }
}
