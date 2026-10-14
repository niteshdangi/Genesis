mod energy;
mod movement;
mod reactions;
mod state;

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    law_system::{LawSystem, SimulationError},
    world::{Cell, HEIGHT, WIDTH},
};

pub use reactions::{Probability, ReactionPattern, ReactionRule};
pub use state::{Bond, ChemistryObservation, ChemistryState, Particle, ParticleId, ReactionCounts};
use state::{adjacent, bond_key};

pub const BOND_ENERGY: u64 = 2;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ChemistryConfig {
    pub seed: u64,
    pub valences: [usize; 4],
    /// Minimum free energy across a group's occupied cells; movement does not consume it.
    pub movement_energy: u64,
    pub energy_difference: u64,
    pub energy_transfer: u64,
    pub rules: Vec<ReactionRule>,
}

impl Default for ChemistryConfig {
    fn default() -> Self {
        Self {
            seed: 0x4745_4e45_5349_5304,
            valences: [0, 1, 2, 2],
            movement_energy: 1,
            energy_difference: 2,
            energy_transfer: 1,
            rules: vec![
                ReactionRule {
                    pattern: ReactionPattern::Exchange {
                        leaving: Cell::A,
                        center: Cell::B,
                        incoming: Cell::C,
                    },
                    probability: Probability {
                        numerator: 1,
                        denominator: 8,
                    },
                    priority: 0,
                },
                ReactionRule {
                    pattern: ReactionPattern::Dissociation(Cell::A, Cell::B),
                    probability: Probability {
                        numerator: 1,
                        denominator: 64,
                    },
                    priority: 1,
                },
                ReactionRule {
                    pattern: ReactionPattern::Dissociation(Cell::B, Cell::C),
                    probability: Probability {
                        numerator: 1,
                        denominator: 64,
                    },
                    priority: 1,
                },
                ReactionRule {
                    pattern: ReactionPattern::Formation(Cell::A, Cell::B),
                    probability: Probability {
                        numerator: 1,
                        denominator: 2,
                    },
                    priority: 2,
                },
                ReactionRule {
                    pattern: ReactionPattern::Formation(Cell::B, Cell::C),
                    probability: Probability {
                        numerator: 1,
                        denominator: 2,
                    },
                    priority: 2,
                },
            ],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Chemistry {
    config: ChemistryConfig,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ChemistryBaseline {
    particles: BTreeMap<ParticleId, Cell>,
    populations: [usize; 4],
    #[serde(with = "crate::persistence::decimal_u128")]
    energy: u128,
}

impl Chemistry {
    pub fn new(config: ChemistryConfig) -> Result<Self, SimulationError> {
        if config.valences[0] != 0
            || config.valences.iter().any(|&valence| valence > 4)
            || config.energy_difference == 0
        {
            return Err(SimulationError::new(
                "invalid valence or energy propagation configuration",
            ));
        }
        let mut rules = BTreeSet::new();
        for rule in &config.rules {
            rule.validate()?;
            if !rules.insert(rule.key()) {
                return Err(SimulationError::new("duplicate reaction pattern"));
            }
        }
        Ok(Self { config })
    }

    fn validate_structure(&self, state: &ChemistryState) -> Result<(), SimulationError> {
        if state
            .particles
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
        {
            return Err(SimulationError::new(
                "particle identities must be uniquely ordered",
            ));
        }
        if state.grid.cells().len() != WIDTH * HEIGHT
            || state.grid.cells().iter().any(|&kind| kind > 3)
            || state.energy.len() != WIDTH * HEIGHT
            || state.occupancy.len() != WIDTH * HEIGHT
        {
            return Err(SimulationError::new("invalid chemistry grid dimensions"));
        }
        let mut ids = BTreeSet::new();
        let mut positions = BTreeSet::new();
        let mut degrees = BTreeMap::new();
        for particle in &state.particles {
            if particle.id == 0
                || particle.kind == Cell::Empty
                || particle.position >= WIDTH * HEIGHT
                || !ids.insert(particle.id)
                || !positions.insert(particle.position)
            {
                return Err(SimulationError::new(
                    "invalid or duplicate particle identity/position",
                ));
            }
            if state.occupancy[particle.position] != Some(particle.id)
                || state
                    .grid
                    .get(particle.position % WIDTH, particle.position / WIDTH)
                    != particle.kind
            {
                return Err(SimulationError::new(
                    "particle identity and grid occupancy disagree",
                ));
            }
            degrees.insert(particle.id, 0usize);
        }
        for (position, &kind) in state.grid.cells().iter().enumerate() {
            if (kind == 0) != state.occupancy[position].is_none()
                || (kind != 0 && !positions.contains(&position))
            {
                return Err(SimulationError::new("grid contains an untracked particle"));
            }
        }
        for (&key, bond) in &state.bonds {
            let a = state
                .particle(bond.a)
                .ok_or_else(|| SimulationError::new("dangling bond endpoint"))?;
            let b = state
                .particle(bond.b)
                .ok_or_else(|| SimulationError::new("dangling bond endpoint"))?;
            if key != bond_key(bond.a, bond.b)
                || bond.a >= bond.b
                || bond.energy != BOND_ENERGY
                || !adjacent(a.position, b.position)
            {
                return Err(SimulationError::new(
                    "invalid bond geometry or stored energy",
                ));
            }
            degrees.entry(bond.a).and_modify(|degree| *degree += 1);
            degrees.entry(bond.b).and_modify(|degree| *degree += 1);
        }
        for particle in &state.particles {
            if degrees[&particle.id] > self.config.valences[particle.kind as usize] {
                return Err(SimulationError::new("particle exceeds its valence"));
            }
        }
        Ok(())
    }
}

impl LawSystem for Chemistry {
    type State = ChemistryState;
    type Workspace = ();
    type Baseline = ChemistryBaseline;
    type Observation = ChemistryObservation;

    fn stages(&self) -> &'static [&'static str] {
        &[
            "energy propagation",
            "particle movement",
            "chemical reactions",
        ]
    }

    fn workspace(&self, _: &Self::State) {}

    fn baseline(&self, initial: &Self::State) -> Result<Self::Baseline, SimulationError> {
        self.validate_structure(initial)?;
        Ok(ChemistryBaseline {
            particles: initial
                .particles
                .iter()
                .map(|particle| (particle.id, particle.kind))
                .collect(),
            populations: initial.grid.populations(),
            energy: initial.total_energy(),
        })
    }

    fn advance(
        &self,
        stage: usize,
        current: &Self::State,
        next: &mut Self::State,
        _: &mut Self::Workspace,
        generation: u64,
    ) -> Result<(), SimulationError> {
        match stage {
            0 => energy::propagate(current, next, &self.config),
            1 => movement::translate(current, next, &self.config, generation),
            2 => reactions::react(current, next, &self.config, generation),
            _ => Err(SimulationError::new("unknown chemistry stage")),
        }
    }

    fn validate(
        &self,
        state: &Self::State,
        baseline: &Self::Baseline,
    ) -> Result<(), SimulationError> {
        self.validate_structure(state)?;
        let identities: BTreeMap<_, _> = state
            .particles
            .iter()
            .map(|particle| (particle.id, particle.kind))
            .collect();
        if identities != baseline.particles || state.grid.populations() != baseline.populations {
            return Err(SimulationError::new(
                "chemistry changed particle identities or types",
            ));
        }
        if state.total_energy() != baseline.energy {
            return Err(SimulationError::new(
                "free plus bond energy is not conserved",
            ));
        }
        Ok(())
    }

    fn observe(&self, state: &Self::State) -> Result<Self::Observation, SimulationError> {
        let groups = state.groups()?;
        let free_particles = groups.iter().filter(|group| group.len() == 1).count();
        let molecules: Vec<_> = groups
            .into_iter()
            .filter(|group| group.len() >= 2)
            .collect();
        let mut size_distribution = BTreeMap::new();
        for molecule in &molecules {
            *size_distribution.entry(molecule.len()).or_insert(0) += 1;
        }
        Ok(ChemistryObservation {
            valences: self.config.valences,
            particles: state.particles.len(),
            free_energy: state.free_energy(),
            bond_energy: state.bond_energy(),
            total_energy: state.total_energy(),
            largest_molecule: molecules.iter().map(Vec::len).max().unwrap_or(0),
            molecules,
            size_distribution,
            free_particles,
            reactions: state.reactions,
        })
    }
}

pub fn seeded_value(seed: u64, generation: u64, stage: u64, identity: u64, salt: u64) -> u64 {
    fn mix(mut value: u64) -> u64 {
        value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
    [generation, stage, identity, salt]
        .into_iter()
        .fold(mix(seed), |value, key| mix(value ^ mix(key)))
}
