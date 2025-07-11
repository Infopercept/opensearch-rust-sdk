# Mapper Extensions

## Overview

Mapper Extensions allow developers to create custom field types and mappings in OpenSearch. This enables support for specialized data types, custom indexing strategies, and domain-specific field behaviors that aren't covered by the built-in field types.

## Goals

- Enable custom field type implementations
- Support custom indexing and storage strategies
- Provide custom query and aggregation support for new types
- Allow custom field data loading
- Maintain compatibility with existing mapping APIs
- Optimize for indexing and search performance

## Design

### Mapper Extension Trait

```rust
/// Main trait for mapper extensions
pub trait MapperExtension: Extension {
    /// Register custom field mappers
    fn field_mappers(&self) -> Vec<Box<dyn FieldMapperFactory>> {
        vec![]
    }
    
    /// Register custom metadata mappers
    fn metadata_mappers(&self) -> Vec<Box<dyn MetadataMapperFactory>> {
        vec![]
    }
}

/// Factory for creating field mappers
pub trait FieldMapperFactory: Send + Sync + 'static {
    /// Field type name
    fn type_name(&self) -> &str;
    
    /// Create mapper from mapping configuration
    fn create(
        &self,
        name: String,
        config: MappingConfig,
        context: MapperContext,
    ) -> Result<Box<dyn FieldMapper>, MapperError>;
    
    /// Default mapping configuration
    fn default_mapping(&self) -> MappingConfig {
        MappingConfig::default()
    }
}
```

### Field Mapper Framework

```rust
/// Base trait for field mappers
pub trait FieldMapper: Send + Sync + 'static {
    /// Field name
    fn name(&self) -> &str;
    
    /// Field type name
    fn type_name(&self) -> &str;
    
    /// Parse and index a field value
    fn parse(
        &self,
        context: &mut DocumentParserContext,
        value: &Value,
    ) -> Result<(), MapperError>;
    
    /// Create fields for indexing
    fn create_fields(
        &self,
        value: &Value,
        field_data: &mut FieldData,
    ) -> Result<Vec<IndexableField>, MapperError>;
    
    /// Get field type for queries
    fn field_type(&self) -> MappedFieldType;
    
    /// Merge with another mapper (for mapping updates)
    fn merge(&self, other: &dyn FieldMapper) -> Result<Box<dyn FieldMapper>, MapperError>;
    
    /// Convert stored value back to source format
    fn value_for_source(&self, value: &StoredValue) -> Result<Value, MapperError>;
}

/// Field type information for queries
#[derive(Debug, Clone)]
pub struct MappedFieldType {
    pub name: String,
    pub type_name: String,
    pub searchable: bool,
    pub aggregatable: bool,
    pub has_doc_values: bool,
    pub stored: bool,
    pub index_options: IndexOptions,
}

/// Indexable field representation
#[derive(Debug)]
pub struct IndexableField {
    pub name: String,
    pub value: FieldValue,
    pub field_type: FieldType,
    pub store: bool,
    pub index: bool,
    pub doc_values: bool,
    pub norms: bool,
    pub boost: f32,
}

/// Field value types
#[derive(Debug, Clone)]
pub enum FieldValue {
    String(String),
    Bytes(Vec<u8>),
    Number(f64),
    Boolean(bool),
    Date(DateTime<Utc>),
    GeoPoint(GeoPoint),
    Custom(Box<dyn CustomFieldValue>),
}
```

### Example: Vector Field Mapper

