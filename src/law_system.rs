use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationError(String);

impl SimulationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for SimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SimulationError {}

/// Owns universe-specific state and deterministic transitions.
/// Stage hooks may mutate only candidate state and generation-local workspace.
pub trait LawSystem {
    /// Clones must be independent snapshots, not shared mutable storage.
    type State: Clone + serde::Serialize + serde::de::DeserializeOwned;
    /// Clones must not share mutable generation-local work.
    type Workspace: Clone + serde::Serialize + serde::de::DeserializeOwned;
    type Baseline: Clone + serde::Serialize + serde::de::DeserializeOwned;
    type Observation;
    fn prepare_state(&self,_:&mut Self::State)->Result<(),SimulationError> {Ok(())}
    /// Opt in only when all future autonomous stages leave physical state unchanged.
    /// Clear generation-local traces on success. No scheduled external input may be skipped.
    fn prepare_quiescent_skip(&self,_:&mut Self::State)->Result<bool,SimulationError> {Ok(false)}

    fn stages(&self) -> &'static [&'static str];
    fn workspace(&self, initial: &Self::State) -> Self::Workspace;
    fn validate_workspace(&self, _: &Self::State, _: &Self::Workspace) -> Result<(), SimulationError> { Ok(()) }
    fn baseline(&self, initial: &Self::State) -> Result<Self::Baseline, SimulationError>;
    /// `next` starts as an independent copy of `current`.
    fn advance(
        &self,
        stage: usize,
        current: &Self::State,
        next: &mut Self::State,
        workspace: &mut Self::Workspace,
        generation: u64,
    ) -> Result<(), SimulationError>;
    fn validate(
        &self,
        state: &Self::State,
        baseline: &Self::Baseline,
    ) -> Result<(), SimulationError>;
    fn observe(&self, state: &Self::State) -> Result<Self::Observation, SimulationError>;
}
