use jieba_rs::Jieba;

/// Tokenizer that handles both Chinese and English text
pub struct Tokenizer {
    jieba: Jieba,
}

impl Tokenizer {
    pub fn new() -> Self {
        Self {
            jieba: Jieba::new(),
        }
    }

    /// Tokenize input text into lowercase tokens, filtering out stopwords and short tokens
    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let words = self.jieba.cut(text, true);
        words
            .into_iter()
            .map(|w| w.to_lowercase().trim().to_string())
            .filter(|w| !w.is_empty() && !is_stopword(w) && w.len() > 1)
            .collect()
    }
}

fn is_stopword(word: &str) -> bool {
    const STOPWORDS: &[&str] = &[
        "的", "了", "在", "是", "我", "有", "和", "就", "不", "人",
        "都", "一", "一个", "上", "也", "很", "到", "说", "要", "去",
        "你", "会", "着", "没有", "看", "好", "自己", "这", "他", "她",
        "the", "a", "an", "is", "are", "was", "were", "be", "been",
        "to", "of", "in", "for", "on", "with", "at", "by", "from",
        "it", "this", "that", "and", "or", "but", "if", "then",
        "all", "how", "me", "my", "i", "do", "can", "please",
        "帮", "帮我", "请", "把", "用", "让", "给", "想", "能",
    ];
    STOPWORDS.contains(&word)
}
