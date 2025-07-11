# Ingest Extensions

## Overview

Ingest Extensions enable developers to create custom ingest processors that transform, enrich, and filter documents before they are indexed in OpenSearch. This allows for sophisticated data preprocessing pipelines that can handle complex ETL requirements.

## Goals

- Enable custom document transformation logic
- Support document enrichment from external sources
- Provide filtering and routing capabilities
- Allow custom parsing of various data formats
- Maintain high throughput for bulk operations
- Support conditional processing and error handling

## Design

### Ingest Extension Trait

```rust
/// Main trait for ingest extensions
pub trait IngestExtension: Extension {
    /// Register custom processors
    fn processors(&self) -> Vec<Box<dyn ProcessorFactory>> {
        vec![]
    }
}

/// Factory for creating processor instances
pub trait ProcessorFactory: Send + Sync + 'static {
    /// Processor type name
    fn type_name(&self) -> &str;
    
    /// Create processor instance from configuration
    fn create(
        &self,
        tag: Option<String>,
        description: Option<String>,
        config: ProcessorConfig,
    ) -> Result<Box<dyn Processor>, ProcessorError>;
    
    /// Validate processor configuration
    fn validate_config(&self, config: &ProcessorConfig) -> Result<(), ProcessorError> {
        Ok(())
    }
}
```

### Processor Framework

```rust
/// Base trait for all processors
#[async_trait]
pub trait Processor: Send + Sync + 'static {
    /// Process a document
    async fn process(
        &self,
        document: &mut IngestDocument,
        context: &ProcessorContext,
    ) -> Result<ProcessorResult, ProcessorError>;
    
    /// Get processor type
    fn type_name(&self) -> &str;
    
    /// Get processor tag
    fn tag(&self) -> Option<&str>;
    
    /// Get processor description
    fn description(&self) -> Option<&str>;
}

/// Document being processed
#[derive(Debug, Clone)]
pub struct IngestDocument {
    /// Document fields
    fields: HashMap<String, Value>,
    /// Document metadata
    metadata: DocumentMetadata,
    /// Ingest metadata
    ingest: IngestMetadata,
}

impl IngestDocument {
    /// Get field value
    pub fn get_field(&self, path: &str) -> Option<&Value> {
        // Support nested field access with dot notation
        self.get_field_by_path(path)
    }
    
    /// Set field value
    pub fn set_field(&mut self, path: &str, value: Value) -> Result<(), ProcessorError> {
        // Support nested field creation
        self.set_field_by_path(path, value)
    }
    
    /// Remove field
    pub fn remove_field(&mut self, path: &str) -> Option<Value> {
        self.remove_field_by_path(path)
    }
    
    /// Add value to array field
    pub fn append_field(&mut self, path: &str, value: Value) -> Result<(), ProcessorError> {
        match self.get_field_mut(path) {
            Some(Value::Array(arr)) => {
                arr.push(value);
                Ok(())
            }
            Some(_) => Err(ProcessorError::FieldNotArray(path.to_string())),
            None => {
                self.set_field(path, Value::Array(vec![value]))
            }
        }
    }
}

/// Processing context
pub struct ProcessorContext {
    /// Pipeline configuration
    pub pipeline_config: PipelineConfig,
    /// Shared services
    pub services: Arc<ProcessorServices>,
    /// Current timestamp
    pub timestamp: DateTime<Utc>,
}

/// Result of processing
#[derive(Debug)]
pub enum ProcessorResult {
    /// Continue to next processor
    Continue,
    /// Skip remaining processors
    Skip,
    /// Drop the document
    Drop,
    /// Route to different index
    Route(String),
}
```

### Built-in Processor Types