```rust
/// Mapper for vector fields (for similarity search)
pub struct VectorFieldMapper {
    name: String,
    dimension: usize,
    similarity: VectorSimilarity,
    index_options: VectorIndexOptions,
}

impl FieldMapper for VectorFieldMapper {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn type_name(&self) -> &str {
        "vector"
    }
    
    fn parse(
        &self,
        context: &mut DocumentParserContext,
        value: &Value,
    ) -> Result<(), MapperError> {
        let vector = match value {
            Value::Array(values) => {
                if values.len() != self.dimension {
                    return Err(MapperError::InvalidDimension(
                        values.len(),
                        self.dimension,
                    ));
                }
                
                values.iter()
                    .map(|v| v.as_f64()
                        .ok_or_else(|| MapperError::InvalidVectorElement))
                    .collect::<Result<Vec<f64>, _>>()?
            }
            _ => return Err(MapperError::InvalidFieldType(
                "vector field requires array value".to_string()
            )),
        };
        
        // Normalize if required
        let vector = if self.similarity == VectorSimilarity::Cosine {
            normalize_vector(&vector)
        } else {
            vector
        };
        
        // Add to context for indexing
        context.add_vector_field(&self.name, vector, &self.index_options)?;
        
        Ok(())
    }
    
    fn create_fields(
        &self,
        value: &Value,
        field_data: &mut FieldData,
    ) -> Result<Vec<IndexableField>, MapperError> {
        let vector = self.parse_vector(value)?;
        
        // Create HNSW index field
        let hnsw_field = IndexableField {
            name: self.name.clone(),
            value: FieldValue::Custom(Box::new(VectorFieldValue {
                vector: vector.clone(),
                similarity: self.similarity,
            })),
            field_type: FieldType::Custom("vector".to_string()),
            store: true,
            index: true,
            doc_values: false,
            norms: false,
            boost: 1.0,
        };
        
        // Store original vector for exact retrieval
        let stored_field = IndexableField {
            name: format!("{}_stored", self.name),
            value: FieldValue::Bytes(serialize_vector(&vector)),
            field_type: FieldType::Binary,
            store: true,
            index: false,
            doc_values: false,
            norms: false,
            boost: 1.0,
        };
        
        Ok(vec![hnsw_field, stored_field])
    }
    
    fn field_type(&self) -> MappedFieldType {
        MappedFieldType {
            name: self.name.clone(),
            type_name: "vector".to_string(),
            searchable: true,
            aggregatable: false,
            has_doc_values: false,
            stored: true,
            index_options: IndexOptions::None,
        }
    }
}

/// Vector similarity options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VectorSimilarity {
    Cosine,
    DotProduct,
    L2,
}

/// Vector index options
#[derive(Debug, Clone)]
pub struct VectorIndexOptions {
    pub algorithm: VectorIndexAlgorithm,
    pub ef_construction: usize,
    pub m: usize,
}

#[derive(Debug, Clone)]
pub enum VectorIndexAlgorithm {
    HNSW,
    IVF,
    LSH,
}
```

### Custom Query Support

```rust
/// Query support for custom field types
pub trait FieldQueryExtension: Send + Sync {
    /// Create term query
    fn term_query(
        &self,
        field: &str,
        value: &Value,
    ) -> Result<Box<dyn Query>, QueryError>;
    
    /// Create range query
    fn range_query(
        &self,
        field: &str,
        range: RangeSpec,
    ) -> Result<Box<dyn Query>, QueryError>;
    
    /// Create custom query
    fn custom_query(
        &self,
        field: &str,
        query_type: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Box<dyn Query>, QueryError>;
}

/// Vector field query support
impl FieldQueryExtension for VectorFieldMapper {
    fn custom_query(
        &self,
        field: &str,
        query_type: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Box<dyn Query>, QueryError> {
        match query_type {
            "knn" => {
                let vector = params.get("vector")
                    .ok_or_else(|| QueryError::MissingParameter("vector"))?;
                let k = params.get("k")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(10) as usize;
                
                Ok(Box::new(KnnVectorQuery {
                    field: field.to_string(),
                    query_vector: self.parse_vector(vector)?,
                    k,
                    similarity: self.similarity,
                }))
            }
            _ => Err(QueryError::UnsupportedQueryType(query_type.to_string())),
        }
    }
    
    fn term_query(
        &self,
        _field: &str,
        _value: &Value,
    ) -> Result<Box<dyn Query>, QueryError> {
        Err(QueryError::UnsupportedForFieldType(
            "term query not supported for vector fields".to_string()
        ))
    }
    
    fn range_query(
        &self,
        _field: &str,
        _range: RangeSpec,
    ) -> Result<Box<dyn Query>, QueryError> {
        Err(QueryError::UnsupportedForFieldType(
            "range query not supported for vector fields".to_string()
        ))
    }
}
```

