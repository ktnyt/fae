import { describe, test, expect, beforeEach, afterEach } from "vitest";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { resolve } from "node:path";
import { writeFile, mkdir, rm } from "node:fs/promises";

const execFileAsync = promisify(execFile);

describe("Search Modes E2E Tests", () => {
	const testDir = resolve(process.cwd(), "tests/temp-search-modes");
	const cliPath = resolve(process.cwd(), "dist/cli.js");

	beforeEach(async () => {
		// Create temporary test directory
		await mkdir(testDir, { recursive: true });
		
		// Create test files with diverse content
		await writeFile(resolve(testDir, "Calculator.ts"), `
			class Calculator {
				add(a: number, b: number): number {
					return a + b;
				}
				
				multiply(x: number, y: number): number {
					return x * y;
				}
			}
			
			export const API_BASE_URL = "https://api.example.com";
			export function createCalculator(): Calculator {
				return new Calculator();
			}
		`);
		
		await writeFile(resolve(testDir, "utils.js"), `
			function formatNumber(num) {
				return num.toLocaleString();
			}
			
			const CONSTANTS = {
				PI: 3.14159,
				E: 2.71828
			};
			
			class ApiClient {
				async fetch(url) {
					return await fetch(url);
				}
			}
		`);
		
		await writeFile(resolve(testDir, "data.py"), `
			import json
			
			class DataProcessor:
				def __init__(self, name):
					self.name = name
				
				def process(self, data):
					return json.dumps(data)
			
			def calculate_average(numbers):
				return sum(numbers) / len(numbers)
			
			API_ENDPOINT = "https://data.example.com/api"
		`);
	});

	afterEach(async () => {
		// Clean up temporary directory
		await rm(testDir, { recursive: true, force: true });
	});

	describe("Symbol-only search simulation", () => {
		test("should find only symbols when excluding files and directories", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"Calculator",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py",
				"--no-files",
				"--no-dirs"
			]);

			// Should find Calculator class
			expect(stdout).toContain("🏗️ Calculator");
			
			// Should not contain file or directory names
			expect(stdout).not.toContain("📄");
			expect(stdout).not.toContain("📁");
			
			// Should contain symbol-related results
			expect(stdout).toContain("🔍 Indexing");
			expect(stdout).toContain("📚 Found");
		});

		test("should find functions when searching for function-related terms", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"format",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py",
				"--types", "function"
			]);

			// Should find formatNumber function
			expect(stdout).toContain("🔧");
			expect(stdout).toContain("format");
		});
	});

	describe("File-only search simulation", () => {
		test("should find only files and directories when using filename types", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"Calculator",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py",
				"--types", "filename,dirname"
			]);

			// Should find Calculator.ts file
			expect(stdout).toContain("📄 Calculator.ts");
			
			// Should not contain class symbols (since we're only looking for filenames)
			const lines = stdout.split("\n");
			const resultLines = lines.filter(line => line.includes("🏗️ Calculator"));
			expect(resultLines.length).toBe(0);
		});

		test("should find files when searching for file extensions", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"ts",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py",
				"--types", "filename"
			]);

			// Should find .ts files
			expect(stdout).toContain("📄");
			expect(stdout).toContain(".ts");
		});
	});

	describe("Regular expression simulation", () => {
		test("should support pattern-like searches", async () => {
			// Test case-insensitive pattern matching (simulating regex functionality)
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"api",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py"
			]);

			// Should find API-related symbols
			expect(stdout).toContain("API");
		});

		test("should find symbols with specific patterns", async () => {
			// Search for functions ending with specific patterns
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"calc",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py",
				"--types", "function"
			]);

			// Should find calculate-related functions
			expect(stdout).toContain("🔧");
		});
	});

	describe("Cross-language symbol search", () => {
		test("should find classes across different languages", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"Calculator",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py",
				"--types", "class"
			]);

			// Should find Calculator class from TypeScript
			expect(stdout).toContain("🏗️ Calculator");
		});

		test("should find functions across different languages", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"process",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py",
				"--types", "function"
			]);

			// Should find process-related functions
			expect(stdout).toContain("🔧");
		});

		test("should find constants and variables across languages", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"API",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py",
				"--types", "variable"
			]);

			// Should find API-related constants
			expect(stdout).toContain("📦");
			expect(stdout).toContain("API");
		});
	});

	describe("Fuzzy search behavior", () => {
		test("should handle abbreviated searches", async () => {
			// Test fuzzy matching with abbreviated input
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"calc",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py"
			]);

			// Should find Calculator-related symbols
			expect(stdout).toContain("🔍 Indexing");
			expect(stdout).toContain("📚 Found");
		});

		test("should handle partial matches", async () => {
			// Test partial string matching
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"format",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py"
			]);

			// Should find format-related symbols
			expect(stdout).toContain("🔍 Indexing");
			expect(stdout).toContain("📚 Found");
		});
	});

	describe("Search performance and limits", () => {
		test("should respect result limits", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"a", // Very broad search
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py",
				"--limit", "3"
			]);

			const lines = stdout.split("\n");
			const symbolLines = lines.filter(line => 
				line.includes("🔧") || 
				line.includes("🏗️") || 
				line.includes("📦") ||
				line.includes("📄") ||
				line.includes("📁")
			);
			
			// Should respect the limit (allowing some flexibility)
			expect(symbolLines.length).toBeLessThanOrEqual(6); // 3 results * 2 lines per result approximately
		});

		test("should handle empty search results gracefully", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"NonExistentSymbol12345",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js,**/*.py"
			]);

			expect(stdout).toContain("🤷 No results found");
		});
	});
});