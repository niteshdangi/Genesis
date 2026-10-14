use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use crate::{chemistry::seeded_value, law_system::{LawSystem, SimulationError}, spatial::{Torus, add}};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentConfig {
    pub space: Torus,
    pub seed: u64,
    pub population: usize,
    pub food_per_site: u64,
    pub regrowth: u64,
    pub site_capacity: u64,
    pub initial_credits: u64,
    pub harvest: u64,
    pub metabolism: u64,
    pub reproduction_threshold: u64,
    pub max_age: u64,
    pub max_population: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self { space: Torus::default(), seed: 0x4745_4e45_5349_5306, population: 96,
            food_per_site: 8, regrowth: 1, site_capacity: 24, initial_credits: 32,
            harvest: 4, metabolism: 1, reproduction_threshold: 32, max_age: 1_000, max_population: 512 }
    }
}

impl AgentConfig {
    pub fn validate(&self) -> Result<(), SimulationError> {
        self.space.validate()?;
        if self.population == 0 || self.population > self.max_population || self.max_population > 4_096
            || self.max_population > self.space.len() || self.food_per_site > self.site_capacity
            || self.site_capacity > 1_000_000 || self.regrowth > 1_000_000
            || self.initial_credits > 1_000_000 || self.harvest > 1_000_000
            || self.metabolism == 0 || self.metabolism > 1_000_000 || self.max_age == 0
            || self.reproduction_threshold < 8 || self.reproduction_threshold > 1_000_000
        { return Err(SimulationError::new("invalid agent universe configuration")); }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agent {
    pub id: u64,
    pub parent: Option<u64>,
    pub position: usize,
    pub goods: [u64; 2],
    pub credits: u64,
    pub specialty: usize,
    pub preference: usize,
    pub age: u64,
    pub descendants: u64,
    pub signal: u8,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentCounters { pub trades: u64, pub births: u64, pub deaths: u64, pub messages: u64, pub cultural_changes: u64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentState {
    pub space: Torus,
    pub resources: Vec<[u64; 2]>,
    pub agents: BTreeMap<u64, Agent>,
    pub treasury: u64,
    pub next_id: u64,
    pub resource_input: u64,
    pub consumed: u64,
    pub counters: AgentCounters,
    pub events: Vec<Value>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AgentBaseline { pub material: u64, pub credits: u64 }

#[derive(Clone)]
pub struct AgentLaw { config: AgentConfig }

impl AgentLaw {
    pub fn new(config: AgentConfig) -> Result<Self, SimulationError> { config.validate()?; Ok(Self { config }) }

    pub fn initial(&self) -> Result<AgentState, SimulationError> {
        let config = &self.config;
        let resources = (0..config.space.len()).map(|p| {
            let kind = (p % config.space.width / (config.space.width / 2)) % 2;
            let mut resources = [0, 0];
            resources[kind] = config.food_per_site;
            resources
        }).collect();
        let mut state = AgentState { space: config.space, resources, agents: BTreeMap::new(),
            treasury: 0, next_id: config.population as u64 + 1, resource_input: 0, consumed: 0,
            counters: AgentCounters::default(), events: Vec::new() };
        let mut available: BTreeSet<_> = (0..config.space.len()).collect();
        for i in 0..config.population {
            let start = seeded_value(config.seed, 0, 20, i as u64, 0) as usize % config.space.len();
            let position = available.range(start..).next().copied().or_else(|| available.first().copied())
                .ok_or_else(|| SimulationError::new("no initial agent position"))?;
            available.remove(&position);
            let specialty = i % 2;
            let mut goods = [0, 0];
            for kind in 0..2 {
                let amount = state.resources[position][kind].min(4);
                state.resources[position][kind] -= amount;
                goods[kind] = amount;
            }
            let id = i as u64 + 1;
            state.agents.insert(id, Agent { id, parent: None, position, goods, credits: config.initial_credits,
                specialty, preference: specialty, age: 0, descendants: 0, signal: 0 });
        }
        Ok(state)
    }

    fn totals(state: &AgentState) -> Result<AgentBaseline, SimulationError> {
        let mut material = 0;
        let mut credits = state.treasury;
        for pool in &state.resources { for &amount in pool { add(&mut material, amount)?; } }
        for agent in state.agents.values() {
            for &amount in &agent.goods { add(&mut material, amount)?; }
            add(&mut credits, agent.credits)?;
        }
        Ok(AgentBaseline { material, credits })
    }

    fn environment(&self, current: &AgentState, next: &mut AgentState) -> Result<(), SimulationError> {
        next.events.clear();
        for position in 0..current.space.len() {
            let kind = (position % current.space.width / (current.space.width / 2)) % 2;
            let amount = self.config.regrowth.min(self.config.site_capacity.saturating_sub(current.resources[position][kind]));
            add(&mut next.resources[position][kind], amount)?;
            add(&mut next.resource_input, amount)?;
        }
        Ok(())
    }

    fn movement(&self, current: &AgentState, next: &mut AgentState, generation: u64) -> Result<(), SimulationError> {
        let occupied: BTreeSet<_> = current.agents.values().map(|a| a.position).collect();
        let mut reserved = BTreeSet::new();
        let mut order: Vec<_> = current.agents.keys().copied().collect();
        order.sort_by_key(|&id| (seeded_value(self.config.seed, generation, 21, id, 0), id));
        for id in order {
            let agent = &current.agents[&id];
            let mut positions: Vec<_> = (0..4).map(|direction| current.space.neighbor(agent.position, direction)).collect();
            positions.push(agent.position);
            positions.sort_by_key(|&position| (
                std::cmp::Reverse(current.resources[position][agent.specialty]),
                seeded_value(self.config.seed, generation, 21, id, position as u64), position));
            if let Some(position) = positions.into_iter().find(|position| !reserved.contains(position)
                && (*position == agent.position || !occupied.contains(position))) {
                reserved.insert(position);
                next.agents.get_mut(&id).ok_or_else(|| SimulationError::new("missing moving agent"))?.position = position;
            }
        }
        Ok(())
    }

    fn interactions(&self, current: &AgentState, next: &mut AgentState, generation: u64) -> Result<(), SimulationError> {
        let mut order: Vec<_> = current.agents.keys().copied().collect();
        order.sort_by_key(|&id| (seeded_value(self.config.seed, generation, 22, id, 0), id));
        for &id in &order {
            let agent = next.agents.get_mut(&id).ok_or_else(|| SimulationError::new("missing harvesting agent"))?;
            let amount = next.resources[agent.position][agent.specialty].min(self.config.harvest);
            next.resources[agent.position][agent.specialty] -= amount;
            add(&mut agent.goods[agent.specialty], amount)?;
        }
        let mut pairs = BTreeSet::new();
        let positions: BTreeMap<_, _> = current.agents.values().map(|a| (a.position, a.id)).collect();
        for agent in current.agents.values() {
            for direction in 0..4 {
                if let Some(&other) = positions.get(&current.space.neighbor(agent.position, direction)) {
                    pairs.insert((agent.id.min(other), agent.id.max(other)));
                }
            }
        }
        let mut pairs: Vec<_> = pairs.into_iter().collect();
        pairs.sort_by_key(|&(a, b)| (seeded_value(self.config.seed, generation, 23, a, b), a, b));
        let mut traded = BTreeSet::new();
        for (a, b) in pairs {
            if traded.contains(&a) || traded.contains(&b) { continue; }
            let first = next.agents[&a].clone();
            let second = next.agents[&b].clone();
            let options = [(first.clone(), second.clone()), (second.clone(), first.clone())];
            for (buyer, seller) in options {
                let kind = 1 - buyer.specialty;
                let price = if buyer.goods[kind] == 0 { 2 } else { 1 };
                if seller.goods[kind] > self.config.metabolism && buyer.goods[kind] < 4 && buyer.credits >= price {
                    let buyer_state = next.agents.get_mut(&buyer.id).ok_or_else(|| SimulationError::new("missing buyer"))?;
                    buyer_state.credits -= price;
                    add(&mut buyer_state.goods[kind], 1)?;
                    let seller_state = next.agents.get_mut(&seller.id).ok_or_else(|| SimulationError::new("missing seller"))?;
                    seller_state.goods[kind] -= 1;
                    add(&mut seller_state.credits, price)?;
                    traded.insert(a); traded.insert(b);
                    add(&mut next.counters.trades, 1)?;
                    next.events.push(json!({"kind":"trade", "generation":generation.to_string(),
                        "buyer":buyer.id.to_string(), "seller":seller.id.to_string(), "resource":kind, "price":price}));
                    break;
                }
            }
            // A local message exposes a neighbor's preference and economic result.
            let (learner, model) = if first.credits < second.credits { (first, second) } else { (second, first) };
            add(&mut next.counters.messages, 1)?;
            if learner.preference != model.preference && seeded_value(self.config.seed, generation, 24, learner.id, model.id) % 4 == 0 {
                next.agents.get_mut(&learner.id).ok_or_else(|| SimulationError::new("missing message recipient"))?.preference = model.preference;
                add(&mut next.counters.cultural_changes, 1)?;
                next.events.push(json!({"kind":"learn", "generation":generation.to_string(), "agent":learner.id.to_string(),
                    "neighbor":model.id.to_string(), "preference":model.preference}));
            }
        }
        let mut dying = Vec::new();
        for &id in &order {
            let agent = next.agents.get_mut(&id).ok_or_else(|| SimulationError::new("missing consuming agent"))?;
            let mut needed = self.config.metabolism;
            for kind in [agent.preference, 1 - agent.preference] {
                let amount = agent.goods[kind].min(needed);
                agent.goods[kind] -= amount;
                needed -= amount;
                add(&mut next.consumed, amount)?;
            }
            add(&mut agent.age, 1)?;
            agent.signal = if needed > 0 { 0 } else if agent.credits >= self.config.initial_credits { 2 } else { 1 };
            if needed > 0 || agent.age >= self.config.max_age { dying.push(id); }
        }
        for id in dying {
            if let Some(agent) = next.agents.remove(&id) {
                for kind in 0..2 { add(&mut next.resources[agent.position][kind], agent.goods[kind])?; }
                add(&mut next.treasury, agent.credits)?;
                add(&mut next.counters.deaths, 1)?;
                next.events.push(json!({"kind":"death", "generation":generation.to_string(), "agent":id.to_string()}));
            }
        }
        let mut occupied: BTreeSet<_> = next.agents.values().map(|a| a.position).collect();
        for id in order {
            if next.agents.len() >= self.config.max_population { break; }
            let Some(parent) = next.agents.get(&id).cloned() else { continue; };
            if parent.goods[0].saturating_add(parent.goods[1]) < self.config.reproduction_threshold { continue; }
            let start = seeded_value(self.config.seed, generation, 25, id, 0) as usize % 4;
            let Some(position) = (0..4).map(|d| current.space.neighbor(parent.position, (start + d) % 4))
                .find(|p| !occupied.contains(p)) else { continue; };
            let child_id = next.next_id;
            add(&mut next.next_id, 1)?;
            let mut goods = [0, 0];
            for kind in 0..2 { goods[kind] = parent.goods[kind] / 2; }
            let credits = parent.credits / 2;
            let parent_state = next.agents.get_mut(&id).ok_or_else(|| SimulationError::new("missing reproducing parent"))?;
            for kind in 0..2 { parent_state.goods[kind] -= goods[kind]; }
            parent_state.credits -= credits;
            add(&mut parent_state.descendants, 1)?;
            let specialty = if seeded_value(self.config.seed, generation, 25, id, 1) % 64 == 0 { 1 - parent.specialty } else { parent.specialty };
            next.agents.insert(child_id, Agent { id: child_id, parent: Some(id), position, goods, credits,
                specialty, preference: parent.preference, age: 0, descendants: 0, signal: 1 });
            occupied.insert(position);
            add(&mut next.counters.births, 1)?;
            next.events.push(json!({"kind":"birth", "generation":generation.to_string(), "agent":child_id.to_string(), "parent":id.to_string()}));
        }
        Ok(())
    }

    pub fn observation(&self, state: &AgentState) -> Value {
        let mut culture = [0usize; 2];
        let mut specialties = [0usize; 2];
        let mut wealth: Vec<_> = state.agents.values().map(|a| a.credits).collect();
        wealth.sort_unstable();
        for agent in state.agents.values() { culture[agent.preference] += 1; specialties[agent.specialty] += 1; }
        let wealth_sum: u128 = wealth.iter().map(|&x| u128::from(x)).sum();
        let weighted: u128 = wealth.iter().enumerate().map(|(i, &x)| (i as u128 + 1) * u128::from(x)).sum();
        let gini = if wealth.is_empty() || wealth_sum == 0 { 0.0 }
            else { 2.0 * weighted as f64 / (wealth.len() as f64 * wealth_sum as f64) - (wealth.len() + 1) as f64 / wealth.len() as f64 };
        json!({"population":state.agents.len(), "culture":culture, "specialties":specialties,
            "gini":gini, "treasury":state.treasury.to_string(), "resource_input":state.resource_input.to_string(),
            "consumed":state.consumed.to_string(), "counters":state.counters, "events":state.events,
            "resources":state.resources.iter().map(|r| [r[0].to_string(), r[1].to_string()]).collect::<Vec<_>>(),
            "agents":state.agents.values().map(|a| json!({"id":a.id.to_string(), "parent":a.parent.map(|id| id.to_string()),
                "position":a.position, "goods":a.goods.map(|x| x.to_string()), "credits":a.credits.to_string(),
                "specialty":a.specialty, "preference":a.preference, "age":a.age.to_string(), "signal":a.signal})).collect::<Vec<_>>()})
    }
    pub fn cells(state: &AgentState) -> Vec<u8> {
        let mut cells = vec![0; state.space.len()];
        for agent in state.agents.values() { cells[agent.position] = agent.specialty as u8 + 1; }
        cells
    }
}

impl LawSystem for AgentLaw {
    type State = AgentState;
    type Workspace = ();
    type Baseline = AgentBaseline;
    type Observation = Value;
    fn stages(&self) -> &'static [&'static str] { &["resource renewal", "agent movement", "harvest trade communication and demography"] }
    fn workspace(&self, _: &Self::State) {}
    fn baseline(&self, state: &Self::State) -> Result<Self::Baseline, SimulationError> { Self::totals(state) }
    fn advance(&self, stage: usize, current: &Self::State, next: &mut Self::State, _: &mut (), generation: u64) -> Result<(), SimulationError> {
        match stage {
            0 => self.environment(current, next),
            1 => self.movement(current, next, generation),
            2 => self.interactions(current, next, generation.checked_add(1).ok_or_else(|| SimulationError::new("generation overflow"))?),
            _ => Err(SimulationError::new("unknown agent stage")),
        }
    }
    fn validate(&self, state: &Self::State, baseline: &Self::Baseline) -> Result<(), SimulationError> {
        if state.space != self.config.space || state.resources.len() != state.space.len() || state.agents.len() > self.config.max_population {
            return Err(SimulationError::new("invalid agent spatial state"));
        }
        let mut positions = BTreeSet::new();
        for (&id, agent) in &state.agents {
            if id == 0 || id != agent.id || id >= state.next_id || agent.position >= state.space.len()
                || !positions.insert(agent.position) || agent.specialty > 1 || agent.preference > 1 || agent.signal > 2
                || agent.parent.is_some_and(|parent| parent >= id)
            { return Err(SimulationError::new("invalid agent identity, traits, ancestry or occupancy")); }
        }
        let totals = Self::totals(state)?;
        if totals.credits != baseline.credits { return Err(SimulationError::new("agent currency conservation failed")); }
        if totals.material.checked_add(state.consumed) != baseline.material.checked_add(state.resource_input)
            || totals.material.checked_add(state.consumed).is_none()
        { return Err(SimulationError::new("agent material ledger failed")); }
        Ok(())
    }
    fn observe(&self, state: &Self::State) -> Result<Value, SimulationError> { Ok(self.observation(state)) }
}
