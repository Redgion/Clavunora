# License scope and third-party materials

The original Clavunora application source code is GPL-3.0-only; see LICENSE and
COPYRIGHT. GPLv3 permits charging for copies and support, with the source and
redistribution obligations stated in the license. No noncommercial condition
is added to the application code.

## Music library — separate data

pianoflow/resources/catalog and the distributed resources/catalog are separate
music data, not GPL-relicensed application code. Each edition retains its exact
license, authorship, original MIDI/notation sources and change record in its
provenance.json and accompanying files. This collection includes Public Domain,
CC0, CC BY-SA editions of different versions and CC BY-NC-SA 4.0 material.
Consult the individual records rather than assuming a uniform license.
The MAESTRO performance in repertoire-095 has a noncommercial restriction.
Consequently, the supplied music collection must not be advertised as entirely
GPL-covered or freely usable commercially. GPL rights in the application code
do not override these separate music rights. The GPL binary loads the editions
from external files; third-party score contents are not embedded in it.

Previously dedicated CC0 educational arrangements retain that dedication.
Missing permissions for editions 48 and 76 are not cured by this license change;
those editions remain excluded from distributed score files.

## Rust dependencies and fonts

Rust dependencies keep their original licenses. Release source bundles include
vendored crate sources and their notices. Binary bundles include dependency
license/copyright notices and an inventory. Alternative license expressions
remain alternatives; Clavunora does not relicense third-party crate source files.

brand-serif.ttf is Liberation Serif Regular under SIL OFL 1.1; see
rust/crates/clavunora-app/assets/FONT-LICENSE.txt. Fonts in egui/epaint retain their
own OFL/Ubuntu font notices in the dependency sources and notices. Font license
conditions apply to those font files; they do not replace the program license.
The original generated landscape is documented in AI_USAGE.md and the asset
README. No third-party font is claimed as original Clavunora code.

## Python application and optional system software

The repository also includes the earlier Python application. PySide6/Qt,
Shiboken, mido and python-rtmidi retain their upstream licenses. This Rust Linux
release does not bundle the Python/Qt runtime. Existing Qt notices are retained
in pianoflow/resources/licenses. System ALSA, graphics libraries and optional
FluidSynth/SoundFonts keep their own licenses; no SoundFont or FluidSynth
library is bundled. Distributors adding them must meet their licensing duties.
