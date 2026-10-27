export interface SelectTrackOptions {
	shift?: boolean;
	toggle?: boolean;
	totalTracks?: number;
}

export class TrackSelectionStore {
	selectedTracks = $state<Set<number>>(new Set([0]));
	lastClicked = $state<number>(0);

	isSelected(index: number): boolean {
		return this.selectedTracks.has(index);
	}

	selectTrack(index: number, options?: SelectTrackOptions): void {
		if (options?.shift) {
			const start = Math.min(this.lastClicked, index);
			const end = Math.max(this.lastClicked, index);
			const next = new Set<number>();
			for (let i = start; i <= end; i++) {
				if (options.totalTracks !== undefined && (i < 0 || i >= options.totalTracks)) {
					continue;
				}
				next.add(i);
			}
			this.selectedTracks = next.size > 0 ? next : new Set([index]);
		} else if (options?.toggle) {
			const next = new Set(this.selectedTracks);
			if (next.has(index)) {
				if (next.size > 1) {
					next.delete(index);
				}
			} else {
				next.add(index);
			}
			this.selectedTracks = next;
			this.lastClicked = index;
		} else {
			this.selectedTracks = new Set([index]);
			this.lastClicked = index;
		}
	}

	selectAll(totalTracks: number): void {
		if (totalTracks <= 0) return;
		const next = new Set<number>();
		for (let i = 0; i < totalTracks; i++) {
			next.add(i);
		}
		this.selectedTracks = next;
	}

	clear(activeTrack?: number): void {
		const active = activeTrack ?? 0;
		this.selectedTracks = new Set([active]);
		this.lastClicked = active;
	}

	deleteTracks(deleted: number[]): void {
		if (deleted.length === 0) return;
		const deletedSet = new Set(deleted);
		const sortedDeleted = Array.from(deletedSet).sort((a, b) => a - b);
		const next = new Set<number>();
		for (const index of this.selectedTracks) {
			if (deletedSet.has(index)) continue;
			let shift = 0;
			for (const d of sortedDeleted) {
				if (d < index) shift += 1;
				else break;
			}
			next.add(index - shift);
		}
		this.selectedTracks = next;
		let lastClickedShift = 0;
		for (const d of sortedDeleted) {
			if (d < this.lastClicked) lastClickedShift += 1;
			else break;
		}
		this.lastClicked = Math.max(0, this.lastClicked - lastClickedShift);
	}

	reorderTrack(from: number, to: number): void {
		if (from === to) return;
		const next = new Set<number>();
		for (const index of this.selectedTracks) {
			if (index === from) {
				next.add(to);
			} else if (from < to && index > from && index <= to) {
				next.add(index - 1);
			} else if (from > to && index >= to && index < from) {
				next.add(index + 1);
			} else {
				next.add(index);
			}
		}
		this.selectedTracks = next;
		if (this.lastClicked === from) {
			this.lastClicked = to;
		} else if (from < to && this.lastClicked > from && this.lastClicked <= to) {
			this.lastClicked -= 1;
		} else if (from > to && this.lastClicked >= to && this.lastClicked < from) {
			this.lastClicked += 1;
		}
	}
}

export const trackSelection = new TrackSelectionStore();
