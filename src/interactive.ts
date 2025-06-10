import prompts from "prompts";
import type { CodeSymbol, SearchOptions, SearchResult, SymbolType } from "./types.js";
import type { FuzzySearcher } from "./searcher.js";
import type { TreeSitterIndexer } from "./tree-sitter-indexer.js";

interface InteractiveConfig {
	directory: string;
	patterns: string[];
	includeFiles: boolean;
	includeDirs: boolean;
	types: SymbolType[];
	limit: number;
	threshold: number;
}

export class InteractiveInterface {
	private indexer: TreeSitterIndexer;
	private searcher?: FuzzySearcher;
	private symbols: CodeSymbol[] = [];
	private config: InteractiveConfig;

	constructor(
		indexer: TreeSitterIndexer,
		initialConfig: Partial<InteractiveConfig> = {},
		existingSymbols?: CodeSymbol[],
	) {
		this.indexer = indexer;
		this.config = {
			directory: ".",
			patterns: ["**/*.ts", "**/*.js", "**/*.tsx", "**/*.jsx", "**/*.py"],
			includeFiles: true,
			includeDirs: true,
			types: [],
			limit: 50,
			threshold: 0.4,
			...initialConfig,
		};

		// Use existing symbols if provided
		if (existingSymbols) {
			this.symbols = existingSymbols;
		}
	}

	async start(): Promise<void> {
		console.log("🔍 Symbol Fuzzy Search - Interactive Mode");
		console.log("Press Ctrl+C to exit at any time");
		console.log("🚀 Quick shortcuts: s=search, t=settings, i=stats, r=reindex, q=quit\n");

		// Index files if not already provided
		if (this.symbols.length === 0) {
			await this.indexFiles();
		} else {
			// Create searcher with existing symbols
			const { FuzzySearcher } = await import("./searcher.js");
			this.searcher = new FuzzySearcher(this.symbols);
			console.log(`📚 Using ${this.symbols.length} indexed symbols\n`);
		}

		// Main interactive loop
		while (true) {
			try {
				await this.showMainMenu();
			} catch (error) {
				if (error instanceof Error && error.name === "PromptAbort") {
					console.log("\n👋 Goodbye!");
					break;
				}
				console.error("\n❌ Error:", error);
			}
		}
	}

	private async indexFiles(): Promise<void> {
		console.log(`🔍 Indexing ${this.config.directory}...`);
		console.log("🌳 Using Tree-sitter for enhanced parsing");

		const fg = await import("fast-glob");
		const files = await fg.default(this.config.patterns, {
			cwd: this.config.directory,
			absolute: true,
			ignore: ["node_modules/**", "dist/**", "build/**", ".git/**"],
		});

		await Promise.all(files.map((file) => this.indexer.indexFile(file)));
		this.symbols = this.indexer.getAllSymbols();

		console.log(`📚 Found ${this.symbols.length} symbols\n`);

		// Create searcher
		const { FuzzySearcher } = await import("./searcher.js");
		this.searcher = new FuzzySearcher(this.symbols);
	}

	private async showMainMenu(): Promise<void> {
		const response = await prompts({
			type: "autocomplete",
			name: "action",
			message: "What would you like to do?",
			choices: [
				{
					title: "🔍 Search symbols",
					description: "Search for symbols in your codebase",
					value: "search",
				},
				{
					title: "⚙️  Settings",
					description: "Configure search options",
					value: "settings",
				},
				{
					title: "📊 Statistics",
					description: "View symbol statistics",
					value: "stats",
				},
				{
					title: "🔄 Re-index",
					description: "Re-index files (refresh symbols)",
					value: "reindex",
				},
				{
					title: "🚪 Exit",
					description: "Exit interactive mode",
					value: "exit",
				},
			],
		});

		switch (response.action) {
			case "search":
				await this.startSearch();
				break;
			case "settings":
				await this.showSettings();
				break;
			case "stats":
				await this.showStatistics();
				break;
			case "reindex":
				await this.indexFiles();
				break;
			case "exit":
				console.log("\n👋 Goodbye!");
				process.exit(0);
				break;
		}
	}

	private async startSearch(): Promise<void> {
		if (!this.searcher) {
			console.log("❌ Searcher not initialized");
			return;
		}

		console.log("\n🔍 Interactive Search Mode");
		console.log("Type your search query (empty to return to main menu):\n");

		while (true) {
			const response = await prompts({
				type: "text",
				name: "query",
				message: "Search:",
				validate: (value) => value.length >= 0, // Allow empty to exit
			});

			if (!response.query || response.query.trim() === "") {
				console.log("🔙 Returning to main menu...\n");
				break;
			}

			await this.performSearch(response.query.trim());
		}
	}

