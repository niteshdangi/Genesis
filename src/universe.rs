use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use crate::{agents::{AgentConfig, AgentLaw}, chemistry::{Chemistry, ChemistryConfig, ChemistryState},
    engine::Engine, law_system::SimulationError, laws::Laws, persistence::Checkpoint,
    polymers::{PolymerConfig, PolymerLaw}, world::{Cell, HEIGHT, WIDTH, World}};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "config", rename_all = "snake_case", deny_unknown_fields)]
pub enum UniverseSpec {
    Cellular(Laws),
    Chemistry(ChemistryConfig),
    Polymers(PolymerConfig),
    Agents(AgentConfig),
    Matter(crate::matter::MatterConfig),
}

impl UniverseSpec {
    pub fn named(id: &str) -> Result<Self, SimulationError> {
        Ok(match id {
            "conway" => Self::Cellular(Laws::default()),
            "cyclic" => Self::Cellular(Laws::Cyclic),
            "transport" => Self::Cellular(Laws::Transport),
            "chemistry" => Self::Chemistry(ChemistryConfig::default()),
            "polymers" => Self::Polymers(PolymerConfig::default()),
            "agents" => Self::Agents(AgentConfig::default()),
            "matter" => Self::Matter(crate::matter::MatterConfig::default()),
            _ => return Err(SimulationError::new("select matter, conway, cyclic, transport, chemistry, polymers or agents")),
        })
    }
    pub fn id(&self) -> &'static str {
        match self {
            Self::Cellular(laws) => laws.id(), Self::Chemistry(_) => "chemistry",
            Self::Polymers(_) => "polymers", Self::Agents(_) => "agents",
            Self::Matter(_) => "matter",
        }
    }
    pub fn validate(&self) -> Result<(), SimulationError> {
        match self {
            Self::Cellular(Laws::Conway { birth, survival }) if (*birth | *survival) & !0x1ff != 0 => Err(SimulationError::new("invalid Conway masks")),
            Self::Cellular(_) => Ok(()),
            Self::Chemistry(config) => Chemistry::new(config.clone()).map(|_| ()),
            Self::Polymers(config) => config.validate(),
            Self::Agents(config) => config.validate(),
            Self::Matter(config) => config.validate(),
        }
    }
}

enum Runtime {
    Cellular(Engine<Laws>),
    Chemistry(Box<Engine<Chemistry>>),
    Polymers(Box<Engine<PolymerLaw>>),
    Agents(Box<Engine<AgentLaw>>),
    Matter(Box<Engine<crate::matter::MatterLaw>>),
}

pub struct Universe { spec: UniverseSpec, runtime: Runtime }