### Field Data Loading

```rust
/// Custom field data for aggregations and sorting
pub trait FieldDataLoader: Send + Sync {
    /// Load field data for a segment
    fn load(
        &self,
        reader: &LeafReader,
        field: &str,
    ) -> Result<Box<dyn FieldData>, FieldDataError>;
}

/// Field data representation
pub trait FieldData: Send + Sync {
    /// Get value for document
    fn get_value(&self, doc_id: DocId) -> Option<FieldDataValue>;
    
    /// Get all values for document (multi-valued fields)
    fn get_values(&self, doc_id: DocId) -> Vec<FieldDataValue>;
    
    /// Memory usage in bytes
    fn memory_usage(&self) -> usize;
}

/// Vector field data loader
pub struct VectorFieldDataLoader;

impl FieldDataLoader for VectorFieldDataLoader {
    fn load(
        &self,
        reader: &LeafReader,
        field: &str,
    ) -> Result<Box<dyn FieldData>, FieldDataError> {
        let stored_field = format!("{}_stored", field);
        let mut vectors = Vec::with_capacity(reader.max_doc() as usize);
        
        for doc_id in 0..reader.max_doc() {
            if let Some(value) = reader.get_stored_field(doc_id, &stored_field)? {
                let vector = deserialize_vector(&value)?;
                vectors.push(Some(vector));
            } else {
                vectors.push(None);
            }
        }
        
        Ok(Box::new(VectorFieldData { vectors }))
    }
}
```

### Mapping Configuration

```rust
/// Mapping configuration for custom fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingConfig {
    #[serde(flatten)]
    pub properties: HashMap<String, Value>,
}

impl MappingConfig {
    /// Get typed property
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.properties.get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
    
    /// Get required property
    pub fn get_required<T: DeserializeOwned>(&self, key: &str) -> Result<T, MapperError> {
        self.get(key)
            .ok_or_else(|| MapperError::MissingRequiredProperty(key.to_string()))
    }
}

/// Example vector field mapping
/// ```json
/// {
///   "type": "vector",
///   "dimension": 768,
///   "similarity": "cosine",
///   "index": {
///     "algorithm": "hnsw",
///     "ef_construction": 200,
///     "m": 16
///   }
/// }
/// ```
```

## Implementation Plan

### Phase 1: Core Framework (Week 1)
- [ ] Define mapper traits
- [ ] Implement field type system
- [ ] Create mapper registry
- [ ] Basic parsing framework

### Phase 2: Vector Field Implementation (Week 2)
- [ ] Vector field mapper
- [ ] HNSW index integration
- [ ] KNN query support
- [ ] Similarity metrics

### Phase 3: Additional Field Types (Week 3)
- [ ] IP address field
- [ ] Version field
- [ ] Histogram field
- [ ] Custom date formats

### Phase 4: Integration (Week 4)
- [ ] Mapping API integration
- [ ] Query integration
- [ ] Field data support
- [ ] Performance optimization

## Testing Strategy

### Unit Tests
- Field parsing
- Mapping configuration
- Query generation
- Field data loading

### Integration Tests
- End-to-end indexing
- Search with custom fields
- Mapping updates
- Performance benchmarks

## Performance Considerations

- Efficient field parsing
- Optimized index structures
- Memory-efficient field data
- Cache field type information
- Minimize allocations during indexing

## Future Enhancements

- Composite field types
- Field type migrations
- Custom aggregation support
- Machine learning field types
- Spatial field types
- Time series optimized fields