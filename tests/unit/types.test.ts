import { describe, test, expect } from "vitest";
import type { CodeSymbol, SymbolType, SearchOptions, SearchResult, IndexedFile } from "../../src/types.js";

describe("Type definitions", () => {
	describe("CodeSymbol", () => {
		test("should accept valid CodeSymbol objects", () => {
			const symbol: CodeSymbol = {
				name: "testFunction",
				type: "function",
				file: "/src/test.ts",
				line: 10,
				column: 5,
				context: "function testFunction() {",
			};

			expect(symbol.name).toBe("testFunction");
			expect(symbol.type).toBe("function");
			expect(symbol.file).toBe("/src/test.ts");
			expect(symbol.line).toBe(10);
			expect(symbol.column).toBe(5);
			expect(symbol.context).toBe("function testFunction() {");
		});

		test("should accept CodeSymbol without optional context", () => {
			const symbol: CodeSymbol = {
				name: "testVariable",
				type: "variable",
				file: "/src/test.ts",
				line: 5,
				column: 1,
			};

			expect(symbol.context).toBeUndefined();
		});
	});

	describe("SymbolType", () => {
		test("should include all expected symbol types", () => {
			const validTypes: SymbolType[] = [
				"function",
				"variable", 
				"class",
				"interface",
				"type",
				"enum",
				"constant",
				"method",
				"property",
				"filename",
				"dirname",
			];

			// This test ensures all types are recognized by TypeScript
			validTypes.forEach(type => {
				const symbol: CodeSymbol = {
					name: "test",
					type: type,
					file: "/test.ts",
					line: 1,
					column: 1,
				};
				expect(symbol.type).toBe(type);
			});
		});
	});

	describe("SearchOptions", () => {
		test("should accept empty options object", () => {
			const options: SearchOptions = {};
			expect(options).toEqual({});
		});

		test("should accept all optional properties", () => {
			const options: SearchOptions = {
				includeFiles: false,
				includeDirs: true,
				types: ["function", "class"],
				threshold: 0.5,
				limit: 10,
			};

			expect(options.includeFiles).toBe(false);
			expect(options.includeDirs).toBe(true);
			expect(options.types).toEqual(["function", "class"]);
			expect(options.threshold).toBe(0.5);
			expect(options.limit).toBe(10);
		});

		test("should accept partial options", () => {
			const options1: SearchOptions = { limit: 5 };
			const options2: SearchOptions = { types: ["variable"] };
			const options3: SearchOptions = { threshold: 0.2, includeFiles: false };

			expect(options1.limit).toBe(5);
			expect(options2.types).toEqual(["variable"]);
			expect(options3.threshold).toBe(0.2);
			expect(options3.includeFiles).toBe(false);
		});
	});

	describe("SearchResult", () => {
		test("should structure search results correctly", () => {
			const symbol: CodeSymbol = {
				name: "testSymbol",
				type: "function",
				file: "/test.ts",
				line: 1,
				column: 1,
			};

			const result: SearchResult = {
				symbol: symbol,
				score: 0.25,
			};

			expect(result.symbol).toBe(symbol);
			expect(result.score).toBe(0.25);
		});
	});

	describe("IndexedFile", () => {
		test("should structure indexed file data correctly", () => {
			const symbols: CodeSymbol[] = [
				{
					name: "testFunction",
					type: "function",
					file: "/test.ts",
					line: 1,
					column: 1,
				}
			];

			const indexedFile: IndexedFile = {
				path: "/test.ts",
				symbols: symbols,
				lastModified: Date.now(),
			};

			expect(indexedFile.path).toBe("/test.ts");
			expect(indexedFile.symbols).toBe(symbols);
			expect(typeof indexedFile.lastModified).toBe("number");
		});
	});
});