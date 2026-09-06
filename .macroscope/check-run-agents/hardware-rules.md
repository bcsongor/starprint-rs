---
title: Hardware rules
effort: medium
input: full_diff
include:
  - "crates/**"
  - "apps/starprint-api/**"
  - "apps/starprint-gui/src/**"
  - "apps/starprint-gui/src-tauri/**"
  - "skills/**"
requires:
  - lint
conclusion: failure
requiredStatusCheck: true
maxBudgetPerRun: 2
maxBudgetPerPR: 6
---

# Hardware rules

This repository drives Star thermal and impact printers. Several values
in it were measured on the printers and cannot be checked from a screen,
and several files must match a reference byte for byte. Review only the
lines this pull request changed, against the rules below. Style, naming
and formatting are CI's job, not yours. Untouched code is not a finding.

## Measured values

These were tuned on a TSP700II, a TSP800II and an SP700 and must not
change unless the pull request describes a print test on that hardware:

- `ToneCurve::IMPACT` (gamma 1.8, equalise) and `ToneCurve::THERMAL`
  (gamma 0.55, no equalise).
- `Pacing::STAR_ETHERNET` (1400 bytes every 20 ms).
- The GUI preview's thermal dot size: 150 % of the pitch at normal
  resolution, 200 % in double.
- The QR module on the SP700: 7 dots by 3 at double density. The default
  symbol size and the 1.7 module ceiling on the corner radius.
- Density +4 in a profile selects two-colour mode, which sends neither
  the density nor the speed command. Double-resolution sections use +3.

A change to any of these with no hardware test in the description is a
finding.

## Command bytes

Every command `crates/starprint/src/document.rs` emits cites the Star
manual it comes from. A new or changed byte sequence without a citation
to a manual is a finding. So is one whose bytes differ from what the
cited manual gives, if you can check.

## Golden fixtures

`crates/starprint/tests/fixtures/` holds output from the Python
reference implementation. The impact pipeline, dithering and `ESC ^`
serialisation must produce those bytes exactly. A change to a fixture,
or a change to that code that alters its output, is a finding unless the
pull request says the reference was rerun.

## Where job behaviour lives

Jobs are defined in `crates/starprint-workflows`. The desktop app and
the API only call them. A front end that adds a printing rule of its own
is a finding, and so is a preview drawn from anything other than the
bitmap that prints.

The SP700 has no QR command. A QR symbol is a bitmap on both printers.
Reaching for `ESC GS y` is a finding.

`PrintMode::DoubleResolution` survives `ESC @`. A job that selects it
and does not switch back at the end is a finding.

## The API skill

`skills/starprint-print/` is the only description of the API's endpoints
and job fields. An endpoint or field added to `apps/starprint-api`
without a matching change there is a finding.

## Reporting

Post each finding as one inline comment on the smallest range that
shows it, naming the rule and what would satisfy it. A clear violation
fails the check. A doubt does not; leave it as a comment.

When there are no findings, make the entire final response exactly
`All clear` on one line with nothing else.