```rust
/// Grok processor for parsing structured text
pub struct GrokProcessor {
    patterns: Vec<GrokPattern>,
    field: String,
    target_field: Option<String>,
    ignore_missing: bool,
}

#[async_trait]
impl Processor for GrokProcessor {
    async fn process(
        &self,
        document: &mut IngestDocument,
        _context: &ProcessorContext,
    ) -> Result<ProcessorResult, ProcessorError> {
        let value = match document.get_field(&self.field) {
            Some(Value::String(s)) => s,
            Some(_) => return Err(ProcessorError::InvalidFieldType(
                self.field.clone(),
                "string".to_string()
            )),
            None if self.ignore_missing => return Ok(ProcessorResult::Continue),
            None => return Err(ProcessorError::MissingField(self.field.clone())),
        };
        
        // Try each pattern until one matches
        for pattern in &self.patterns {
            if let Some(captures) = pattern.match_str(value) {
                let target = self.target_field.as_ref().unwrap_or(&self.field);
                
                // Add captured groups to document
                for (name, value) in captures {
                    let field_path = format!("{}.{}", target, name);
                    document.set_field(&field_path, Value::String(value))?;
                }
                
                return Ok(ProcessorResult::Continue);
            }
        }
        
        Err(ProcessorError::GrokNoMatch(value.clone()))
    }
    
    fn type_name(&self) -> &str {
        "grok"
    }
}

/// Enrich processor for adding data from external sources
pub struct EnrichProcessor {
    policy_name: String,
    field: String,
    target_field: String,
    max_matches: usize,
}

#[async_trait]
impl Processor for EnrichProcessor {
    async fn process(
        &self,
        document: &mut IngestDocument,
        context: &ProcessorContext,
    ) -> Result<ProcessorResult, ProcessorError> {
        let lookup_value = document.get_field(&self.field)
            .ok_or_else(|| ProcessorError::MissingField(self.field.clone()))?;
        
        // Get enrich policy
        let policy = context.services.enrich_service
            .get_policy(&self.policy_name)
            .await?;
        
        // Lookup matching documents
        let matches = policy.lookup(lookup_value, self.max_matches).await?;
        
        // Add enriched data to document
        if !matches.is_empty() {
            document.set_field(
                &self.target_field,
                Value::Array(matches.into_iter().map(Value::Object).collect()),
            )?;
        }
        
        Ok(ProcessorResult::Continue)
    }
    
    fn type_name(&self) -> &str {
        "enrich"
    }
}

/// Script processor for custom logic
pub struct ScriptProcessor {
    script_engine: String,
    script_source: String,
    script_params: HashMap<String, Value>,
}

#[async_trait]
impl Processor for ScriptProcessor {
    async fn process(
        &self,
        document: &mut IngestDocument,
        context: &ProcessorContext,
    ) -> Result<ProcessorResult, ProcessorError> {
        let script_service = &context.services.script_service;
        
        // Compile script
        let compiled = script_service.compile(
            &self.script_engine,
            &self.script_source,
            ScriptContext::Ingest,
        ).await?;
        
        // Execute script with document context
        let result = compiled.execute(IngestScriptContext {
            doc: document,
            params: &self.script_params,
            metadata: &document.metadata,
        }).await?;
        
        // Handle script result
        match result {
            ScriptResult::Continue => Ok(ProcessorResult::Continue),
            ScriptResult::Drop => Ok(ProcessorResult::Drop),
            ScriptResult::Error(msg) => Err(ProcessorError::ScriptError(msg)),
        }
    }
    
    fn type_name(&self) -> &str {
        "script"
    }
}
```

### Pipeline Management

