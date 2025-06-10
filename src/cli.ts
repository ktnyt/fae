#!/usr/bin/env node

import { Command } from "commander";
import { resolve } from "node:path";
import { CodeIndexer } from "./indexer.js";
import { TreeSitterIndexer } from "./tree-sitter-indexer.js";
import { FuzzySearcher } from "./searcher.js";
import type { SearchOptions, SymbolType } from "./types.js";

const program = new Command();

program
	.name("sfs")
	.description("Symbol Fuzzy Search - Search for symbols in your codebase")
	.version("0.1.0");

program
	.argument("[query]", "Search query")
	.option("-d, --directory <path>", "Directory to search", ".")
	.option("-t, --types <types>", "Symbol types to include (comma-separated)")
	.option("--no-files", "Exclude filenames from search")
	.option("--no-dirs", "Exclude directory names from search")
	.option("-l, --limit <number>", "Maximum number of results", "50")
	.option("--threshold <number>", "Fuzzy search threshold (0-1)", "0.4")
	.option(
		"--patterns <patterns>",
		"File patterns to include",
		"**/*.{ts,js,tsx,jsx,py,rs,go,java,cpp,c,h}",
	)
	.option("--use-tree-sitter", "Use Tree-sitter for more accurate parsing")
	.action(async (query, options) => {
		try {
			const directory = resolve(options.directory);
			const patterns = options.patterns.split(",").map((p: string) => p.trim());

			console.log(`🔍 Indexing ${directory}...`);

			let symbols;

			if (options.useTreeSitter) {
				console.log("🌳 Using Tree-sitter for enhanced parsing");
				try {
					const indexer = new TreeSitterIndexer();
					await indexer.initialize();
					
					// Get files to index
					const fg = await import("fast-glob");
					const files = await fg.default(patterns, {
						cwd: directory,
						absolute: true,
						ignore: ["node_modules/**", "dist/**", "build/**", ".git/**"],
					});
					
					await Promise.all(files.map((file) => indexer.indexFile(file)));
					symbols = indexer.getAllSymbols();
				} catch (error) {
					console.warn("⚠️ Tree-sitter failed, falling back to regex parsing:", error);
					const indexer = new CodeIndexer();
					await indexer.indexDirectory(directory, patterns);
					symbols = indexer.getAllSymbols();
				}
			} else {
				const indexer = new CodeIndexer();
				await indexer.indexDirectory(directory, patterns);
				symbols = indexer.getAllSymbols();
			}

			console.log(`📚 Found ${symbols.length} symbols`);

			if (!query) {
				console.log("💡 Use 'sfs <query>' to search for symbols");
				return;
			}

			const searcher = new FuzzySearcher(symbols);

			const searchOptions: SearchOptions = {
				includeFiles: options.files,
				includeDirs: options.dirs,
				limit: parseInt(options.limit),
				threshold: parseFloat(options.threshold),
			};

			if (options.types) {
				searchOptions.types = options.types
					.split(",")
					.map((t: string) => t.trim() as SymbolType);
			}

			const results = searcher.search(query, searchOptions);

			if (results.length === 0) {
				console.log("🤷 No results found");
				return;
			}

			console.log(`\n🎯 Found ${results.length} results for "${query}":\n`);

			for (const result of results) {
				const { symbol, score } = result;
				const scorePercent = Math.round((1 - score) * 100);
				const typeIcon = getTypeIcon(symbol.type);

				console.log(`${typeIcon} ${symbol.name}`);
				console.log(`   📍 ${symbol.file}:${symbol.line}:${symbol.column}`);
				console.log(`   🎯 ${scorePercent}% match`);

				if (symbol.context) {
					console.log(`   📝 ${symbol.context}`);
				}

				console.log();
			}
		} catch (error) {
			console.error("❌ Error:", error);
			process.exit(1);
		}
	});

function getTypeIcon(type: SymbolType): string {
	const icons: Record<SymbolType, string> = {
		function: "🔧",
		variable: "📦",
		class: "🏗️",
		interface: "🔗",
		type: "🏷️",
		enum: "📋",
		constant: "🔒",
		method: "⚙️",
		property: "🔑",
		filename: "📄",
		dirname: "📁",
	};

	return icons[type] ?? "❓";
}

program.parse();
