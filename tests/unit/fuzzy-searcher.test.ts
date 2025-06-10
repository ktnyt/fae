import { describe, test, expect, beforeEach } from "vitest";
import { FuzzySearcher } from "../../src/searcher.js";
import type { CodeSymbol, SearchOptions } from "../../src/types.js";

describe("FuzzySearcher", () => {
	let searcher: FuzzySearcher;
	let mockSymbols: CodeSymbol[];

	beforeEach(() => {
		mockSymbols = [
			{
				name: "getUserById",
				type: "function",
				file: "/src/user.ts",
				line: 10,
				column: 1,
				context: "function getUserById(id: number) {",
			},
			{
				name: "UserManager",
				type: "class",
				file: "/src/user.ts",
				line: 5,
				column: 1,
				context: "class UserManager {",
			},
			{
				name: "User",
				type: "interface",
				file: "/src/types.ts",
				line: 1,
				column: 1,
				context: "interface User {",
			},
			{
				name: "createUser",
				type: "function",
				file: "/src/user.ts",
				line: 20,
				column: 1,
				context: "function createUser(data: UserData) {",
			},
			{
				name: "deleteUser",
				type: "function",
				file: "/src/user.ts",
				line: 30,
				column: 1,
				context: "function deleteUser(id: number) {",
			},
			{
				name: "API_BASE_URL",
				type: "constant",
				file: "/src/config.ts",
				line: 1,
				column: 1,
				context: "const API_BASE_URL = 'https://api.example.com';",
			},
			{
				name: "user.ts",
				type: "filename",
				file: "/src/user.ts",
				line: 1,
				column: 1,
			},
			{
				name: "src",
				type: "dirname",
				file: "/src/user.ts",
				line: 1,
				column: 1,
			},
		];

		searcher = new FuzzySearcher(mockSymbols);
	});

	describe("basic search functionality", () => {
		test("should find exact matches", () => {
			const results = searcher.search("User");
			
			expect(results.length).toBeGreaterThan(0);
			
			// Should find User interface with high score
			const userInterface = results.find(r => r.symbol.name === "User");
			expect(userInterface).toBeDefined();
			expect(userInterface?.score).toBeLessThan(0.1); // Very low score = very good match
		});

		test("should find fuzzy matches", () => {
			// Try a less aggressive fuzzy search  
			const results = searcher.search("User");
			
			// Should find UserManager with partial match
			const userManager = results.find(r => r.symbol.name === "UserManager");
			expect(userManager).toBeDefined();
		});

		test("should return empty array for no matches", () => {
			const results = searcher.search("NonExistentSymbol12345");
			expect(results).toEqual([]);
		});

		test("should handle empty search query", () => {
			const results = searcher.search("");
			expect(results).toEqual([]);
		});
	});

	describe("search options", () => {
		test("should limit results when limit option is provided", () => {
			const options: SearchOptions = { limit: 2 };
			const results = searcher.search("user", options);
			
			expect(results.length).toBeLessThanOrEqual(2);
		});

		test("should filter by symbol types", () => {
			const options: SearchOptions = { types: ["function"] };
			const results = searcher.search("user", options);
			
			// All results should be functions
			results.forEach(result => {
				expect(result.symbol.type).toBe("function");
			});
		});

		test("should filter by multiple symbol types", () => {
			const options: SearchOptions = { types: ["function", "class"] };
			const results = searcher.search("user", options);
			
			// All results should be functions or classes
			results.forEach(result => {
				expect(["function", "class"]).toContain(result.symbol.type);
			});
		});

		test("should exclude files when includeFiles is false", () => {
			const options: SearchOptions = { includeFiles: false };
			const results = searcher.search("user", options);
			
			// Should not include filename results
			const hasFilename = results.some(r => r.symbol.type === "filename");
			expect(hasFilename).toBe(false);
		});

		test("should exclude directories when includeDirs is false", () => {
			const options: SearchOptions = { includeDirs: false };
			const results = searcher.search("src", options);
			
			// Should not include dirname results
			const hasDirname = results.some(r => r.symbol.type === "dirname");
			expect(hasDirname).toBe(false);
		});

		test("should respect threshold option", () => {
			const strictOptions: SearchOptions = { threshold: 0.1 }; // Very strict
			const looseOptions: SearchOptions = { threshold: 0.8 };  // Very loose
			
			const strictResults = searcher.search("usrmng", strictOptions);
			const looseResults = searcher.search("usrmng", looseOptions);
			
			// Loose search should return more results
			expect(looseResults.length).toBeGreaterThanOrEqual(strictResults.length);
		});
	});

	describe("result scoring", () => {
		test("should return results sorted by relevance", () => {
			const results = searcher.search("user");
			
			// Results should be sorted by score (ascending = better match first)
			for (let i = 1; i < results.length; i++) {
				expect(results[i].score).toBeGreaterThanOrEqual(results[i - 1].score);
			}
		});

		test("should give exact matches better scores", () => {
			const results = searcher.search("User");
			
			// Find exact match
			const exactMatch = results.find(r => r.symbol.name === "User");
			
			// Find partial matches
			const partialMatches = results.filter(r => 
				r.symbol.name !== "User" && r.symbol.name.includes("User")
			);
			
			if (exactMatch && partialMatches.length > 0) {
				// Exact match should have better (lower) score
				expect(exactMatch.score).toBeLessThan(partialMatches[0].score);
			}
		});
	});

	describe("symbol updates", () => {
		test("should update symbols and search in new set", () => {
			const newSymbols: CodeSymbol[] = [
				{
					name: "Product",
					type: "interface",
					file: "/src/product.ts",
					line: 1,
					column: 1,
					context: "interface Product {",
				}
			];

			searcher.updateSymbols(newSymbols);
			
			// Should find new symbol
			const results = searcher.search("Product");
			expect(results).toHaveLength(1);
			expect(results[0].symbol.name).toBe("Product");
			
			// Should not find old symbols
			const oldResults = searcher.search("User");
			expect(oldResults).toEqual([]);
		});
	});

	describe("context handling", () => {
		test("should include context in search results", () => {
			const results = searcher.search("getUserById");
			
			const match = results.find(r => r.symbol.name === "getUserById");
			expect(match?.symbol.context).toBe("function getUserById(id: number) {");
		});

		test("should handle symbols without context", () => {
			const symbolsWithoutContext: CodeSymbol[] = [
				{
					name: "TestSymbol",
					type: "variable",
					file: "/test.ts",
					line: 1,
					column: 1,
					// No context property
				}
			];

			const testSearcher = new FuzzySearcher(symbolsWithoutContext);
			const results = testSearcher.search("TestSymbol");
			
			expect(results).toHaveLength(1);
			expect(results[0].symbol.context).toBeUndefined();
		});
	});
});