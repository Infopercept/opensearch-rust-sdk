# Analysis Extensions

## Overview

Analysis Extensions enable developers to create custom text analysis components including tokenizers, token filters, character filters, and analyzers. These components are essential for text processing in OpenSearch, affecting how text is indexed and searched.

## Goals

- Enable custom tokenization strategies
- Support custom token filtering and transformation
- Provide character-level text preprocessing
- Allow composition of custom analyzers
- Maintain compatibility with existing analysis chains
- Optimize for text processing performance

## Design

### Analysis Extension Trait

```rust
/// Main trait for analysis extensions
pub trait AnalysisExtension: Extension {
    /// Register custom tokenizers
    fn tokenizers(&self) -> Vec<Box<dyn TokenizerFactory>> {
        vec![]
    }
    
    /// Register custom token filters
    fn token_filters(&self) -> Vec<Box<dyn TokenFilterFactory>> {
        vec![]
    }
    
    /// Register custom character filters
    fn char_filters(&self) -> Vec<Box<dyn CharFilterFactory>> {
        vec![]
    }
    
    /// Register custom analyzers
    fn analyzers(&self) -> Vec<Box<dyn AnalyzerFactory>> {
        vec![]
    }
}
```

### Tokenizer Framework

```rust
/// Factory for creating tokenizers
pub trait TokenizerFactory: Send + Sync + 'static {
    /// Tokenizer name
    fn name(&self) -> &str;
    
    /// Create tokenizer instance
    fn create(&self, settings: &Settings) -> Result<Box<dyn Tokenizer>, AnalysisError>;
}

/// Base trait for tokenizers
pub trait Tokenizer: Send + Sync {
    /// Tokenize input text
    fn tokenize(&self, text: &str) -> Result<TokenStream, AnalysisError>;
    
    /// Reset tokenizer state
    fn reset(&mut self) {}
}

/// Token representation
#[derive(Debug, Clone)]
pub struct Token {
    /// Token text
    pub term: String,
    /// Start offset in original text
    pub start_offset: usize,
    /// End offset in original text
    pub end_offset: usize,
    /// Position increment
    pub position_increment: u32,
    /// Token type
    pub token_type: TokenType,
    /// Additional attributes
    pub attributes: HashMap<String, AttributeValue>,
}

/// Stream of tokens
pub struct TokenStream {
    tokens: Vec<Token>,
    position: usize,
}

impl TokenStream {
    /// Get next token
    pub fn next(&mut self) -> Option<&Token> {
        if self.position < self.tokens.len() {
            let token = &self.tokens[self.position];
            self.position += 1;
            Some(token)
        } else {
            None
        }
    }
    
    /// Reset stream to beginning
    pub fn reset(&mut self) {
        self.position = 0;
    }
}
```

### Example: Custom Pattern Tokenizer

```rust
/// Tokenizer that splits on regex pattern
pub struct PatternTokenizer {
    pattern: Regex,
    group: usize,
}

impl Tokenizer for PatternTokenizer {
    fn tokenize(&self, text: &str) -> Result<TokenStream, AnalysisError> {
        let mut tokens = Vec::new();
        let mut position = 0;
        
        for capture in self.pattern.captures_iter(text) {
            if let Some(match_) = capture.get(self.group) {
                tokens.push(Token {
                    term: match_.as_str().to_string(),
                    start_offset: match_.start(),
                    end_offset: match_.end(),
                    position_increment: 1,
                    token_type: TokenType::Word,
                    attributes: HashMap::new(),
                });
                position += 1;
            }
        }
        
        Ok(TokenStream { tokens, position: 0 })
    }
}

pub struct PatternTokenizerFactory;

impl TokenizerFactory for PatternTokenizerFactory {
    fn name(&self) -> &str {
        "pattern"
    }
    
    fn create(&self, settings: &Settings) -> Result<Box<dyn Tokenizer>, AnalysisError> {
        let pattern_str = settings.get_string("pattern")
            .ok_or_else(|| AnalysisError::MissingSetting("pattern"))?;
        let group = settings.get_int("group").unwrap_or(0) as usize;
        
        let pattern = Regex::new(&pattern_str)
            .map_err(|e| AnalysisError::InvalidPattern(e.to_string()))?;
        
        Ok(Box::new(PatternTokenizer { pattern, group }))
    }
}
```

### Token Filter Framework

