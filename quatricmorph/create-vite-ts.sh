#!/bin/bash

set -e

echo "🚀 Create Vite + TypeScript Project"
echo

read -p "Project name: " PROJECT_NAME

if [ -z "$PROJECT_NAME" ]; then
  echo "❌ Project name cannot be empty."
  exit 1
fi

echo
echo "📦 Creating project: $PROJECT_NAME"

npm create vite@latest "$PROJECT_NAME" -- --template vanilla-ts

cd "$PROJECT_NAME"

echo
echo "📥 Installing dependencies..."

npm install

echo
echo "✅ Project created successfully!"
echo
echo "Next steps:"
echo
echo "  cd $PROJECT_NAME"
echo "  npm run dev"
echo