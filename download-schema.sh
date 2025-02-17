#!/bin/sh

# Download the schema file from the raw GitHub URL
curl -L "https://raw.githubusercontent.com/modelcontextprotocol/specification/main/schema/2024-11-05/schema.json" -o schema.json

echo "Schema downloaded successfully to schema.json" 