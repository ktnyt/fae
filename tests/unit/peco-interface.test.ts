import { describe, test, expect, beforeEach, afterEach, vi } from "vitest";
import { FuzzySearcher } from "../../src/searcher.js";
import { PecoInterface } from "../../src/peco-interface.js";
import type { CodeSymbol } from "../../src/types.js";

// Mock blessed to avoid terminal issues in test environment
vi.mock("blessed", () => ({
	default: {
		screen: vi.fn(() => ({
			smartCSR: true,
			title: "test",
			key: vi.fn(),
			on: vi.fn(),
			render: vi.fn(),
			destroy: vi.fn(),
		})),
		textbox: vi.fn(() => ({
			setLabel: vi.fn(),
			getValue: vi.fn(),
			on: vi.fn(),
			focus: vi.fn(),
		})),
		list: vi.fn(() => ({
			setItems: vi.fn(),
			select: vi.fn(),
			on: vi.fn(),
		})),
		box: vi.fn(() => ({
			setContent: vi.fn(),
		})),
	},
}));

describe("PecoInterface", () => {
	let peco: PecoInterface;
	let searcher: FuzzySearcher;
	let mockSymbols: CodeSymbol[];

	beforeEach(() => {
		// Create mock symbols for testing
		mockSymbols = [
			{
				name: "Calculator",
				type: "class",
				file: "/test/Calculator.ts",
				line: 1,
				column: 1,
				context: "class Calculator {",
			},
			{
				name: "add",
				type: "function",
				file: "/test/Calculator.ts",
				line: 5,
				column: 2,
				context: "add(a: number, b: number) {",
			},
			{
				name: "sample.ts",
				type: "filename",
				file: "/test/sample.ts",
				line: 1,
				column: 1,
			},
			{
				name: "test",
				type: "dirname",
				file: "/test/sample.ts",
				line: 1,
				column: 1,
			},
			{
				name: "ApiService",
				type: "class",
				file: "/test/api.ts",
				line: 10,
				column: 1,
				context: "class ApiService {",
			},
		];

		searcher = new FuzzySearcher(mockSymbols);
		peco = new PecoInterface(searcher, mockSymbols);
	});

	afterEach(() => {
		vi.clearAllMocks();
	});

	describe("Search Mode Detection", () => {
		test("should detect fuzzy search mode by default", () => {
			// Access private method through any casting for testing
			const pecoAny = peco as any;
			const mode = pecoAny.detectSearchMode("Calculator");
			
			expect(mode.name).toBe("Fuzzy");
			expect(mode.prefix).toBe("");
			expect(mode.icon).toBe("🔍");
		});

		test("should detect symbol search mode with # prefix", () => {
			const pecoAny = peco as any;
			const mode = pecoAny.detectSearchMode("#Calculator");
			
			expect(mode.name).toBe("Symbol");
			expect(mode.prefix).toBe("#");
			expect(mode.icon).toBe("🏷️");
		});

		test("should detect file search mode with > prefix", () => {
			const pecoAny = peco as any;
			const mode = pecoAny.detectSearchMode(">sample");
			
			expect(mode.name).toBe("File");
			expect(mode.prefix).toBe(">");
			expect(mode.icon).toBe("📁");
		});

		test("should detect regex search mode with / prefix", () => {
			const pecoAny = peco as any;
			const mode = pecoAny.detectSearchMode("/Cal.*");
			
			expect(mode.name).toBe("Regex");
			expect(mode.prefix).toBe("/");
			expect(mode.icon).toBe("🔧");
		});
	});

	describe("Query Extraction", () => {
		test("should extract query without prefix for fuzzy search", () => {
			const pecoAny = peco as any;
			// Set mode to fuzzy first
			pecoAny.currentSearchMode = pecoAny.searchModes[0];
			
			const query = pecoAny.extractSearchQuery("Calculator");
			expect(query).toBe("Calculator");
		});

		test("should extract query without # prefix for symbol search", () => {
			const pecoAny = peco as any;
			// Set mode to symbol search
			pecoAny.currentSearchMode = pecoAny.searchModes[1];
			
			const query = pecoAny.extractSearchQuery("#Calculator");
			expect(query).toBe("Calculator");
		});

		test("should extract query without > prefix for file search", () => {
			const pecoAny = peco as any;
			// Set mode to file search
			pecoAny.currentSearchMode = pecoAny.searchModes[2];
			
			const query = pecoAny.extractSearchQuery(">sample");
			expect(query).toBe("sample");
		});

		test("should extract query without / prefix for regex search", () => {
			const pecoAny = peco as any;
			// Set mode to regex search
			pecoAny.currentSearchMode = pecoAny.searchModes[3];
			
			const query = pecoAny.extractSearchQuery("/Cal.*");
			expect(query).toBe("Cal.*");
		});
	});

	describe("Mode-Specific Search", () => {
		test("should perform symbol search excluding files and directories", () => {
			const pecoAny = peco as any;
			// Set mode to symbol search
			pecoAny.currentSearchMode = pecoAny.searchModes[1];
			
			const results = pecoAny.performModeSpecificSearch("Calculator");
			
			// Should find Calculator class but not files/directories
			expect(results.length).toBeGreaterThan(0);
			expect(results.some(r => r.symbol.name === "Calculator")).toBe(true);
			// Should not include filename or dirname types
			expect(results.every(r => r.symbol.type !== "filename" && r.symbol.type !== "dirname")).toBe(true);
		});

		test("should perform file search including only files and directories", () => {
			const pecoAny = peco as any;
			// Set mode to file search
			pecoAny.currentSearchMode = pecoAny.searchModes[2];
			
			const results = pecoAny.performModeSpecificSearch("sample");
			
			// Should only include filename and dirname types
			expect(results.every(r => r.symbol.type === "filename" || r.symbol.type === "dirname")).toBe(true);
		});

		test("should find files without extension in file search mode", () => {
			const pecoAny = peco as any;
			
			// Test the new performFileSearch method directly
			const results = pecoAny.performFileSearch("sample", 10);
			
			// Should find sample.ts when searching for "sample"
			expect(results.some(r => r.symbol.name === "sample.ts")).toBe(true);
			expect(results.every(r => r.symbol.type === "filename" || r.symbol.type === "dirname")).toBe(true);
		});

		test("should prioritize prefix matches in file search", () => {
			const pecoAny = peco as any;
			
			// Add a mock symbol that starts with "test"
			const additionalSymbols = [
				{
					name: "test-file.js",
					type: "filename" as const,
					file: "/path/test-file.js",
					line: 1,
					column: 1,
				},
				{
					name: "another-test.js",
					type: "filename" as const,
					file: "/path/another-test.js", 
					line: 1,
					column: 1,
				},
			];
			
			pecoAny.symbols = [...mockSymbols, ...additionalSymbols];
			
			const results = pecoAny.performFileSearch("test", 10);
			
			// Should find files containing "test"
			expect(results.length).toBeGreaterThan(0);
			expect(results.every(r => r.symbol.type === "filename" || r.symbol.type === "dirname")).toBe(true);
			
			// Results should be sorted by relevance (prefix matches first)
			if (results.length > 1) {
				// test-file.js should come before another-test.js (prefix vs partial match)
				const testFileIndex = results.findIndex(r => r.symbol.name === "test-file.js");
				const anotherTestIndex = results.findIndex(r => r.symbol.name === "another-test.js");
				
				if (testFileIndex >= 0 && anotherTestIndex >= 0) {
					expect(testFileIndex).toBeLessThan(anotherTestIndex);
				}
			}
		});

		test("should perform regex search with valid patterns", () => {
			const pecoAny = peco as any;
			
			const results = pecoAny.performRegexSearch("Cal.*", 100);
			
			// Should find Calculator symbols
			expect(results.some(r => r.symbol.name === "Calculator")).toBe(true);
		});

		test("should handle invalid regex patterns gracefully", () => {
			const pecoAny = peco as any;
			
			// Invalid regex pattern
			const results = pecoAny.performRegexSearch("[invalid", 100);
			
			// Should return empty array for invalid regex
			expect(results).toEqual([]);
		});

		test("should perform default fuzzy search", () => {
			const pecoAny = peco as any;
			// Set mode to fuzzy search
			pecoAny.currentSearchMode = pecoAny.searchModes[0];
			
			const results = pecoAny.performModeSpecificSearch("Calculator");
			
			// Should perform normal fuzzy search
			expect(results.length).toBeGreaterThan(0);
			expect(results.some(r => r.symbol.name === "Calculator")).toBe(true);
		});
	});

	describe("Empty Query Handling", () => {
		test("should show all symbols when query is empty", () => {
			const pecoAny = peco as any;
			
			// Simulate empty query search
			pecoAny.performSearch("");
			
			// Should show symbols (limited to 100)
			expect(pecoAny.currentResults.length).toBeGreaterThan(0);
			expect(pecoAny.currentResults.length).toBeLessThanOrEqual(100);
		});
	});

	describe("Search Mode Integration", () => {
		test("should change mode when prefix is detected", () => {
			const pecoAny = peco as any;
			
			// Start with fuzzy mode
			expect(pecoAny.currentSearchMode.name).toBe("Fuzzy");
			
			// Simulate search with # prefix
			pecoAny.performSearch("#Calculator");
			
			// Should switch to Symbol mode
			expect(pecoAny.currentSearchMode.name).toBe("Symbol");
		});

		test("should return to fuzzy mode when no prefix is used", () => {
			const pecoAny = peco as any;
			
			// Start with symbol mode
			pecoAny.performSearch("#Calculator");
			expect(pecoAny.currentSearchMode.name).toBe("Symbol");
			
			// Search without prefix
			pecoAny.performSearch("Calculator");
			
			// Should return to fuzzy mode
			expect(pecoAny.currentSearchMode.name).toBe("Fuzzy");
		});
	});

	describe("Search Results Formatting", () => {
		test("should format search results correctly", () => {
			const pecoAny = peco as any;
			
			// Perform a search
			pecoAny.performSearch("Calculator");
			
			// Should have results with proper structure
			expect(pecoAny.currentResults).toBeDefined();
			expect(Array.isArray(pecoAny.currentResults)).toBe(true);
			
			if (pecoAny.currentResults.length > 0) {
				const result = pecoAny.currentResults[0];
				expect(result).toHaveProperty("symbol");
				expect(result).toHaveProperty("score");
				expect(result).toHaveProperty("matches");
			}
		});
	});
});