# Search Extensions

## Overview

Search Extensions enable developers to extend OpenSearch's search capabilities with custom queries, aggregations, scoring functions, suggesters, and highlighters. This provides deep integration with OpenSearch's search infrastructure while maintaining type safety and performance.

## Goals

- Enable custom query types with full Lucene integration
- Support custom aggregations and metrics
- Implement custom scoring and rescoring functions
- Provide custom suggesters and highlighters
- Maintain compatibility with OpenSearch's query DSL
- Optimize for search performance

## Design

### Search Extension Trait

```rust
/// Main trait for search extensions
pub trait SearchExtension: Extension {
    /// Register custom queries
    fn queries(&self) -> Vec<Box<dyn QuerySpec>> {
        vec![]
    }
    
    /// Register custom aggregations
    fn aggregations(&self) -> Vec<Box<dyn AggregationSpec>> {
        vec![]
    }
    
    /// Register custom score functions
    fn score_functions(&self) -> Vec<Box<dyn ScoreFunctionSpec>> {
        vec![]
    }
    
    /// Register custom suggesters
    fn suggesters(&self) -> Vec<Box<dyn SuggesterSpec>> {
        vec![]
    }
    
    /// Register custom highlighters
    fn highlighters(&self) -> Vec<Box<dyn HighlighterSpec>> {
        vec![]
    }
    
    /// Register custom rescorers
    fn rescorers(&self) -> Vec<Box<dyn RescorerSpec>> {
        vec![]
    }
}
```

### Custom Query Types

```rust
/// Specification for a custom query
pub trait QuerySpec: Send + Sync + 'static {
    /// Query name in the DSL
    fn name(&self) -> &str;
    
    /// Parse query from JSON
    fn parse(&self, json: &serde_json::Value) -> Result<Box<dyn Query>, QueryError>;
    
    /// Query builder for programmatic construction
    fn builder(&self) -> Box<dyn QueryBuilder>;
}

/// Base trait for all queries
pub trait Query: Send + Sync + 'static {
    /// Convert to Lucene query
    fn to_lucene_query(&self, context: &QueryContext) -> Result<LuceneQuery, QueryError>;
    
    /// Rewrite query for optimization
    fn rewrite(&self, context: &QueryContext) -> Result<Box<dyn Query>, QueryError> {
        Ok(Box::new(self.clone()))
    }
    
    /// Extract terms for highlighting
    fn extract_terms(&self) -> Vec<Term> {
        vec![]
    }
    
    /// Cache key for query caching
    fn cache_key(&self) -> Option<String> {
        None
    }
}

/// Context provided during query execution
pub struct QueryContext {
    /// Index searcher
    pub searcher: Arc<IndexSearcher>,
    /// Index settings
    pub index_settings: IndexSettings,
    /// Search context with runtime parameters
    pub search_context: SearchContext,
    /// Field mappings
    pub mappings: Arc<Mappings>,
}

/// Builder pattern for queries
pub trait QueryBuilder: Send + Sync {
    /// Build the final query
    fn build(&self) -> Result<Box<dyn Query>, QueryError>;
}
```

### Example: Custom Vector Query

```rust
/// Vector similarity query
pub struct VectorQuery {
    field: String,
    vector: Vec<f32>,
    k: usize,
    similarity: VectorSimilarity,
}

impl Query for VectorQuery {
    fn to_lucene_query(&self, context: &QueryContext) -> Result<LuceneQuery, QueryError> {
        // Create KNN query for vector search
        let field_info = context.mappings.get_field(&self.field)
            .ok_or_else(|| QueryError::UnknownField(self.field.clone()))?;
        
        if !field_info.is_vector() {
            return Err(QueryError::InvalidFieldType(
                self.field.clone(),
                "vector".to_string()
            ));
        }
        
        // Create Lucene KNN query
        Ok(LuceneQuery::knn(
            &self.field,
            &self.vector,
            self.k,
            self.similarity.to_lucene(),
        ))
    }
    
    fn cache_key(&self) -> Option<String> {
        Some(format!(
            "vector:{}:{}:{}",
            self.field,
            hash(&self.vector),
            self.k
        ))
    }
}

/// Vector query builder
pub struct VectorQueryBuilder {
    field: Option<String>,
    vector: Option<Vec<f32>>,
    k: usize,
    similarity: VectorSimilarity,
}

impl VectorQueryBuilder {
    pub fn new() -> Self {
        Self {
            field: None,
            vector: None,
            k: 10,
            similarity: VectorSimilarity::Cosine,
        }
    }
    
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }
    
    pub fn vector(mut self, vector: Vec<f32>) -> Self {
        self.vector = Some(vector);
        self
    }
    
    pub fn k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }
    
    pub fn similarity(mut self, similarity: VectorSimilarity) -> Self {
        self.similarity = similarity;
        self
    }
}

impl QueryBuilder for VectorQueryBuilder {
    fn build(&self) -> Result<Box<dyn Query>, QueryError> {
        Ok(Box::new(VectorQuery {
            field: self.field.clone()
                .ok_or_else(|| QueryError::MissingField("field"))?,
            vector: self.vector.clone()
                .ok_or_else(|| QueryError::MissingField("vector"))?,
            k: self.k,
            similarity: self.similarity,
        }))
    }
}
```

