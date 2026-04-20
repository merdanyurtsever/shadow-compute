use anyhow::{Context, Result};
use shadow_compute::Task;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/shadow_compute.sock";

#[tokio::main]
async fn main() -> Result<()> {
	// 1. define the task
	let task = Task::ProcessDataset {
		dir_path: String::from("/home/merdan/Desktop/bitirme/bitirme/bitirme-proj/archive/G/G/"),
	};

	// 2. Serialize the task into raw bytes
	let payload = bincode::serialize(&task)?;

	// 3. connect to the daemon's socket
	let mut stream = UnixStream::connect(SOCKET_PATH)
		.await
		.context("Failed to connect to Shadow Daemon. Is it running?")?;

	//4. Fire the payload into the socket
	stream.write_all(&payload).await?;

	println!("Payload dispached to Shadow Matrix!");
	Ok(())
}
