use std::{collections::BTreeMap,fs,path::Path,sync::Arc};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use crate::{engine::{Engine,BinaryParts},law_system::SimulationError,matter::{MatterConfig,LegacyMatterConfig,MatterLaw,MatterState,Chunk},persistence::io_error};
use super::{compress,decompress,read_bounded,write_immutable,MAX_BINARY_BYTES};

const MANIFEST_MAGIC:&[u8;8]=b"GNST3D01";
const BUNDLE_MAGIC:&[u8;8]=b"GNSPACK1";
#[derive(Clone,Serialize,Deserialize)]
struct ChunkRef { key:u32,hash:String,bytes:u64 }
#[derive(Clone,Serialize,Deserialize)]
struct Manifest<C> { schema:u32,semantics:String,config:C,digest:String,parts:BinaryParts,chunks:Vec<ChunkRef> }
#[derive(Clone,Serialize,Deserialize)]
struct Bundle<C> { schema:u32,semantics:String,config:C,digest:String,parts:BinaryParts }
fn encode<T:Serialize>(value:&T)->Result<Vec<u8>,SimulationError> { postcard::to_allocvec(value).map_err(|e|SimulationError::new(e.to_string())) }
fn decode<T:serde::de::DeserializeOwned>(bytes:&[u8])->Result<T,SimulationError> { postcard::from_bytes(bytes).map_err(|e|SimulationError::new(e.to_string())) }
fn hash(bytes:&[u8])->String { format!("{:x}",Sha256::digest(bytes)) }
fn valid_hash(hash:&str)->bool { hash.len()==64 && hash.bytes().all(|b|b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) }
fn schema(bytes:&[u8])->Result<u32,SimulationError> {
    postcard::take_from_bytes::<u32>(bytes).map(|(schema,_)|schema).map_err(|e|SimulationError::new(e.to_string()))
}
fn manifest(bytes:&[u8])->Result<Manifest<MatterConfig>,SimulationError> {
    match schema(bytes)? {
        1=>{
            let old:Manifest<LegacyMatterConfig>=decode(bytes)?;
            Ok(Manifest {schema:old.schema,semantics:old.semantics,config:old.config.into(),
                digest:old.digest,parts:old.parts,chunks:old.chunks})
        }
        2|3=>decode(bytes),
        _=>Err(SimulationError::new("unsupported material checkpoint schema")),
    }
}
fn bundled(bytes:&[u8])->Result<Bundle<MatterConfig>,SimulationError> {
    match schema(bytes)? {
        1=>{
            let old:Bundle<LegacyMatterConfig>=decode(bytes)?;
            Ok(Bundle {schema:old.schema,semantics:old.semantics,config:old.config.into(),digest:old.digest,parts:old.parts})
        }
        2|3=>decode(bytes),
        _=>Err(SimulationError::new("unsupported material bundle schema")),
    }
}
fn parts_path(path:&Path)->Result<std::path::PathBuf,SimulationError> {
    let filename=path.file_name().ok_or_else(||SimulationError::new("checkpoint must name a file"))?;
    Ok(path.with_file_name(format!("{}.parts",filename.to_string_lossy())))
}

