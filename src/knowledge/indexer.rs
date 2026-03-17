use std::collections::HashMap;

use super::CommandEntry;

/// Inverted index: token → list of command indices
pub struct KeywordIndex {
    index: HashMap<String, Vec<usize>>,
    doc_count: usize,
    /// How many documents contain each token
    doc_freq: HashMap<String, usize>,
}

impl KeywordIndex {
    /// Build an inverted index from command entries.
    /// Indexes: keywords, description tokens, example input tokens.
    pub fn build(commands: &[CommandEntry], tokenize: &dyn Fn(&str) -> Vec<String>) -> Self {
        let mut index: HashMap<String, Vec<usize>> = HashMap::new();
        let mut doc_freq: HashMap<String, usize> = HashMap::new();

        for (i, cmd) in commands.iter().enumerate() {
            let mut tokens_for_doc: std::collections::HashSet<String> =
                std::collections::HashSet::new();

            // Index keywords directly
            for kw in &cmd.keywords {
                let lower = kw.to_lowercase();
                tokens_for_doc.insert(lower.clone());
                index.entry(lower).or_default().push(i);
            }

            // Index description tokens
            for token in tokenize(&cmd.description) {
                if !tokens_for_doc.contains(&token) {
                    tokens_for_doc.insert(token.clone());
                    index.entry(token).or_default().push(i);
                }
            }

            // Index binary name
            let bin = cmd.binary.to_lowercase();
            if !tokens_for_doc.contains(&bin) {
                tokens_for_doc.insert(bin.clone());
                index.entry(bin).or_default().push(i);
            }

            // Index example inputs
            for ex in &cmd.examples {
                for token in tokenize(&ex.input) {
                    if !tokens_for_doc.contains(&token) {
                        tokens_for_doc.insert(token.clone());
                        index.entry(token).or_default().push(i);
                    }
                }
            }

            for token in &tokens_for_doc {
                *doc_freq.entry(token.clone()).or_insert(0) += 1;
            }
        }

        Self {
            index,
            doc_count: commands.len(),
            doc_freq,
        }
    }

    /// BM25 search: returns (command_index, score) sorted by score descending
    pub fn search(&self, query_tokens: &[String], top_k: usize) -> Vec<(usize, f64)> {
        let k1 = 1.2;
        let b = 0.75;
        let avg_dl = 10.0; // approximate average document length

        let mut scores: HashMap<usize, f64> = HashMap::new();

        for token in query_tokens {
            let lower = token.to_lowercase();
            if let Some(postings) = self.index.get(&lower) {
                let df = self.doc_freq.get(&lower).copied().unwrap_or(1) as f64;
                let idf = ((self.doc_count as f64 - df + 0.5) / (df + 0.5) + 1.0).ln();

                // Count term frequency per document in this query
                let mut tf_map: HashMap<usize, f64> = HashMap::new();
                for &doc_id in postings {
                    *tf_map.entry(doc_id).or_insert(0.0) += 1.0;
                }

                for (&doc_id, &tf) in &tf_map {
                    let dl = avg_dl; // simplified: assume uniform doc length
                    let numerator = tf * (k1 + 1.0);
                    let denominator = tf + k1 * (1.0 - b + b * dl / avg_dl);
                    *scores.entry(doc_id).or_insert(0.0) += idf * numerator / denominator;
                }
            }
        }

        let mut results: Vec<(usize, f64)> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }
}
