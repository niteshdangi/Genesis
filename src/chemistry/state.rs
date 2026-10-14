use std::collections::{BTreeMap, BTreeSet};

use crate::{
    law_system::SimulationError,
    world::{Cell, HEIGHT, WIDTH, World},
};

use super::BOND_ENERGY;

pub type ParticleId = u32;
pub type BondKey = (ParticleId, ParticleId);

mod bond_map {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(value: &BTreeMap<BondKey, Bond>, serializer: S) -> Result<S::Ok, S::Error> {
        value.values().collect::<Vec<_>>().serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<BondKey, Bond>, D::Error> {
        let values = Vec::<Bond>::deserialize(deserializer)?;
        let mut bonds = BTreeMap::new();
        for bond in values {
            let key = (bond.a, bond.b);
            if bonds.insert(key, bond).is_some() {
                return Err(serde::de::Error::custom("duplicate checkpoint bond"));
            }
        }
        Ok(bonds)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Particle {
    pub id: ParticleId,
    pub kind: Cell,
    pub position: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bond {
    pub a: ParticleId,
    pub b: ParticleId,
    pub energy: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReactionCounts {
    pub formation: u64,
    pub dissociation: u64,
    pub exchange: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChemistryState {
    pub(super) grid: World,
    pub(super) energy: Vec<u64>,
    pub(super) particles: Vec<Particle>,
    pub(super) occupancy: Vec<Option<ParticleId>>,
    #[serde(with = "bond_map")]
    pub(super) bonds: BTreeMap<BondKey, Bond>,
    pub(super) reactions: ReactionCounts,
    #[serde(default)]
    pub(super) events: Vec<serde_json::Value>,
}

impl ChemistryState {
    pub fn new(grid: World, energy: Vec<u64>) -> Result<Self, SimulationError> {
        if energy.len() != WIDTH * HEIGHT {
            return Err(SimulationError::new("energy grid must contain 16384 cells"));
        }
        let mut particles = Vec::new();
        let mut occupancy = vec![None; WIDTH * HEIGHT];
        for (position, &kind) in grid.cells().iter().enumerate() {
            if kind != 0 {
                let id = (particles.len() + 1) as ParticleId;
                particles.push(Particle {
                    id,
                    kind: grid.get(position % WIDTH, position / WIDTH),
                    position,
                });
                occupancy[position] = Some(id);
            }
        }
        Ok(Self {
            grid,
            energy,
            particles,
            occupancy,
            bonds: BTreeMap::new(),
            reactions: ReactionCounts::default(),
            events: Vec::new(),
        })
    }

    pub fn grid(&self) -> &World {
        &self.grid
    }

    pub fn energy(&self) -> &[u64] {
        &self.energy
    }

    pub fn events(&self) -> &[serde_json::Value] { &self.events }

    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    pub fn bonds(&self) -> impl Iterator<Item = &Bond> {
        self.bonds.values()
    }

    pub fn particle(&self, id: ParticleId) -> Option<&Particle> {
        self.particles
            .binary_search_by_key(&id, |particle| particle.id)
            .ok()
            .map(|index| &self.particles[index])
    }

    pub fn free_energy(&self) -> u128 {
        self.energy.iter().map(|&energy| u128::from(energy)).sum()
    }

    pub fn bond_energy(&self) -> u128 {
        self.bonds
            .values()
            .map(|bond| u128::from(bond.energy))
            .sum()
    }

    pub fn total_energy(&self) -> u128 {
        self.free_energy() + self.bond_energy()
    }

    pub fn add_initial_bond(
        &mut self,
        a: ParticleId,
        b: ParticleId,
    ) -> Result<(), SimulationError> {
        let key = bond_key(a, b);
        let first = self
            .particle(key.0)
            .ok_or_else(|| SimulationError::new("unknown bond endpoint"))?;
        let second = self
            .particle(key.1)
            .ok_or_else(|| SimulationError::new("unknown bond endpoint"))?;
        let positions = [first.position, second.position];
        if a == b || self.bonds.contains_key(&key) || !adjacent(positions[0], positions[1]) {
            return Err(SimulationError::new(
                "initial bond must be unique and orthogonally adjacent",
            ));
        }
        if positions
            .iter()
            .map(|&position| u128::from(self.energy[position]))
            .sum::<u128>()
            < u128::from(BOND_ENERGY)
        {
            return Err(SimulationError::new(
                "initial bond requires two free energy units",
            ));
        }
        let first_debit = self.energy[positions[0]].min(BOND_ENERGY);
        self.energy[positions[0]] -= first_debit;
        self.energy[positions[1]] -= BOND_ENERGY - first_debit;
        self.bonds.insert(
            key,
            Bond {
                a: key.0,
                b: key.1,
                energy: BOND_ENERGY,
            },
        );
        Ok(())
    }

    pub fn groups(&self) -> Result<Vec<Vec<ParticleId>>, SimulationError> {
        let mut adjacency: BTreeMap<_, Vec<_>> = self
            .particles
            .iter()
            .map(|particle| (particle.id, Vec::new()))
            .collect();
        for bond in self.bonds.values() {
            adjacency
                .get_mut(&bond.a)
                .ok_or_else(|| SimulationError::new("bond references unknown particle"))?
                .push(bond.b);
            adjacency
                .get_mut(&bond.b)
                .ok_or_else(|| SimulationError::new("bond references unknown particle"))?
                .push(bond.a);
        }
        let mut visited = BTreeSet::new();
        let mut groups = Vec::new();
        for &id in adjacency.keys() {
            if visited.contains(&id) {
                continue;
            }
            let mut pending = vec![id];
            let mut group = Vec::new();
            while let Some(member) = pending.pop() {
                if visited.insert(member) {
                    group.push(member);
                    pending.extend(&adjacency[&member]);
                }
            }
            group.sort_unstable();
            groups.push(group);
        }
        Ok(groups)
    }
}

pub(super) fn bond_key(a: ParticleId, b: ParticleId) -> BondKey {
    (a.min(b), a.max(b))
}

pub(super) fn cardinal(position: usize) -> [usize; 4] {
    let x = position % WIDTH;
    let y = position / WIDTH;
    [
        y * WIDTH + (x + WIDTH - 1) % WIDTH,
        y * WIDTH + (x + 1) % WIDTH,
        ((y + HEIGHT - 1) % HEIGHT) * WIDTH + x,
        ((y + 1) % HEIGHT) * WIDTH + x,
    ]
}

pub(super) fn adjacent(a: usize, b: usize) -> bool {
    a != b && cardinal(a).contains(&b)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChemistryObservation {
    pub valences: [usize; 4],
    pub particles: usize,
    pub free_energy: u128,
    pub bond_energy: u128,
    pub total_energy: u128,
    pub molecules: Vec<Vec<ParticleId>>,
    pub largest_molecule: usize,
    pub size_distribution: BTreeMap<usize, usize>,
    pub free_particles: usize,
    pub reactions: ReactionCounts,
}
