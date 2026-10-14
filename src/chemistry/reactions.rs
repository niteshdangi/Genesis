use std::collections::{BTreeMap, BTreeSet};

use crate::{law_system::SimulationError, world::Cell};

use super::{
    BOND_ENERGY, Bond, ChemistryConfig, ChemistryState, ParticleId, seeded_value,
    state::{BondKey, bond_key, cardinal},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Probability {
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReactionPattern {
    Formation(Cell, Cell),
    Dissociation(Cell, Cell),
    Exchange {
        leaving: Cell,
        center: Cell,
        incoming: Cell,
    },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ReactionRule {
    pub pattern: ReactionPattern,
    pub probability: Probability,
    /// Lower priorities win, then sorted participant IDs and the stable pattern key.
    pub priority: u32,
}

impl ReactionRule {
    pub(super) fn validate(&self) -> Result<(), SimulationError> {
        if self.probability.denominator == 0
            || self.probability.numerator > self.probability.denominator
        {
            return Err(SimulationError::new(
                "reaction probability must be between zero and one",
            ));
        }
        let valid = match self.pattern {
            ReactionPattern::Formation(a, b) | ReactionPattern::Dissociation(a, b) => {
                a != Cell::Empty && b != Cell::Empty
            }
            ReactionPattern::Exchange {
                leaving,
                center,
                incoming,
            } => [leaving, center, incoming]
                .iter()
                .all(|&kind| kind != Cell::Empty),
        };
        if !valid {
            return Err(SimulationError::new(
                "reaction patterns cannot contain Empty",
            ));
        }
        Ok(())
    }

    pub(super) fn key(&self) -> u64 {
        let (category, a, b, c) = match self.pattern {
            ReactionPattern::Formation(a, b) => {
                (0u64, (a as u8).min(b as u8), (a as u8).max(b as u8), 0)
            }
            ReactionPattern::Dissociation(a, b) => {
                (1, (a as u8).min(b as u8), (a as u8).max(b as u8), 0)
            }
            ReactionPattern::Exchange {
                leaving,
                center,
                incoming,
            } => (2, leaving as u8, center as u8, incoming as u8),
        };
        (category << 24) | (u64::from(a) << 16) | (u64::from(b) << 8) | u64::from(c)
    }
}

#[derive(Clone, Copy, Debug)]
enum Action {
    Formation(ParticleId, ParticleId),
    Dissociation(ParticleId, ParticleId),
    Exchange {
        leaving: ParticleId,
        center: ParticleId,
        incoming: ParticleId,
    },
}

impl Action {
    fn participants(self) -> Vec<ParticleId> {
        let mut ids = match self {
            Self::Formation(a, b) | Self::Dissociation(a, b) => vec![a, b],
            Self::Exchange {
                leaving,
                center,
                incoming,
            } => vec![leaving, center, incoming],
        };
        ids.sort_unstable();
        ids
    }
}

struct Edit {
    action: Action,
    remove: Option<BondKey>,
    add: Option<BondKey>,
    energy: Vec<(usize, u64)>,
}

struct Proposal {
    priority: u32,
    rule: u64,
    participants: Vec<ParticleId>,
    edit: Edit,
}

fn pair_matches(a: Cell, b: Cell, expected_a: Cell, expected_b: Cell) -> bool {
    (a == expected_a && b == expected_b) || (a == expected_b && b == expected_a)
}

fn eligible(
    state: &ChemistryState,
    config: &ChemistryConfig,
    degrees: &BTreeMap<ParticleId, usize>,
    action: Action,
) -> Result<Option<Edit>, SimulationError> {
    let mut edit = Edit {
        action,
        remove: None,
        add: None,
        energy: Vec::new(),
    };
    let mut changes = BTreeMap::<ParticleId, i32>::new();
    match action {
        Action::Formation(a, b) => {
            let key = bond_key(a, b);
            if state.bonds.contains_key(&key) {
                return Ok(None);
            }
            let positions = [
                state.particle(key.0).expect("validated particle").position,
                state.particle(key.1).expect("validated particle").position,
            ];
            if positions
                .iter()
                .map(|&position| u128::from(state.energy[position]))
                .sum::<u128>()
                < u128::from(BOND_ENERGY)
            {
                return Ok(None);
            }
            let debit = state.energy[positions[0]].min(BOND_ENERGY);
            edit.energy = vec![
                (positions[0], state.energy[positions[0]] - debit),
                (
                    positions[1],
                    state.energy[positions[1]] - (BOND_ENERGY - debit),
                ),
            ];
            edit.add = Some(key);
            changes.insert(a, 1);
            changes.insert(b, 1);
        }
        Action::Dissociation(a, b) => {
            edit.remove = Some(bond_key(a, b));
            for id in [a, b] {
                let position = state.particle(id).expect("validated particle").position;
                let energy = state.energy[position]
                    .checked_add(BOND_ENERGY / 2)
                    .ok_or_else(|| {
                        SimulationError::new("free energy overflow during dissociation")
                    })?;
                edit.energy.push((position, energy));
                changes.insert(id, -1);
            }
        }
        Action::Exchange {
            leaving,
            center,
            incoming,
        } => {
            let new_key = bond_key(center, incoming);
            if state.bonds.contains_key(&new_key) {
                return Ok(None);
            }
            edit.remove = Some(bond_key(leaving, center));
            edit.add = Some(new_key);
            changes.insert(leaving, -1);
            changes.insert(center, 0);
            changes.insert(incoming, 1);
        }
    }
    if edit
        .remove
        .is_some_and(|key| !state.bonds.contains_key(&key))
    {
        return Ok(None);
    }
    for (id, delta) in changes {
        let kind = state.particle(id).expect("validated particle").kind;
        let final_degree = i128::try_from(degrees[&id])
            .map_err(|_| SimulationError::new("bond degree overflow"))?
            + i128::from(delta);
        if final_degree < 0 || final_degree > config.valences[kind as usize] as i128 {
            return Ok(None);
        }
    }
    Ok(Some(edit))
}

pub(super) fn react(
    current: &ChemistryState,
    next: &mut ChemistryState,
    config: &ChemistryConfig,
    generation: u64,
) -> Result<(), SimulationError> {
    next.events.clear();
    let mut degrees: BTreeMap<_, _> = current
        .particles
        .iter()
        .map(|particle| (particle.id, 0usize))
        .collect();
    for bond in current.bonds.values() {
        *degrees.get_mut(&bond.a).expect("validated endpoint") += 1;
        *degrees.get_mut(&bond.b).expect("validated endpoint") += 1;
    }
    let mut proposals = Vec::new();
    let mut propose = |rule: &ReactionRule, action: Action| -> Result<(), SimulationError> {
        let participants = action.participants();
        let identity = participants.iter().fold(0, |identity, &id| {
            seeded_value(identity, 0, 0, u64::from(id), 0)
        });
        let random = seeded_value(config.seed, generation, 2, identity, rule.key());
        if u128::from(random) * u128::from(rule.probability.denominator)
            >= u128::from(rule.probability.numerator) << 64
        {
            return Ok(());
        }
        if let Some(edit) = eligible(current, config, &degrees, action)? {
            proposals.push(Proposal {
                priority: rule.priority,
                rule: rule.key(),
                participants,
                edit,
            });
        }
        Ok(())
    };
    for rule in &config.rules {
        match rule.pattern {
            ReactionPattern::Formation(a, b) => {
                for particle in &current.particles {
                    for position in cardinal(particle.position) {
                        if let Some(id) = current.occupancy[position]
                            && id > particle.id
                            && pair_matches(
                                particle.kind,
                                current.particle(id).expect("validated particle").kind,
                                a,
                                b,
                            )
                        {
                            propose(rule, Action::Formation(particle.id, id))?;
                        }
                    }
                }
            }
            ReactionPattern::Dissociation(a, b) => {
                for bond in current.bonds.values() {
                    if pair_matches(
                        current.particle(bond.a).expect("validated particle").kind,
                        current.particle(bond.b).expect("validated particle").kind,
                        a,
                        b,
                    ) {
                        propose(rule, Action::Dissociation(bond.a, bond.b))?;
                    }
                }
            }
            ReactionPattern::Exchange {
                leaving,
                center,
                incoming,
            } => {
                for bond in current.bonds.values() {
                    for (a, b) in [(bond.a, bond.b), (bond.b, bond.a)] {
                        let first = current.particle(a).expect("validated particle");
                        let middle = current.particle(b).expect("validated particle");
                        if first.kind != leaving || middle.kind != center {
                            continue;
                        }
                        for position in cardinal(middle.position) {
                            if let Some(id) = current.occupancy[position]
                                && id != a
                                && id != b
                                && current.particle(id).expect("validated particle").kind
                                    == incoming
                            {
                                propose(
                                    rule,
                                    Action::Exchange {
                                        leaving: a,
                                        center: b,
                                        incoming: id,
                                    },
                                )?;
                            }
                        }
                    }
                }
            }
        }
    }
    proposals.sort_by(|a, b| {
        (a.priority, &a.participants, a.rule).cmp(&(b.priority, &b.participants, b.rule))
    });
    let mut claimed = BTreeSet::<ParticleId>::new();
    for proposal in proposals {
        if proposal.participants.iter().any(|id| claimed.contains(id)) {
            continue;
        }
        claimed.extend(&proposal.participants);
        let edit = proposal.edit;
        let kind = match edit.action { Action::Formation(..) => "formation", Action::Dissociation(..) => "dissociation", Action::Exchange { .. } => "exchange" };
        next.events.push(serde_json::json!({"kind":kind,
            "generation":generation.checked_add(1).ok_or_else(|| SimulationError::new("generation overflow"))?.to_string(),
            "participants":proposal.participants, "removed_bond":edit.remove, "added_bond":edit.add}));
        if let Some(key) = edit.remove {
            next.bonds.remove(&key);
        }
        if let Some(key) = edit.add {
            next.bonds.insert(
                key,
                Bond {
                    a: key.0,
                    b: key.1,
                    energy: BOND_ENERGY,
                },
            );
        }
        for (position, energy) in edit.energy {
            next.energy[position] = energy;
        }
        let counter = match edit.action {
            Action::Formation(..) => &mut next.reactions.formation,
            Action::Dissociation(..) => &mut next.reactions.dissociation,
            Action::Exchange { .. } => &mut next.reactions.exchange,
        };
        *counter = counter
            .checked_add(1)
            .ok_or_else(|| SimulationError::new("reaction counter overflow"))?;
    }
    Ok(())
}
