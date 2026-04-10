use std::path::Path;

use candle_core::{Device, Tensor};
use candle_transformers::models::bert::BertModel;
use tokenizers::Tokenizer;

use crate::error::{HypatiaError, Result};

/// All-MiniLM-L6-v2 embedding dimension
pub const EMBEDDING_DIM: usize = 384;

/// BERT-style text embedder using candle (ONNX backend)
pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl Embedder {
    /// Create a new embedder. Downloads or loads model from model_dir.
    pub fn new(model_dir: &Path) -> Result<Self> {
        let device = Device::Cpu;

        // Load tokenizer
        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| HypatiaError::Validation(format!("Failed to load tokenizer: {}", e)))?;

        // Load ONNX model
        let model_path = model_dir.join("model.onnx");
        let config_path = model_dir.join("config.json");

        // Use candle_onnx or candle-nn to load
        // For simplicity, use candle-transformers' BertModel
        let model = Self::load_model(&model_path, &config_path, &device)?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    fn load_model(_model_path: &Path, _config_path: &Path, _device: &Device) -> Result<BertModel> {
        // Simplified: candle-transformers BertModel requires config
        // We'll use a stub that loads all-MiniLM-L6-v2
        // For now, return an error - will be replaced with actual implementation
        Err(HypatiaError::Validation("Model loading not yet implemented".into()))
    }

    /// Embed a single text into a 384-dim vector
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let tokens = self.tokenizer.encode(text, true)
            .map_err(|e| HypatiaError::Validation(format!("Tokenization failed: {}", e)))?;

        let token_ids = tokens.get_ids();
        let token_ids_tensor = Tensor::new(token_ids, &self.device)
            .map_err(|e| HypatiaError::Validation(format!("Tensor creation failed: {}", e)))?;
        let token_ids_tensor = token_ids_tensor.unsqueeze(0)
            .map_err(|e| HypatiaError::Validation(format!("Unsqueeze failed: {}", e)))?;

        let token_type_ids = tokens.get_type_ids();
        let token_type_ids_tensor = Tensor::new(token_type_ids, &self.device)
            .map_err(|e| HypatiaError::Validation(format!("Tensor creation failed: {}", e)))?;
        let token_type_ids_tensor = token_type_ids_tensor.unsqueeze(0)
            .map_err(|e| HypatiaError::Validation(format!("Unsqueeze failed: {}", e)))?;

        let attention_mask: Vec<i64> = tokens.get_attention_mask().iter().map(|&x| x as i64).collect();
        let attention_mask_tensor = Tensor::new(&attention_mask[..], &self.device)
            .map_err(|e| HypatiaError::Validation(format!("Tensor creation failed: {}", e)))?;
        let attention_mask_tensor = attention_mask_tensor.unsqueeze(0)
            .map_err(|e| HypatiaError::Validation(format!("Unsqueeze failed: {}", e)))?;

        let output = self.model.forward(&token_ids_tensor, &token_type_ids_tensor, Some(&attention_mask_tensor))
            .map_err(|e| HypatiaError::Validation(format!("Model forward pass failed: {}", e)))?;

        // Mean pooling over sequence length
        let mean = output.mean(1)
            .map_err(|e| HypatiaError::Validation(format!("Mean pooling failed: {}", e)))?;
        let mean = mean.to_vec2::<f32>()
            .map_err(|e| HypatiaError::Validation(format!("Tensor to vec failed: {}", e)))?;

        Ok(mean[0].clone())
    }

    /// Embed multiple texts at once (batch)
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            embeddings.push(self.embed(text)?);
        }
        Ok(embeddings)
    }

    /// Get the embedding dimension
    pub fn dim(&self) -> usize {
        EMBEDDING_DIM
    }
}
