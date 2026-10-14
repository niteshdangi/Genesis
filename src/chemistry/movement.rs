use std::collections::{BTreeMap, BTreeSet};

use crate::{
    law_system::SimulationError,
    world::{Cell, WIDTH},
};

use super::{ChemistryConfig, ChemistryState, seeded_value, state::cardinal};

pub(super) fn translate(
    current: &ChemistryState,
    next: &mut ChemistryState,
    config: &ChemistryConfig,
    generation: u64,
) -> Result<(), SimulationError> {
    let indices: BTreeMap<_, _> = current
        .particles
        .iter()
        .enumerate()
        .map(|(index, particle)| (particle.id, index))
        .collect();
    let mut reserved = vec![false; current.occupancy.len()];
    let mut accepted = Vec::new();
    for group in current.groups()? {
        let members: BTreeSet<_> = group.iter().copied().collect();
        let energy: u128 = group
            .iter()
            .map(|id| u128::from(current.energy[current.particles[indices[id]].position]))
            .sum();
        if energy < u128::from(config.movement_energy) {
            continue;
        }
        let mut directions = [0usize, 1, 2, 3];
        directions.sort_by_key(|&direction| {
            (
                seeded_value(
                    config.seed,
                    generation,
                    1,
                    u64::from(group[0]),
                    direction as u64,
                ),
                direction,
            )
        });
        let targets = directions.iter().find_map(|&direction| {
            let targets: Vec<_> = group
                .iter()
                .map(|id| {
                    let index = indices[id];
                    let target = cardinal(current.particles[index].position)[direction];
                    (index, target)
                })
                .collect();
            targets
                .iter()
                .all(|&(_, target)| {
                    current.occupancy[target].is_none_or(|id| members.contains(&id))
                })
                .then_some(targets)
        });
        if let Some(targets) = targets
            && targets.iter().all(|&(_, target)| !reserved[target])
        {
            for &(_, target) in &targets {
                reserved[target] = true;
            }
            accepted.push(targets);
        }
    }
    // Clear all accepted footprints before writing, including self-overlapping translations.
    for targets in &accepted {
        for &(index, _) in targets {
            let source = current.particles[index].position;
            next.occupancy[source] = None;
            next.grid.set(source % WIDTH, source / WIDTH, Cell::Empty);
        }
    }
    for targets in accepted {
        for (index, target) in targets {
            let particle = &mut next.particles[index];
            particle.position = target;
            next.occupancy[target] = Some(particle.id);
            next.grid.set(target % WIDTH, target / WIDTH, particle.kind);
        }
    }
    Ok(())
}