	private async performSearch(query: string): Promise<void> {
		if (!this.searcher) return;

		const searchOptions: SearchOptions = {
			includeFiles: this.config.includeFiles,
			includeDirs: this.config.includeDirs,
			limit: this.config.limit,
			threshold: this.config.threshold,
		};

		if (this.config.types.length > 0) {
			searchOptions.types = this.config.types;
		}

		const results = this.searcher.search(query, searchOptions);

		if (results.length === 0) {
			console.log("🤷 No results found\n");
			return;
		}

		console.log(`\n🎯 Found ${results.length} results for "${query}":\n`);

		// Show results with selection
		const choices = results.map((result, index) => {
			const { symbol, score } = result;
			const scorePercent = Math.round((1 - score) * 100);
			const typeIcon = this.getTypeIcon(symbol.type);
			const fileName = symbol.file.split("/").pop() || symbol.file;

			return {
				title: `${typeIcon} ${symbol.name}`,
				description: `${fileName}:${symbol.line}:${symbol.column} (${scorePercent}% match)`,
				value: index,
			};
		});

		const selection = await prompts({
			type: "select",
			name: "resultIndex",
			message: "Select a result to view details:",
			choices: [
				...choices,
				{
					title: "🔙 Back to search",
					description: "Return to search input",
					value: -1,
				},
			],
		});

		if (selection.resultIndex >= 0) {
			const selectedResult = results[selection.resultIndex];
			if (selectedResult) {
				await this.showResultDetails(selectedResult);
			}
		}
	}

	private async showResultDetails(result: SearchResult): Promise<void> {
		const { symbol, score } = result;
		const scorePercent = Math.round((1 - score) * 100);
		const typeIcon = this.getTypeIcon(symbol.type);

		console.log(`\n📋 Symbol Details:\n`);
		console.log(`${typeIcon} ${symbol.name}`);
		console.log(`   📍 Location: ${symbol.file}:${symbol.line}:${symbol.column}`);
		console.log(`   🏷️  Type: ${symbol.type}`);
		console.log(`   🎯 Match: ${scorePercent}%`);

		if (symbol.context) {
			console.log(`   📝 Context: ${symbol.context}`);
		}

		console.log();

		await prompts({
			type: "confirm",
			name: "continue",
			message: "Press Enter to continue...",
			initial: true,
		});
	}

	private async showSettings(): Promise<void> {
		const response = await prompts([
			{
				type: "multiselect",
				name: "types",
				message: "Select symbol types to include:",
				choices: [
					{ title: "🔧 Functions", value: "function", selected: this.config.types.includes("function") },
					{ title: "📦 Variables", value: "variable", selected: this.config.types.includes("variable") },
					{ title: "🏗️ Classes", value: "class", selected: this.config.types.includes("class") },
					{ title: "🔗 Interfaces", value: "interface", selected: this.config.types.includes("interface") },
					{ title: "📋 Enums", value: "enum", selected: this.config.types.includes("enum") },
					{ title: "📄 Filenames", value: "filename", selected: this.config.types.includes("filename") },
					{ title: "📁 Directories", value: "dirname", selected: this.config.types.includes("dirname") },
				],
				hint: "Leave empty to include all types",
			},
			{
				type: "number",
				name: "limit",
				message: "Maximum number of results:",
				initial: this.config.limit,
				min: 1,
				max: 500,
			},
			{
				type: "number",
				name: "threshold",
				message: "Fuzzy search threshold (0-1, lower = stricter):",
				initial: this.config.threshold,
				min: 0,
				max: 1,
				increment: 0.1,
			},
		]);

		// Update config
		this.config.types = response.types || [];
		this.config.limit = response.limit || this.config.limit;
		this.config.threshold = response.threshold !== undefined ? response.threshold : this.config.threshold;

		console.log("\n✅ Settings updated!");
		console.log(`   🏷️  Symbol types: ${this.config.types.length > 0 ? this.config.types.join(", ") : "All types"}`);
		console.log(`   📊 Result limit: ${this.config.limit}`);
		console.log(`   🎯 Search threshold: ${this.config.threshold}\n`);
	}

	private async showStatistics(): Promise<void> {
		const typeStats = this.symbols.reduce(
			(stats, symbol) => {
				stats[symbol.type] = (stats[symbol.type] || 0) + 1;
				return stats;
			},
			{} as Record<string, number>,
		);

		console.log("\n📊 Symbol Statistics:\n");
		console.log(`Total symbols: ${this.symbols.length}\n`);

		const sortedTypes = Object.entries(typeStats).sort(([, a], [, b]) => b - a);

		for (const [type, count] of sortedTypes) {
			const icon = this.getTypeIcon(type as SymbolType);
			const percentage = ((count / this.symbols.length) * 100).toFixed(1);
			console.log(`${icon} ${type}: ${count} (${percentage}%)`);
		}

		console.log();

		await prompts({
			type: "confirm",
			name: "continue",
			message: "Press Enter to continue...",
			initial: true,
		});
	}

	private getTypeIcon(type: SymbolType): string {
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
}