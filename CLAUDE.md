# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**fae** (Fast And Elegant) is a high-performance code symbol search tool written in Rust. It provides blazingly fast fuzzy search across codebases with Tree-sitter-based symbol extraction, supporting 25+ programming languages with a beautiful TUI interface.

### Design Philosophy
- **Real-time First**: 入力に応じた即座の結果更新
- **Memory Efficient**: 巨大プロジェクトでもスマートなキャッシュ戦略
- **Async Design**: UIブロッキングなしの快適な操作性
- **Test Driven**: 全機能に対して網羅的なテスト
- **Code Quality**: こまめにフォーマッタとリンタを実行すること、警告はすべて治すこと

### Multi-mode Search
1. **Content Search** (default) - ファイル内容のテキスト検索
2. **Symbol Search** (`#prefix`) - 関数・クラス・変数名での検索
3. **File Search** (`>prefix`) - ファイル名・パスでの検索
4. **Regex Search** (`/prefix`) - 高度なパターンマッチング

[... rest of the existing content remains unchanged ...]