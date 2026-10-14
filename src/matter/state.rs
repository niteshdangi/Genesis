use std::{collections::{BTreeMap,BTreeSet}, sync::Arc};
use serde::{Deserialize,Serialize};
use serde_json::Value;
use crate::{chemistry::seeded_value, law_system::SimulationError};
use super::{MatterConfig, Space3};

pub const PAGE_SIZE: usize = 1024;
pub const CHUNK_CELLS: usize = 4096;

#[derive(Clone,Default)]
pub struct SiteSet { words:Vec<u64> }
impl SiteSet {
    pub(crate) fn insert(&mut self,position:u32) {
        let index=position as usize/64;
        if self.words.len()<=index {self.words.resize(index+1,0);}
        self.words[index]|=1u64<<(position%64);
    }
    pub(crate) fn union_with(&mut self,other:&Self) {
        self.words.resize(self.words.len().max(other.words.len()),0);
        for (left,right) in self.words.iter_mut().zip(&other.words){*left|=*right;}
    }
    pub(crate) fn len(&self)->usize {self.words.iter().map(|word|word.count_ones() as usize).sum()}
    pub(crate) fn iter(&self)->impl Iterator<Item=u32>+'_ {
        self.words.iter().enumerate().flat_map(|(index,&word)| {
            let mut remaining=word;
            std::iter::from_fn(move|| {
                if remaining==0 {return None;}
                let bit=remaining.trailing_zeros();remaining&=remaining-1;
                Some(index as u32*64+bit)
            })
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Chunk { pub occupancy: Vec<u64>, pub heat: Vec<u32> }
impl Default for Chunk { fn default() -> Self { Self { occupancy:vec![0;CHUNK_CELLS],heat:vec![0;CHUNK_CELLS] } } }
impl Chunk { pub fn empty(&self) -> bool { self.occupancy.iter().all(|&n| n==0) && self.heat.iter().all(|&n| n==0) } }

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ParticlePage {
    pub positions: Vec<u32>,
    pub materials: Vec<u8>,
    pub conformations: Vec<u8>,
    pub orientations: Vec<u8>,
    pub excitation: Vec<u32>,
    pub bonds: Vec<[u64;6]>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum EventKind { Input, Output, Move, Rotate, Bind, Unbind, Switch, Charge, Dissipate, Transfer, Intervention, ContactCatalysis }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatterEvent {
    pub tick: u64,
    pub kind: EventKind,
    pub particles: [u64;2],
    pub positions: [u32;2],
    pub before: [u32;2],
    pub after: [u32;2],
    pub energy: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MatterState {
    pub space: Space3,
    pub pages: Vec<Arc<ParticlePage>>,
    pub chunks: BTreeMap<u32,Arc<Chunk>>,
    #[serde(with="crate::persistence::decimal_u128")]
    pub energy_input: u128,
    #[serde(with="crate::persistence::decimal_u128")]
    pub energy_output: u128,
    pub events: Vec<MatterEvent>,
    pub events_truncated: bool,
    #[serde(skip)]
    pub events_json: Vec<Value>,
    #[serde(skip)]
    pub heat_active: Arc<SiteSet>,
    #[serde(skip)]
    pub heat_dirty: Arc<SiteSet>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MatterBaseline {
    pub materials: [u64;4],
    #[serde(with="crate::persistence::decimal_u128")]
    pub energy: u128,
}

#[derive(Clone, Copy)]
pub struct Particle {
    pub id: u64, pub position: u32, pub material: u8, pub conformation: u8,
    pub orientation: u8, pub excitation: u32, pub bonds: [u64;6],
}
impl Particle { pub fn degree(self) -> usize { self.bonds.iter().filter(|&&n| n!=0).count() } }

impl MatterState {
    pub fn initial(config: &MatterConfig) -> Result<Self,SimulationError> {
        config.validate()?;
        let mut state = Self { space:config.space,pages:Vec::new(),chunks:BTreeMap::new(),energy_input:0,energy_output:0,
            events:Vec::new(),events_truncated:false,events_json:Vec::new(),heat_active:Arc::new(SiteSet::default()),heat_dirty:Arc::new(SiteSet::default()) };
        if config.initial_heat>0 { for position in 0..config.space.len() as u32 { state.set_heat(position,config.initial_heat); } }
        let mut positions: Vec<u32> = (0..config.space.len() as u32).collect();
        for index in 0..config.particles {
            let remaining=positions.len()-index;
            let selected=index+(seeded_value(config.seed,0,100,index as u64,0)%remaining as u64) as usize;
            positions.swap(index,selected);
            if index%PAGE_SIZE==0 { state.pages.push(Arc::new(ParticlePage::default())); }
            let page=Arc::make_mut(state.pages.last_mut().ok_or_else(|| SimulationError::new("missing particle page"))?);
            page.positions.push(positions[index]); page.materials.push((index%4) as u8); page.conformations.push(0);
            page.orientations.push((seeded_value(config.seed,0,101,index as u64,0)%3) as u8);
            page.excitation.push(config.initial_excitation); page.bonds.push([0;6]);
            state.set_occupant(positions[index],index as u64+1);
        }
        Ok(state)
    }
    pub fn count(&self) -> usize { self.pages.iter().map(|p|p.positions.len()).sum() }
    pub fn particle(&self,id:u64) -> Option<Particle> {
        let index=usize::try_from(id.checked_sub(1)?).ok()?;
        let page=self.pages.get(index/PAGE_SIZE)?; let local=index%PAGE_SIZE;
        Some(Particle { id,position:*page.positions.get(local)?,material:*page.materials.get(local)?,
            conformation:*page.conformations.get(local)?,orientation:*page.orientations.get(local)?,
            excitation:*page.excitation.get(local)?,bonds:*page.bonds.get(local)? })
    }
    pub fn write_particle(&mut self,p:Particle) -> Result<(),SimulationError> {
        let index=usize::try_from(p.id.checked_sub(1).ok_or_else(||SimulationError::new("zero particle ID"))?)
            .map_err(|_|SimulationError::new("particle ID out of range"))?;
        let page=Arc::make_mut(self.pages.get_mut(index/PAGE_SIZE).ok_or_else(||SimulationError::new("unknown particle page"))?);
        let local=index%PAGE_SIZE;
        if local>=page.positions.len() { return Err(SimulationError::new("unknown particle slot")); }
        page.positions[local]=p.position; page.materials[local]=p.material; page.conformations[local]=p.conformation;
        page.orientations[local]=p.orientation; page.excitation[local]=p.excitation; page.bonds[local]=p.bonds;
        Ok(())
    }
    pub fn occupant(&self,position:u32) -> u64 { let (key,local)=self.space.chunk(position); self.chunks.get(&key).map_or(0,|c|c.occupancy[local]) }
    pub fn heat(&self,position:u32) -> u32 { let (key,local)=self.space.chunk(position); self.chunks.get(&key).map_or(0,|c|c.heat[local]) }
    fn chunk_mut(&mut self,position:u32) -> (&mut Chunk,usize) {
        let (key,local)=self.space.chunk(position);
        (Arc::make_mut(self.chunks.entry(key).or_insert_with(||Arc::new(Chunk::default()))),local)
    }
    pub fn set_heat(&mut self,position:u32,value:u32) {
        if value==self.heat(position) { return; }
        let (chunk,local)=self.chunk_mut(position); chunk.heat[local]=value;
        let space=self.space;let dirty=Arc::make_mut(&mut self.heat_dirty);dirty.insert(position);
        for direction in 0..6{dirty.insert(space.neighbor(position,direction));}
    }
    pub fn rebuild_field_index(&mut self) {
        let mut active=SiteSet::default();
        for (&key,chunk) in &self.chunks {for (local,&energy) in chunk.heat.iter().enumerate() {
            if energy<2{continue;}
            if let Some(position)=self.space.chunk_position(key,local) {
                if (0..6).any(|direction|energy>self.heat(self.space.neighbor(position,direction)).saturating_add(1)){active.insert(position);}
            }
        }}
        self.heat_active=Arc::new(active);self.heat_dirty=Arc::new(SiteSet::default());
    }
    pub fn set_occupant(&mut self,position:u32,id:u64) {
        let (chunk,local)=self.chunk_mut(position); chunk.occupancy[local]=id;
    }
    pub fn add_heat(&mut self,position:u32,amount:u32) -> Result<(),SimulationError> {
        let value=self.heat(position).checked_add(amount).ok_or_else(||SimulationError::new("site heat overflow"))?;
        self.set_heat(position,value); Ok(())
    }
    pub fn spend_heat(&mut self,position:u32,amount:u32) -> Result<(),SimulationError> {
        let value=self.heat(position).checked_sub(amount).ok_or_else(||SimulationError::new("site heat overdraft"))?;
        self.set_heat(position,value); Ok(())
    }
    pub fn event(&mut self,event:MatterEvent,config:&MatterConfig) -> Result<(),SimulationError> {
        if self.events.len()>=config.execution.event_limit {
            if config.execution.full_provenance { return Err(SimulationError::new("full-provenance event budget exceeded; generation not committed")); }
            self.events_truncated=true; return Ok(());
        }
        self.events.push(event); Ok(())
    }
    pub fn refresh_events(&mut self) -> Result<(),SimulationError> {
        self.events_json=self.events.iter().take(256).map(|e|serde_json::json!({"kind":format!("{:?}",e.kind).to_lowercase(),
            "generation":e.tick.to_string(),"particles":e.particles.map(|id|id.to_string()),"positions":e.positions,
            "before":e.before,"after":e.after,"energy":e.energy.to_string()})).collect();
        Ok(())
    }
    pub fn material_counts(&self) -> [u64;4] {
        let mut counts=[0;4];
        for page in &self.pages { for &material in &page.materials { counts[material as usize]+=1; } }
        counts
    }
    pub fn energies(&self,config:&MatterConfig) -> Result<[u128;3],SimulationError> {
        let heat=self.chunks.values().flat_map(|c|&c.heat).map(|&n|u128::from(n)).sum();
        let mut excitation=0u128; let mut structure=0u128;
        for id in 1..=self.count() as u64 {
            let p=self.particle(id).ok_or_else(||SimulationError::new("missing particle"))?;
            excitation+=u128::from(p.excitation);
            structure+=u128::from(p.conformation)*u128::from(config.materials[p.material as usize].conformation_energy);
            for &other in &p.bonds { if other>id { structure+=u128::from(config.bond_energy); } }
        }
        Ok([heat,excitation,structure])
    }
    pub fn total_energy(&self,config:&MatterConfig) -> Result<u128,SimulationError> { Ok(self.energies(config)?.into_iter().sum()) }
    pub fn port_open(p:Particle,direction:usize,config:&MatterConfig) -> bool {
        let rotated=((direction/2+p.orientation as usize)%3)*2+direction%2;
        config.materials[p.material as usize].ports&(1<<rotated)!=0
    }
    pub fn validate_structure(&self,config:&MatterConfig) -> Result<(),SimulationError> {
        if self.space!=config.space || self.count()!=config.particles || self.pages.len()!=config.particles.div_ceil(PAGE_SIZE)
            || self.chunks.len()>self.space.chunk_count() || self.events.len()>config.execution.event_limit {
            return Err(SimulationError::new("invalid matter state dimensions or budget"));
        }
        if self.heat_active.iter().chain(self.heat_dirty.iter()).any(|p|p as usize>=self.space.len()) {
            return Err(SimulationError::new("invalid derived field index"));
        }
        for (index,page) in self.pages.iter().enumerate() {
            let expected=(config.particles-index*PAGE_SIZE).min(PAGE_SIZE);
            if [page.positions.len(),page.materials.len(),page.conformations.len(),page.orientations.len(),page.excitation.len(),page.bonds.len()]
                .iter().any(|&len|len!=expected) { return Err(SimulationError::new("invalid particle page arrays")); }
        }
        for (&key,chunk) in &self.chunks {
            if key as usize>=self.space.chunk_count() || chunk.occupancy.len()!=CHUNK_CELLS || chunk.heat.len()!=CHUNK_CELLS {
                return Err(SimulationError::new("invalid spatial chunk"));
            }
            for local in 0..CHUNK_CELLS {
                let position=self.space.chunk_position(key,local);
                if position.is_none() && (chunk.heat[local]!=0 || chunk.occupancy[local]!=0) { return Err(SimulationError::new("state outside world boundary")); }
                let id=chunk.occupancy[local];
                if id!=0 && self.particle(id).map(|p|p.position)!=position { return Err(SimulationError::new("untracked spatial occupant")); }
            }
        }
        for id in 1..=self.count() as u64 {
            let p=self.particle(id).ok_or_else(||SimulationError::new("missing particle slot"))?;
            // One occupancy slot cannot equal two IDs; the reverse lookup also proves position uniqueness.
            if p.material!=((id-1)%4) as u8 || p.conformation>1 || p.orientation>2 || p.position as usize>=self.space.len()
                || self.occupant(p.position)!=id || p.degree()>config.materials[p.material as usize].valence as usize {
                return Err(SimulationError::new("invalid particle identity, material, conformation or occupancy"));
            }
            let mut endpoints=BTreeSet::new();
            for (direction,&other) in p.bonds.iter().enumerate() {
                if other==0 { continue; }
                let q=self.particle(other).ok_or_else(||SimulationError::new("dangling matter bond"))?;
                if other==id || !endpoints.insert(other) || q.position!=self.space.neighbor(p.position,direction)
                    || q.bonds[direction^1]!=id || !Self::port_open(p,direction,config) || !Self::port_open(q,direction^1,config) {
                    return Err(SimulationError::new("invalid bond geometry, port or reciprocity"));
                }
            }
        }
        Ok(())
    }
}
