/**
 * The audio extensions the shell accepts. Mirrors `is_audio` in the Tauri shell
 * (`src-tauri/src/lib.rs`); change both together.
 */
export const AUDIO_EXTS = /\.(wav|mp3|flac|m4a|aac|ogg|oga|opus|aiff|aif|caf|alac|wma)$/i;

/** Beats between the starts of files dropped together, one bar of 4/4. */
export const DROP_STAGGER_BEATS = 4;

/** Start positions for `count` files dropped at `first`, one stagger apart. */
export function dropStarts(first: number, count: number, step = DROP_STAGGER_BEATS): number[] {
	return Array.from({ length: count }, (_, index) => first + index * step);
}