### Custom Aggregations

```rust
/// Specification for custom aggregations
pub trait AggregationSpec: Send + Sync + 'static {
    /// Aggregation name in the DSL
    fn name(&self) -> &str;
    
    /// Parse aggregation from JSON
    fn parse(&self, json: &serde_json::Value) -> Result<Box<dyn Aggregation>, AggregationError>;
    
    /// Create aggregation builder
    fn builder(&self) -> Box<dyn AggregationBuilder>;
}

/// Base trait for aggregations
pub trait Aggregation: Send + Sync + 'static {
    /// Create the aggregator
    fn create_aggregator(&self, context: &AggregationContext) -> Result<Box<dyn Aggregator>, AggregationError>;
    
    /// Get sub-aggregations
    fn sub_aggregations(&self) -> &[Box<dyn Aggregation>] {
        &[]
    }
}

/// Aggregator that collects data
pub trait Aggregator: Send + Sync {
    /// Collect data from a document
    fn collect(&mut self, doc: DocId, context: &LeafReaderContext) -> Result<(), AggregationError>;
    
    /// Get the aggregation result
    fn get_result(&self) -> AggregationResult;
    
    /// Merge with another aggregator (for parallel execution)
    fn merge(&mut self, other: Box<dyn Aggregator>) -> Result<(), AggregationError>;
}

/// Example: Percentile aggregation
pub struct PercentileAggregation {
    field: String,
    percentiles: Vec<f64>,
    compression: f64,
}

pub struct PercentileAggregator {
    tdigest: TDigest,
    percentiles: Vec<f64>,
}

impl Aggregator for PercentileAggregator {
    fn collect(&mut self, doc: DocId, context: &LeafReaderContext) -> Result<(), AggregationError> {
        if let Some(value) = context.get_numeric_value(&self.field, doc)? {
            self.tdigest.add(value);
        }
        Ok(())
    }
    
    fn get_result(&self) -> AggregationResult {
        let mut results = HashMap::new();
        for &percentile in &self.percentiles {
            results.insert(
                format!("{}", percentile),
                self.tdigest.quantile(percentile / 100.0),
            );
        }
        AggregationResult::Percentiles(results)
    }
}
```

### Custom Scoring

```rust
/// Custom scoring function
pub trait ScoreFunction: Send + Sync + 'static {
    /// Calculate score for a document
    fn score(&self, doc: DocId, context: &ScoringContext) -> Result<f32, ScoringError>;
    
    /// Explanation for debugging
    fn explain(&self, doc: DocId, context: &ScoringContext) -> Result<Explanation, ScoringError>;
    
    /// Whether this function needs document scores
    fn needs_scores(&self) -> bool {
        false
    }
}

/// Example: Field value factor scoring
pub struct FieldValueFactorFunction {
    field: String,
    factor: f32,
    modifier: Modifier,
    missing: f32,
}

impl ScoreFunction for FieldValueFactorFunction {
    fn score(&self, doc: DocId, context: &ScoringContext) -> Result<f32, ScoringError> {
        let value = context.get_numeric_value(&self.field, doc)?
            .unwrap_or(self.missing);
        
        let modified = match self.modifier {
            Modifier::None => value,
            Modifier::Log => (value + 1.0).ln(),
            Modifier::Log1p => value.ln_1p(),
            Modifier::Log2p => (value + 2.0).ln(),
            Modifier::Ln => value.ln(),
            Modifier::Ln1p => value.ln_1p(),
            Modifier::Ln2p => (value + 2.0).ln(),
            Modifier::Square => value * value,
            Modifier::Sqrt => value.sqrt(),
            Modifier::Reciprocal => 1.0 / value,
        };
        
        Ok(modified * self.factor)
    }
    
    fn explain(&self, doc: DocId, context: &ScoringContext) -> Result<Explanation, ScoringError> {
        let value = context.get_numeric_value(&self.field, doc)?
            .unwrap_or(self.missing);
        let score = self.score(doc, context)?;
        
        Ok(Explanation::new(
            score,
            format!(
                "field value factor: field={}, value={}, factor={}, modifier={:?}",
                self.field, value, self.factor, self.modifier
            ),
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Modifier {
    None,
    Log,
    Log1p,
    Log2p,
    Ln,
    Ln1p,
    Ln2p,
    Square,
    Sqrt,
    Reciprocal,
}
```

