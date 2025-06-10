export interface CodeSymbol {
	name: string;
	type: SymbolType;
	file: string;
	line: number;
	column: number;
	context?: string;
}

export type SymbolType =
	| "function"
	| "variable"
	| "class"
	| "interface"
	| "type"
	| "enum"
	| "constant"
	| "method"
	| "property"
	| "filename"
	| "dirname";

export interface IndexedFile {
	path: string;
	symbols: CodeSymbol[];
	lastModified: number;
}

export interface SearchOptions {
	includeFiles?: boolean;
	includeDirs?: boolean;
	types?: SymbolType[];
	threshold?: number;
	limit?: number;
}

export interface SearchResult {
	symbol: CodeSymbol;
	score: number;
	matches: Array<{
		indices: [number, number];
		value: string;
	}>;
}
