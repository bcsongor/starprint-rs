/**
 * How a fax number is drawn: in runs of uneven length, each a chunk of
 * its own colour. The runs come from a hash of the whole number, so
 * changing one character moves every run and two numbers that differ
 * look different at a glance. This is for the eye only; the
 * checksum and the key check are what make a number safe.
 *
 * The phone page, `apps/starprint-api/src/phone.html`, has no build
 * step and carries a copy of this rule, and of `fold` for the names in
 * the fax book. Change both together.
 */

/** What every number starts with; the runs are cut from what follows. */
const PREFIX = "star1";
const BODY_LENGTH = 32;
/** A run covers 2 to 7 characters. */
const SHORTEST = 2;
const LONGEST = 7;

/** Plain, then the three colours a run can take. */
export type Tint = "plain" | "amber" | "teal" | "violet";
const TINTS: Tint[] = ["plain", "amber", "teal", "violet"];

export interface Run {
  text: string;
  tint: Tint;
}

/** Whether `text` has the shape of a fax number, `*star1` and 32
 * characters of Bech32's alphabet, spaces aside. The server checks its
 * checksum. */
export function isFaxNumber(text: string): boolean {
  return /^\*?star1[02-9ac-hj-np-z]{32}$/i.test(text.replace(/\s/g, ""));
}

/** A fax-book name as the server compares it: ASCII letters in lower
 * case and nothing else, so Ä and ä stay two people. */
export function fold(name: string): string {
  return name.replace(/[A-Z]/g, (letter) => letter.toLowerCase());
}

/** A stream of well-mixed 32-bit numbers seeded from `text`: FNV-1a over
 * its characters, then MurmurHash3's finaliser at every step, so a
 * change anywhere in `text` changes every number that follows. */
function stream(text: string): () => number {
  let state = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    state = Math.imul(state ^ text.charCodeAt(i), 0x01000193);
  }
  return () => {
    state = (state + 0x9e3779b9) | 0;
    let h = state;
    h = Math.imul(h ^ (h >>> 16), 0x85ebca6b);
    h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35);
    return (h ^ (h >>> 16)) >>> 0;
  };
}

/** The part of a number after `star1`, cut into runs, or null for text
 * that is not a number. */
export function runs(number: string): Run[] | null {
  if (!isFaxNumber(number)) return null;
  const address = number.replace(/\s/g, "").replace(/^\*/, "").toLowerCase();
  const body = address.slice(PREFIX.length);
  const next = stream(address);
  const cut: Run[] = [];
  let at = 0;
  let previous: Tint | null = null;
  while (at < BODY_LENGTH) {
    let length = Math.min(
      SHORTEST + (next() % (LONGEST - SHORTEST + 1)),
      BODY_LENGTH - at,
    );
    // A lone character left over joins this run rather than stand alone.
    if (BODY_LENGTH - at - length === 1) length += 1;
    // Neighbours never match, so every boundary shows.
    const choices = TINTS.filter((tint) => tint !== previous);
    const tint = choices[next() % choices.length];
    cut.push({ text: body.slice(at, at + length), tint });
    previous = tint;
    at += length;
  }
  return cut;
}
