use std::sync::mpsc;
use wgpu::util::DeviceExt;
use crate::{law_system::SimulationError,matter::Space3};

pub struct GpuFields { device:wgpu::Device, queue:wgpu::Queue, pipeline:wgpu::ComputePipeline }
impl GpuFields {
    pub fn new() -> Result<Self,SimulationError> {
        pollster::block_on(async {
            let instance=wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            let adapter=instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference:wgpu::PowerPreference::HighPerformance,compatible_surface:None,force_fallback_adapter:false
            }).await.ok_or_else(||SimulationError::new("no compute adapter available"))?;
            let (device,queue)=adapter.request_device(&wgpu::DeviceDescriptor {
                label:Some("Genesis material fields"),required_features:wgpu::Features::empty(),
                required_limits:wgpu::Limits::default(),memory_hints:wgpu::MemoryHints::MemoryUsage,
            },None).await.map_err(|e|SimulationError::new(e.to_string()))?;
            device.push_error_scope(wgpu::ErrorFilter::Validation);
            let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label:Some("Exact integer heat diffusion"),source:wgpu::ShaderSource::Wgsl(include_str!("heat.wgsl").into()),
            });
            let pipeline=device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label:Some("Genesis heat"),layout:None,module:&shader,entry_point:Some("main"),
                compilation_options:wgpu::PipelineCompilationOptions::default(),cache:None,
            });
            if let Some(error)=device.pop_error_scope().await { return Err(SimulationError::new(error.to_string())); }
            Ok(Self { device,queue,pipeline })
        })
    }
    pub fn diffuse(&self,heat:&[u32],space:Space3) -> Result<Vec<u32>,SimulationError> {
        if heat.len()!=space.len() { return Err(SimulationError::new("GPU heat input shape mismatch")); }
        let mut bytes=Vec::with_capacity(heat.len()*4);
        for &value in heat { bytes.extend_from_slice(&value.to_le_bytes()); }
        let size=bytes.len() as u64;
        if size>u64::from(self.device.limits().max_storage_buffer_binding_size) { return Err(SimulationError::new("heat field exceeds GPU binding limit")); }
        let source=self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label:Some("heat source"),contents:&bytes,usage:wgpu::BufferUsages::STORAGE });
        let target=self.device.create_buffer(&wgpu::BufferDescriptor { label:Some("heat target"),size,usage:wgpu::BufferUsages::STORAGE|wgpu::BufferUsages::COPY_SRC,mapped_at_creation:false });
        let flags=self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label:Some("overflow flag"),contents:&[0;4],usage:wgpu::BufferUsages::STORAGE|wgpu::BufferUsages::COPY_SRC });
        let mut dimensions=Vec::new();
        for value in [space.width,space.height,space.depth,heat.len() as u32] { dimensions.extend_from_slice(&value.to_le_bytes()); }
        let params=self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label:Some("dimensions"),contents:&dimensions,usage:wgpu::BufferUsages::UNIFORM });
        let bind=self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:Some("heat bindings"),layout:&self.pipeline.get_bind_group_layout(0),entries:&[
                wgpu::BindGroupEntry { binding:0,resource:source.as_entire_binding() },wgpu::BindGroupEntry { binding:1,resource:target.as_entire_binding() },
                wgpu::BindGroupEntry { binding:2,resource:flags.as_entire_binding() },wgpu::BindGroupEntry { binding:3,resource:params.as_entire_binding() },
            ],
        });
        let readback=self.device.create_buffer(&wgpu::BufferDescriptor { label:Some("heat readback"),size:size+4,usage:wgpu::BufferUsages::MAP_READ|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false });
        let mut encoder=self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label:Some("heat transport") });
        {
            let mut pass=encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label:Some("heat"),timestamp_writes:None });
            pass.set_pipeline(&self.pipeline); pass.set_bind_group(0,&bind,&[]);
            // A 2D dispatch stays within the workgroup-count limits for larger volumes.
            let groups=(heat.len() as u32).div_ceil(64); let x=groups.min(65_535); let y=groups.div_ceil(x);
            pass.dispatch_workgroups(x,y,1);
        }
        encoder.copy_buffer_to_buffer(&target,0,&readback,0,size);
        encoder.copy_buffer_to_buffer(&flags,0,&readback,size,4);
        self.queue.submit(Some(encoder.finish()));
        let slice=readback.slice(..); let (sender,receiver)=mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read,move |result|{ let _=sender.send(result); });
        self.device.poll(wgpu::Maintain::Wait);
        receiver.recv().map_err(|e|SimulationError::new(e.to_string()))?.map_err(|e|SimulationError::new(e.to_string()))?;
        let mapping=slice.get_mapped_range();
        let flag=u32::from_le_bytes(mapping[size as usize..size as usize+4].try_into().map_err(|_|SimulationError::new("GPU flag shape"))?);
        let result=mapping[..size as usize].chunks_exact(4).map(|value|u32::from_le_bytes([value[0],value[1],value[2],value[3]])).collect();
        drop(mapping); readback.unmap();
        if flag!=0 { return Err(SimulationError::new("GPU heat arithmetic overflow; generation not committed")); }
        Ok(result)
    }
}