impl Universe {
    pub fn new(id: &str) -> Result<Self, SimulationError> { Self::from_spec(UniverseSpec::named(id)?) }
    pub fn from_spec(spec: UniverseSpec) -> Result<Self, SimulationError> {
        spec.validate()?;
        let runtime = match &spec {
            UniverseSpec::Cellular(laws) => Runtime::Cellular(Engine::new(initial_world(*laws), *laws)?),
            UniverseSpec::Chemistry(config) => {
                let energy = (0..WIDTH * HEIGHT).map(|position| ((position % WIDTH * 13 + position / WIDTH * 7) % 11) as u64).collect();
                Runtime::Chemistry(Box::new(Engine::new(ChemistryState::new(initial_world(Laws::Transport), energy)?, Chemistry::new(config.clone())?)?))
            }
            UniverseSpec::Polymers(config) => {
                let law = PolymerLaw::new(config.clone())?;
                Runtime::Polymers(Box::new(Engine::new(law.initial()?, law)?))
            }
            UniverseSpec::Agents(config) => {
                let law = AgentLaw::new(config.clone())?;
                Runtime::Agents(Box::new(Engine::new(law.initial()?, law)?))
            }
            UniverseSpec::Matter(config) => {
                let law=crate::matter::MatterLaw::new(config.clone())?;
                Runtime::Matter(Box::new(Engine::new(law.initial()?,law)?))
            }
        };
        Ok(Self { spec, runtime })
    }
    pub fn from_chemistry_state(config: ChemistryConfig, state: ChemistryState) -> Result<Self, SimulationError> {
        let engine = Engine::new(state, Chemistry::new(config.clone())?)?;
        Ok(Self { spec: UniverseSpec::Chemistry(config), runtime: Runtime::Chemistry(Box::new(engine)) })
    }
    pub fn restore(checkpoint: Checkpoint) -> Result<Self, SimulationError> {
        checkpoint.verify()?;
        let spec = checkpoint.payload.spec;
        spec.validate()?;
        let archive = checkpoint.payload.engine;
        let runtime = match &spec {
            UniverseSpec::Cellular(laws) => Runtime::Cellular(Engine::restore(*laws, archive)?),
            UniverseSpec::Chemistry(config) => Runtime::Chemistry(Box::new(Engine::restore(Chemistry::new(config.clone())?, archive)?)),
            UniverseSpec::Polymers(config) => Runtime::Polymers(Box::new(Engine::restore(PolymerLaw::new(config.clone())?, archive)?)),
            UniverseSpec::Agents(config) => Runtime::Agents(Box::new(Engine::restore(AgentLaw::new(config.clone())?, archive)?)),
            UniverseSpec::Matter(config) => Runtime::Matter(Box::new(Engine::restore(crate::matter::MatterLaw::new(config.clone())?,archive)?)),
        };
        Ok(Self { spec, runtime })
    }
    pub fn checkpoint(&self) -> Result<Checkpoint, SimulationError> {
        let archive = match &self.runtime {
            Runtime::Cellular(engine) => engine.archive()?, Runtime::Chemistry(engine) => engine.archive()?,
            Runtime::Polymers(engine) => engine.archive()?, Runtime::Agents(engine) => engine.archive()?,
            Runtime::Matter(engine) => engine.archive()?,
        };
        Checkpoint::new(self.spec.clone(), archive)
    }
    pub fn spec(&self) -> &UniverseSpec { &self.spec }
    pub fn fork_with(&self, spec: UniverseSpec) -> Result<Self, SimulationError> {
        if self.id() != spec.id() { return Err(SimulationError::new("interventions cannot replace the universe family")); }
        spec.validate()?;
        let runtime=match (&self.runtime,&spec) {
            (Runtime::Cellular(e),UniverseSpec::Cellular(laws))=>Runtime::Cellular(e.fork_with(*laws)?),
            (Runtime::Chemistry(e),UniverseSpec::Chemistry(c))=>Runtime::Chemistry(Box::new(e.fork_with(Chemistry::new(c.clone())?)?)),
            (Runtime::Polymers(e),UniverseSpec::Polymers(c))=>Runtime::Polymers(Box::new(e.fork_with(PolymerLaw::new(c.clone())?)?)),
            (Runtime::Agents(e),UniverseSpec::Agents(c))=>Runtime::Agents(Box::new(e.fork_with(AgentLaw::new(c.clone())?)?)),
            (Runtime::Matter(e),UniverseSpec::Matter(c))=>Runtime::Matter(Box::new(e.fork_with(crate::matter::MatterLaw::new(c.clone())?)?)),
            _=>return Err(SimulationError::new("incompatible runtime for fork")),
        };
        Ok(Self {spec,runtime})
    }
    pub fn fork_current(&self)->Result<Self,SimulationError> { self.fork_with(self.spec.clone()) }
    pub fn semantics(&self)->&'static str {
        match &self.spec {UniverseSpec::Matter(config)=>config.semantics(),_=>crate::persistence::SEMANTICS}
    }
    pub fn digest(&self)->Result<String,SimulationError> {
        match (&self.runtime,&self.spec) {
            (Runtime::Matter(e),UniverseSpec::Matter(c))=>crate::matter::digest(e,c),
            _=>Ok(self.checkpoint()?.digest),
        }
    }
    pub fn save_checkpoint(&self,path:&std::path::Path)->Result<u64,SimulationError> {
        match (&self.runtime,&self.spec) {
            (Runtime::Matter(e),UniverseSpec::Matter(c))=>crate::storage::checkpoint::save(e,c,path),
            _=>{self.checkpoint()?.save(path)?;Ok(std::fs::metadata(path).map_err(crate::persistence::io_error)?.len())},
        }
    }
    pub fn load_checkpoint(path:&std::path::Path)->Result<Self,SimulationError> {
        if crate::storage::checkpoint::is_native(path)? {
            let (engine,config,_)=crate::storage::checkpoint::load(path)?;
            Ok(Self {spec:UniverseSpec::Matter(config),runtime:Runtime::Matter(Box::new(engine))})
        } else { Self::restore(Checkpoint::load(path)?) }
    }
    pub fn checkpoint_download(&self)->Result<(Vec<u8>,&'static str),SimulationError> {
        match (&self.runtime,&self.spec) {
            (Runtime::Matter(e),UniverseSpec::Matter(c))=>Ok((crate::storage::checkpoint::bundle(e,c)?,"application/octet-stream")),
            _=>Ok((serde_json::to_vec(&self.checkpoint()?).map_err(crate::persistence::json_error)?,"application/json")),
        }
    }
    pub fn restore_bundle(bytes:&[u8])->Result<Self,SimulationError> {
        let (engine,config,_)=crate::storage::checkpoint::load_bundle(bytes)?;
        Ok(Self {spec:UniverseSpec::Matter(config),runtime:Runtime::Matter(Box::new(engine))})
    }
    pub fn checkpoint_name(&self)->&'static str { if self.id()=="matter" {"initial.gns"} else {"initial.json"} }
    pub fn perturb_conformation(&self,id:u64,conformation:u8)->Result<Self,SimulationError> {
        if conformation>1{return Err(SimulationError::new("conformation must be 0 or 1"));}
        let (Runtime::Matter(engine),UniverseSpec::Matter(config))=(&self.runtime,&self.spec) else{return Err(SimulationError::new("material perturbations require a Matter checkpoint"));};
        let mut state=engine.world().clone();
        let mut particle=state.particle(id).ok_or_else(||SimulationError::new("unknown perturbation particle"))?;
        let before=particle.conformation;
        if before==conformation{return self.fork_current();}
        let energy=config.materials[particle.material as usize].conformation_energy;
        if conformation==1 {
            if u64::from(particle.excitation)+u64::from(state.heat(particle.position))<u64::from(energy){return Err(SimulationError::new("perturbation lacks locally accounted energy"));}
            let amount=particle.excitation.min(energy);particle.excitation-=amount;state.spend_heat(particle.position,energy-amount)?;
        }else{state.add_heat(particle.position,energy)?;}
        particle.conformation=conformation;state.write_particle(particle)?;
        state.events.clear();state.events_truncated=false;
        state.event(crate::matter::MatterEvent {tick:engine.generation(),kind:crate::matter::EventKind::Intervention,
            particles:[id,0],positions:[particle.position,particle.position],before:[before as u32,0],after:[conformation as u32,0],energy},config)?;
        state.refresh_events()?;
        let parts=engine.binary_parts_with(&state)?;
        let replacement=Engine::restore_parts(crate::matter::MatterLaw::new(config.clone())?,parts,state)?;
        Ok(Self {spec:self.spec.clone(),runtime:Runtime::Matter(Box::new(replacement))})
    }
    pub fn material_events(&self)->Option<(&[crate::matter::MatterEvent],bool)> {
        match &self.runtime { Runtime::Matter(e)=>Some((&e.world().events,e.world().events_truncated)),_=>None }
    }
    pub fn material_state(&self)->Option<&crate::matter::MatterState> {
        match &self.runtime {Runtime::Matter(engine)=>Some(engine.world()),_=>None}
    }
    pub(crate) fn edit_material_state<F>(&self, edit:F)->Result<Self,SimulationError>
    where F:FnOnce(&mut crate::matter::MatterState,&crate::matter::MatterConfig,u64)->Result<(),SimulationError> {
        let (Runtime::Matter(engine),UniverseSpec::Matter(config))=(&self.runtime,&self.spec)
            else{return Err(SimulationError::new("material intervention requires a Matter world"));};
        let mut state=engine.world().clone();
        state.events.clear();state.events_truncated=false;
        edit(&mut state,config,engine.generation())?;
        state.refresh_events()?;
        let parts=engine.binary_parts_with(&state)?;
        let replacement=Engine::restore_parts(crate::matter::MatterLaw::new(config.clone())?,parts,state)?;
        Ok(Self {spec:self.spec.clone(),runtime:Runtime::Matter(Box::new(replacement))})
    }
    pub fn id(&self) -> &'static str { self.spec.id() }
    pub fn generation(&self) -> u64 {
        match &self.runtime {
            Runtime::Cellular(e) => e.generation(), Runtime::Chemistry(e) => e.generation(),
            Runtime::Polymers(e) => e.generation(), Runtime::Agents(e) => e.generation(),
            Runtime::Matter(e) => e.generation(),
        }
    }
    pub fn step(&mut self) -> Result<(), SimulationError> {
        match &mut self.runtime {
            Runtime::Cellular(e) => e.step(), Runtime::Chemistry(e) => e.step(),
            Runtime::Polymers(e) => e.step(), Runtime::Agents(e) => e.step(),
            Runtime::Matter(e) => e.step(),
        }
    }
    pub fn skip_quiescent(&mut self,steps:u64)->Result<bool,SimulationError> {
        match &mut self.runtime {Runtime::Matter(engine)=>engine.skip_quiescent(steps),
            _=>Err(SimulationError::new("exact quiescent skips are supported only for Matter worlds"))}
    }
    pub fn metrics(&self) -> Value {
        match &self.runtime {
            Runtime::Cellular(e) => json!({"populations":e.observation()}),
            Runtime::Chemistry(e) => {
                let o = e.observation();
                json!({"particle_count":o.particles, "free_particles":o.free_particles,
                    "free_energy":o.free_energy.to_string(), "bond_energy":o.bond_energy.to_string(), "total_energy":o.total_energy.to_string(),
                    "molecule_count":o.molecules.len(), "largest_molecule":o.largest_molecule, "size_distribution":o.size_distribution,
                    "reactions":{"formation":o.reactions.formation.to_string(), "dissociation":o.reactions.dissociation.to_string(), "exchange":o.reactions.exchange.to_string()}})
            }
            Runtime::Polymers(e) => {
                let mut metrics = e.observation().clone();
                if let Some(object) = metrics.as_object_mut() { for key in ["chains", "energy", "events"] { object.remove(key); } }
                metrics
            }
            Runtime::Agents(e) => {
                let mut metrics = e.observation().clone();
                if let Some(object) = metrics.as_object_mut() { for key in ["agents", "resources", "events"] { object.remove(key); } }
                metrics
            }
            Runtime::Matter(e) => {
                e.laws().metrics(e.world(),e.observation())
            }
        }
    }
    pub fn events(&self) -> &[Value] {
        match &self.runtime {
            Runtime::Chemistry(e) => e.world().events(),
            Runtime::Polymers(e) => &e.world().events, Runtime::Agents(e) => &e.world().events, _ => &[],
        }
    }
    pub fn snapshot(&self, running: bool, error: Option<&str>) -> Value {
        if let Runtime::Matter(e)=&self.runtime { return self.snapshot_slice(e.world().space.depth/2,running,error).unwrap_or_else(|e|json!({"error":e.to_string()})); }
        let (width, height, cells, rule, chemistry, polymers, agents) = match &self.runtime {
            Runtime::Cellular(e) => (WIDTH, HEIGHT, e.world().cells().to_vec(),
                match &self.spec { UniverseSpec::Cellular(laws) => laws.label(), _ => String::new() }, Value::Null, Value::Null, Value::Null),
            Runtime::Chemistry(e) => {
                let state = e.world();
                let o = e.observation();
                let mut chemistry = self.metrics();
                chemistry["valences"] = json!(o.valences);
                chemistry["energy"] = json!(state.energy().iter().map(u64::to_string).collect::<Vec<_>>());
                chemistry["particles"] = json!(state.particles().iter().map(|p| json!({"id":p.id,"kind":p.kind as u8,"position":p.position})).collect::<Vec<_>>());
                chemistry["bonds"] = json!(state.bonds().map(|b| json!({"a":b.a,"b":b.b,
                    "from":state.particle(b.a).expect("validated bond").position,"to":state.particle(b.b).expect("validated bond").position,"energy":b.energy.to_string()})).collect::<Vec<_>>());
                chemistry["molecules"] = json!(o.molecules);
                (WIDTH, HEIGHT, state.grid().cells().to_vec(), "Artificial chemistry".into(), chemistry, Value::Null, Value::Null)
            }
            Runtime::Polymers(e) => (e.world().space.width, e.world().space.height, PolymerLaw::cells(e.world()),
                "Local templated polymer assembly".into(), Value::Null, e.observation().clone(), Value::Null),
            Runtime::Agents(e) => (e.world().space.width, e.world().space.height, AgentLaw::cells(e.world()),
                "Resource exchange and agent demography".into(), Value::Null, Value::Null, e.observation().clone()),
            Runtime::Matter(_) => unreachable!("matter uses the slice inspector"),
        };
        let mut populations = [0usize; 4];
        for &cell in &cells { populations[cell as usize] += 1; }
        json!({"width":width,"height":height,"generation":self.generation().to_string(),"running":running,
            "law":self.id(),"rule":rule,"cells":cells,"populations":populations,"chemistry":chemistry,
            "polymers":polymers,"agents":agents,"metrics":self.metrics(),"error":error})
    }
    pub fn snapshot_slice(&self,z:u32,running:bool,error:Option<&str>)->Result<Value,SimulationError> {
        let Runtime::Matter(engine)=&self.runtime else {return Ok(self.snapshot(running,error));};
        let state=engine.world(); let space=state.space;
        if z>=space.depth {return Err(SimulationError::new("slice is outside world depth"));}
        let mut cells=Vec::new(); let mut heat=Vec::new(); let mut particles=Vec::new(); let mut bonds=Vec::new();
        for y in 0..space.height {for x in 0..space.width {
            let position=space.position([x,y,z]); heat.push(state.heat(position).to_string());
            if let Some(p)=state.particle(state.occupant(position)) {
                let local=y*space.width+x; cells.push((p.material%3)+1);
                particles.push(json!({"id":p.id.to_string(),"site":position,"position":local,"xyz":[x,y,z],
                    "material":p.material,"conformation":p.conformation,"excitation":p.excitation.to_string(),"orientation":p.orientation,
                    "bonds":p.bonds.map(|id|id.to_string())}));
                for other in p.bonds {if other>p.id {if let Some(q)=state.particle(other) {
                    let [qx,qy,qz]=space.xyz(q.position);
                    bonds.push(json!({"a":p.id.to_string(),"b":other.to_string(),"from":local,
                        "to":if qz==z {Some(qy*space.width+qx)} else {None},"from_xyz":[x,y,z],"to_xyz":[qx,qy,qz]}));
                }}}
            } else {cells.push(0);}
        }}
        let mut populations=[0usize;4]; for &cell in &cells {populations[cell as usize]+=1;}
        let preview:Vec<_>=state.events.iter().take(256).map(|event|serde_json::json!({"kind":format!("{:?}",event.kind).to_lowercase(),
            "generation":event.tick.to_string(),"particles":event.particles.map(|id|id.to_string()),"positions":event.positions,"energy":event.energy.to_string()})).collect();
        Ok(json!({"width":space.width,"height":space.height,"generation":engine.generation().to_string(),"running":running,"law":"matter",
            "rule":"Local 3D material physics","cells":cells,"populations":populations,"chemistry":null,"polymers":null,"agents":null,
            "matter":{"depth":space.depth,"z":z,"heat":heat,"particles":particles,"bonds":bonds,"events":preview,
                "preview_limit":256,"retained_events":state.events.len(),"events_truncated":state.events_truncated},"metrics":self.metrics(),"error":error}))
    }
}

fn initial_world(laws: Laws) -> World {
    let mut world = World::new();
    match laws {
        Laws::Conway { .. } => {
            for (x, y) in [(6, 5), (7, 6), (5, 7), (6, 7), (7, 7)] { world.set(x, y, Cell::A); }
        }
        Laws::Cyclic => {
            for y in 0..HEIGHT { for x in 0..WIDTH {
                if (x * 17 + y * 31) % 37 != 0 { world.set(x, y, [Cell::A, Cell::B, Cell::C][(x / 8 + y / 8) % 3]); }
            } }
        }
        Laws::Transport => {
            for y in HEIGHT / 4..HEIGHT * 3 / 4 { for x in WIDTH / 4..WIDTH * 3 / 4 {
                if (x * 17 + y * 31) % 7 < 2 { world.set(x, y, [Cell::A, Cell::B, Cell::C][(x / 8 + y / 8) % 3]); }
            } }
        }
    }
    world
}
