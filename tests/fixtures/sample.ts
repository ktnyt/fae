// TypeScript sample file for testing
export interface User {
	id: number;
	name: string;
	email: string;
}

export type UserRole = "admin" | "user" | "guest";

export enum Status {
	ACTIVE = "active",
	INACTIVE = "inactive",
	PENDING = "pending",
}

export class UserManager {
	private users: User[] = [];

	constructor(private readonly apiUrl: string) {}

	async addUser(user: User): Promise<void> {
		this.users.push(user);
	}

	findUserById(id: number): User | undefined {
		return this.users.find((user) => user.id === id);
	}

	get userCount(): number {
		return this.users.length;
	}
}

export const DEFAULT_TIMEOUT = 5000;

export function formatUserName(user: User): string {
	return `${user.name} <${user.email}>`;
}

const internalHelper = (data: any) => {
	return data.toString();
};