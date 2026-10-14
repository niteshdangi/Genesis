pub mod checkpoint;
pub mod catalog;
pub mod journal;

use std::{fs::{self,OpenOptions},io::{Read,Write},path::Path,sync::atomic::{AtomicU64,Ordering}};
use crate::{law_system::SimulationError,persistence::io_error};
static SERIAL:AtomicU64=AtomicU64::new(0);
pub const MAX_BINARY_BYTES:u64=512*1024*1024;

pub fn write_immutable(path:&Path,bytes:&[u8]) -> Result<(),SimulationError> {
    if bytes.len() as u64>MAX_BINARY_BYTES { return Err(SimulationError::new("binary artifact exceeds storage budget")); }
    let parent=path.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(io_error)?;
    let temporary=parent.join(format!(".genesis-{}-{}.pending",std::process::id(),SERIAL.fetch_add(1,Ordering::Relaxed)));
    let mut file=OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(io_error)?;
    let result=(|| {
        file.write_all(bytes).map_err(io_error)?; file.sync_all().map_err(io_error)?; drop(file);
        fs::hard_link(&temporary,path).map_err(io_error)?;
        #[cfg(unix)]
        fs::File::open(parent).and_then(|file|file.sync_all()).map_err(io_error)?;
        Ok(())
    })();
    let _=fs::remove_file(temporary); result
}
pub fn read_bounded(path:&Path,limit:u64) -> Result<Vec<u8>,SimulationError> {
    let mut bytes=Vec::new();
    fs::File::open(path).map_err(io_error)?.take(limit+1).read_to_end(&mut bytes).map_err(io_error)?;
    if bytes.len() as u64>limit { return Err(SimulationError::new("binary artifact exceeds read budget")); }
    Ok(bytes)
}
pub fn compress(bytes:&[u8]) -> Result<Vec<u8>,SimulationError> { zstd::stream::encode_all(bytes,3).map_err(io_error) }
pub fn decompress(bytes:&[u8],limit:u64) -> Result<Vec<u8>,SimulationError> {
    let decoder=zstd::stream::read::Decoder::new(bytes).map_err(io_error)?;
    let mut result=Vec::new(); decoder.take(limit+1).read_to_end(&mut result).map_err(io_error)?;
    if result.len() as u64>limit { return Err(SimulationError::new("decompressed payload exceeds budget")); }
    Ok(result)
}
