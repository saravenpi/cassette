/**
 * Project path, stem and document title helpers.
 */

/** Resulting event payload when saving a project, or null if cancelled. */
export type SaveProjectEvent = { exportProject: { path: string | null } };

/** Extract the file stem from a path, or 'Untitled' when null or empty. */
export function projectStem(path: string | null): string {
	if (!path) return 'Untitled';
	const base = path.split(/[/\\]/).pop() ?? '';
	if (!base) return 'Untitled';
	const stripped = base.replace(/\.(cassette|json)$/i, '');
	return stripped || base;
}

/** The title shown in the transport and in document.title. */
export function projectTitle(path: string | null, dirty: boolean): string {
	const stem = projectStem(path);
	return dirty ? `${stem} •` : stem;
}

/** Determine the save project event for a regular Save. */
export async function resolveSaveAction(
	projectPath: string | null,
	pick: () => Promise<string | null>
): Promise<SaveProjectEvent | null> {
	if (!projectPath) {
		const path = await pick();
		if (path === null) return null;
		return { exportProject: { path } };
	}
	return { exportProject: { path: null } };
}

/** Determine the save project event for a Save As. */
export async function resolveSaveAsAction(
	pick: () => Promise<string | null>
): Promise<SaveProjectEvent | null> {
	const path = await pick();
	if (path === null) return null;
	return { exportProject: { path } };
}
