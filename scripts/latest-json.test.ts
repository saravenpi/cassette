import { describe, expect, test } from 'bun:test';
import { ASSETS, buildLatest, changelogSection } from './latest-json';

const CHANGELOG = `# Changelog

## [Unreleased]

## [0.16.3] - 2026-10-27

### Added

- Self-update.

## [0.16.2] - 2026-10-25

### Added

- Sidebar.
`;

function signatures(): Record<string, string> {
	const out: Record<string, string> = {};
	for (const asset of new Set(Object.values(ASSETS))) {
		out[asset] = `sig-of-${asset}\n`;
	}
	return out;
}

const NOW = new Date('2026-10-27T09:30:15.123Z');

describe('changelogSection', () => {
	test('returns the body of the matching version only', () => {
		expect(changelogSection(CHANGELOG, '0.16.3')).toBe('### Added\n\n- Self-update.');
		expect(changelogSection(CHANGELOG, '0.16.2')).toBe('### Added\n\n- Sidebar.');
	});

	test('returns an empty string for an unknown version', () => {
		expect(changelogSection(CHANGELOG, '9.9.9')).toBe('');
	});
});

describe('buildLatest', () => {
	const latest = buildLatest({
		tag: 'v0.16.3',
		repo: 'Saravenpi/cassette',
		notes: 'notes',
		now: NOW,
		signatures: signatures()
	});

	test('has the manifest fields', () => {
		expect(latest.version).toBe('0.16.3');
		expect(latest.notes).toBe('notes');
		expect(latest.pub_date).toBe('2026-10-27T09:30:15Z');
	});

	test('lists the four platforms with asset urls and signature text', () => {
		expect(Object.keys(latest.platforms).sort()).toEqual([
			'darwin-aarch64',
			'darwin-x86_64',
			'linux-x86_64',
			'windows-x86_64'
		]);
		const linux = latest.platforms['linux-x86_64'];
		expect(linux.url).toBe(
			'https://github.com/Saravenpi/cassette/releases/download/v0.16.3/Cassette-linux-x64.AppImage'
		);
		expect(linux.signature).toBe('sig-of-Cassette-linux-x64.AppImage');
		expect(latest.platforms['darwin-aarch64'].url).toBe(latest.platforms['darwin-x86_64'].url);
	});

	test('refuses a missing signature or a tag that is not a version', () => {
		const missing = { ...signatures(), 'Cassette-windows-x64-setup.exe': ' ' };
		expect(() =>
			buildLatest({ tag: 'v0.16.3', repo: 'a/b', notes: '', now: NOW, signatures: missing })
		).toThrow();
		expect(() =>
			buildLatest({ tag: 'nightly', repo: 'a/b', notes: '', now: NOW, signatures: signatures() })
		).toThrow();
	});

	test('falls back to a pointer when the changelog has no section', () => {
		const bare = buildLatest({
			tag: 'v0.16.3',
			repo: 'a/b',
			notes: '',
			now: NOW,
			signatures: signatures()
		});
		expect(bare.notes).toContain('v0.16.3');
	});
});
