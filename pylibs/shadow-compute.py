import socket
import json
import os

class ShadowCompute:
    def __init__(self, socket_path="/tmp/shadow_compute.sock"):
        self.socket_path = socket_path
        if not os.path.exists(self.socket_path):
            raise ConnectionError(f"Shadow Daemon socket not found at {self.socket_path}. Is the service running?")

    def _send_task(self, task_dict):
        """Internal method to send JSON over the Unix socket."""
        # Convert Python dictionary to JSON string, then to bytes
        payload = json.dumps(task_dict).encode('utf-8')
        
        # Connect to the Rust Daemon
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
            client.connect(self.socket_path)
            # Send the length of the payload first (8 bytes, little-endian)
            # (Note: You may need to adapt your Rust Daemon's Tokio frame reader to match this, 
            # or simply send a newline-terminated JSON string depending on your setup).
            client.sendall(payload + b'\n')
            
            # Wait for response
            response = client.recv(1024)
            return response.decode('utf-8').strip()

    # ==========================================
    # PUBLIC API METHODS
    # ==========================================
    
    def ping(self, message="System Check"):
        """Test the connection to the Vulkan hardware."""
        return self._send_task({
            "Ping": {
                "message": message
            }
        })

    def process_image(self, path):
        """Format a single image for the neural network."""
        return self._send_task({
            "ProcessImage": {
                "path": os.path.abspath(path)
            }
        })

    def process_dataset(self, dir_path):
        """Batch process an entire directory via the AVX2/Vulkan pipeline."""
        return self._send_task({
            "ProcessDataset": {
                "dir_path": os.path.abspath(dir_path)
            }
        })
