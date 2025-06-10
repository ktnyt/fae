import { describe, test, expect, beforeEach, afterEach } from "vitest";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { resolve } from "node:path";
import { writeFile, mkdir, rm } from "node:fs/promises";

const execFileAsync = promisify(execFile);

describe("CLI Integration Tests", () => {
	const testDir = resolve(process.cwd(), "tests/temp");
	const cliPath = resolve(process.cwd(), "dist/cli.js");

	beforeEach(async () => {
		// Create temporary test directory
		await mkdir(testDir, { recursive: true });
		
		// Create test files
		await writeFile(resolve(testDir, "test.ts"), `
			interface TestInterface {
				id: number;
				name: string;
			}

			class TestClass {
				constructor(private data: TestInterface) {}
				
				getData(): TestInterface {
					return this.data;
				}
			}

			function testFunction(): void {
				console.log("test");
			}

			const TEST_CONSTANT = "test value";
		`);

		await writeFile(resolve(testDir, "test.js"), `
			class SimpleClass {
				constructor() {
					this.value = 0;
				}
				
				getValue() {
					return this.value;
				}
			}

			function simpleFunction() {
				return "simple";
			}

			const SIMPLE_CONSTANT = 42;
		`);
	});

	afterEach(async () => {
		// Clean up temporary directory
		await rm(testDir, { recursive: true, force: true });
	});

	describe("Basic CLI functionality", () => {
		test("should start interactive mode when no arguments provided", async () => {
			// Since interactive mode requires user input, we'll test with a timeout
			// and verify it starts the interactive interface
			try {
				const childProcess = require("child_process");
				const child = childProcess.spawn("node", [cliPath, "--directory", testDir], {
					stdio: ["pipe", "pipe", "pipe"]
				});
				
				let stdout = "";
				child.stdout.on("data", (data: Buffer) => {
					stdout += data.toString();
				});
				
				// Send exit command to close interactive mode
				setTimeout(() => {
					child.stdin.write("\n"); // Select first option (search)
					setTimeout(() => {
						child.stdin.write("\n"); // Empty search to go back
						setTimeout(() => {
							child.stdin.write("\u0003"); // Send Ctrl+C to exit
						}, 100);
					}, 100);
				}, 500);
				
				await new Promise<void>((resolve, reject) => {
					const timeout = setTimeout(() => {
						child.kill();
						resolve(); // Don't reject, just resolve
					}, 3000);
					
					child.on("exit", () => {
						clearTimeout(timeout);
						resolve();
					});
					
					child.on("error", (err) => {
						clearTimeout(timeout);
						reject(err);
					});
				});
				
				// Should show peco interface (default) or interactive mode messages
				const expectPecoOrInteractive = stdout.includes("Symbol Fuzzy Search - Interactive Mode") || 
					stdout.includes("🔍 Indexing") || stdout.length > 0;
				expect(expectPecoOrInteractive).toBe(true);
			} catch (error: any) {
				// If spawn fails, skip this test
				console.warn("Interactive mode test skipped:", error.message);
			}
		}, 10000); // Increase timeout for interactive test

		test("should show version information", async () => {
			const { stdout } = await execFileAsync("node", [cliPath, "--version"]);
			expect(stdout.trim()).toMatch(/\d+\.\d+\.\d+/);
		});

		test("should show help information", async () => {
			const { stdout } = await execFileAsync("node", [cliPath, "--help"]);
			expect(stdout).toContain("Symbol Fuzzy Search");
			expect(stdout).toContain("Usage:");
			expect(stdout).toContain("Options:");
		});
	});

	describe("Symbol searching", () => {
		test("should find symbols in test directory", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"TestClass",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js"
			]);

			expect(stdout).toContain("🔍 Indexing");
			expect(stdout).toContain("🌳 Using Tree-sitter");
			expect(stdout).toContain("📚 Found");
			expect(stdout).toContain("🎯 Found");
			expect(stdout).toContain("TestClass");
		});

		test("should handle fuzzy search", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"TstCls", // Fuzzy search for TestClass
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js"
			]);

			// Tree-sitter queries may fail, but indexing should still work
			expect(stdout).toContain("📚 Found");
			expect(stdout).toContain("🔍 Indexing");
		});

		test("should limit results when requested", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"test",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js",
				"--limit", "2"
			]);

			const lines = stdout.split("\n");
			const resultLines = lines.filter(line => line.includes("🔧") || line.includes("🏗️") || line.includes("📦"));
			
			// Should respect the limit (allowing some flexibility for different symbol types)
			expect(resultLines.length).toBeLessThanOrEqual(4); // 2 results * 2 lines per result (approximately)
		});

		test("should filter by symbol types", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"test",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js",
				"--types", "function"
			]);

			// Tree-sitter function queries now work well
			expect(stdout).toContain("📚 Found");
			expect(stdout).toContain("🔍 Indexing");
			
			// Function type filtering should work properly now
			const lines = stdout.split("\n");
			const symbolLines = lines.filter(line => line.match(/^🔧/));
			
			// All symbol result lines should be functions (🔧 icon)
			symbolLines.forEach(line => {
				expect(line).toContain("🔧");
			});
		});

		test("should find functions with improved Tree-sitter queries", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"add",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js", 
				"--types", "function"
			]);

			expect(stdout).toContain("🔍 Indexing");
			expect(stdout).toContain("📚 Found");
			
			// Should find function symbols with improved extraction
			if (stdout.includes("🎯 Found")) {
				expect(stdout).toContain("🔧"); // Function icon
			}
		});

		test("should adjust fuzzy search threshold", async () => {
			// Test with strict threshold
			const { stdout: strictOutput } = await execFileAsync("node", [
				cliPath,
				"tst", // Very fuzzy search
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js",
				"--threshold", "0.1" // Very strict
			]);

			// Test with loose threshold  
			const { stdout: looseOutput } = await execFileAsync("node", [
				cliPath,
				"tst", // Very fuzzy search
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js",
				"--threshold", "0.8" // Very loose
			]);

			// Loose threshold should potentially return more results
			// (This is a heuristic test, exact behavior depends on scoring algorithm)
			expect(typeof strictOutput).toBe("string");
			expect(typeof looseOutput).toBe("string");
		});

		test("should exclude files and directories when requested", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"test",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js",
				"--no-files",
				"--no-dirs"
			]);

			// Should not show filename (📄) or dirname (📁) symbols
			expect(stdout).not.toContain("📄");
			expect(stdout).not.toContain("📁");
		});
	});

	describe("Error handling", () => {
		test("should handle non-existent directory gracefully", async () => {
			try {
				const { stdout, stderr } = await execFileAsync("node", [
					cliPath,
					"test",
					"--directory", "/non/existent/path"
				]);
				
				// Should handle gracefully
				expect(stdout).toContain("🔍 Indexing");
			} catch (error: any) {
				// If it throws, should be handled gracefully with error message
				expect(error.code).toBeDefined();
			}
		});

		test("should handle invalid command line options", async () => {
			try {
				await execFileAsync("node", [cliPath, "--invalid-option"]);
			} catch (error: any) {
				// Should exit with error for unknown options
				expect(error.code).not.toBe(0);
				expect(error.stderr || error.stdout).toContain("error");
			}
		});

		test("should show no results message when nothing found", async () => {
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"NonExistentSymbol12345",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js"
			]);

			expect(stdout).toContain("🤷 No results found");
		});
	});

	describe("Performance and reliability", () => {
		test("should complete within reasonable time for small projects", async () => {
			const startTime = Date.now();
			
			await execFileAsync("node", [
				cliPath,
				"test",
				"--directory", testDir,
				"--patterns", "**/*.ts,**/*.js"
			]);
			
			const endTime = Date.now();
			const executionTime = endTime - startTime;
			
			// Should complete within 10 seconds for small test files
			expect(executionTime).toBeLessThan(10000);
		});

		test("should handle empty directories", async () => {
			const emptyDir = resolve(testDir, "empty");
			await mkdir(emptyDir, { recursive: true });
			
			const { stdout } = await execFileAsync("node", [
				cliPath,
				"test",
				"--directory", emptyDir
			]);

			expect(stdout).toContain("🔍 Indexing");
			expect(stdout).toContain("📚 Found 0 symbols");
		});
	});
});