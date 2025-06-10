import { defineConfig } from "vitest/config";

export default defineConfig({
	test: {
		globals: true,
		environment: "node",
		include: ["src/**/*.test.ts", "tests/**/*.test.ts"],
		coverage: {
			provider: "c8",
			reporter: ["text", "json", "html"],
			exclude: ["node_modules/", "dist/", "*.config.*"],
		},
	},
});