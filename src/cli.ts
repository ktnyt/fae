#!/usr/bin/env node

import { resolve } from "node:path";
import { Command } from "commander";
import { FuzzySearcher } from "./searcher.js";
import { TreeSitterIndexer } from "./tree-sitter-indexer.js";
import { InteractiveInterface } from "./interactive.js";
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
		"**/*.ts,**/*.js,**/*.tsx,**/*.jsx,**/*.py",
	)
	.option("--peco", "Use peco-like interface for interactive search")
	.action(async (query, options) => {
		try {
			const directory = resolve(options.directory);
			const patterns = options.patterns.split(",").map((p: string) => p.trim());

			console.log(`🔍 Indexing ${directory}...`);
			console.log("🌳 Using Tree-sitter for enhanced parsing");

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
			const symbols = indexer.getAllSymbols();

			console.log(`📚 Found ${symbols.length} symbols`);

			if (!query) {
				// Default to peco mode for better UX (can be overridden with --no-peco in future)
				if (!process.env.SFS_NO_PECO && (options.peco !== false)) {
					const { PecoInterface } = await import("./peco-interface.js");
					const searcher = new FuzzySearcher(symbols);
					const peco = new PecoInterface(searcher, symbols);
					await peco.start();
					return;
				}

				// Start menu-based interactive mode with existing symbols
				const interactive = new InteractiveInterface(
					indexer,
					{
						directory,
						patterns,
						includeFiles: options.files,
						includeDirs: options.dirs,
						limit: Number.parseInt(options.limit),
						threshold: Number.parseFloat(options.threshold),
						types: options.types
							? options.types.split(",").map((t: string) => t.trim() as SymbolType)
							: [],
					},
					symbols, // Pass existing symbols to avoid re-indexing
				);

				// Clear console and start interactive mode
				console.clear();
				await interactive.start();
				return;
			}

			const searcher = new FuzzySearcher(symbols);

			const searchOptions: SearchOptions = {
				includeFiles: options.files,
				includeDirs: options.dirs,
				limit: Number.parseInt(options.limit),
				threshold: Number.parseFloat(options.threshold),
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
