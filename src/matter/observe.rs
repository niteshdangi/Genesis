use std::collections::BTreeMap;
use serde::{Deserialize,Serialize};
use crate::law_system::SimulationError;
use super::{MatterConfig,MatterState};

#[derive(Clone,Default,Serialize,Deserialize)]
pub struct MatterObservation {
    pub bonds: usize, pub components: usize, pub largest_component: usize,
    pub sizes: BTreeMap<usize,usize>, pub active_conformations: usize,
    #[serde(with="crate::persistence::decimal_u128")]
    pub heat: u128,
    #[serde(with="crate::persistence::decimal_u128")]
    pub excitation: u128,
    #[serde(with="crate::persistence::decimal_u128")]
    pub structural_energy: u128,
    #[serde(with="crate::persistence::decimal_u128")]
    pub total_energy: u128,
}
pub fn measure(state:&MatterState,config:&MatterConfig) -> Result<MatterObservation,SimulationError> {
    let [heat,excitation,structural_energy]=state.energies(config)?;
    let mut observation=MatterObservation { heat,excitation,structural_energy,total_energy:heat+excitation+structural_energy,..Default::default() };
    let mut visited=vec![false;state.count()];
    for id in 1..=state.count() as u64 {
        let p=state.particle(id).ok_or_else(||SimulationError::new("missing observed particle"))?;
        observation.active_conformations+=usize::from(p.conformation==1);
        observation.bonds+=p.bonds.iter().filter(|&&other|other>id).count();
        if visited[id as usize-1] { continue; }
        let mut pending=vec![id]; let mut size=0;
        while let Some(member)=pending.pop() {
            if visited[member as usize-1] { continue; }
            visited[member as usize-1]=true; size+=1;
            let member=state.particle(member).ok_or_else(||SimulationError::new("missing component member"))?;
            pending.extend(member.bonds.into_iter().filter(|&other|other!=0));
        }
        observation.components+=1; observation.largest_component=observation.largest_component.max(size);
        *observation.sizes.entry(size).or_default()+=1;
    }
    Ok(observation)
}
