// V29: Tensor Accelerator Support
//
// Provides tensor operation abstraction, model loading/unloading,
// and inference pipeline for the AI-Native OS.
//
// Features:
//   - TensorOp: typed tensor operations (MATMUL, CONV, RELU, SOFTMAX, ADD)
//   - ModelRegistry: tracks loaded ML models in GPU memory
//   - Inference pipeline: submit model + input -> workload_id
//   - Inference latency tracking (count, total_us, max_us)

// gpu_alloc/gpu_free are provided by the parent module (mod.rs)

// ── Tensor Operations ──────────────────────────────────────────────────────

/// Supported tensor operation types.
pub const TENSOR_MATMUL: u8 = 0;
pub const TENSOR_CONV: u8 = 1;
pub const TENSOR_RELU: u8 = 2;
pub const TENSOR_SOFTMAX: u8 = 3;
pub const TENSOR_ADD: u8 = 4;

/// Supported data types.
pub const DTYPE_F32: u8 = 0;
pub const DTYPE_F16: u8 = 1;
pub const DTYPE_INT8: u8 = 2;

/// Tensor operation descriptor.
#[derive(Clone, Copy)]
pub struct TensorOp {
    pub op_type: u8,   // MATMUL=0, CONV=1, RELU=2, SOFTMAX=3, ADD=4
    pub input_a: u64,  // GPU VA of first input tensor
    pub input_b: u64,  // GPU VA of second input (for binary ops)
    pub output: u64,   // GPU VA of output tensor
    pub m: u32,        // rows / batch dimension
    pub n: u32,        // columns / features dimension
    pub k: u32,        // inner dimension (for matmul: A[MxK] * B[KxN])
    pub dtype: u8,     // F32=0, F16=1, INT8=2
}

// ── Model Registry ─────────────────────────────────────────────────────────

pub(crate) const MAX_MODELS: usize = 8;

/// A loaded ML model stored in GPU memory.
#[derive(Clone, Copy)]
pub(crate) struct Model {
    pub model_id: u32,
    pub gpu_id: u32,
    pub weights_gpu_va: usize, // GPU VA where model weights are stored
    pub weights_size: usize,   // size of weights in bytes
    pub num_layers: u32,
    pub total_params: u64,
    pub in_use: bool,
    // Inference stats
    pub inf_count: u64,
    pub inf_total_us: u64,
    pub inf_max_us: u64,
}

pub(crate) static mut MODEL_REGISTRY: [Model; MAX_MODELS] = [Model {
    model_id: 0,
    gpu_id: 0,
    weights_gpu_va: 0,
    weights_size: 0,
    num_layers: 0,
    total_params: 0,
    in_use: false,
    inf_count: 0,
    inf_total_us: 0,
    inf_max_us: 0,
}; MAX_MODELS];

pub(crate) static mut MODEL_COUNT: usize = 0;

/// Load a model into GPU memory.
/// model_data: pointer to model binary (weights), model_len: size in bytes.
/// Returns model_id on success.
pub(crate) fn model_load(gpu_id: u32, model_data: *const u8, model_len: usize) -> Option<u32> {
    unsafe {
        if MODEL_COUNT >= MAX_MODELS {
            return None;
        }

        // Allocate GPU memory for the model weights (from parent mod.rs)
        let gpu_va = crate::ai::gpu_alloc(gpu_id, model_len)?;

        // Copy model data to GPU physical pages
        // (In a real system this would DMA directly to GPU memory)
        let region = &crate::ai::gpu_mem::GPU_MEM_REGIONS;
        for i in 0..crate::ai::gpu_mem::MAX_GPU_MEM_REGIONS {
            if region[i].in_use && region[i].gpu_va == gpu_va {
                let pages = &region[i].phys_pages;
                for j in 0..region[i].page_count {
                    let kva = crate::mem::sv39::pa_to_kva(pages[j]);
                    let src_off = j * 4096;
                    let copy_len = core::cmp::min(model_len - src_off, 4096);
                    if copy_len > 0 {
                        core::ptr::copy_nonoverlapping(
                            model_data.add(src_off),
                            kva as *mut u8,
                            copy_len,
                        );
                    }
                }
                break;
            }
        }

        // Estimate model parameters: assume 4 bytes per weight (F32)
        let total_params = (model_len as u64) / 4;
        let num_layers = ((total_params as u32) / 1024).max(1).min(128);

        let id = MODEL_COUNT as u32;
        MODEL_REGISTRY[MODEL_COUNT] = Model {
            model_id: id,
            gpu_id,
            weights_gpu_va: gpu_va,
            weights_size: model_len,
            num_layers,
            total_params,
            in_use: true,
            inf_count: 0,
            inf_total_us: 0,
            inf_max_us: 0,
        };
        MODEL_COUNT += 1;
        Some(id)
    }
}

