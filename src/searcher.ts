import Fuse from "fuse.js";
import type { CodeSymbol, SearchOptions, SearchResult } from "./types.js";

export class FuzzySearcher {
	private fuse: Fuse<CodeSymbol>;
	private symbols: CodeSymbol[];

	constructor(symbols: CodeSymbol[]) {
		this.symbols = symbols;
		this.fuse = new Fuse(symbols, {
			keys: [
				{ name: "name", weight: 0.7 },
				{ name: "context", weight: 0.3 },
			],
			threshold: 0.4,
			includeScore: true,
			includeMatches: true,
			minMatchCharLength: 1,
			findAllMatches: true,
		});
	}

	search(query: string, options: SearchOptions = {}): SearchResult[] {
		const {
			includeFiles = true,
			includeDirs = true,
			types,
			threshold = 0.4,
			limit = 50,
		} = options;

		// Create new Fuse instance with updated threshold if needed
		const currentOptions = {
			keys: [
				{ name: "name", weight: 0.7 },
				{ name: "context", weight: 0.3 },
			],
			threshold,
			includeScore: true,
			includeMatches: true,
			minMatchCharLength: 1,
			findAllMatches: true,
		};

		if (threshold !== 0.4) {
			this.fuse = new Fuse(this.symbols, currentOptions);
		}

		let results = this.fuse.search(query);

		// Filter by symbol types
		if (types && types.length > 0) {
			results = results.filter((result) => types.includes(result.item.type));
		}

		// Filter files/directories if requested
		if (!includeFiles) {
			results = results.filter((result) => result.item.type !== "filename");
		}
		if (!includeDirs) {
			results = results.filter((result) => result.item.type !== "dirname");
		}

		// Limit results
		results = results.slice(0, limit);

		return results.map((result) => ({
			symbol: result.item,
			score: result.score ?? 1,
			matches:
				result.matches?.map((match) => ({
					indices: match.indices[0] ?? [0, 0],
					value: match.value ?? "",
				})) ?? [],
		}));
	}

	updateSymbols(symbols: CodeSymbol[]): void {
		this.symbols = symbols;
		this.fuse.setCollection(symbols);
	}
}
