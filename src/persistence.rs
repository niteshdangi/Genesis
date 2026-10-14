use std::{fs::{self, File, OpenOptions}, io::{Read, Write}, path::Path, sync::atomic::{AtomicU64, Ordering}};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use crate::{engine::EngineArchive, law_system::SimulationError, universe::UniverseSpec};

pub const CHECKPOINT_VERSION: u32 = 1;
pub const SEMANTICS: &str = "genesis-staged-snapshot-v1";
pub const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;
static SERIAL: AtomicU64 = AtomicU64::new(0);

pub mod decimal_u128 {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &u128, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u128, D::Error> {
        String::deserialize(deserializer)?.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointPayload {
    pub schema: u32,
    pub semantics: String,
    pub spec: UniverseSpec,
    pub engine: EngineArchive,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub digest: String,
    pub payload: CheckpointPayload,
}

fn canonical(value: &Value, output: &mut Vec<u8>) -> Result<(), SimulationError> {
    match value {
        Value::Object(object) => {
            output.push(b'{');
            let mut keys: Vec<_> = object.keys().collect();
            keys.sort();
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 { output.push(b','); }
                serde_json::to_writer(&mut *output, key).map_err(json_error)?;
                output.push(b':');
                canonical(&object[key], output)?;
            }
            output.push(b'}');
        }
        Value::Array(array) => {
            output.push(b'[');
            for (index, item) in array.iter().enumerate() {
                if index != 0 { output.push(b','); }
                canonical(item, output)?;
            }
            output.push(b']');
        }
        _ => serde_json::to_writer(output, value).map_err(json_error)?,
    }
    Ok(())
}

pub fn json_error(error: serde_json::Error) -> SimulationError { SimulationError::new(error.to_string()) }
pub fn io_error(error: std::io::Error) -> SimulationError { SimulationError::new(error.to_string()) }

pub fn digest<T: Serialize>(value: &T) -> Result<String, SimulationError> {
    let value = serde_json::to_value(value).map_err(json_error)?;
    let mut bytes = Vec::new();
    canonical(&value, &mut bytes)?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

impl Checkpoint {
    pub fn new(spec: UniverseSpec, engine: EngineArchive) -> Result<Self, SimulationError> {
        let payload = CheckpointPayload { schema: CHECKPOINT_VERSION, semantics: SEMANTICS.into(), spec, engine };
        Ok(Self { digest: digest(&payload)?, payload })
    }
    pub fn verify(&self) -> Result<(), SimulationError> {
        if self.payload.schema != CHECKPOINT_VERSION || self.payload.semantics != SEMANTICS {
            return Err(SimulationError::new("unsupported checkpoint schema or execution semantics"));
        }
        if self.digest != digest(&self.payload)? {
            return Err(SimulationError::new("checkpoint digest mismatch"));
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, SimulationError> {
        let checkpoint: Self = read_json(path)?;
        checkpoint.verify()?;
        Ok(checkpoint)
    }
    pub fn save(&self, path: &Path) -> Result<(), SimulationError> {
        self.verify()?;
        write_new_json(path, self)
    }
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, SimulationError> {
    let file = File::open(path).map_err(io_error)?;
    let mut bytes = Vec::new();
    file.take(MAX_DOCUMENT_BYTES + 1).read_to_end(&mut bytes).map_err(io_error)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES { return Err(SimulationError::new("JSON document exceeds 64 MiB")); }
    serde_json::from_slice(&bytes).map_err(json_error)
}

/// Publish an immutable file only after its complete contents have been synced.
/// The exclusive hard link prevents replacing an existing artifact, including races.
pub fn write_new_json<T: Serialize>(path: &Path, value: &T) -> Result<(), SimulationError> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(io_error)?;
    let filename = path.file_name().ok_or_else(|| SimulationError::new("output must name a file"))?;
    let temporary = parent.join(format!(".{}.{}.{}.pending", filename.to_string_lossy(), std::process::id(), SERIAL.fetch_add(1, Ordering::Relaxed)));
    let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(io_error)?;
    let result = (|| {
        serde_json::to_writer_pretty(&mut file, value).map_err(json_error)?;
        file.write_all(b"\n").map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        if file.metadata().map_err(io_error)?.len() > MAX_DOCUMENT_BYTES {
            return Err(SimulationError::new("output JSON exceeds the checkpoint size limit"));
        }
        drop(file);
        fs::hard_link(&temporary, path).map_err(io_error)?;
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    result
}
