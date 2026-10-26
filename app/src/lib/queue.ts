/** First wait after a failed poll, in milliseconds. */
export const BACKOFF_MIN_MS = 250;
/** Longest wait between polls while the engine keeps failing. */
export const BACKOFF_MAX_MS = 4000;

/** Latest value per key, drained once per frame so each key sends at most once. */
export class KeyedQueue<T> {
	private items = new Map<number, T>();

	set(key: number, value: T): void {
		this.items.set(key, value);
	}

	drain(): T[] {
		const out = [...this.items.values()];
		this.items.clear();
		return out;
	}
}

/** Hands out ticket numbers so only the newest async request may apply its result. */
export class Latest {
	private ticket = 0;

	begin(): number {
		this.ticket += 1;
		return this.ticket;
	}

	isCurrent(ticket: number): boolean {
		return ticket === this.ticket;
	}
}

/** The wait after one more consecutive failure, doubling up to a cap. */
export function nextBackoff(previous: number): number {
	return previous === 0 ? BACKOFF_MIN_MS : Math.min(previous * 2, BACKOFF_MAX_MS);
}
