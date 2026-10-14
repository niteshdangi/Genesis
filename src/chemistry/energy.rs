use crate::law_system::SimulationError;

use super::{ChemistryConfig, ChemistryState, state::cardinal};

pub(super) fn propagate(
    current: &ChemistryState,
    next: &mut ChemistryState,
    config: &ChemistryConfig,
) -> Result<(), SimulationError> {
    let mut outgoing = vec![0u64; current.energy.len()];
    let mut incoming = vec![0u128; current.energy.len()];
    for (source, &energy) in current.energy.iter().enumerate() {
        let mut budget = energy;
        for target in cardinal(source) {
            if energy > current.energy[target]
                && energy - current.energy[target] >= config.energy_difference
            {
                let transfer = config.energy_transfer.min(budget);
                budget -= transfer;
                outgoing[source] += transfer;
                incoming[target] += u128::from(transfer);
            }
        }
    }
    for position in 0..current.energy.len() {
        let energy = u128::from(current.energy[position] - outgoing[position]) + incoming[position];
        next.energy[position] = u64::try_from(energy)
            .map_err(|_| SimulationError::new("free energy overflow during propagation"))?;
    }
    Ok(())
}
