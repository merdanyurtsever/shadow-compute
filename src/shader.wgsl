@group(0) @binding(0) var<storage, read_write> pixels: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>
) {
    // Unroll the 2D hardware grid back into a flat 1D memory index.
    // We multiply by 64u because our workgroup_size is 64.
    let index = global_id.y * (num_workgroups.x * 64u) + global_id.x;

    // HARDWARE SAFETY: Prevent VRAM segfaults at the edge of the grid overhang
    if (index >= arrayLength(&pixels)) {
        return;
    }

    let pixel = pixels[index];

    // Unpack Little-Endian RGBA 
    let r = f32(pixel & 0xFFu);
    let g = f32((pixel >> 8u) & 0xFFu);
    let b = f32((pixel >> 16u) & 0xFFu);
    let a = pixel & 0xFF000000u; 

    // Standard Luminance Formula
    let gray = u32(0.299 * r + 0.587 * g + 0.114 * b);
    
    // Repack the pixel
    pixels[index] = a | (gray << 16u) | (gray << 8u) | gray;
}
