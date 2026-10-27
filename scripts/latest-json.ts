import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { parseArgs } from 'node:util';

export const ASSETS: Record<string, string> = {
	'linux-x86_64': 'Cassette-linux-x64.AppImage',
	'darwin-aarch64': 'Cassette-macos-universal.app.tar.gz',
	'darwin-x86_64': 'Cassette-macos-universal.app.tar.gz',
	'windows-x86_64': 'Cassette-windows-x64-setup.exe'
};

export interface Latest {
	version: string;
	notes: string;
	pub_date: string;
	platforms: Record<string, { url: string; signature: string }>;
}

export interface LatestInput {
	tag: string;
	repo: string;
	notes: string;
	now: Date;
	signatures: Record<string, string>;
}

export function changelogSection(changelog: string, version: string): string {
	const lines = changelog.split('\n');
	const start = lines.findIndex((line) => line.startsWith(`## [${version}]`));
	if (start < 0) return '';
	const rest = lines.slice(start + 1);
	const end = rest.findIndex((line) => line.startsWith('## ['));
	return (end < 0 ? rest : rest.slice(0, end)).join('\n').trim();
}

export function buildLatest(input: LatestInput): Latest {
	const version = input.tag.replace(/^v/, '');
	if (!/^\d+\.\d+\.\d+/.test(version)) throw new Error(`The tag ${input.tag} is not a version`);
	const base = `https://github.com/${input.repo}/releases/download/${encodeURIComponent(input.tag)}`;
	const platforms: Latest['platforms'] = {};
	for (const [platform, asset] of Object.entries(ASSETS)) {
		const signature = input.signatures[asset]?.trim();
		if (!signature) throw new Error(`No signature for ${asset}`);
		platforms[platform] = { url: `${base}/${asset}`, signature };
	}
	const pub_date = input.now.toISOString().replace(/\.\d{3}Z$/, 'Z');
	const notes = input.notes || `See ${input.repo} release ${input.tag}.`;
	return { version, notes, pub_date, platforms };
}

function readSignatures(dir: string): Record<string, string> {
	const signatures: Record<string, string> = {};
	for (const asset of new Set(Object.values(ASSETS))) {
		const file = join(dir, `${asset}.sig`);
		if (!existsSync(file)) throw new Error(`${file} is missing`);
		signatures[asset] = readFileSync(file, 'utf8');
	}
	return signatures;
}

function main(): void {
	const { values } = parseArgs({
		options: {
			tag: { type: 'string' },
			repo: { type: 'string' },
			dir: { type: 'string' },
			changelog: { type: 'string' },
			out: { type: 'string' }
		}
	});
	const { tag, repo, dir, changelog, out } = values;
	if (!tag || !repo || !dir || !changelog || !out) {
		throw new Error('Pass --tag, --repo, --dir, --changelog and --out');
	}
	const notes = changelogSection(readFileSync(changelog, 'utf8'), tag.replace(/^v/, ''));
	const latest = buildLatest({ tag, repo, notes, now: new Date(), signatures: readSignatures(dir) });
	writeFileSync(out, `${JSON.stringify(latest, null, 2)}\n`);
	console.log(`Wrote ${out} for ${latest.version}`);
}

if (import.meta.main) main();
