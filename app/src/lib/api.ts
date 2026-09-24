import { invoke } from '@tauri-apps/api/core';

import type { EngineEvent, UiState } from './types';

/** Read the current snapshot. Cheap enough to call once per animation frame. */
export function state(): Promise<UiState> {
	return invoke<UiState>('state');
}

/** Apply one intent. Resolves once the engine has taken it. */
export function send(event: EngineEvent): Promise<void> {
	return invoke<void>('send', { event });
}

/** Fire an intent without waiting, for note-off and other high-rate messages. */
export function sendNow(event: EngineEvent): void {
	void send(event).catch((error) => {
		console.error('zboub69: intent rejected', event, error);
	});
}