import {
  HANDLE_PX,
  MIDDLE_C,
  MIN_NOTE_BEATS,
  PITCH_MAX,
  PREVIEW_MIN_ROWS,
  ROW_PX,
  VELOCITY_MAX,
  VELOCITY_MIN,
  VELOCITY_STEP,
  cellAt,
  clampLength,
  clampMove,
  clampPitch,
  floorToGrid,
  hitNote,
  isBlackKey,
  locateBlock,
  noteName,
  pitchToY,
  previewRange,
  snapToGrid,
  velocityOpacity,
  velocityStep,
  yToPitch,
} from "./pianoRoll";
import type { BlockView, NoteView } from "./wire";

// bun test injects these as globals; @types/bun is not a dependency, so svelte-check
// needs the shapes declared here to see them.
interface Matchers {
  toBe(expected: unknown): void;
  toEqual(expected: unknown): void;
  toBeNull(): void;
  toBeCloseTo(expected: number, digits?: number): void;
  toBeLessThan(expected: number): void;
  not: Matchers;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

function note(
  pitch: number,
  startBeats: number,
  lengthBeats: number,
  velocity = 100,
): NoteView {
  return { pitch, startBeats, lengthBeats, velocity };
}

function block(
  name: string,
  startBeats: number,
  kind: "midi" | "audio" = "midi",
): BlockView {
  return {
    name,
    kind,
    startBeats,
    lengthBeats: 4,
    source: "",
    selected: false,
    notes: [],
    offsetSeconds: 0,
    rate: 1,
  };
}

describe("pitch rows", () => {
  test("puts pitch 127 in the top row and pitch 0 in the bottom one", () => {
    expect(pitchToY(PITCH_MAX)).toBe(0);
    expect(pitchToY(0)).toBe(PITCH_MAX * ROW_PX);
  });

  test("reads any offset inside a row back as that row pitch", () => {
    for (const pitch of [0, 59, MIDDLE_C, 127]) {
      const top = pitchToY(pitch);
      expect(yToPitch(top)).toBe(pitch);
      expect(yToPitch(top + ROW_PX - 0.5)).toBe(pitch);
    }
  });

  test("clamps an offset above or below the grid to the range ends", () => {
    expect(yToPitch(-40)).toBe(127);
    expect(yToPitch(10_000)).toBe(0);
  });

  test("rounds and clamps a pitch", () => {
    expect(clampPitch(60.4)).toBe(60);
    expect(clampPitch(-3)).toBe(0);
    expect(clampPitch(400)).toBe(127);
  });
});

describe("the note grid", () => {
  test("floors a click to the start of its sixteenth", () => {
    expect(floorToGrid(1.2)).toBe(1);
    expect(floorToGrid(1.3)).toBe(1.25);
    expect(floorToGrid(-0.1)).toBe(0);
  });

  test("rounds a drag to the nearest sixteenth unless it is free", () => {
    expect(snapToGrid(1.13)).toBe(1.25);
    expect(snapToGrid(1.1)).toBe(1);
    expect(snapToGrid(1.13, true)).toBe(1.13);
    expect(snapToGrid(-1)).toBe(0);
  });

  test("reads a grid point as a beat and a pitch", () => {
    expect(cellAt(128, pitchToY(64) + 1, 64)).toEqual({ beats: 2, pitch: 64 });
    expect(cellAt(-5, 0, 64)).toEqual({ beats: 0, pitch: 127 });
  });
});

describe("hitNote", () => {
  const notes = [note(60, 0, 1), note(60, 0.5, 1), note(62, 2, 0.125)];
  const zoom = 64;

  test("finds the note under a point, preferring the one drawn last", () => {
    expect(hitNote(notes, 10, pitchToY(60) + 2, zoom)).toEqual({
      note: 0,
      edge: false,
    });
    expect(hitNote(notes, 40, pitchToY(60) + 2, zoom)).toEqual({
      note: 1,
      edge: false,
    });
  });

  test("reports the right edge as the resize handle", () => {
    const right = 1.5 * zoom;
    expect(hitNote(notes, right - 1, pitchToY(60) + 2, zoom)).toEqual({
      note: 1,
      edge: true,
    });
    expect(
      hitNote(notes, right - HANDLE_PX - 1, pitchToY(60) + 2, zoom),
    ).toEqual({
      note: 1,
      edge: false,
    });
  });

  test("keeps the left half of a narrow note for moving", () => {
    const left = 2 * zoom;
    expect(hitNote(notes, left + 1, pitchToY(62) + 2, zoom)).toEqual({
      note: 2,
      edge: false,
    });
    expect(hitNote(notes, left + 7, pitchToY(62) + 2, zoom)).toEqual({
      note: 2,
      edge: true,
    });
  });

  test("misses on another row, past the end, and outside the grid", () => {
    expect(hitNote(notes, 10, pitchToY(61) + 2, zoom)).toBeNull();
    expect(hitNote(notes, 200, pitchToY(60) + 2, zoom)).toBeNull();
    expect(hitNote([note(0, 0, 1)], 10, 10_000, zoom)).toBeNull();
    expect(hitNote([note(127, 0, 1)], 10, -1, zoom)).toBeNull();
  });
});

describe("clamping an edited note to its block", () => {
  test("keeps a moved note inside the block", () => {
    expect(clampMove(60, 3.5, 1, 4)).toEqual({ pitch: 60, startBeats: 3 });
    expect(clampMove(60, -2, 1, 4)).toEqual({ pitch: 60, startBeats: 0 });
    expect(clampMove(200, 1, 1, 4)).toEqual({ pitch: 127, startBeats: 1 });
  });

  test("pins a note longer than its block to the block start", () => {
    expect(clampMove(60, 2, 8, 4)).toEqual({ pitch: 60, startBeats: 0 });
  });

  test("keeps a resized note between the minimum and the block end", () => {
    expect(clampLength(1, 0, 4)).toBe(MIN_NOTE_BEATS);
    expect(clampLength(1, 10, 4)).toBe(3);
    expect(clampLength(1, 1.5, 4)).toBe(1.5);
  });
});

describe("velocity", () => {
  test("rolls up away from the user and down toward them", () => {
    expect(velocityStep(100, -100)).toBe(100 + VELOCITY_STEP);
    expect(velocityStep(100, 100)).toBe(100 - VELOCITY_STEP);
    expect(velocityStep(100, 100, true)).toBe(99);
    expect(velocityStep(100, 0)).toBe(100);
  });

  test("stays inside the range the engine accepts", () => {
    expect(velocityStep(VELOCITY_MAX - 1, -100)).toBe(VELOCITY_MAX);
    expect(velocityStep(VELOCITY_MIN + 1, 100)).toBe(VELOCITY_MIN);
  });

  test("draws a quiet note lighter than a loud one", () => {
    expect(velocityOpacity(VELOCITY_MAX)).toBe(1);
    expect(velocityOpacity(VELOCITY_MIN)).toBeLessThan(velocityOpacity(64));
  });
});

describe("note names", () => {
  test("names middle C C4", () => {
    expect(noteName(60)).toBe("C4");
    expect(noteName(61)).toBe("C#4");
    expect(noteName(59)).toBe("B3");
    expect(noteName(0)).toBe("C-1");
    expect(noteName(127)).toBe("G9");
  });

  test("marks the five black keys of an octave", () => {
    const black = [60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71].filter(
      isBlackKey,
    );
    expect(black).toEqual([61, 63, 66, 68, 70]);
  });
});

describe("previewRange", () => {
  test("widens a single pitch to the minimum row count around it", () => {
    const range = previewRange([note(60, 0, 1)]);
    expect(range.rows).toBe(PREVIEW_MIN_ROWS);
    expect(range.top).toBe(66);
  });

  test("spans a wide range exactly", () => {
    expect(previewRange([note(40, 0, 1), note(80, 1, 1)])).toEqual({
      top: 80,
      rows: 41,
    });
  });

  test("keeps every note inside the rows it hands out", () => {
    const notes = [note(50, 0, 1), note(55, 1, 1)];
    const range = previewRange(notes);
    for (const item of notes) {
      const row = range.top - item.pitch;
      expect(row >= 0 && row < range.rows).toBe(true);
    }
  });
});

describe("locateBlock", () => {
  test("keeps the index while the block is still there", () => {
    expect(locateBlock([block("a", 0), block("b", 4)], 1, "b", 4)).toBe(1);
  });

  test("keeps the index through a rename, since the start still matches", () => {
    expect(locateBlock([block("a", 0), block("renamed", 4)], 1, "b", 4)).toBe(
      1,
    );
  });

  test("follows a move that re-sorted the lane", () => {
    expect(locateBlock([block("b", 0), block("a", 8)], 1, "b", 4)).toBe(0);
  });

  test("returns null once the block is gone or no longer MIDI", () => {
    expect(locateBlock([block("a", 0)], 1, "b", 4)).toBeNull();
    expect(
      locateBlock([block("a", 0), block("b", 4, "audio")], 1, "b", 4),
    ).toBeNull();
  });
});
