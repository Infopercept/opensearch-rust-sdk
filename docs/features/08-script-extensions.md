# Script Extensions

## Overview

Script Extensions allow developers to implement custom scripting languages and script contexts in OpenSearch. This enables dynamic computation during indexing, searching, and aggregation operations while maintaining security and performance.

## Goals

- Enable custom scripting language implementations
- Support various script contexts (search, aggregation, update)
- Provide secure script execution environment
- Allow script compilation and caching
- Maintain compatibility with existing script APIs
- Optimize script execution performance

## Design

### Script Extension Trait

```rust
/// Main trait for script extensions
pub trait ScriptExtension: Extension {
    /// Get supported script languages
    fn script_engines(&self) -> Vec<Box<dyn ScriptEngine>> {
        vec![]
    }
    
    /// Get custom script contexts
    fn script_contexts(&self) -> Vec<Box<dyn ScriptContext>> {
        vec![]
    }
}

/// Script engine for a specific language
pub trait ScriptEngine: Send + Sync + 'static {
    /// Language name (e.g., "painless", "expressions")
    fn language(&self) -> &str;
    
    /// Compile a script
    fn compile(
        &self,
        source: &str,
        context: &ScriptContext,
        params: &ScriptParams,
    ) -> Result<Box<dyn CompiledScript>, ScriptError>;
    
    /// Get supported contexts
    fn supported_contexts(&self) -> Vec<String>;
    
    /// Validate script syntax
    fn validate(&self, source: &str) -> Result<(), ScriptError> {
        Ok(())
    }
}

/// Compiled script ready for execution
pub trait CompiledScript: Send + Sync {
    /// Execute the script
    fn execute(&self, context: &mut ExecutionContext) -> Result<ScriptValue, ScriptError>;
    
    /// Get script metadata
    fn metadata(&self) -> &ScriptMetadata;
}
```

### Script Contexts

```rust
/// Context in which scripts execute
pub trait ScriptContext: Send + Sync + 'static {
    /// Context name
    fn name(&self) -> &str;
    
    /// Available variables in this context
    fn variables(&self) -> Vec<VariableDescriptor>;
    
    /// Available methods in this context
    fn methods(&self) -> Vec<MethodDescriptor>;
    
    /// Validate script for this context
    fn validate_script(&self, script: &dyn CompiledScript) -> Result<(), ScriptError>;
}

/// Built-in script contexts
pub enum StandardContext {
    /// Search scripts (e.g., script queries)
    Search,
    /// Aggregation scripts
    Aggregation,
    /// Update scripts
    Update,
    /// Ingest scripts
    Ingest,
    /// Score scripts
    Score,
    /// Field scripts
    Field,
}

/// Variable available in script context
#[derive(Debug, Clone)]
pub struct VariableDescriptor {
    pub name: String,
    pub type_: ScriptType,
    pub description: String,
    pub read_only: bool,
}

/// Method available in script context
#[derive(Debug, Clone)]
pub struct MethodDescriptor {
    pub name: String,
    pub parameters: Vec<ParameterDescriptor>,
    pub return_type: ScriptType,
    pub description: String,
}
```

### Script Values and Types

```rust
/// Values that scripts can work with
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<ScriptValue>),
    Object(HashMap<String, ScriptValue>),
    Binary(Vec<u8>),
    Date(DateTime<Utc>),
}

impl ScriptValue {
    /// Convert to specific type
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ScriptValue::Bool(b) => Some(*b),
            ScriptValue::Integer(i) => Some(*i != 0),
            ScriptValue::String(s) => Some(!s.is_empty()),
            _ => None,
        }
    }
    
    /// Perform arithmetic operations
    pub fn add(&self, other: &ScriptValue) -> Result<ScriptValue, ScriptError> {
        match (self, other) {
            (ScriptValue::Integer(a), ScriptValue::Integer(b)) => {
                Ok(ScriptValue::Integer(a + b))
            }
            (ScriptValue::Float(a), ScriptValue::Float(b)) => {
                Ok(ScriptValue::Float(a + b))
            }
            (ScriptValue::String(a), ScriptValue::String(b)) => {
                Ok(ScriptValue::String(format!("{}{}", a, b)))
            }
            _ => Err(ScriptError::TypeMismatch(
                "Cannot add these types".to_string()
            )),
        }
    }
}

/// Script type system
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptType {
    Any,
    Void,
    Bool,
    Integer,
    Float,
    String,
    Array(Box<ScriptType>),
    Object(HashMap<String, ScriptType>),
    Function(Vec<ScriptType>, Box<ScriptType>),
}
```