/// Unload a model, freeing its GPU memory.
pub(crate) fn model_unload(model_id: u32) -> bool {
    unsafe {
        for i in 0..MODEL_COUNT {
            if MODEL_REGISTRY[i].in_use && MODEL_REGISTRY[i].model_id == model_id {
                // Free GPU memory (from parent mod.rs)
                crate::ai::gpu_free(MODEL_REGISTRY[i].gpu_id, MODEL_REGISTRY[i].weights_gpu_va);
                MODEL_REGISTRY[i].in_use = false;
                return true;
            }
        }
        false
    }
}

/// Find a model by ID.
pub(crate) fn model_find(model_id: u32) -> Option<&'static Model> {
    unsafe {
        for i in 0..MODEL_COUNT {
            if MODEL_REGISTRY[i].in_use && MODEL_REGISTRY[i].model_id == model_id {
                return Some(&MODEL_REGISTRY[i]);
            }
        }
        None
    }
}

/// Submit an inference job.
/// Returns a workload_id that can be used to track progress.
pub(crate) fn inference_submit(model_id: u32, input_tensor: u64, output_tensor: u64) -> Option<usize> {
    unsafe {
        let model = model_find(model_id)?;

        // Create a tensor operation for the inference
        // For simplicity, we create a single MATMUL op representing
        // the full forward pass. Real impl would create a graph of ops.
        let op = TensorOp {
            op_type: TENSOR_MATMUL,
            input_a: input_tensor,
            input_b: model.weights_gpu_va as u64,
            output: output_tensor,
            m: 1,     // batch size 1
            n: 512,   // hidden dimension
            k: 512,   // input dimension
            dtype: DTYPE_F32,
        };

        // Serialize the tensor op into a workload and submit to AI queue
        let pid = crate::sched::current_thread()
            .map(|t| unsafe { (*t).owner })
            .unwrap_or(0);

        let op_bytes = core::slice::from_raw_parts(
            &op as *const TensorOp as *const u8,
            core::mem::size_of::<TensorOp>(),
        );

        // Submit as a REALTIME priority inference workload
        let wl_id = crate::ai::ai_submit_with_data(pid, model.gpu_id, 3, 1, op_bytes);
        wl_id
    }
}

/// Get inference statistics for a model.
pub(crate) fn inference_stats(model_id: u32) -> Option<(u64, u64, u64)> {
    unsafe {
        for i in 0..MODEL_COUNT {
            if MODEL_REGISTRY[i].in_use && MODEL_REGISTRY[i].model_id == model_id {
                return Some((
                    MODEL_REGISTRY[i].inf_count,
                    MODEL_REGISTRY[i].inf_total_us,
                    MODEL_REGISTRY[i].inf_max_us,
                ));
            }
        }
        None
    }
}

/// Record inference latency for a model.
pub(crate) fn inference_record_latency(model_id: u32, elapsed_us: u64) {
    unsafe {
        for i in 0..MODEL_COUNT {
            if MODEL_REGISTRY[i].in_use && MODEL_REGISTRY[i].model_id == model_id {
                MODEL_REGISTRY[i].inf_count += 1;
                MODEL_REGISTRY[i].inf_total_us += elapsed_us;
                if elapsed_us > MODEL_REGISTRY[i].inf_max_us {
                    MODEL_REGISTRY[i].inf_max_us = elapsed_us;
                }
                break;
            }
        }
    }
}

/// Execute a tensor operation (simulated).
/// In a real implementation, this would program the GPU's tensor cores.
/// Here we simulate the computation for benchmarking/scheduling purposes.
pub(crate) fn tensor_op_execute(op: &TensorOp) {
    // Simulate tensor operation execution time based on dimensions
    let ops_estimate = match op.op_type {
        TENSOR_MATMUL => (op.m as u64) * (op.n as u64) * (op.k as u64) * 2,
        TENSOR_CONV => (op.m as u64) * (op.n as u64) * (op.k as u64) * 9,
        TENSOR_RELU => (op.m as u64) * (op.n as u64),
        TENSOR_SOFTMAX => (op.m as u64) * (op.n as u64) * 2,
        TENSOR_ADD => (op.m as u64) * (op.n as u64),
        _ => 1000,
    };

    // Simulated: spin for a short duration proportional to op count
    let spin_count = core::cmp::min((ops_estimate / 1000) as usize, 50000);
    for _ in 0..spin_count {
        unsafe { core::arch::asm!("nop"); }
    }
}

/// List models into a buffer.
pub(crate) fn model_list(buf: &mut [u8]) -> usize {
    unsafe {
        let mut pos = 0;
        for i in 0..MODEL_COUNT {
            if MODEL_REGISTRY[i].in_use {
                if pos + 24 > buf.len() {
                    break;
                }
                let m = &MODEL_REGISTRY[i];
                buf[pos..pos+4].copy_from_slice(&m.model_id.to_le_bytes());
                buf[pos+4..pos+8].copy_from_slice(&m.gpu_id.to_le_bytes());
                buf[pos+8..pos+12].copy_from_slice(&(m.weights_size as u32).to_le_bytes());
                buf[pos+12..pos+16].copy_from_slice(&m.num_layers.to_le_bytes());
                buf[pos+16..pos+24].copy_from_slice(&m.total_params.to_le_bytes());
                pos += 24;
            }
        }
        pos
    }
}
