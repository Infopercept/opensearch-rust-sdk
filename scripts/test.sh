#!/bin/bash

# OpenSearch Rust SDK Test Script

echo "🦀 Testing OpenSearch Rust SDK"
echo "================================"

# Build the project
echo "📦 Building project..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "❌ Build failed!"
    exit 1
fi

echo "✅ Build successful!"

# Run tests
echo "🧪 Running tests..."
cargo test

if [ $? -ne 0 ]; then
    echo "❌ Tests failed!"
    exit 1
fi

echo "✅ All tests passed!"

# Check for warnings
echo "🔍 Checking for warnings..."
cargo clippy --all-targets --all-features -- -D warnings

if [ $? -eq 0 ]; then
    echo "✅ No clippy warnings!"
else
    echo "⚠️ Clippy warnings found"
fi

# Format check
echo "📝 Checking formatting..."
cargo fmt --all -- --check

if [ $? -eq 0 ]; then
    echo "✅ Code is properly formatted!"
else
    echo "❌ Code needs formatting. Run: cargo fmt --all"
fi

echo ""
echo "🎉 OpenSearch Rust SDK ready!"
echo "💡 To start the extension: cargo run"
echo "📡 Extension will listen on localhost:1234"
echo ""
echo "🔗 Register with OpenSearch:"
echo "curl -XPOST \"http://localhost:9200/_extensions/initialize\" \\"
echo "  -H \"Content-Type:application/json\" \\"
echo "  --data @examples/hello/hello.json"
