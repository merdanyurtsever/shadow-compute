use anyhow::Result;
use shadow_compute::Task;
use std::fs;
use std::sync::Arc; // FIX: Import the Arc pointer
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;

#[path = "../gpu.rs"]
mod gpu;

const SOCKET_PATH: &str = "/tmp/shadow_compute.sock";

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("Initializing Shadow Compute Matrix...");
    // FIX: Wrap the initialized GPU in the Arc pointer
    let gpu = Arc::new(gpu::GpuContext::init().await?);

    let _ = fs::remove_file(SOCKET_PATH);
    let listener = UnixListener::bind(SOCKET_PATH)?;
    println!("Shadow Daemon listening on {}", SOCKET_PATH);

    loop {
        let (mut stream, _) = listener.accept().await?;
        
        // FIX: Clone the Arc pointer for the new thread (Zero overhead)
        let gpu_clone = Arc::clone(&gpu); 

        tokio::spawn(async move {
            let mut buffer = vec![0; 1024]; 
            
            match stream.read(&mut buffer).await {
                Ok(n) if n > 0 => {
                    match bincode::deserialize::<Task>(&buffer[..n]) {
                        // FIX: Pass the cloned pointer to the handler
                        Ok(task) => handle_task(task, &gpu_clone).await, 
                        Err(e) => eprintln!("Failed to decode payload: {}", e),
                    }
                }
                Ok(_) => println!("Connection closed by client."),
                Err(e) => eprintln!("Socket read error: {}", e),
            }
        });
    }
}

async fn handle_task(task: Task, gpu: &gpu::GpuContext) {
    match task {
        Task::Ping { message } => println!("[IDLE] Ping: {}", message),
        Task::ProcessImage { path } => { /* ... existing code ... */ },
        Task::ProcessDataset { dir_path } => {
            if let Err(e) = gpu.benchmark_dataset(&dir_path).await {
                eprintln!("[ERROR] Dataset processing failed: {}", e);
            }
        }
    }
}
