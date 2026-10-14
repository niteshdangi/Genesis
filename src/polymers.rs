use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use crate::{chemistry::seeded_value, law_system::{LawSystem, SimulationError}, spatial::{Torus, add}};

const BOUND_ENERGY: u64 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PolymerConfig {
    pub space: Torus,
    pub seed: u64,
    pub monomers_per_site: usize,
    pub initial_energy: u64,
    pub energy_input: u64,
    pub energy_capacity: u64,
    pub maintenance: u64,
    pub max_age: u64,
    pub error_per_million: u64,
    pub assembly_per_million: u64,
    pub nucleation_per_million: u64,
    pub copying_enabled: bool,
    pub neutral_copying: bool,
    pub seeded_templates: bool,
    pub resource_period: u64,
    pub templates: Vec<Vec<u8>>,
}

impl Default for PolymerConfig {
    fn default() -> Self {
        Self {
            space: Torus::default(), seed: 0x4745_4e45_5349_5305,
            monomers_per_site: 6, initial_energy: 12, energy_input: 1,
            energy_capacity: 32, maintenance: 1, max_age: 600,
            error_per_million: 5_000, assembly_per_million: 650_000,
            nucleation_per_million: 100, copying_enabled: true,
            neutral_copying: false, seeded_templates: true, resource_period: 1_000,
            templates: vec![vec![0, 1, 0, 0, 1, 2], vec![1, 0, 1, 1, 0, 2]],
        }
    }
}

