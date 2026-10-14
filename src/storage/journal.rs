use std::path::Path;
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use crate::{law_system::SimulationError,matter::MatterEvent};
use super::{compress,decompress,read_bounded,write_immutable,MAX_BINARY_BYTES};

#[derive(Serialize,Deserialize)]
pub struct EventBatch { pub schema:u32,pub generation:u64,pub events:Vec<MatterEvent>,pub truncated:bool }
pub fn save(path:&Path,generation:u64,events:&[MatterEvent],truncated:bool)->Result<String,SimulationError> {
    let batch=EventBatch {schema:1,generation,events:events.to_vec(),truncated};
    let raw=postcard::to_allocvec(&batch).map_err(|e|SimulationError::new(e.to_string()))?;
    let digest=format!("{:x}",Sha256::digest(&raw));
    write_immutable(path,&compress(&raw)?)?; Ok(digest)
}
pub fn load(path:&Path,digest:&str)->Result<EventBatch,SimulationError> {
    let raw=decompress(&read_bounded(path,MAX_BINARY_BYTES)?,MAX_BINARY_BYTES)?;
    if format!("{:x}",Sha256::digest(&raw))!=digest { return Err(SimulationError::new("event batch digest mismatch")); }
    let batch:EventBatch=postcard::from_bytes(&raw).map_err(|e|SimulationError::new(e.to_string()))?;
    if batch.schema!=1 { return Err(SimulationError::new("unsupported event batch schema")); }
    Ok(batch)
}
