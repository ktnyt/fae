import fg from "fast-glob";
import { readFile, stat } from "node:fs/promises";
import { basename, dirname, extname } from "node:path";
import type { IndexedFile, CodeSymbol, SymbolType } from "./types.js";

export class CodeIndexer {
	private cache = new Map<string, IndexedFile>();

	async indexDirectory(
		directory: string,
		patterns: string[] = ["**/*.{ts,js,tsx,jsx,py,rs,go,java,cpp,c,h}"],
	): Promise<void> {
		const files = await fg(patterns, {
			cwd: directory,
			absolute: true,
			ignore: ["node_modules/**", "dist/**", "build/**", ".git/**"],
		});

		await Promise.all(files.map((file) => this.indexFile(file)));
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

	private async extractSymbols(
		filePath: string,
		content: string,
	): Promise<CodeSymbol[]> {
		const ext = extname(filePath).toLowerCase();
		const symbols: CodeSymbol[] = [];

		switch (ext) {
			case ".ts":
			case ".tsx":
			case ".js":
			case ".jsx":
				symbols.push(...this.extractJavaScriptSymbols(filePath, content));
				break;
			case ".py":
				symbols.push(...this.extractPythonSymbols(filePath, content));
				break;
			case ".rs":
				symbols.push(...this.extractRustSymbols(filePath, content));
				break;
			case ".go":
				symbols.push(...this.extractGoSymbols(filePath, content));
				break;
			default:
				symbols.push(...this.extractGenericSymbols(filePath, content));
		}

		return symbols;
	}

	private extractJavaScriptSymbols(
		filePath: string,
		content: string,
	): CodeSymbol[] {
		const symbols: CodeSymbol[] = [];
		const lines = content.split("\n");

		for (let i = 0; i < lines.length; i++) {
			const line = lines[i];
			if (!line) continue;
			const lineNumber = i + 1;

			// Functions
			const functionMatch = line.match(
				/(?:function\s+|const\s+|let\s+|var\s+)(\w+)\s*(?:=\s*(?:async\s+)?(?:function|\()|(?:\(.*?\)\s*=>))/,
			);
			if (functionMatch && functionMatch[1]) {
				symbols.push({
					name: functionMatch[1],
					type: "function",
					file: filePath,
					line: lineNumber,
					column: (functionMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}

			// Classes
			const classMatch = line.match(/class\s+(\w+)/);
			if (classMatch && classMatch[1]) {
				symbols.push({
					name: classMatch[1],
					type: "class",
					file: filePath,
					line: lineNumber,
					column: (classMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}

			// Interfaces
			const interfaceMatch = line.match(/interface\s+(\w+)/);
			if (interfaceMatch && interfaceMatch[1]) {
				symbols.push({
					name: interfaceMatch[1],
					type: "interface",
					file: filePath,
					line: lineNumber,
					column: (interfaceMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}

			// Type aliases
			const typeMatch = line.match(/type\s+(\w+)/);
			if (typeMatch && typeMatch[1]) {
				symbols.push({
					name: typeMatch[1],
					type: "type",
					file: filePath,
					line: lineNumber,
					column: (typeMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}

			// Enums
			const enumMatch = line.match(/enum\s+(\w+)/);
			if (enumMatch && enumMatch[1]) {
				symbols.push({
					name: enumMatch[1],
					type: "enum",
					file: filePath,
					line: lineNumber,
					column: (enumMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}

			// Variables/Constants
			const varMatch = line.match(/(?:const|let|var)\s+(\w+)/);
			if (varMatch && varMatch[1] && !functionMatch) {
				symbols.push({
					name: varMatch[1],
					type: line.includes("const") ? "constant" : "variable",
					file: filePath,
					line: lineNumber,
					column: (varMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}
		}

		return symbols;
	}

	private extractPythonSymbols(
		filePath: string,
		content: string,
	): CodeSymbol[] {
		const symbols: CodeSymbol[] = [];
		const lines = content.split("\n");

		for (let i = 0; i < lines.length; i++) {
			const line = lines[i];
			if (!line) continue;
			const lineNumber = i + 1;

			// Functions
			const functionMatch = line.match(/def\s+(\w+)/);
			if (functionMatch && functionMatch[1]) {
				symbols.push({
					name: functionMatch[1],
					type: "function",
					file: filePath,
					line: lineNumber,
					column: (functionMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}

			// Classes
			const classMatch = line.match(/class\s+(\w+)/);
			if (classMatch && classMatch[1]) {
				symbols.push({
					name: classMatch[1],
					type: "class",
					file: filePath,
					line: lineNumber,
					column: (classMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}

			// Variables
			const varMatch = line.match(/^(\w+)\s*=/);
			if (varMatch && varMatch[1] && !functionMatch && !classMatch) {
				symbols.push({
					name: varMatch[1],
					type: line.match(/^[A-Z_]+\s*=/) ? "constant" : "variable",
					file: filePath,
					line: lineNumber,
					column: (varMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}
		}

		return symbols;
	}

	private extractRustSymbols(filePath: string, content: string): CodeSymbol[] {
		const symbols: CodeSymbol[] = [];
		const lines = content.split("\n");

		for (let i = 0; i < lines.length; i++) {
			const line = lines[i];
			if (!line) continue;
			const lineNumber = i + 1;

			// Functions
			const functionMatch = line.match(/fn\s+(\w+)/);
			if (functionMatch && functionMatch[1]) {
				symbols.push({
					name: functionMatch[1],
					type: "function",
					file: filePath,
					line: lineNumber,
					column: (functionMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}

			// Structs
			const structMatch = line.match(/struct\s+(\w+)/);
			if (structMatch && structMatch[1]) {
				symbols.push({
					name: structMatch[1],
					type: "class",
					file: filePath,
					line: lineNumber,
					column: (structMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}

			// Enums
			const enumMatch = line.match(/enum\s+(\w+)/);
			if (enumMatch && enumMatch[1]) {
				symbols.push({
					name: enumMatch[1],
					type: "enum",
					file: filePath,
					line: lineNumber,
					column: (enumMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}
		}

		return symbols;
	}

	private extractGoSymbols(filePath: string, content: string): CodeSymbol[] {
		const symbols: CodeSymbol[] = [];
		const lines = content.split("\n");

		for (let i = 0; i < lines.length; i++) {
			const line = lines[i];
			if (!line) continue;
			const lineNumber = i + 1;

			// Functions
			const functionMatch = line.match(/func\s+(\w+)/);
			if (functionMatch && functionMatch[1]) {
				symbols.push({
					name: functionMatch[1],
					type: "function",
					file: filePath,
					line: lineNumber,
					column: (functionMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}

			// Types
			const typeMatch = line.match(/type\s+(\w+)/);
			if (typeMatch && typeMatch[1]) {
				symbols.push({
					name: typeMatch[1],
					type: "type",
					file: filePath,
					line: lineNumber,
					column: (typeMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}

			// Variables
			const varMatch = line.match(/var\s+(\w+)/);
			if (varMatch && varMatch[1]) {
				symbols.push({
					name: varMatch[1],
					type: "variable",
					file: filePath,
					line: lineNumber,
					column: (varMatch.index ?? 0) + 1,
					context: line.trim(),
				});
			}
		}

		return symbols;
	}

	private extractGenericSymbols(
		filePath: string,
		content: string,
	): CodeSymbol[] {
		const symbols: CodeSymbol[] = [];
		const lines = content.split("\n");

		for (let i = 0; i < lines.length; i++) {
			const line = lines[i];
			if (!line) continue;
			const lineNumber = i + 1;

			// Generic identifier extraction (simple word boundaries)
			const identifiers = line.match(/\b[a-zA-Z_][a-zA-Z0-9_]*\b/g);
			if (identifiers) {
				for (const identifier of identifiers) {
					if (identifier.length > 2) {
						// Skip very short identifiers
						symbols.push({
							name: identifier,
							type: "variable",
							file: filePath,
							line: lineNumber,
							column: line.indexOf(identifier) + 1,
							context: line.trim(),
						});
					}
				}
			}
		}

		return symbols;
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