impl PolymerConfig {
    pub fn validate(&self) -> Result<(), SimulationError> {
        self.space.validate()?;
        if !(3..=32).contains(&self.monomers_per_site)
            || self.initial_energy > self.energy_capacity || self.energy_capacity > 1_000_000
            || self.energy_input > 1_000_000 || self.maintenance > 1_000_000
            || self.max_age == 0 || self.resource_period == 0
            || self.error_per_million > 1_000_000 || self.assembly_per_million > 1_000_000
            || self.nucleation_per_million > 1_000_000 || self.templates.len() > 64
            || self.space.len() * self.monomers_per_site > 500_000
        { return Err(SimulationError::new("invalid polymer law configuration")); }
        for template in &self.templates {
            if template.len() < 2 || template.len() >= self.space.width.min(self.space.height)
                || template.last() != Some(&2) || template[..template.len()-1].iter().any(|&kind| kind > 1)
            { return Err(SimulationError::new("template needs binary residues and one terminal cap")); }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Monomer { pub id: u64, pub kind: u8 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Polymer {
    pub id: u64,
    pub origin: usize,
    pub direction: usize,
    pub residues: Vec<Monomer>,
    /// Only attached products reference a live template. Released ancestry is historical.
    pub attached_to: Option<u64>,
    pub parent: Option<u64>,
    pub depth: u64,
    pub born: u64,
    pub age: u64,
    pub matches: u64,
}

impl Polymer {
    fn positions(&self, space: Torus) -> Vec<usize> {
        (0..self.residues.len()).map(|i| space.offset(self.origin, self.direction, i)).collect()
    }
    fn released(&self) -> bool { self.attached_to.is_none() && self.residues.last().is_some_and(|m| m.kind == 2) }
    fn sequence(&self) -> String { self.residues.iter().map(|m| char::from(b'0' + m.kind)).collect() }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PolymerCounters {
    pub assembled: u64,
    pub released: u64,
    pub decayed: u64,
    pub substitutions: u64,
    pub copied_residues: u64,
    pub matching_residues: u64,
    pub max_depth: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolymerState {
    pub space: Torus,
    pub free: Vec<Vec<Monomer>>,
    pub energy: Vec<u64>,
    pub chains: BTreeMap<u64, Polymer>,
    pub next_chain: u64,
    pub energy_in: u64,
    pub energy_out: u64,
    pub counters: PolymerCounters,
    /// Per-generation events are exported by the experiment runner before the next step.
    pub events: Vec<Value>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PolymerBaseline {
    pub matter: BTreeMap<u64, u8>,
    pub total_energy: u64,
}

#[derive(Clone)]
pub struct PolymerLaw { config: PolymerConfig }

impl PolymerLaw {
    pub fn new(config: PolymerConfig) -> Result<Self, SimulationError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn initial(&self) -> Result<PolymerState, SimulationError> {
        let config = &self.config;
        let mut state = PolymerState {
            space: config.space, free: vec![Vec::new(); config.space.len()],
            energy: vec![config.initial_energy; config.space.len()], chains: BTreeMap::new(),
            next_chain: 1, energy_in: 0, energy_out: 0,
            counters: PolymerCounters::default(), events: Vec::new(),
        };
        let mut id = 1;
        for (position, pool) in state.free.iter_mut().enumerate() {
            for index in 0..config.monomers_per_site {
                let dominant = u8::from(position % config.space.width >= config.space.width / 2);
                let kind = if index + 1 == config.monomers_per_site { 2 }
                    else if index % 3 == 0 { 1 - dominant } else { dominant };
                pool.push(Monomer { id, kind });
                id += 1;
            }
        }
        // The absent-template control retains exactly the same monomers and energy.
        if config.seeded_templates {
            for (index, sequence) in config.templates.iter().enumerate() {
                let y = (index * 4 + 2) % config.space.height;
                let x = ((index / (config.space.height / 4)) * (config.space.width / 2) + 2) % config.space.width;
                let origin = y * config.space.width + x;
                let mut residues = Vec::new();
                for (offset, &kind) in sequence.iter().enumerate() {
                    let position = config.space.offset(origin, 0, offset);
                    if !Self::vacant(&state, position, None) || state.energy[position] < BOUND_ENERGY {
                        return Err(SimulationError::new("designed templates overlap or lack energy"));
                    }
                    let token = state.free[position].iter().position(|m| m.kind == kind)
                        .ok_or_else(|| SimulationError::new("template lacks a local monomer"))?;
                    residues.push(state.free[position].remove(token));
                    state.energy[position] -= BOUND_ENERGY;
                }
                let id = state.next_chain;
                add(&mut state.next_chain, 1)?;
                state.chains.insert(id, Polymer { id, origin, direction: 0, residues, attached_to: None,
                    parent: None, depth: 0, born: 0, age: 0, matches: 0 });
            }
        }
        Ok(state)
    }

    fn vacant(state: &PolymerState, position: usize, ignore: Option<u64>) -> bool {
        state.chains.values().filter(|chain| Some(chain.id) != ignore)
            .all(|chain| !chain.positions(state.space).contains(&position))
    }

    fn occupancy(state: &PolymerState) -> Vec<Option<u64>> {
        let mut occupancy = vec![None; state.space.len()];
        for chain in state.chains.values() {
            for position in chain.positions(state.space) { occupancy[position] = Some(chain.id); }
        }
        occupancy
    }

    fn total_energy(state: &PolymerState) -> Result<u64, SimulationError> {
        let mut total = 0;
        for &energy in &state.energy { add(&mut total, energy)?; }
        for chain in state.chains.values() { add(&mut total, chain.residues.len() as u64 * BOUND_ENERGY)?; }
        Ok(total)
    }

    fn matter(state: &PolymerState) -> Result<BTreeMap<u64, u8>, SimulationError> {
        let mut matter = BTreeMap::new();
        for monomer in state.free.iter().flatten().chain(state.chains.values().flat_map(|p| &p.residues)) {
            if monomer.id == 0 || monomer.kind > 2 || matter.insert(monomer.id, monomer.kind).is_some() {
                return Err(SimulationError::new("invalid or duplicate monomer identity"));
            }
        }
        Ok(matter)
    }

    fn release_matter(state: &mut PolymerState, chain: Polymer, generation: u64) -> Result<(), SimulationError> {
        let positions = chain.positions(state.space);
        for (position, monomer) in positions.into_iter().zip(chain.residues) {
            state.free[position].push(monomer);
            add(&mut state.energy[position], BOUND_ENERGY)?;
        }
        add(&mut state.counters.decayed, 1)?;
        state.events.push(json!({"kind":"decay", "generation":generation.to_string(), "chain":chain.id.to_string()}));
        Ok(())
    }

    fn environment(&self, current: &PolymerState, next: &mut PolymerState, generation: u64) -> Result<(), SimulationError> {
        next.events.clear();
        let space = current.space;
        for position in 0..space.len() {
            let phase = (generation / self.config.resource_period) % 2;
            // Alternating driven regions create a reproducible changing environment.
            let driven = (position % space.width < space.width / 2) == (phase == 0);
            if driven {
                let amount = self.config.energy_input.min(self.config.energy_capacity.saturating_sub(next.energy[position]));
                add(&mut next.energy[position], amount)?;
                add(&mut next.energy_in, amount)?;
            }
        }
        // Each source can spend only snapshot energy; incoming energy is not re-spent.
        let mut transfers = Vec::new();
        for position in 0..space.len() {
            let target = space.neighbor(position, (generation % 4) as usize);
            if current.energy[position] > current.energy[target].saturating_add(1) {
                transfers.push((position, target));
            }
        }
        for (source, target) in transfers { next.energy[source] -= 1; add(&mut next.energy[target], 1)?; }
        let mut free = vec![Vec::new(); space.len()];
        for (position, pool) in current.free.iter().enumerate() {
            for monomer in pool {
                let random = seeded_value(self.config.seed, generation, 10, monomer.id, 0);
                let target = if random % 5 == 4 { position } else { space.neighbor(position, (random % 4) as usize) };
                free[target].push(monomer.clone());
            }
        }
        for pool in &mut free { pool.sort_by_key(|m| m.id); }
        next.free = free;
        Ok(())
    }

    fn movement(&self, current: &PolymerState, next: &mut PolymerState, generation: u64) -> Result<(), SimulationError> {
        let mut reserved = BTreeSet::new();
        let occupancy = Self::occupancy(current);
        let attached: BTreeSet<_> = current.chains.values().filter_map(|chain| chain.attached_to).collect();
        let mut proposals: Vec<_> = current.chains.values().filter(|p| p.attached_to.is_none() && !attached.contains(&p.id))
            .map(|p| (seeded_value(self.config.seed, generation, 11, p.id, 0), p.id)).collect();
        proposals.sort_unstable();
        for (random, id) in proposals {
            let chain = &current.chains[&id];
            if current.energy[chain.origin] == 0 { continue; }
            for attempt in 0..4 {
                let direction = ((random % 4) as usize + attempt) % 4;
                let origin = current.space.neighbor(chain.origin, direction);
                let targets: Vec<_> = (0..chain.residues.len()).map(|i| current.space.offset(origin, chain.direction, i)).collect();
                if targets.iter().all(|&position| !reserved.contains(&position) && occupancy[position].is_none_or(|owner| owner == id)) {
                    reserved.extend(targets);
                    next.chains.get_mut(&id).ok_or_else(|| SimulationError::new("missing movement chain"))?.origin = origin;
                    next.energy[chain.origin] -= 1;
                    add(&mut next.energy_out, 1)?;
                    break;
                }
            }
        }
        Ok(())
    }

    fn assembly(&self, current: &PolymerState, next: &mut PolymerState, generation: u64) -> Result<(), SimulationError> {
        let mut occupancy = Self::occupancy(current);
        let attached: BTreeMap<_, _> = current.chains.values().filter_map(|p| p.attached_to.map(|parent| (parent, p.id))).collect();
        let mut templates: Vec<_> = current.chains.values().filter(|p| p.released()).map(|p| p.id).collect();
        templates.sort_by_key(|&id| (seeded_value(self.config.seed, generation, 12, id, 0), id));
        if self.config.copying_enabled {
            for parent_id in templates {
                let parent = &current.chains[&parent_id];
                let product_id = attached.get(&parent_id).copied();
                let offset = product_id.map(|id| current.chains[&id].residues.len()).unwrap_or(0);
                let Some(expected) = parent.residues.get(offset).map(|m| m.kind) else { continue; };
                let origin = product_id.map(|id| current.chains[&id].origin)
                    .unwrap_or_else(|| current.space.neighbor(parent.origin, (parent.direction + 1) % 4));
                let position = current.space.offset(origin, parent.direction, offset);
                if occupancy[position].is_some() || next.energy[position] < 2 { continue; }
                let random = seeded_value(self.config.seed, generation, 12, parent_id, offset as u64 + 1);
                if random % 1_000_000 >= self.config.assembly_per_million { continue; }
                let mismatch = expected != 2 && seeded_value(self.config.seed, generation, 13, parent_id, offset as u64) % 1_000_000 < self.config.error_per_million;
                let wanted = if expected == 2 { 2 }
                    else if self.config.neutral_copying { (seeded_value(self.config.seed, generation, 15, parent_id, offset as u64) % 2) as u8 }
                    else if mismatch { 1 - expected } else { expected };
                let Some(token) = next.free[position].iter().position(|m| m.kind == wanted) else { continue; };
                let monomer = next.free[position].remove(token);
                let monomer_id = monomer.id;
                let id = if let Some(id) = product_id { id } else {
                    let id = next.next_chain;
                    add(&mut next.next_chain, 1)?;
                    next.chains.insert(id, Polymer { id, origin, direction: parent.direction, residues: Vec::new(),
                        attached_to: Some(parent_id), parent: Some(parent_id), depth: parent.depth.checked_add(1)
                        .ok_or_else(|| SimulationError::new("lineage depth overflow"))?, born: generation, age: 0, matches: 0 });
                    id
                };
                next.energy[position] -= 2;
                occupancy[position] = Some(id);
                add(&mut next.energy_out, 1)?;
                add(&mut next.counters.assembled, 1)?;
                add(&mut next.counters.copied_residues, 1)?;
                if wanted == expected { add(&mut next.counters.matching_residues, 1)?; }
                else { add(&mut next.counters.substitutions, 1)?; }
                let product = next.chains.get_mut(&id).ok_or_else(|| SimulationError::new("missing attached product"))?;
                product.residues.push(monomer);
                if wanted == expected { add(&mut product.matches, 1)?; }
                // A recruited terminal cap ends attachment; no full sequence is copied atomically.
                if wanted == 2 {
                    product.attached_to = None;
                    product.born = generation;
                    product.age = 0;
                    add(&mut next.counters.released, 1)?;
                    next.counters.max_depth = next.counters.max_depth.max(product.depth);
                    next.events.push(json!({"kind":"release", "generation":generation.to_string(), "chain":id.to_string(),
                        "parent":parent_id.to_string(), "sequence":product.sequence(), "depth":product.depth.to_string(),
                        "matching_residues":product.matches, "residues":product.residues.len()}));
                }
                next.events.push(json!({"kind":"recruit", "generation":generation.to_string(), "chain":id.to_string(),
                    "parent":parent_id.to_string(), "monomer":monomer_id.to_string(), "offset":offset, "kind_code":wanted}));
            }
        }
        // Local untemplated pair assembly provides a background assembly control.
        for position in 0..current.space.len() {
            if seeded_value(self.config.seed, generation, 14, position as u64, 0) % 1_000_000 >= self.config.nucleation_per_million { continue; }
            let target = current.space.neighbor(position, 0);
            if occupancy[position].is_some() || occupancy[target].is_some()
                || next.energy[position] < 2 || next.energy[target] < 2 { continue; }
            let first = next.free[position].iter().position(|m| m.kind < 2);
            let cap = next.free[target].iter().position(|m| m.kind == 2);
            if let (Some(first), Some(cap)) = (first, cap) {
                let residues = vec![next.free[position].remove(first), next.free[target].remove(cap)];
                next.energy[position] -= 2;
                next.energy[target] -= 2;
                add(&mut next.energy_out, 2)?;
                let id = next.next_chain;
                add(&mut next.next_chain, 1)?;
                occupancy[position] = Some(id);
                occupancy[target] = Some(id);
                let chain = Polymer { id, origin: position, direction: 0, residues, attached_to: None, parent: None,
                    depth: 0, born: generation, age: 0, matches: 0 };
                next.events.push(json!({"kind":"nucleation", "generation":generation.to_string(), "chain":id.to_string(), "sequence":chain.sequence()}));
                next.chains.insert(id, chain);
            }
        }
        Ok(())
    }

    fn decay(&self, current: &PolymerState, next: &mut PolymerState, generation: u64) -> Result<(), SimulationError> {
        let mut dying = BTreeSet::new();
        for chain in current.chains.values() {
            if chain.age >= self.config.max_age || current.energy[chain.origin] < self.config.maintenance {
                dying.insert(chain.id);
            } else {
                next.energy[chain.origin] -= self.config.maintenance;
                add(&mut next.energy_out, self.config.maintenance)?;
                add(&mut next.chains.get_mut(&chain.id).ok_or_else(|| SimulationError::new("missing aging chain"))?.age, 1)?;
            }
        }
        // Incomplete products cannot continue using a template that has disassembled.
        for chain in current.chains.values() {
            if chain.attached_to.is_some_and(|id| dying.contains(&id)) { dying.insert(chain.id); }
        }
        for id in dying {
            if let Some(chain) = next.chains.remove(&id) { Self::release_matter(next, chain, generation)?; }
        }
        for pool in &mut next.free { pool.sort_by_key(|m| m.id); }
        Ok(())
    }

    pub fn observation(&self, state: &PolymerState) -> Value {
        let mut variants = BTreeMap::<String, usize>::new();
        let mut distribution = BTreeMap::<usize, usize>::new();
        let mut descendants = 0;
        for chain in state.chains.values().filter(|p| p.released()) {
            *variants.entry(chain.sequence()).or_default() += 1;
            *distribution.entry(chain.residues.len()).or_default() += 1;
            if chain.parent.is_some() { descendants += 1; }
        }
        json!({"released_population":variants.values().sum::<usize>(), "descendants":descendants,
            "attached_products":state.chains.values().filter(|p| p.attached_to.is_some()).count(),
            "variants":variants, "size_distribution":distribution,
            "free_monomers":state.free.iter().map(Vec::len).sum::<usize>(), "counters":state.counters,
            "energy_in":state.energy_in.to_string(), "energy_out":state.energy_out.to_string(),
            "energy":state.energy.iter().map(u64::to_string).collect::<Vec<_>>(), "events":state.events,
            "chains":state.chains.values().map(|p| json!({"id":p.id.to_string(), "parent":p.parent.map(|id| id.to_string()),
                "attached_to":p.attached_to.map(|id| id.to_string()), "sequence":p.sequence(), "positions":p.positions(state.space),
                "age":p.age.to_string(), "depth":p.depth.to_string()})).collect::<Vec<_>>()})
    }

    pub fn cells(state: &PolymerState) -> Vec<u8> {
        let mut cells = vec![0; state.space.len()];
        for chain in state.chains.values() {
            for (position, monomer) in chain.positions(state.space).into_iter().zip(&chain.residues) { cells[position] = monomer.kind + 1; }
        }
        cells
    }
}

impl LawSystem for PolymerLaw {
    type State = PolymerState;
    type Workspace = ();
    type Baseline = PolymerBaseline;
    type Observation = Value;
    fn stages(&self) -> &'static [&'static str] { &["environment", "rigid polymer movement", "local assembly", "maintenance and decay"] }
    fn workspace(&self, _: &Self::State) {}
    fn baseline(&self, state: &Self::State) -> Result<Self::Baseline, SimulationError> {
        Ok(PolymerBaseline { matter: Self::matter(state)?, total_energy: Self::total_energy(state)? })
    }
    fn advance(&self, stage: usize, current: &Self::State, next: &mut Self::State, _: &mut (), generation: u64) -> Result<(), SimulationError> {
        let generation = generation.checked_add(1).ok_or_else(|| SimulationError::new("generation overflow"))?;
        match stage {
            0 => self.environment(current, next, generation),
            1 => self.movement(current, next, generation),
            2 => self.assembly(current, next, generation),
            3 => self.decay(current, next, generation),
            _ => Err(SimulationError::new("unknown polymer stage")),
        }
    }
    fn validate(&self, state: &Self::State, baseline: &Self::Baseline) -> Result<(), SimulationError> {
        if state.space != self.config.space || state.free.len() != state.space.len() || state.energy.len() != state.space.len() {
            return Err(SimulationError::new("invalid polymer spatial state"));
        }
        if Self::matter(state)? != baseline.matter { return Err(SimulationError::new("monomer identity or type conservation failed")); }
        let accounted = Self::total_energy(state)?.checked_add(state.energy_out)
            .ok_or_else(|| SimulationError::new("energy ledger overflow"))?;
        if Some(accounted) != baseline.total_energy.checked_add(state.energy_in) { return Err(SimulationError::new("polymer energy ledger mismatch")); }
        let mut occupied = BTreeSet::new();
        let mut parents = BTreeSet::new();
        for (&id, chain) in &state.chains {
            if id == 0 || id != chain.id || id >= state.next_chain || chain.origin >= state.space.len()
                || chain.direction > 3 || chain.residues.is_empty() || chain.residues.len() >= state.space.width.min(state.space.height)
                || chain.residues[..chain.residues.len()-1].iter().any(|m| m.kind == 2)
                || chain.parent.is_some_and(|parent| parent >= id) || chain.matches > chain.residues.len() as u64
            { return Err(SimulationError::new("invalid polymer structure or ancestry")); }
            for position in chain.positions(state.space) {
                if !occupied.insert(position) { return Err(SimulationError::new("overlapping polymer geometry")); }
            }
            if let Some(parent_id) = chain.attached_to {
                let parent = state.chains.get(&parent_id).ok_or_else(|| SimulationError::new("attached template is absent"))?;
                if !parent.released() || !parents.insert(parent_id) || chain.parent != Some(parent_id)
                    || chain.residues.len() >= parent.residues.len() || chain.direction != parent.direction
                    || chain.origin != state.space.neighbor(parent.origin, (parent.direction + 1) % 4)
                { return Err(SimulationError::new("invalid template contact geometry")); }
            } else if !chain.released() { return Err(SimulationError::new("released polymer lacks a terminal cap")); }
        }
        Ok(())
    }
    fn observe(&self, state: &Self::State) -> Result<Value, SimulationError> { Ok(self.observation(state)) }
}