pub fn save(engine:&Engine<MatterLaw>,config:&MatterConfig,path:&Path)->Result<u64,SimulationError> {
    if path.exists() { return Err(SimulationError::new("checkpoint already exists")); }
    let directory=parts_path(path)?; fs::create_dir_all(&directory).map_err(io_error)?;
    let mut chunks=Vec::new(); let mut total=0u64;
    for (&key,chunk) in &engine.world().chunks {
        if chunk.empty() { continue; }
        let raw=encode(chunk.as_ref())?; let fingerprint=hash(&raw); let encoded=compress(&raw)?;
        let file=directory.join(format!("{fingerprint}.zst"));
        if !file.exists() { write_immutable(&file,&encoded)?; }
        else if hash(&decompress(&read_bounded(&file,256*1024)?,256*1024)?)!=fingerprint { return Err(SimulationError::new("existing chunk object is corrupt")); }
        let bytes=fs::metadata(&file).map_err(io_error)?.len();
        total=total.checked_add(bytes).ok_or_else(||SimulationError::new("checkpoint size overflow"))?;
        chunks.push(ChunkRef {key,hash:fingerprint,bytes});
    }
    let mut stripped=engine.world().clone(); stripped.chunks.clear(); stripped.events_json.clear();
    let manifest=Manifest {schema:config.law_version,semantics:config.semantics().into(),config:config.clone(),digest:crate::matter::digest(engine,config)?,
        parts:engine.binary_parts_with(&stripped)?,chunks};
    let raw=if config.law_version==1 {
        encode(&Manifest {schema:1,semantics:manifest.semantics,config:config.legacy(),digest:manifest.digest,
            parts:manifest.parts,chunks:manifest.chunks})?
    }else{encode(&manifest)?};
    let mut bytes=MANIFEST_MAGIC.to_vec(); bytes.extend(compress(&raw)?);
    write_immutable(path,&bytes)?;
    Ok(total+bytes.len() as u64)
}
pub fn load(path:&Path)->Result<(Engine<MatterLaw>,MatterConfig,String),SimulationError> {
    let bytes=read_bounded(path,MAX_BINARY_BYTES)?;
    if bytes.starts_with(BUNDLE_MAGIC) { return load_bundle(&bytes); }
    if !bytes.starts_with(MANIFEST_MAGIC) { return Err(SimulationError::new("unsupported material checkpoint header")); }
    let manifest=manifest(&decompress(&bytes[8..],MAX_BINARY_BYTES)?)?;
    if manifest.schema!=manifest.config.law_version || manifest.semantics!=manifest.config.semantics() || !valid_hash(&manifest.digest) {
        return Err(SimulationError::new("unsupported material checkpoint schema or semantics"));
    }
    manifest.config.validate()?;
    if manifest.chunks.len()>manifest.config.space.chunk_count() { return Err(SimulationError::new("too many checkpoint chunks")); }
    let mut state:MatterState=decode(&manifest.parts.state)?;
    if !state.chunks.is_empty() { return Err(SimulationError::new("material manifest contains inline spatial chunks")); }
    let directory=parts_path(path)?; let mut chunks=BTreeMap::new(); let mut budget=0u64;
    for entry in &manifest.chunks {
        if !valid_hash(&entry.hash) || entry.key as usize>=manifest.config.space.chunk_count() || entry.bytes>256*1024 {
            return Err(SimulationError::new("invalid checkpoint chunk reference"));
        }
        let packed=read_bounded(&directory.join(format!("{}.zst",entry.hash)),256*1024)?;
        if packed.len() as u64!=entry.bytes { return Err(SimulationError::new("chunk byte count mismatch")); }
        let raw=decompress(&packed,256*1024)?;
        budget=budget.checked_add(raw.len() as u64).ok_or_else(||SimulationError::new("chunk byte budget overflow"))?;
        if budget>manifest.config.execution.memory_limit_mib*1024*1024 { return Err(SimulationError::new("chunk load exceeds configured budget")); }
        if hash(&raw)!=entry.hash { return Err(SimulationError::new("chunk digest mismatch")); }
        let chunk:Chunk=decode(&raw)?;
        if chunks.insert(entry.key,Arc::new(chunk)).is_some() { return Err(SimulationError::new("duplicate checkpoint chunk index")); }
    }
    state.chunks=chunks; state.refresh_events()?;
    let config=manifest.config;
    let engine=Engine::restore_parts(MatterLaw::new(config.clone())?,manifest.parts,state)?;
    if crate::matter::digest(&engine,&config)?!=manifest.digest { return Err(SimulationError::new("material checkpoint logical digest mismatch")); }
    Ok((engine,config,manifest.digest))
}
pub fn bundle(engine:&Engine<MatterLaw>,config:&MatterConfig)->Result<Vec<u8>,SimulationError> {
    let bundle=Bundle {schema:config.law_version,semantics:config.semantics().into(),config:config.clone(),digest:crate::matter::digest(engine,config)?,
        parts:engine.binary_parts_with(engine.world())?};
    let raw=if config.law_version==1 {
        encode(&Bundle {schema:1,semantics:bundle.semantics,config:config.legacy(),digest:bundle.digest,parts:bundle.parts})?
    }else{encode(&bundle)?};
    if raw.len() as u64>MAX_BINARY_BYTES { return Err(SimulationError::new("checkpoint bundle exceeds memory budget")); }
    let mut result=BUNDLE_MAGIC.to_vec(); result.extend(compress(&raw)?); Ok(result)
}
pub fn load_bundle(bytes:&[u8])->Result<(Engine<MatterLaw>,MatterConfig,String),SimulationError> {
    if bytes.len() as u64>MAX_BINARY_BYTES || !bytes.starts_with(BUNDLE_MAGIC) { return Err(SimulationError::new("invalid material bundle")); }
    let bundle=bundled(&decompress(&bytes[8..],MAX_BINARY_BYTES)?)?;
    if bundle.schema!=bundle.config.law_version || bundle.semantics!=bundle.config.semantics() || !valid_hash(&bundle.digest) {
        return Err(SimulationError::new("unsupported material bundle schema or semantics"));
    }
    bundle.config.validate()?;
    let mut state:MatterState=decode(&bundle.parts.state)?; state.refresh_events()?;
    let config=bundle.config; let engine=Engine::restore_parts(MatterLaw::new(config.clone())?,bundle.parts,state)?;
    if crate::matter::digest(&engine,&config)?!=bundle.digest { return Err(SimulationError::new("bundle digest mismatch")); }
    Ok((engine,config,bundle.digest))
}
pub fn is_native(path:&Path)->Result<bool,SimulationError> {
    use std::io::Read;
    let mut header=[0;8]; let mut file=fs::File::open(path).map_err(io_error)?;
    match file.read_exact(&mut header) {
        Ok(())=>Ok(&header==MANIFEST_MAGIC || &header==BUNDLE_MAGIC),
        Err(error) if error.kind()==std::io::ErrorKind::UnexpectedEof=>Ok(false),
        Err(error)=>Err(io_error(error)),
    }
}
