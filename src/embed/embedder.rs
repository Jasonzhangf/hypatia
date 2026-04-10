use std::path::Path;
use std::sync::OnceLock;

use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use tokenizers::Tokenizer;

use crate::error::{HypatiaError, Result};

/// All-MiniLM-L6-v2 embedding dimension
pub const EMBEDDING_DIM: usize = 384;
const MAX_TEXT_CHARS: usize = 8000;

static MODEL_CACHE: OnceLock<Option<CachedModel>> = OnceLock::new();

struct CachedModel {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

/// Get or initialize cached model (singleton)
pub fn get_embedder() -> Option<&'static CachedModel> {
    MODEL_CACHE.get_or_init(|| {
        let model_dir = dirs::home_dir()
            .map(|h| h.join(".hypatia").join("models"));
        
        match model_dir {
            Some(dir) if dir.exists() => {
                match load_model(&dir) {
                    Ok(m) => { 
                        eprintln!("[hypatia] Embedding model loaded from {}", dir.display());
                        Some(m)
                    }
                    Err(e) => { 
                        eprintln!("[hypatia] Model load failed: {} - falling back to hash embedding", e);
                        None
                    }
                }
            }
            _ => {
                eprintln!("[hypatia] Model directory not found - using hash embedding fallback");
                None
            }
        }
    }).as_ref()
}

fn load_model(model_dir: &Path) -> Result<CachedModel> {
    let config_path = model_dir.join("config.json");
    let config: Config = serde_json::from_str(&std::fs::read_to_string(&config_path)?)
        .map_err(|e| HypatiaError::Validation(format!("Config parse error: {}", e)))?;
    
    let tokenizer = Tokenizer::from_file(model_dir.join("tokenizer.json"))
        .map_err(|e| HypatiaError::Validation(format!("Tokenizer error: {}", e)))?;
    
    let device = Device::Cpu;
    let safetensors_path = model_dir.join("model.safetensors");
    
    let vb = unsafe { 
        VarBuilder::from_mmaped_safetensors(&[safetensors_path], DTYPE, &device)
            .map_err(|e| HypatiaError::Validation(format!("Safetensors load error: {}", e)))?
    };
    
    let model = BertModel::load(vb, &config)
        .map_err(|e| HypatiaError::Validation(format!("BERT model load error: {}", e)))?;
    
    Ok(CachedModel { model, tokenizer, device })
}

/// Embed text to 384-dim vector. Uses BERT if model available, falls back to hash.
pub fn embed(text: &str) -> Vec<f32> {
    let truncated: String = text.chars().take(MAX_TEXT_CHARS).collect();
    
    if let Some(cached) = get_embedder() {
        match embed_with_model(cached, &truncated) {
            Ok(v) => return v,
            Err(e) => eprintln!("[hypatia] Embedding error: {} - using fallback", e)
        }
    }
    
    simple_hash_embed(&truncated)
}

fn embed_with_model(cached: &CachedModel, text: &str) -> Result<Vec<f32>> {
    let enc = cached.tokenizer.encode(text, true)
        .map_err(|e| HypatiaError::Validation(format!("Tokenize: {}", e)))?;
    
    let ids = enc.get_ids();
    let mask = enc.get_attention_mask();
    let types = enc.get_type_ids();
    
    if ids.is_empty() {
        return Ok(vec![0.0; EMBEDDING_DIM]);
    }
    
    let seq = ids.len();
    let ids_t = Tensor::from_vec(
        ids.iter().map(|&i| i as i64).collect::<Vec<_>>(),
        (1, seq),
        &cached.device
    ).map_err(|e| HypatiaError::Validation(e.to_string()))?;
    
    let mask_t = Tensor::from_vec(
        mask.iter().map(|&m| m as f32).collect::<Vec<_>>(),
        (1, seq),
        &cached.device
    ).map_err(|e| HypatiaError::Validation(e.to_string()))?;
    
    let types_t = Tensor::from_vec(
        types.iter().map(|&t| t as i64).collect::<Vec<_>>(),
        (1, seq),
        &cached.device
    ).map_err(|e| HypatiaError::Validation(e.to_string()))?;
    
    let embeddings = cached.model.forward(&ids_t, &types_t, Some(&mask_t))
        .map_err(|e| HypatiaError::Validation(e.to_string()))?;
    
    // Mean pooling with attention mask
    let mask_exp = mask_t.unsqueeze(2)
        .and_then(|t| t.expand((1, seq, EMBEDDING_DIM)))
        .map_err(|e| HypatiaError::Validation(e.to_string()))?;
    
    let masked = (&embeddings * &mask_exp)
        .map_err(|e| HypatiaError::Validation(e.to_string()))?;
    
    let sum_emb = masked.sum(1)
        .map_err(|e| HypatiaError::Validation(e.to_string()))?;
    
    let sum_mask = mask_t.sum(1)
        .and_then(|t| t.unsqueeze(1))
        .map_err(|e| HypatiaError::Validation(e.to_string()))?;
    
    let pooled = sum_emb.broadcast_div(&sum_mask)
        .map_err(|e| HypatiaError::Validation(e.to_string()))?;
    
    // L2 normalize
    let squeezed = pooled.squeeze(0)
        .map_err(|e| HypatiaError::Validation(e.to_string()))?;
    
    let vec_data = squeezed.to_vec1::<f32>()
        .map_err(|e| HypatiaError::Validation(e.to_string()))?;
    
    let norm = vec_data.iter().map(|x| x * x).sum::<f32>().sqrt();
    let normalized: Vec<f32> = if norm > 0.0 {
        vec_data.iter().map(|x| x / norm).collect()
    } else {
        vec_data
    };
    
    Ok(normalized)
}

/// Simple hash-based fallback embedding (deterministic, no model needed)
pub fn simple_hash_embed(text: &str) -> Vec<f32> {
    use sha2::{Digest, Sha256};
    
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let hash = hasher.finalize();
    
    let mut result = vec![0.0; EMBEDDING_DIM];
    for (i, byte) in hash.iter().cycle().take(EMBEDDING_DIM).enumerate() {
        result[i] = (*byte as f32) / 255.0 - 0.5;
    }
    
    let norm = result.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        result.iter_mut().for_each(|x| *x /= norm);
    }
    
    result
}

/// Cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if na > 0.0 && nb > 0.0 {
        dot / (na * nb)
    } else {
        0.0
    }
}
