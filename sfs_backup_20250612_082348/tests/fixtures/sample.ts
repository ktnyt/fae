// TypeScript sample file for testing
export interface User {
    id: number;
    name: string;
    email: string;
}

export type Status = "active" | "inactive" | "pending";

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

export function formatUserName(user: User): string {
    return `${user.name} <${user.email}>`;
}

const internalHelper = (value: string) => value.toUpperCase();

const DEFAULT_TIMEOUT = 5000;