### Execution Context

```rust
/// Runtime context for script execution
pub struct ExecutionContext {
    /// Variables available to the script
    variables: HashMap<String, ScriptValue>,
    /// Document being processed (if applicable)
    document: Option<Document>,
    /// Field values
    fields: HashMap<String, Vec<ScriptValue>>,
    /// User-provided parameters
    params: HashMap<String, ScriptValue>,
    /// Execution metrics
    metrics: ExecutionMetrics,
}

impl ExecutionContext {
    /// Get variable value
    pub fn get_variable(&self, name: &str) -> Option<&ScriptValue> {
        self.variables.get(name)
    }
    
    /// Set variable value
    pub fn set_variable(&mut self, name: String, value: ScriptValue) {
        self.variables.insert(name, value);
    }
    
    /// Get field value
    pub fn get_field(&self, name: &str) -> Option<&Vec<ScriptValue>> {
        self.fields.get(name)
    }
    
    /// Get document source
    pub fn get_source(&self) -> Option<&HashMap<String, ScriptValue>> {
        self.document.as_ref().map(|d| &d.source)
    }
    
    /// Emit a score
    pub fn emit_score(&mut self, score: f64) {
        self.metrics.scores_computed += 1;
        // Store or process score
    }
}

/// Metrics collected during execution
#[derive(Debug, Default)]
pub struct ExecutionMetrics {
    pub execution_time: Duration,
    pub memory_used: usize,
    pub scores_computed: usize,
    pub field_accesses: usize,
}
```

### Example: Expression Language

```rust
/// Simple expression language implementation
pub struct ExpressionEngine {
    parser: ExpressionParser,
    compiler: ExpressionCompiler,
}

impl ScriptEngine for ExpressionEngine {
    fn language(&self) -> &str {
        "expressions"
    }
    
    fn compile(
        &self,
        source: &str,
        context: &ScriptContext,
        params: &ScriptParams,
    ) -> Result<Box<dyn CompiledScript>, ScriptError> {
        // Parse expression
        let ast = self.parser.parse(source)?;
        
        // Type check
        let typed_ast = self.compiler.type_check(ast, context)?;
        
        // Compile to bytecode
        let bytecode = self.compiler.compile(typed_ast)?;
        
        Ok(Box::new(CompiledExpression {
            bytecode,
            metadata: ScriptMetadata {
                language: "expressions".to_string(),
                source: source.to_string(),
                context: context.name().to_string(),
            },
        }))
    }
    
    fn supported_contexts(&self) -> Vec<String> {
        vec![
            "score".to_string(),
            "filter".to_string(),
            "aggregation".to_string(),
        ]
    }
}

/// Compiled expression
struct CompiledExpression {
    bytecode: Vec<Instruction>,
    metadata: ScriptMetadata,
}

impl CompiledScript for CompiledExpression {
    fn execute(&self, context: &mut ExecutionContext) -> Result<ScriptValue, ScriptError> {
        let mut vm = VirtualMachine::new(context);
        vm.execute(&self.bytecode)
    }
    
    fn metadata(&self) -> &ScriptMetadata {
        &self.metadata
    }
}

/// Simple VM for expression execution
struct VirtualMachine<'a> {
    stack: Vec<ScriptValue>,
    context: &'a mut ExecutionContext,
}

impl<'a> VirtualMachine<'a> {
    fn execute(&mut self, bytecode: &[Instruction]) -> Result<ScriptValue, ScriptError> {
        for instruction in bytecode {
            match instruction {
                Instruction::LoadConst(value) => {
                    self.stack.push(value.clone());
                }
                Instruction::LoadVar(name) => {
                    let value = self.context.get_variable(name)
                        .ok_or_else(|| ScriptError::UndefinedVariable(name.clone()))?;
                    self.stack.push(value.clone());
                }
                Instruction::Add => {
                    let b = self.stack.pop().ok_or(ScriptError::StackUnderflow)?;
                    let a = self.stack.pop().ok_or(ScriptError::StackUnderflow)?;
                    self.stack.push(a.add(&b)?);
                }
                // ... other instructions
            }
        }
        
        self.stack.pop().ok_or(ScriptError::EmptyStack)
    }
}

#[derive(Debug, Clone)]
enum Instruction {
    LoadConst(ScriptValue),
    LoadVar(String),
    LoadField(String),
    Add,
    Subtract,
    Multiply,
    Divide,
    Compare(CompareOp),
    Jump(usize),
    JumpIf(usize),
    Call(String, usize),
    Return,
}
```