### Custom Suggesters

```rust
/// Custom suggester implementation
pub trait Suggester: Send + Sync + 'static {
    /// Generate suggestions
    fn suggest(
        &self,
        text: &str,
        context: &SuggestionContext,
    ) -> Result<Vec<Suggestion>, SuggestionError>;
    
    /// Suggester name
    fn name(&self) -> &str;
}

/// Example: Fuzzy suggester
pub struct FuzzySuggester {
    field: String,
    max_edits: u8,
    prefix_length: usize,
    max_suggestions: usize,
}

impl Suggester for FuzzySuggester {
    fn suggest(
        &self,
        text: &str,
        context: &SuggestionContext,
    ) -> Result<Vec<Suggestion>, SuggestionError> {
        let analyzer = context.get_analyzer(&self.field)?;
        let tokens = analyzer.analyze(text)?;
        
        let mut suggestions = Vec::new();
        for token in tokens {
            let fuzzy_query = FuzzyQuery::new(
                &self.field,
                &token.text,
                self.max_edits,
                self.prefix_length,
            );
            
            let matches = context.search(fuzzy_query, self.max_suggestions)?;
            for (term, score) in matches {
                suggestions.push(Suggestion {
                    text: term,
                    score,
                    frequency: context.term_frequency(&self.field, &term)?,
                });
            }
        }
        
        suggestions.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        suggestions.truncate(self.max_suggestions);
        
        Ok(suggestions)
    }
    
    fn name(&self) -> &str {
        "fuzzy"
    }
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub text: String,
    pub score: f32,
    pub frequency: u64,
}
```

### Registration and Integration

```rust
impl MySearchExtension {
    pub fn new() -> Self {
        Self {
            // Initialize extension
        }
    }
}

impl SearchExtension for MySearchExtension {
    fn queries(&self) -> Vec<Box<dyn QuerySpec>> {
        vec![
            Box::new(VectorQuerySpec),
            Box::new(GraphQuerySpec),
        ]
    }
    
    fn aggregations(&self) -> Vec<Box<dyn AggregationSpec>> {
        vec![
            Box::new(PercentileAggregationSpec),
            Box::new(TopKAggregationSpec),
        ]
    }
    
    fn score_functions(&self) -> Vec<Box<dyn ScoreFunctionSpec>> {
        vec![
            Box::new(FieldValueFactorSpec),
            Box::new(DecayFunctionSpec),
        ]
    }
    
    fn suggesters(&self) -> Vec<Box<dyn SuggesterSpec>> {
        vec![
            Box::new(FuzzySuggesterSpec),
            Box::new(CompletionSuggesterSpec),
        ]
    }
}
```

## Implementation Plan

### Phase 1: Query Framework (Week 1)
- [ ] Query trait system
- [ ] Query parsing and building
- [ ] Basic query implementations
- [ ] Query context and execution

### Phase 2: Aggregations (Week 2)
- [ ] Aggregation framework
- [ ] Metric aggregations
- [ ] Bucket aggregations
- [ ] Pipeline aggregations

### Phase 3: Scoring & Relevance (Week 3)
- [ ] Scoring function framework
- [ ] Common scoring functions
- [ ] Rescorer implementation
- [ ] Explanation framework

### Phase 4: Suggesters & Highlighters (Week 4)
- [ ] Suggester framework
- [ ] Highlighter framework
- [ ] Integration with analyzers
- [ ] Performance optimization

## Testing Strategy

### Unit Tests
- Query parsing and serialization
- Aggregation collection
- Scoring calculations
- Suggestion generation

### Integration Tests
- End-to-end search with custom components
- Performance with large datasets
- Compatibility with existing queries
- Memory usage patterns

### Performance Tests
- Query execution speed
- Aggregation performance
- Scoring overhead
- Cache effectiveness

## Performance Considerations

- Cache compiled queries
- Optimize Lucene query generation
- Use appropriate data structures for aggregations
- Implement efficient scoring with SIMD where possible
- Minimize allocations in hot paths

## Future Enhancements

- Machine learning scoring functions
- Graph-based queries
- Geospatial search extensions
- Time series aggregations
- Natural language queries
- Query optimization hints