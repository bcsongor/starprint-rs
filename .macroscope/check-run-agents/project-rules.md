---
title: Project rules
effort: medium
input: full_diff
include:
  - "crates/**"
  - "apps/starprint-api/**"
  - "apps/starprint-gui/src/**"
  - "apps/starprint-gui/src-tauri/**"
  - "skills/**"
requires:
  - "CI / lint"
conclusion: failure
requiredStatusCheck: true
maxBudgetPerRun: 2
maxBudgetPerPR: 6
---

# Project rules

Review only the changed lines, against the rules below. Style is CI's
job. Untouched code is not a finding.

## Measured values

Tuned on a TSP700II, TSP800II and SP700. Changing one without a print
test described in the PR is a finding.

- `ToneCurve::IMPACT` (gamma 1.8, equalise) and `ToneCurve::THERMAL`
  (gamma 0.55, no equalise).
- `Pacing::STAR_ETHERNET`: 1400 bytes every 20 ms.
- Preview thermal dots: 150 % of the pitch, 200 % in double resolution.
- The SP700 QR module: 7 dots by 3 at double density. The default symbol
  size and the 1.7 module ceiling on the corner radius.
- Density +4 selects two-colour mode and sends neither the density nor
  the speed command. Double-resolution sections use +3.

## Command bytes

Every command `crates/starprint/src/document.rs` emits cites its Star
manual. A new or changed byte sequence without one is a finding, and so
is one that disagrees with the manual it cites.

## Golden fixtures

`crates/starprint/tests/fixtures/` is output from the Python reference.
The impact pipeline, dithering and `ESC ^` serialisation must produce
those bytes exactly. Changing a fixture, or code that alters one, is a
finding unless the PR says the reference was rerun.

## Job behaviour

Jobs live in `crates/starprint-workflows`; the desktop app and the API
call them. Findings:

- A printing rule in a front end.
- A preview drawn from anything but the bitmap that prints.
- `ESC GS y`. The SP700 has no QR command; a QR symbol is a bitmap on
  both printers.
- A job that selects `PrintMode::DoubleResolution` and does not switch
  back at the end. The mode survives `ESC @`.

## The API skill

`skills/starprint-print/` is the only description of the API. An
endpoint or job field added to `apps/starprint-api` without a matching
change there is a finding.

## Reporting

One inline comment per finding on the smallest range that shows it,
naming the rule and the fix. A clear violation fails the check; a doubt
is a comment only.

With no findings, reply exactly `All clear` and nothing else.
