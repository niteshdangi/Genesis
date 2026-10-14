#[cfg(feature="gpu")]
mod fields;
#[cfg(feature="gpu")]
pub use fields::GpuFields;

#[cfg(not(feature="gpu"))]
pub struct GpuFields;
#[cfg(not(feature="gpu"))]
impl GpuFields {
    pub fn new() -> Result<Self,crate::law_system::SimulationError> {
        Err(crate::law_system::SimulationError::new("GPU fields require the gpu Cargo feature"))
    }
    pub fn diffuse(&self,_:&[u32],_:crate::matter::Space3) -> Result<Vec<u32>,crate::law_system::SimulationError> {
        Err(crate::law_system::SimulationError::new("GPU fields unavailable"))
    }
}
