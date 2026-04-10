use std::path::PathBuf;
use crate::error::{HypatiaError, Result};

const MODEL_FILES: [&str; 3] = ["config.json", "tokenizer.json", "model.safetensors"];
const MODEL_BASE_URL: &str = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main";

pub fn ensure_models() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| HypatiaError::Validation("Cannot find home directory".into()))?;
    let model_dir = home.join(".hypatia").join("models");

    if model_dir.exists() && all_files_present(&model_dir) {
        return Ok(model_dir);
    }

    std::fs::create_dir_all(&model_dir)
        .map_err(|e| HypatiaError::Validation(format!("Cannot create model dir: {}", e)))?;

    eprintln!("[hypatia] Downloading embedding model (all-MiniLM-L6-v2)...");
    for file in MODEL_FILES {
        let url = format!("{}/{}", MODEL_BASE_URL, file);
        let dest = model_dir.join(file);

        if dest.exists() {
            eprintln!("[hypatia] {} already exists, skipping", file);
            continue;
        }

        eprintln!("[hypatia] Downloading {}...", file);
        download_file(&url, &dest)?;
    }

    eprintln!("[hypatia] Model downloaded to {}", model_dir.display());
    Ok(model_dir)
}

fn all_files_present(dir: &PathBuf) -> bool {
    MODEL_FILES.iter().all(|f| dir.join(f).exists())
}

fn download_file(url: &str, dest: &PathBuf) -> Result<()> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|e| HypatiaError::Validation(format!("Download failed: {}", e)))?;

    let mut out = std::fs::File::create(dest)
        .map_err(|e| HypatiaError::Validation(format!("Cannot create file: {}", e)))?;

    let mut reader = response.body_mut().as_reader();
    std::io::copy(&mut reader, &mut out)
        .map_err(|e| HypatiaError::Validation(format!("Write failed: {}", e)))?;

    Ok(())
}
