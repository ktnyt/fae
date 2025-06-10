import Parser from "tree-sitter";
import { readFile, stat } from "node:fs/promises";
import { basename, dirname, extname } from "node:path";
import type { IndexedFile, CodeSymbol, SymbolType } from "./types.js";

// Language imports
let JavaScript: any, TypeScript: any, Python: any;

// Lazy load languages to avoid import errors
async function loadLanguages() {
	if (!JavaScript) {
		try {
			// Use require for CommonJS modules
			const { createRequire } = await import("node:module");
			const require = createRequire(import.meta.url);
			
			JavaScript = require("tree-sitter-javascript");
			const tsModule = require("tree-sitter-typescript");
			TypeScript = tsModule.typescript;
			Python = require("tree-sitter-python");
		} catch (error) {
			console.warn("Failed to load Tree-sitter languages:", error);
			throw error;
		}
	}
}

export class TreeSitterIndexer {
	private cache = new Map<string, IndexedFile>();
	private parsers = new Map<string, Parser>();

	async initialize() {
		await loadLanguages();
		
		// Initialize parsers for each language
		const jsParser = new Parser();
		jsParser.setLanguage(JavaScript);
		this.parsers.set("javascript", jsParser);

		const tsParser = new Parser();
		tsParser.setLanguage(TypeScript);
		this.parsers.set("typescript", tsParser);

		const pyParser = new Parser();
		pyParser.setLanguage(Python);
		this.parsers.set("python", pyParser);
	}

	async indexFile(filePath: string): Promise<void> {
		try {
			const stats = await stat(filePath);
			const existing = this.cache.get(filePath);

			if (existing && existing.lastModified >= stats.mtime.getTime()) {
				return;
			}

			const content = await readFile(filePath, "utf-8");
			const symbols = await this.extractSymbols(filePath, content);

			// Add filename and dirname as symbols
			const fileSymbols: CodeSymbol[] = [
				{
					name: basename(filePath),
					type: "filename",
					file: filePath,
					line: 1,
					column: 1,
				},
				{
					name: basename(dirname(filePath)),
					type: "dirname",
					file: filePath,
					line: 1,
					column: 1,
				},
				...symbols,
			];

			this.cache.set(filePath, {
				path: filePath,
				symbols: fileSymbols,
				lastModified: stats.mtime.getTime(),
			});
		} catch (error) {
			console.warn(`Failed to index ${filePath}:`, error);
		}
	}

	private async extractSymbols(filePath: string, content: string): Promise<CodeSymbol[]> {
		const ext = extname(filePath).toLowerCase();
		const language = this.getLanguageForExtension(ext);

		if (!language) {
			return [];
		}

		const parser = this.parsers.get(language);
		if (!parser) {
			return [];
		}

		try {
			const tree = parser.parse(content);
			return this.extractSymbolsFromTree(tree, filePath, content, language);
		} catch (error) {
			console.warn(`Failed to parse ${filePath}:`, error);
			return [];
		}
	}

	private getLanguageForExtension(ext: string): string | null {
		switch (ext) {
			case ".js":
			case ".jsx":
				return "javascript";
			case ".ts":
			case ".tsx":
				return "typescript";
			case ".py":
				return "python";
			default:
				return null;
		}
	}

	private extractSymbolsFromTree(tree: Parser.Tree, filePath: string, content: string, language: string): CodeSymbol[] {
		const symbols: CodeSymbol[] = [];
		const lines = content.split("\n");

		// Define Tree-sitter queries for different symbol types
		const queries = this.getQueriesForLanguage(language);
		
		for (const { query, type } of queries) {
			try {
				const matches = query.matches(tree.rootNode);
				
				for (const match of matches) {
					for (const capture of match.captures) {
						const node = capture.node;
						const startPos = node.startPosition;
						const text = node.text;
						
						if (text && text.trim()) {
							symbols.push({
								name: text,
								type,
								file: filePath,
								line: startPos.row + 1,
								column: startPos.column + 1,
								context: lines[startPos.row]?.trim() || "",
							});
						}
					}
				}
			} catch (error) {
				console.warn(`Query failed for ${type}:`, error);
			}
		}

		return symbols;
	}

	private getQueriesForLanguage(languageName: string): Array<{ query: Parser.Query; type: SymbolType }> {
		const queries: Array<{ query: Parser.Query; type: SymbolType }> = [];
		const parser = this.parsers.get(languageName);
		
		if (!parser) {
			return queries;
		}

		const language = parser.getLanguage();

		try {
			// Language-specific queries - tested working patterns
			if (languageName === "javascript" || languageName === "typescript") {
				// Function declarations
				try {
					queries.push({
						query: new Parser.Query(language, `(function_declaration (identifier) @name)`),
						type: "function"
					});
				} catch (e) { console.debug("Function query failed"); }

				// Class declarations  
				try {
					queries.push({
						query: new Parser.Query(language, `(class_declaration (identifier) @name)`),
						type: "class"
					});
				} catch (e) { console.debug("Class query failed"); }

				// All identifiers as fallback
				queries.push({
					query: new Parser.Query(language, `(identifier) @name`),
					type: "variable"
				});
			}

			// Python queries
			if (languageName === "python") {
				// Function definitions
				try {
					queries.push({
						query: new Parser.Query(language, `(function_definition (identifier) @name)`),
						type: "function"
					});
				} catch (e) { console.debug("Python function query failed"); }

				// Class definitions
				try {
					queries.push({
						query: new Parser.Query(language, `(class_definition (identifier) @name)`),
						type: "class"
					});
				} catch (e) { console.debug("Python class query failed"); }

				// All identifiers as fallback
				queries.push({
					query: new Parser.Query(language, `(identifier) @name`),
					type: "variable"
				});
			}
		} catch (error) {
			console.warn("Failed to create queries:", error);
		}

		return queries;
	}

	getAllSymbols(): CodeSymbol[] {
		const allSymbols: CodeSymbol[] = [];
		for (const indexedFile of this.cache.values()) {
			allSymbols.push(...indexedFile.symbols);
		}
		return allSymbols;
	}

	getSymbolsByFile(filePath: string): CodeSymbol[] {
		return this.cache.get(filePath)?.symbols ?? [];
	}

	clearCache(): void {
		this.cache.clear();
	}
}