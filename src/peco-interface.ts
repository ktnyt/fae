import blessed from "blessed";
import type { CodeSymbol, SearchResult } from "./types.js";
import type { FuzzySearcher } from "./searcher.js";

interface SearchMode {
	prefix: string;
	name: string;
	description: string;
	icon: string;
}

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
	private currentSearchMode: SearchMode;
	private searchModes: SearchMode[] = [
		{
			prefix: "",
			name: "Fuzzy",
			description: "Default fuzzy search across all symbols",
			icon: "🔍",
		},
		{
			prefix: "#",
			name: "Symbol",
			description: "Search symbol names only",
			icon: "🏷️",
		},
		{
			prefix: ">",
			name: "File",
			description: "Search file and directory names",
			icon: "📁",
		},
		{
			prefix: "/",
			name: "Regex",
			description: "Regular expression search",
			icon: "🔧",
		},
	];

	constructor(searcher: FuzzySearcher, symbols: CodeSymbol[]) {
		this.searcher = searcher;
		this.symbols = symbols;
		this.currentSearchMode = this.searchModes[0]!; // Default to fuzzy search

		// Create screen
		this.screen = blessed.screen({
			smartCSR: true,
			title: "Symbol Fuzzy Search - Interactive Mode",
		});

		// Create search input box using textarea for better key control
		this.searchBox = blessed.textarea({
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
			label: ` ${this.currentSearchMode.icon} ${this.currentSearchMode.name} Search `,
			inputOnFocus: true,
			keys: true,
			mouse: true,
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
			content: `${this.symbols.length} symbols | 🔍 Fuzzy Mode | Ctrl+C: Exit | Enter: Select | ↑↓/C-p/C-n: Navigate | ?: Help`,
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

		// Handle enter key at screen level and searchBox level
		const handleEnter = () => {
			this.selectCurrentResult();
		};

		this.screen.key(["enter"], handleEnter);
		
		// Also register on searchBox to intercept enter key
		this.searchBox.key(["enter"], (ch: any, key: any) => {
			handleEnter();
			return false; // Prevent default behavior
		});

		// Override key handling for navigation even when searchBox has focus
		const handleUp = () => {
			if (this.currentResults.length > 0) {
				this.selectedIndex = Math.max(0, this.selectedIndex - 1);
				this.updateResultsDisplay();
			}
		};

		const handleDown = () => {
			if (this.currentResults.length > 0) {
				this.selectedIndex = Math.min(
					this.currentResults.length - 1,
					this.selectedIndex + 1,
				);
				this.updateResultsDisplay();
			}
		};

		// Register handlers at screen level
		this.screen.key(["up", "k", "C-p"], handleUp);
		this.screen.key(["down", "j", "C-n"], handleDown);

		// Also register on searchBox to intercept keys
		this.searchBox.key(["up", "k", "C-p"], (ch: any, key: any) => {
			handleUp();
			return false; // Prevent default behavior
		});

		this.searchBox.key(["down", "j", "C-n"], (ch: any, key: any) => {
			handleDown();
			return false; // Prevent default behavior
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

		// Handle text input changes
		let lastValue = "";
		this.searchBox.on("keypress", () => {
			// Use a small delay to let blessed update the value
			setTimeout(() => {
				const currentValue = this.searchBox.getValue();
				if (currentValue !== lastValue) {
					lastValue = currentValue;
					this.query = currentValue;
					this.performSearch(this.query);
				}
			}, 10);
		});

		// Results list handlers
		this.resultsList.on("select", (item: any, index: number) => {
			this.selectedIndex = index;
			this.selectCurrentResult();
		});
	}

	private performSearch(query: string): void {
		// Detect search mode from query prefix
		const detectedMode = this.detectSearchMode(query);
		if (detectedMode !== this.currentSearchMode) {
			this.currentSearchMode = detectedMode;
			this.updateSearchBoxLabel();
		}

		// Extract actual search query (remove prefix)
		const actualQuery = this.extractSearchQuery(query);

		if (actualQuery.trim() === "") {
			// Show all symbols when query is empty
			this.currentResults = this.symbols.slice(0, 100).map((symbol) => ({
				symbol,
				score: 0,
				matches: [],
			}));
		} else {
			// Perform search based on current mode
			this.currentResults = this.performModeSpecificSearch(actualQuery);
		}

		this.selectedIndex = 0;
		this.updateResultsDisplay();
	}

	private detectSearchMode(query: string): SearchMode {
		for (const mode of this.searchModes) {
			if (mode.prefix && query.startsWith(mode.prefix)) {
				return mode;
			}
		}
		return this.searchModes[0]!; // Default fuzzy search
	}

	private extractSearchQuery(query: string): string {
		if (this.currentSearchMode.prefix && query.startsWith(this.currentSearchMode.prefix)) {
			return query.slice(this.currentSearchMode.prefix.length);
		}
		return query;
	}

	private updateSearchBoxLabel(): void {
		this.searchBox.setLabel(` ${this.currentSearchMode.icon} ${this.currentSearchMode.name} Search `);
		this.screen.render();
	}

	private performModeSpecificSearch(query: string): SearchResult[] {
		const limit = 100;

		switch (this.currentSearchMode.name) {
			case "Symbol":
				// Search only symbol names (exclude files/dirs)
				return this.searcher.search(query, {
					limit,
					includeFiles: false,
					includeDirs: false,
				});

			case "File":
				// Search only file and directory names with enhanced matching
				return this.performFileSearch(query, limit);

			case "Regex":
				// Perform regex search
				return this.performRegexSearch(query, limit);

			case "Fuzzy":
			default:
				// Default fuzzy search
				return this.searcher.search(query, { limit });
		}
	}

	private performFileSearch(query: string, limit: number): SearchResult[] {
		// Get file and directory symbols
		const fileSymbols = this.symbols.filter(
			s => s.type === "filename" || s.type === "dirname"
		);

		// First try exact fuzzy search on file symbols
		const fuzzyResults = this.searcher.search(query, {
			limit,
			types: ["filename", "dirname"],
		});

		// If we have good fuzzy results, return them
		if (fuzzyResults.length > 0) {
			return fuzzyResults;
		}

		// If no fuzzy results, try partial matching for better UX
		const partialMatches: SearchResult[] = [];
		const queryLower = query.toLowerCase();

		for (const symbol of fileSymbols) {
			const symbolName = symbol.name.toLowerCase();
			const baseName = symbol.name.replace(/\.[^/.]+$/, "").toLowerCase(); // Remove extension
			
			// Check if query matches filename (with or without extension)
			if (symbolName.includes(queryLower) || baseName.includes(queryLower)) {
				// Calculate a simple relevance score
				let score = 0;
				if (symbolName.startsWith(queryLower) || baseName.startsWith(queryLower)) {
					score = 0.1; // Prefix match gets better score
				} else {
					score = 0.5; // Partial match gets lower score
				}

				partialMatches.push({
					symbol,
					score,
					matches: [
						{
							indices: [0, symbol.name.length - 1],
							value: symbol.name,
						},
					],
				});
			}

			if (partialMatches.length >= limit) break;
		}

		// Sort by score (lower is better)
		return partialMatches.sort((a, b) => a.score - b.score);
	}

	private performRegexSearch(pattern: string, limit: number): SearchResult[] {
		try {
			const regex = new RegExp(pattern, "i");
			const matches: SearchResult[] = [];

			for (const symbol of this.symbols) {
				if (regex.test(symbol.name)) {
					matches.push({
						symbol,
						score: 0,
						matches: [
							{
								indices: [0, symbol.name.length - 1],
								value: symbol.name,
							},
						],
					});
				}

				if (matches.length >= limit) break;
			}

			return matches;
		} catch (error) {
			// Invalid regex, return empty results
			return [];
		}
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
		
		const modeInfo = `${this.currentSearchMode.icon} ${this.currentSearchMode.name} Mode`;
		this.statusBar.setContent(
			`${totalSymbols} symbols | ${resultCount} results${selectedInfo} | ${modeInfo} | Ctrl+C: Exit | Enter: Select | ↑↓/C-p/C-n: Navigate | ?: Help`,
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
↑/↓ or j/k or C-p/C-n       - Navigate results
Page Up/Page Down           - Fast navigation
Enter                       - Select symbol
Ctrl+C or Escape            - Exit
?                          - Show this help

Search Modes:

🔍 Default (fuzzy)          - Smart fuzzy search across all symbols
🏷️  #symbol_name           - Search symbol names only
📁 >file_name              - Search file and directory names only
🔧 /regex_pattern          - Regular expression search

Search Tips:

- Empty search shows all symbols
- Results are sorted by relevance
- Invalid regex patterns return no results
- Prefix mode switches automatically

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