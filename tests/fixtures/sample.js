// JavaScript sample file for testing
class Calculator {
	constructor() {
		this.result = 0;
	}

	add(value) {
		this.result += value;
		return this;
	}

	multiply(value) {
		this.result *= value;
		return this;
	}

	getValue() {
		return this.result;
	}
}

function createCalculator() {
	return new Calculator();
}

const helper = (x, y) => x + y;

const API_BASE_URL = "https://api.example.com";

let globalCounter = 0;

var deprecatedVariable = "old-style";

module.exports = {
	Calculator,
	createCalculator,
	helper,
	API_BASE_URL,
};