### Script Security

```rust
/// Security manager for scripts
pub trait ScriptSecurityManager: Send + Sync {
    /// Check if script operation is allowed
    fn check_permission(
        &self,
        operation: &ScriptOperation,
        context: &SecurityContext,
    ) -> Result<(), SecurityError>;
    
    /// Get execution limits
    fn get_limits(&self) -> &ExecutionLimits;
}

/// Script operations that need permission
#[derive(Debug, Clone)]
pub enum ScriptOperation {
    FieldAccess(String),
    MethodCall(String),
    SystemAccess,
    NetworkAccess,
    FileAccess(PathBuf),
}

/// Execution limits
#[derive(Debug, Clone)]
pub struct ExecutionLimits {
    pub max_execution_time: Duration,
    pub max_memory: usize,
    pub max_iterations: usize,
    pub max_depth: usize,
}
```

## Implementation Plan

### Phase 1: Core Framework (Week 1)
- [ ] Define script engine traits
- [ ] Implement execution context
- [ ] Create value system
- [ ] Basic security framework

### Phase 2: Expression Engine (Week 2)
- [ ] Expression parser
- [ ] Type system
- [ ] Bytecode compiler
- [ ] Virtual machine

### Phase 3: Integration (Week 3)
- [ ] OpenSearch integration
- [ ] Script caching
- [ ] Context implementations
- [ ] Performance optimization

### Phase 4: Advanced Features (Week 4)
- [ ] Debugging support
- [ ] Script profiling
- [ ] Sandboxing
- [ ] Documentation

## Usage Example

```rust
// Register script extension
let script_ext = MyScriptExtension::new();
extension_runner.register_extension(script_ext);

// Use in a search query
let query = json!({
    "script_score": {
        "query": { "match_all": {} },
        "script": {
            "lang": "expressions",
            "source": "_score * doc['boost'].value",
            "params": {
                "factor": 1.2
            }
        }
    }
});

// Use in aggregation
let agg = json!({
    "weighted_avg": {
        "value": {
            "script": {
                "lang": "expressions",
                "source": "doc['price'].value * params.tax_rate",
                "params": {
                    "tax_rate": 1.08
                }
            }
        },
        "weight": {
            "field": "quantity"
        }
    }
});
```

## Testing Strategy

### Unit Tests
- Script parsing
- Type checking
- Execution correctness
- Security validation

### Integration Tests
- End-to-end script execution
- Performance benchmarks
- Memory usage
- Context integration

## Performance Considerations

- Cache compiled scripts
- JIT compilation for hot scripts
- Optimize VM instruction set
- Minimize allocations
- Use stack-based execution

## Future Enhancements

- WebAssembly script support
- Distributed script execution
- Script debugging API
- Visual script editor
- Machine learning script functions
- Cross-language script calls