```rust
/// Factory for creating token filters
pub trait TokenFilterFactory: Send + Sync + 'static {
    /// Filter name
    fn name(&self) -> &str;
    
    /// Create filter instance
    fn create(&self, settings: &Settings) -> Result<Box<dyn TokenFilter>, AnalysisError>;
}

/// Base trait for token filters
pub trait TokenFilter: Send + Sync {
    /// Filter token stream
    fn filter(&self, stream: TokenStream) -> Result<TokenStream, AnalysisError>;
}

/// Example: Synonym token filter
pub struct SynonymFilter {
    synonyms: HashMap<String, Vec<String>>,
    expand: bool,
}

impl TokenFilter for SynonymFilter {
    fn filter(&self, mut stream: TokenStream) -> Result<TokenStream, AnalysisError> {
        let mut output_tokens = Vec::new();
        
        while let Some(token) = stream.next() {
            if let Some(synonyms) = self.synonyms.get(&token.term.to_lowercase()) {
                if self.expand {
                    // Add original token
                    output_tokens.push(token.clone());
                    
                    // Add synonyms at same position
                    for synonym in synonyms {
                        let mut syn_token = token.clone();
                        syn_token.term = synonym.clone();
                        syn_token.position_increment = 0;
                        output_tokens.push(syn_token);
                    }
                } else {
                    // Replace with first synonym
                    let mut syn_token = token.clone();
                    syn_token.term = synonyms[0].clone();
                    output_tokens.push(syn_token);
                }
            } else {
                output_tokens.push(token.clone());
            }
        }
        
        Ok(TokenStream {
            tokens: output_tokens,
            position: 0,
        })
    }
}
```

### Character Filter Framework

```rust
/// Factory for creating character filters
pub trait CharFilterFactory: Send + Sync + 'static {
    /// Filter name
    fn name(&self) -> &str;
    
    /// Create filter instance
    fn create(&self, settings: &Settings) -> Result<Box<dyn CharFilter>, AnalysisError>;
}

/// Base trait for character filters
pub trait CharFilter: Send + Sync {
    /// Filter input text
    fn filter(&self, text: &str) -> Result<String, AnalysisError>;
}

/// Example: HTML strip character filter
pub struct HtmlStripCharFilter {
    escaped_tags: HashSet<String>,
}

impl CharFilter for HtmlStripCharFilter {
    fn filter(&self, text: &str) -> Result<String, AnalysisError> {
        let mut result = String::new();
        let mut chars = text.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if ch == '<' {
                let mut tag = String::new();
                let mut in_tag = true;
                
                while in_tag && chars.peek().is_some() {
                    let next_ch = chars.next().unwrap();
                    tag.push(next_ch);
                    if next_ch == '>' {
                        in_tag = false;
                    }
                }
                
                // Check if tag should be preserved
                let tag_name = extract_tag_name(&tag);
                if self.escaped_tags.contains(&tag_name) {
                    result.push('<');
                    result.push_str(&tag);
                }
            } else {
                result.push(ch);
            }
        }
        
        Ok(result)
    }
}
```

### Custom Analyzer

```rust
/// Factory for creating analyzers
pub trait AnalyzerFactory: Send + Sync + 'static {
    /// Analyzer name
    fn name(&self) -> &str;
    
    /// Create analyzer instance
    fn create(&self, settings: &Settings) -> Result<Box<dyn Analyzer>, AnalysisError>;
}

/// Complete analyzer combining filters and tokenizer
pub trait Analyzer: Send + Sync {
    /// Analyze text
    fn analyze(&self, text: &str) -> Result<TokenStream, AnalysisError>;
}

/// Configurable analyzer
pub struct CustomAnalyzer {
    char_filters: Vec<Box<dyn CharFilter>>,
    tokenizer: Box<dyn Tokenizer>,
    token_filters: Vec<Box<dyn TokenFilter>>,
}

impl Analyzer for CustomAnalyzer {
    fn analyze(&self, text: &str) -> Result<TokenStream, AnalysisError> {
        // Apply character filters
        let mut filtered_text = text.to_string();
        for char_filter in &self.char_filters {
            filtered_text = char_filter.filter(&filtered_text)?;
        }
        
        // Tokenize
        let mut token_stream = self.tokenizer.tokenize(&filtered_text)?;
        
        // Apply token filters
        for token_filter in &self.token_filters {
            token_stream = token_filter.filter(token_stream)?;
        }
        
        Ok(token_stream)
    }
}
```

## Implementation Plan

### Phase 1: Core Framework (Week 1)
- [ ] Define analysis traits
- [ ] Implement token stream
- [ ] Create factory system
- [ ] Basic tokenizer support

### Phase 2: Built-in Components (Week 2)
- [ ] Standard tokenizers
- [ ] Common token filters
- [ ] Character filters
- [ ] Analyzer composition

### Phase 3: Advanced Features (Week 3)
- [ ] Multi-language support
- [ ] Stemming and lemmatization
- [ ] Phonetic analysis
- [ ] Custom attributes

### Phase 4: Integration (Week 4)
- [ ] OpenSearch integration
- [ ] Performance optimization
- [ ] Testing framework
- [ ] Documentation

## Testing Strategy

### Unit Tests
- Tokenization accuracy
- Filter behavior
- Analyzer composition
- Edge cases

### Integration Tests
- End-to-end analysis
- Performance benchmarks
- Memory usage
- Compatibility tests

## Performance Considerations

- Minimize allocations in tokenization
- Use string interning for common tokens
- Cache compiled regular expressions
- Optimize for streaming processing
- Implement zero-copy where possible

## Future Enhancements

- Machine learning tokenizers
- Context-aware analysis
- Streaming analysis for large texts
- Custom scoring based on analysis
- Language detection
- Fuzzy matching in filters