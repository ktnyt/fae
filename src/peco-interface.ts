import blessed from "blessed";
import type { CodeSymbol, SearchResult } from "./types.js";
import type { FuzzySearcher } from "./searcher.js";

export class PecoInterface {
	private screen: any;
	private searchBox: any;
	private resultsList: any;
	private statusBar: any;
	private searcher: FuzzySearcher;
	private symbols: CodeSymbol[];
	private currentResults: SearchResult[] = [];
	private selectedIndex = 0;
	private query = "";
	private searchTimeout?: NodeJS.Timeout;

	constructor(searcher: FuzzySearcher, symbols: CodeSymbol[]) {
		this.searcher = searcher;
		this.symbols = symbols;

		// Create screen
		this.screen = blessed.screen({
			smartCSR: true,
			title: "Symbol Fuzzy Search - Interactive Mode",
		});

		// Create search input box
		this.searchBox = blessed.textbox({
			parent: this.screen,
			top: 0,
			left: 0,
			width: "100%",
			height: 3,
			border: {
				type: "line",
			},
			style: {
				border: {
					fg: "cyan",
				},
				focus: {
					border: {
						fg: "green",
					},
				},
			},
			label: " Search Query ",
			inputOnFocus: true,
		});

		// Create results list
		this.resultsList = blessed.list({
			parent: this.screen,
			top: 3,
			left: 0,
			width: "100%",
			height: "100%-4",
			border: {
				type: "line",
			},
			style: {
				border: {
					fg: "white",
				},
				selected: {
					bg: "blue",
					fg: "white",
				},
				item: {
					hover: {
						bg: "gray",
					},
				},
			},
			label: " Search Results ",
			keys: true,
			vi: true,
			mouse: true,
			scrollable: true,
		});

		// Create status bar
		this.statusBar = blessed.box({
			parent: this.screen,
			bottom: 0,
			left: 0,
			width: "100%",
			height: 1,
			content: `${this.symbols.length} symbols indexed | Ctrl+C: Exit | Enter: Select | ↑↓: Navigate | ?: Help`,
			style: {
				bg: "blue",
				fg: "white",
			},
		});

		this.setupEventHandlers();
		this.performSearch(""); // Show all symbols initially
	}

	private setupEventHandlers(): void {
		// Global key handlers
		this.screen.key(["C-c", "escape"], () => {
			process.exit(0);
		});

		this.screen.key(["?"], () => {
			this.showHelp();
		});

		this.screen.key(["enter"], () => {
			this.selectCurrentResult();
		});

		this.screen.key(["up", "k"], () => {
			if (this.currentResults.length > 0) {
				this.selectedIndex = Math.max(0, this.selectedIndex - 1);
				this.updateResultsDisplay();
			}
		});

		this.screen.key(["down", "j"], () => {
			if (this.currentResults.length > 0) {
				this.selectedIndex = Math.min(
					this.currentResults.length - 1,
					this.selectedIndex + 1,
				);
				this.updateResultsDisplay();
			}
		});

		this.screen.key(["pageup"], () => {
			if (this.currentResults.length > 0) {
				this.selectedIndex = Math.max(0, this.selectedIndex - 10);
				this.updateResultsDisplay();
			}
		});

		this.screen.key(["pagedown"], () => {
			if (this.currentResults.length > 0) {
				this.selectedIndex = Math.min(
					this.currentResults.length - 1,
					this.selectedIndex + 10,
				);
				this.updateResultsDisplay();
			}
		});

		// Search box event handlers
		this.searchBox.on("submit", (value: string) => {
			this.selectCurrentResult();
		});

		// Real-time search on input change
		this.searchBox.on("keypress", (ch: string, key: any) => {
			// Skip navigation keys that are handled globally
			if (key.name === "up" || key.name === "down" || key.name === "enter") {
				return;
			}

			// Handle input changes with debouncing
			if (this.searchTimeout) {
				clearTimeout(this.searchTimeout);
			}
			
			this.searchTimeout = setTimeout(() => {
				const currentValue = this.searchBox.getValue();
				if (currentValue !== this.query) {
					this.query = currentValue;
					this.performSearch(this.query);
				}
			}, 50); // 50ms debounce
		});

		// Results list handlers
		this.resultsList.on("select", (item: any, index: number) => {
			this.selectedIndex = index;
			this.selectCurrentResult();
		});
	}

