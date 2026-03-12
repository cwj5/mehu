#!/bin/bash

# Setup script for git hooks
# This enables pre-commit test hooks for the project

echo "🔧 Configuring git hooks..."

# Configure git to use the .githooks directory
git config core.hooksPath .githooks

# Make hooks executable
chmod +x .githooks/pre-commit
chmod +x .githooks/pre-push

echo "✅ Git hooks configured successfully!"
echo ""
echo "The following hooks are now active:"
echo "  - pre-commit: Runs TypeScript and Rust tests (fast)"
echo "  - pre-push:   Runs Tauri build and cargo build (slow)"
echo ""
echo "Note: To bypass hooks (not recommended), use: git commit --no-verify"
