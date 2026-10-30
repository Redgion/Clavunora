# GPLv3 distribution policy

Original Clavunora source is GPL-3.0-only. See LICENSE and COPYRIGHT. Commercial
redistribution is allowed. The official app may be supplied gratis, but GPLv3
does not prohibit others from charging for compliant copies or support.
When distributing modified or unmodified binaries, provide the complete
corresponding source as required by section 6. For downloads, the project's
chosen method is equivalent access at the same place with no additional charge.
Private modifications do not automatically require public posting.

For each Rust Linux version publish together:

- Clavunora-Rust-VERSION-linux-x86_64-glibc2.36.tar.gz
- Clavunora-Rust-VERSION-source.tar.gz
- Both .sha256 files

The source archive contains locked Cargo dependency sources and offline build
instructions in docs/BUILD_FROM_SOURCE.md. Keep it available for as long as
required by the GPL. A changing branch is not a substitute for matching source.
Retain notices, identify modifications and do not add no-sale or OpenAI-service
conditions to GPL rights. Full duties and warranty provisions are in LICENSE.

Music data, fonts and dependencies retain their own licenses. Third-party
editions are outside the GPL-code grant and load from external resources/catalog.
This aggregate includes noncommercial music data. See THIRD_PARTY_NOTICES.md.
OpenAI-assisted development is documented separately in AI_USAGE.md; recipients
are not required to agree to OpenAI service terms to use this offline program.

Publication status must be checked separately. Redgion/PianoTrainer was private
when this change was prepared (2026-10-30); a private release is not a public
source download. Choose a public destination or explicitly authorize opening
that repository before advertising public availability. Do not expose developer
profiles, credentials, screenshots of personal history or private Git history.
