use serde::{Deserialize, Serialize};
use crate::law_system::SimulationError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Space3 { pub width: u32, pub height: u32, pub depth: u32 }
impl Default for Space3 { fn default() -> Self { Self { width: 32, height: 32, depth: 32 } } }
impl Space3 {
    pub fn validate(self) -> Result<(), SimulationError> {
        if [self.width,self.height,self.depth].iter().any(|&n| n < 8 || n > 256) || self.len64() > 8_388_608 {
            return Err(SimulationError::new("3D dimensions must be 8..256 with at most 8388608 sites"));
        }
        Ok(())
    }
    fn len64(self) -> u64 { u64::from(self.width) * u64::from(self.height) * u64::from(self.depth) }
    pub fn len(self) -> usize { self.len64() as usize }
    pub fn xyz(self, position: u32) -> [u32;3] { [position % self.width, position / self.width % self.height, position / (self.width*self.height)] }
    pub fn position(self, [x,y,z]: [u32;3]) -> u32 { (z*self.height+y)*self.width+x }
    pub fn shift(self, position: u32, offset: [i32;3]) -> u32 {
        let [x,y,z] = self.xyz(position);
        self.position([(x as i32+offset[0]).rem_euclid(self.width as i32) as u32,
            (y as i32+offset[1]).rem_euclid(self.height as i32) as u32,(z as i32+offset[2]).rem_euclid(self.depth as i32) as u32])
    }
    pub fn neighbor(self, position: u32, direction: usize) -> u32 {
        self.shift(position, [[1,0,0],[-1,0,0],[0,1,0],[0,-1,0],[0,0,1],[0,0,-1]][direction])
    }
    pub fn direction(self, a: u32, b: u32) -> Option<usize> { (0..6).find(|&d| self.neighbor(a,d)==b) }
    pub fn chunk(self, position: u32) -> (u32, usize) {
        let [x,y,z] = self.xyz(position);
        let nx = self.width.div_ceil(16); let ny = self.height.div_ceil(16);
        (((z/16)*ny+y/16)*nx+x/16, (((z%16)*16+y%16)*16+x%16) as usize)
    }
    pub fn chunk_position(self, chunk: u32, local: usize) -> Option<u32> {
        let nx = self.width.div_ceil(16); let ny = self.height.div_ceil(16);
        let x = chunk%nx*16+local as u32%16;
        let y = chunk/nx%ny*16+(local as u32/16)%16;
        let z = chunk/(nx*ny)*16+local as u32/256;
        (x<self.width && y<self.height && z<self.depth).then(|| self.position([x,y,z]))
    }
    pub fn chunk_count(self) -> usize { (self.width.div_ceil(16)*self.height.div_ceil(16)*self.depth.div_ceil(16)) as usize }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Execution {
    pub parallel: bool,
    pub gpu_fields: bool,
    pub cpu_fallback: bool,
    pub memory_limit_mib: u64,
    pub event_limit: usize,
    pub full_provenance: bool,
}
impl Default for Execution { fn default() -> Self { Self { parallel: false, gpu_fields: false, cpu_fallback: true,
    memory_limit_mib: 512, event_limit: 200_000, full_provenance: true } } }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Material {
    pub ports: u8,
    pub valence: u8,
    pub affinities: [u32;4],
    pub barrier: u32,
    pub conformation_energy: u32,
    pub switching_rate: u32,
    pub coupling: u32,
    pub catalytic_strength: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Reservoir { pub enabled: bool, pub input_per_site: u32, pub sink_per_site: u32, pub capacity: u32 }
impl Default for Reservoir { fn default() -> Self { Self { enabled: true, input_per_site: 2, sink_per_site: 1, capacity: 64 } } }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MatterConfig {
    pub space: Space3,
    pub seed: u64,
    pub particles: usize,
    pub initial_heat: u32,
    pub initial_excitation: u32,
    pub bond_energy: u32,
    pub binding_rate: u32,
    pub breaking_rate: u32,
    pub motion_rate: u32,
    pub exchange_rate: u32,
    pub reservoir: Reservoir,
    pub materials: [Material;4],
    pub execution: Execution,
    #[serde(default = "legacy_version", skip_serializing_if = "is_legacy_version")]
    pub law_version: u32,
}
fn legacy_version() -> u32 { 1 }
fn is_legacy_version(version: &u32) -> bool { *version == 1 }

// Schema-1 native checkpoints and v1 digests retain this exact field order.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct LegacyMatterConfig {
    pub space: Space3, pub seed: u64, pub particles: usize,
    pub initial_heat: u32, pub initial_excitation: u32, pub bond_energy: u32,
    pub binding_rate: u32, pub breaking_rate: u32, pub motion_rate: u32, pub exchange_rate: u32,
    pub reservoir: Reservoir, pub materials: [Material;4], pub execution: Execution,
}
impl From<LegacyMatterConfig> for MatterConfig {
    fn from(config: LegacyMatterConfig) -> Self {
        Self { space:config.space,seed:config.seed,particles:config.particles,initial_heat:config.initial_heat,
            initial_excitation:config.initial_excitation,bond_energy:config.bond_energy,binding_rate:config.binding_rate,
            breaking_rate:config.breaking_rate,motion_rate:config.motion_rate,exchange_rate:config.exchange_rate,
            reservoir:config.reservoir,materials:config.materials,execution:config.execution,law_version:1 }
    }
}
impl Default for MatterConfig {
    fn default() -> Self {
        Self { space: Space3::default(), seed: 1, particles: 6_000, initial_heat: 4, initial_excitation: 2,
            bond_energy: 2, binding_rate: 160_000, breaking_rate: 5_000, motion_rate: 700_000, exchange_rate: 300_000,
            reservoir: Reservoir::default(), execution: Execution::default(), law_version:2, materials: [
                Material { ports:63,valence:2,affinities:[70,90,30,20],barrier:4,conformation_energy:2,switching_rate:30_000,coupling:20_000,catalytic_strength:1 },
                Material { ports:63,valence:3,affinities:[90,50,80,40],barrier:5,conformation_energy:3,switching_rate:20_000,coupling:40_000,catalytic_strength:2 },
                Material { ports:15,valence:4,affinities:[30,80,80,60],barrier:6,conformation_energy:2,switching_rate:40_000,coupling:30_000,catalytic_strength:1 },
                Material { ports:63,valence:2,affinities:[20,40,60,90],barrier:3,conformation_energy:4,switching_rate:10_000,coupling:50_000,catalytic_strength:3 },
            ] }
    }
}
impl MatterConfig {
    pub fn semantics(&self) -> &'static str {
        match self.law_version { 1=>super::SEMANTICS, 2=>super::CONTACT_SEMANTICS, _=>super::VARIATION_SEMANTICS }
    }
    pub(crate) fn legacy(&self) -> LegacyMatterConfig {
        LegacyMatterConfig {space:self.space,seed:self.seed,particles:self.particles,initial_heat:self.initial_heat,
            initial_excitation:self.initial_excitation,bond_energy:self.bond_energy,binding_rate:self.binding_rate,
            breaking_rate:self.breaking_rate,motion_rate:self.motion_rate,exchange_rate:self.exchange_rate,
            reservoir:self.reservoir.clone(),materials:self.materials.clone(),execution:self.execution.clone()}
    }
    pub fn validate(&self) -> Result<(), SimulationError> {
        self.space.validate()?;
        if !matches!(self.law_version,1|2|3) { return Err(SimulationError::new("unsupported material law version; select 1, 2 or 3")); }
        if self.particles==0 || self.particles>1_000_000 || self.particles>self.space.len()
            || self.bond_energy==0 || self.bond_energy>1_000_000 || self.initial_heat>1_000_000 || self.initial_excitation>1_000_000
            || [self.binding_rate,self.breaking_rate,self.motion_rate,self.exchange_rate].iter().any(|&n| n>1_000_000)
            || self.reservoir.input_per_site>1_000_000 || self.reservoir.sink_per_site>1_000_000 || self.reservoir.capacity>1_000_000
            || self.execution.event_limit==0 || self.execution.event_limit>2_000_000
            || self.execution.memory_limit_mib<32 || self.execution.memory_limit_mib>65_536 {
            return Err(SimulationError::new("invalid matter configuration or resource budget"));
        }
        for material in &self.materials {
            if material.ports==0 || material.ports>63 || material.valence==0 || material.valence>material.ports.count_ones() as u8
                || material.affinities.iter().any(|&a| a>100) || material.barrier>1_000_000 || material.conformation_energy>1_000_000
                || material.switching_rate>1_000_000 || material.coupling>1_000_000 || material.catalytic_strength>1_000_000 {
                return Err(SimulationError::new("invalid material law table"));
            }
        }
        for a in 0..4 { for b in 0..4 { if self.materials[a].affinities[b]!=self.materials[b].affinities[a] {
            return Err(SimulationError::new("material affinities must be symmetric"));
        } } }
        let worst_case = self.space.chunk_count() as u64*4096*384 + self.particles as u64*80*4
            + self.execution.event_limit as u64*256 + self.particles as u64*4096;
        if worst_case > self.execution.memory_limit_mib*1024*1024 { return Err(SimulationError::new("world exceeds the configured conservative memory budget")); }
        Ok(())
    }
}
