use std::path::Path;

use anyhow::{Context, Result};
use ndarray::{Array1, Array2, Axis};

/// Semantic matcher using ONNX embedding model
pub struct SemanticMatcher {
    session: ort::session::Session,
    tokenizer: tokenizers::Tokenizer,
    /// Pre-computed embeddings for all commands, shape (n_commands, embed_dim)
    command_embeddings: Option<Array2<f32>>,
}

impl SemanticMatcher {
    /// Load the ONNX model and tokenizer from the given directory
    pub fn load(model_dir: &Path) -> Result<Self> {
        let model_path = model_dir.join("model.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        let session = ort::session::Session::builder()?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)?
            .commit_from_file(&model_path)
            .with_context(|| format!("Failed to load ONNX model from {}", model_path.display()))?;

        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        Ok(Self {
            session,
            tokenizer,
            command_embeddings: None,
        })
    }

    /// Compute embedding for a single text
    pub fn embed(&self, text: &str) -> Result<Array1<f32>> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&m| m as i64)
            .collect();
        let token_type_ids: Vec<i64> = encoding
            .get_type_ids()
            .iter()
            .map(|&t| t as i64)
            .collect();

        let seq_len = input_ids.len();

        let input_ids =
            ndarray::Array2::from_shape_vec((1, seq_len), input_ids)?;
        let attention_mask =
            ndarray::Array2::from_shape_vec((1, seq_len), attention_mask)?;
        let token_type_ids =
            ndarray::Array2::from_shape_vec((1, seq_len), token_type_ids)?;

        let outputs = self.session.run(ort::inputs![
            "input_ids" => input_ids,
            "attention_mask" => attention_mask,
            "token_type_ids" => token_type_ids,
        ]?)?;

        // Model output: last_hidden_state (1, seq_len, hidden_dim)
        // Mean pooling over sequence dimension
        let output_tensor = outputs[0]
            .try_extract_tensor::<f32>()
            .context("Failed to extract output tensor")?;
        let output_view = output_tensor.view();

        // Shape: (1, seq_len, hidden_dim) → mean over axis 1 → (hidden_dim,)
        let embedding_dyn = output_view
            .index_axis(Axis(0), 0)
            .mean_axis(Axis(0))
            .context("Failed to compute mean pooling")?;

        // Convert from dynamic-dim to 1D
        let (raw_vec, _offset) = embedding_dyn.into_raw_vec_and_offset();
        let embedding = Array1::from_vec(raw_vec);

        // L2 normalize
        let norm = embedding.mapv(|x| x * x).sum().sqrt();
        let normalized = if norm > 0.0 {
            embedding / norm
        } else {
            embedding
        };

        Ok(normalized)
    }

    /// Pre-compute embeddings for all command descriptions
    pub fn precompute_embeddings(&mut self, descriptions: &[String]) -> Result<()> {
        let mut all_embeddings = Vec::new();

        for desc in descriptions {
            let emb = self.embed(desc)?;
            all_embeddings.push(emb);
        }

        if let Some(first) = all_embeddings.first() {
            let dim = first.len();
            let n = all_embeddings.len();
            let flat: Vec<f32> = all_embeddings.into_iter().flat_map(|e| e.to_vec()).collect();
            self.command_embeddings = Some(Array2::from_shape_vec((n, dim), flat)?);
        }

        Ok(())
    }

    /// Rank candidate indices by cosine similarity to query embedding.
    /// Returns (candidate_index, similarity) sorted descending.
    pub fn rank_candidates(
        &self,
        query: &str,
        candidate_indices: &[usize],
        top_k: usize,
    ) -> Result<Vec<(usize, f64)>> {
        let query_emb = self.embed(query)?;

        let embeddings = self
            .command_embeddings
            .as_ref()
            .context("Command embeddings not precomputed")?;

        let mut scores: Vec<(usize, f64)> = candidate_indices
            .iter()
            .filter_map(|&idx| {
                if idx < embeddings.nrows() {
                    let cmd_emb = embeddings.row(idx);
                    let sim: f64 = query_emb
                        .iter()
                        .zip(cmd_emb.iter())
                        .map(|(a, b)| (*a as f64) * (*b as f64))
                        .sum();
                    Some((idx, sim))
                } else {
                    None
                }
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);
        Ok(scores)
    }
}