	private performSearch(query: string): void {
		if (query.trim() === "") {
			// Show all symbols when query is empty
			this.currentResults = this.symbols.slice(0, 100).map((symbol) => ({
				symbol,
				score: 0,
				matches: [],
			}));
		} else {
			// Perform fuzzy search
			this.currentResults = this.searcher.search(query, { limit: 100 });
		}

		this.selectedIndex = 0;
		this.updateResultsDisplay();
	}

	private updateResultsDisplay(): void {
		const items = this.currentResults.map((result, index) => {
			const { symbol, score } = result;
			const scorePercent = Math.round((1 - score) * 100);
			const typeIcon = this.getTypeIcon(symbol.type);
			const fileName = symbol.file.split("/").pop() || symbol.file;
			
			const prefix = index === this.selectedIndex ? "❯ " : "  ";
			const scoreDisplay = this.query.trim() === "" ? "" : ` (${scorePercent}%)`;
			
			return `${prefix}${typeIcon} ${symbol.name}${scoreDisplay} - ${fileName}:${symbol.line}`;
		});

		this.resultsList.setItems(items);
		this.resultsList.select(this.selectedIndex);

		// Update status bar
		const resultCount = this.currentResults.length;
		const totalSymbols = this.symbols.length;
		const selectedInfo = this.currentResults[this.selectedIndex]
			? ` | Selected: ${this.selectedIndex + 1}/${resultCount}`
			: "";
		
		this.statusBar.setContent(
			`${totalSymbols} symbols indexed | ${resultCount} results${selectedInfo} | Ctrl+C: Exit | Enter: Select | ↑↓: Navigate | ?: Help`,
		);

		this.screen.render();
	}

	private selectCurrentResult(): void {
		const selectedResult = this.currentResults[this.selectedIndex];
		if (!selectedResult) return;

		const { symbol } = selectedResult;
		
		// Clear screen and show selected result
		this.screen.destroy();
		
		console.log("\n🎯 Selected Symbol:\n");
		console.log(`${this.getTypeIcon(symbol.type)} ${symbol.name}`);
		console.log(`   📍 ${symbol.file}:${symbol.line}:${symbol.column}`);
		console.log(`   🏷️  Type: ${symbol.type}`);
		
		if (symbol.context) {
			console.log(`   📝 Context: ${symbol.context}`);
		}
		
		console.log();
		process.exit(0);
	}

	private getTypeIcon(type: string): string {
		const icons: Record<string, string> = {
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

	private showHelp(): void {
		// Create help dialog
		const helpBox = blessed.box({
			parent: this.screen,
			top: "center",
			left: "center",
			width: 60,
			height: 16,
			border: {
				type: "line",
			},
			style: {
				border: {
					fg: "yellow",
				},
			},
			label: " Help ",
			content: `
Keyboard Shortcuts:

Type to search               - Filter symbols in real-time
↑/↓ or j/k                  - Navigate results
Page Up/Page Down           - Fast navigation
Enter                       - Select symbol
Ctrl+C or Escape            - Exit
?                          - Show this help

Search Tips:

- Type part of symbol name for fuzzy matching
- Search works across all symbol types
- Empty search shows all symbols
- Results are sorted by relevance

Press any key to close help...`,
			tags: true,
		});

		// Handle key press to close help
		const closeHelp = () => {
			this.screen.remove(helpBox);
			this.screen.render();
			this.screen.removeAllListeners("keypress");
		};

		this.screen.once("keypress", closeHelp);
		this.screen.render();
	}

	async start(): Promise<void> {
		// Focus on search box
		this.searchBox.focus();
		
		// Render screen
		this.screen.render();
		
		// Keep the process alive
		return new Promise((resolve) => {
			this.screen.on("destroy", resolve);
		});
	}
}