```rust
/// Ingest pipeline definition
#[derive(Debug, Clone)]
pub struct Pipeline {
    /// Pipeline ID
    pub id: String,
    /// Pipeline description
    pub description: Option<String>,
    /// Processors in order
    pub processors: Vec<Box<dyn Processor>>,
    /// Error handlers
    pub on_failure: Vec<Box<dyn Processor>>,
    /// Pipeline version
    pub version: u64,
}

impl Pipeline {
    /// Process a document through the pipeline
    pub async fn process(
        &self,
        document: &mut IngestDocument,
        context: &ProcessorContext,
    ) -> Result<ProcessorResult, ProcessorError> {
        let mut result = ProcessorResult::Continue;
        
        for processor in &self.processors {
            match processor.process(document, context).await {
                Ok(ProcessorResult::Continue) => continue,
                Ok(other_result) => {
                    result = other_result;
                    break;
                }
                Err(e) => {
                    // Run failure handlers
                    document.ingest.on_failure_message = Some(e.to_string());
                    document.ingest.on_failure_processor = processor.tag().map(String::from);
                    
                    for failure_processor in &self.on_failure {
                        match failure_processor.process(document, context).await {
                            Ok(_) => {},
                            Err(e) => return Err(e),
                        }
                    }
                    
                    return Err(e);
                }
            }
        }
        
        Ok(result)
    }
}

/// Pipeline service for managing pipelines
pub struct PipelineService {
    pipelines: Arc<RwLock<HashMap<String, Pipeline>>>,
    processor_registry: Arc<ProcessorRegistry>,
}

impl PipelineService {
    /// Create or update a pipeline
    pub async fn put_pipeline(
        &self,
        id: String,
        definition: PipelineDefinition,
    ) -> Result<(), ProcessorError> {
        let mut processors = Vec::new();
        
        // Create processors from definition
        for proc_def in definition.processors {
            let factory = self.processor_registry
                .get_factory(&proc_def.type_name)
                .ok_or_else(|| ProcessorError::UnknownProcessor(proc_def.type_name.clone()))?;
            
            let processor = factory.create(
                proc_def.tag,
                proc_def.description,
                proc_def.config,
            )?;
            
            processors.push(processor);
        }
        
        let pipeline = Pipeline {
            id: id.clone(),
            description: definition.description,
            processors,
            on_failure: vec![], // TODO: Create failure processors
            version: 1,
        };
        
        self.pipelines.write().await.insert(id, pipeline);
        Ok(())
    }
    
    /// Get a pipeline
    pub async fn get_pipeline(&self, id: &str) -> Option<Pipeline> {
        self.pipelines.read().await.get(id).cloned()
    }
    
    /// Delete a pipeline
    pub async fn delete_pipeline(&self, id: &str) -> Option<Pipeline> {
        self.pipelines.write().await.remove(id)
    }
}
```

### Conditional Processing

```rust
/// Conditional processor wrapper
pub struct ConditionalProcessor {
    condition: Box<dyn Condition>,
    processor: Box<dyn Processor>,
}

#[async_trait]
impl Processor for ConditionalProcessor {
    async fn process(
        &self,
        document: &mut IngestDocument,
        context: &ProcessorContext,
    ) -> Result<ProcessorResult, ProcessorError> {
        if self.condition.evaluate(document, context).await? {
            self.processor.process(document, context).await
        } else {
            Ok(ProcessorResult::Continue)
        }
    }
    
    fn type_name(&self) -> &str {
        self.processor.type_name()
    }
}

/// Condition for conditional processing
#[async_trait]
pub trait Condition: Send + Sync {
    async fn evaluate(
        &self,
        document: &IngestDocument,
        context: &ProcessorContext,
    ) -> Result<bool, ProcessorError>;
}
```

## Implementation Plan

### Phase 1: Core Framework (Week 1)
- [ ] Define processor traits
- [ ] Implement document model
- [ ] Create processor registry
- [ ] Basic pipeline execution

### Phase 2: Built-in Processors (Week 2)
- [ ] Grok processor
- [ ] JSON processor
- [ ] Date processor
- [ ] Convert processor

### Phase 3: Advanced Processors (Week 3)
- [ ] Enrich processor
- [ ] Script processor
- [ ] Pipeline processor
- [ ] Conditional processing

### Phase 4: Integration (Week 4)
- [ ] Pipeline management API
- [ ] Bulk processing
- [ ] Error handling
- [ ] Monitoring

## Testing Strategy

### Unit Tests
- Processor logic
- Document manipulation
- Pipeline execution
- Error handling

### Integration Tests
- End-to-end pipeline processing
- Bulk operations
- Performance tests
- Error recovery

## Performance Considerations

- Minimize document copying
- Cache compiled patterns
- Parallel processing for bulk
- Connection pooling for enrichment
- Efficient field path resolution

## Future Enhancements

- Machine learning processors
- Streaming ingest support
- Pipeline composition
- Visual pipeline builder
- Custom data formats
- Distributed processing