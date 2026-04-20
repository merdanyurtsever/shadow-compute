// src/gpu.rs
use rayon::prelude::*;
use anyhow::{Context, Result};
use wgpu::util::DeviceExt;
use wgpu::{Backends, Instance, InstanceDescriptor};

pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuContext {
    pub async fn benchmark_dataset(&self, dir_path: &str) -> Result<()> {
        println!("\n[SYS] Scanning dataset directory: {}", dir_path);
        
        let paths: Vec<_> = std::fs::read_dir(dir_path)?
            .filter_map(|res| res.ok())
            .map(|entry| entry.path())
            .filter(|p| {
                p.extension().map_or(false, |ext| {
                    ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") || ext.eq_ignore_ascii_case("png")
                })
            })
            .collect();

        if paths.is_empty() { anyhow::bail!("No images found in {}", dir_path); }
        let total_files = paths.len();
        println!("[SYS] Found {} images. Assembling Producer/Consumer Pipeline...\n", total_files);

        // 1. PRE-ALLOCATE THE UNIFIED MEMORY ARENA (33MB)
        let max_buffer_size = (3840 * 2160 * 4) as u64; 
        let storage_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Reusable Storage Buffer"),
            size: max_buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Reusable Staging Buffer"),
            size: max_buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Vision Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let compute_pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None, layout: None, module: &shader, entry_point: "main", compilation_options: Default::default(),
        });

        let bind_group_layout = compute_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: storage_buffer.as_entire_binding() }],
        });

        // 2. CREATE THE RING BUFFER (Channel)
        // We buffer up to 32 uncompressed images in RAM at a time.
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(u32, u32, Vec<u8>)>(32);

        // 3. START THE PRODUCER (AVX2 Hardware-Accelerated CPU Decoders)
        println!("[CPU] Spinning up Rayon AVX2 + mmap pool...");
        tokio::task::spawn_blocking(move || {
	    use rayon::prelude::*;
            paths.par_iter().for_each_with(tx, |tx, path| {
                // 1. Read the raw compressed bytes from the SSD into RAM
                if let Ok(file) = std::fs::File::open(path) {
                    // 2. Blast the JPEG decompression using AVX2 SIMD instructions
                    if let Ok(mmap) = unsafe { memmap2::MmapOptions::new().map(&file) } {
			if let Ok(image) = turbojpeg::decompress(&mmap, turbojpeg::PixelFormat::RGBA) {
			    let w = image.width as u32;
			    let h = image.height as u32;
			    let raw = image.pixels;
                    
                        // 3. Push to the Matrix
	                let _ = tx.blocking_send((w, h, raw)); 
			}
               	    }
		}
            });
        });

        // 4. START THE CONSUMER (iGPU Matrix)
        println!("[iGPU] Hardware queue armed. Waiting for CPU feed...\n");
        let mut total_pixels: u64 = 0;
        let mut frames_processed = 0;
        let gpu_start = std::time::Instant::now();

        // The iGPU just loops as fast as it can, pulling bytes out of the channel
        while let Some((width, height, raw_pixels)) = rx.recv().await {
            frames_processed += 1;
            let pixel_count = (width * height) as u64;
            let byte_count = pixel_count * 4;
            total_pixels += pixel_count;

            if byte_count > max_buffer_size { continue; } // Skip if larger than 4K

            print!("\r[Pipeline] Shredding frame {}/{} ({}x{}) ...", frames_processed, total_files, width, height);
            use std::io::Write;
            std::io::stdout().flush().unwrap();

            // HOT-SWAP VRAM
            self.queue.write_buffer(&storage_buffer, 0, bytemuck::cast_slice(&raw_pixels));

            let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
                cpass.set_pipeline(&compute_pipeline);
                cpass.set_bind_group(0, &bind_group, &[]);
                
                let workgroups = (pixel_count as u32 + 63) / 64; 
                let dim_x = if workgroups > 65535 { 256 } else { workgroups };
                let dim_y = if workgroups > 65535 { (workgroups + dim_x - 1) / dim_x } else { 1 };
                
                cpass.dispatch_workgroups(dim_x, dim_y, 1);
            }
            
            encoder.copy_buffer_to_buffer(&storage_buffer, 0, &staging_buffer, 0, byte_count);
            self.queue.submit(Some(encoder.finish()));

            // READBACK
            let buffer_slice = staging_buffer.slice(..byte_count);
            let (sender, receiver) = tokio::sync::oneshot::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |v| { let _ = sender.send(v); });
            self.device.poll(wgpu::Maintain::Wait);
            
            if let Ok(Ok(())) = receiver.await {
                staging_buffer.unmap(); 
            } else {
                anyhow::bail!("Vulkan pipeline readback failed");
            }
        }

        // 5. TELEMETRY
        let total_time = gpu_start.elapsed();
        let total_megapixels = total_pixels as f64 / 1_000_000.0;
        let pipeline_mps = total_megapixels / total_time.as_secs_f64();

        println!("\n\n====== PIPELINE RESULTS ======");
        println!("Images Processed:      {}", frames_processed);
        println!("Total Data:            {:.2} Megapixels", total_megapixels);
        println!("Total Wall-Clock Time: {:.2?}", total_time);
        println!("Pipeline Throughput:   {:.2} MP/s", pipeline_mps);
        println!("==============================\n");

        Ok(())
    }
            
    pub async fn init() -> Result<Self> {
        println!("[Vulkan API] Initializing compute instance...");
        let instance = Instance::new(InstanceDescriptor { backends: Backends::VULKAN, ..Default::default() });
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::LowPower, force_fallback_adapter: false, compatible_surface: None }).await.context("Failed to find adapter")?;
        let info = adapter.get_info();
        println!("[Vulkan API] Hardware hooked: {} ({:?})", info.name, info.backend);
        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor { label: Some("Shadow Device"), required_features: wgpu::Features::empty(), required_limits: wgpu::Limits::default(), ..Default::default() }, None).await?;
        println!("[Vulkan API] Compute queues initialized. Matrix online.");
        Ok(Self { device, queue })
    }
}
