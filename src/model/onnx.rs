use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

/// Get the default model directory path
pub fn default_model_dir() -> PathBuf {
    // Look relative to the executable first, then fall back to a well-known location
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let model_dir = parent.join("models").join("all-MiniLM-L6-v2");
            if model_dir.exists() {
                return model_dir;
            }
        }
    }

    // Fall back to ~/.cmdsage/models/
    if let Some(home) = dirs::home_dir() {
        return home
            .join(".cmdsage")
            .join("models")
            .join("all-MiniLM-L6-v2");
    }

    PathBuf::from("models/all-MiniLM-L6-v2")
}

/// Check if the model files exist
pub fn model_exists(model_dir: &Path) -> bool {
    model_dir.join("model.onnx").exists() && model_dir.join("tokenizer.json").exists()
}

/// Download the model if it doesn't exist (placeholder — prints instructions)
pub fn ensure_model(model_dir: &Path) -> Result<()> {
    if model_exists(model_dir) {
        return Ok(());
    }

    std::fs::create_dir_all(model_dir)
        .with_context(|| format!("Failed to create model dir: {}", model_dir.display()))?;

    anyhow::bail!(
        "Model not found at {}.\n\
         Please download all-MiniLM-L6-v2 ONNX model:\n\
         \n\
         mkdir -p {}\n\
         wget -O {}/model.onnx \\\n\
           https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx\n\
         wget -O {}/tokenizer.json \\\n\
           https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json",
        model_dir.display(),
        model_dir.display(),
        model_dir.display(),
        model_dir.display(),
    )
}
