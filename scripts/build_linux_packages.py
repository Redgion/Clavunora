# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Wrap the verified portable bundle in Debian and RPM packages."""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--formats", choices=("all", "rpm", "deb"), default="all")
    args = parser.parse_args()
    from pianoflow import __version__
    bundle = ROOT/'dist/Clavunora'
    if not (bundle/'Clavunora').exists(): raise RuntimeError('Build the desktop bundle first')
    stage = ROOT/'build'/('linux-packages-'+__version__+'-'+args.formats)
    if stage.exists(): raise RuntimeError('Use a clean package staging directory')
    stage.mkdir(parents=True)
    desktop = '[Desktop Entry]\nType=Application\nName=Clavunora\nComment=Offline piano practice\nExec=/opt/clavunora/Clavunora\nIcon=clavunora\nTerminal=false\nCategories=AudioVideo;Education;Music;\n'
    payload = stage/'payload'
    shutil.copytree(bundle, payload/'opt/clavunora')
    applications = payload/'usr/share/applications'; applications.mkdir(parents=True)
    (applications/'clavunora.desktop').write_text(desktop)
    icons = payload/'usr/share/icons/hicolor/256x256/apps'; icons.mkdir(parents=True)
    shutil.copy2(bundle/'clavunora.png', icons/'clavunora.png')
    deb_version = __version__.replace('rc', '~rc')
    control = payload/'DEBIAN'; control.mkdir()
    (control/'control').write_text(f'''Package: clavunora
Provides: pianoflow
Conflicts: pianoflow
Replaces: pianoflow
Version: {deb_version}
Section: education
Priority: optional
Architecture: amd64
Maintainer: Clavunora project <noreply@github.com>
Depends: libc6 (>= 2.39), libegl1, libopengl0, libasound2t64, libpulse0, libxcb-cursor0, libxkbcommon0, libxkbcommon-x11-0, libx11-xcb1
Description: Offline piano practice with falling notes and MIDI input
 Includes a noncommercial repertoire of 98 pieces and beginner exercises.
''')
    deb = ROOT/'dist'/f'Clavunora-{__version__}-linux-amd64.deb'
    if args.formats in ('all','deb'):
        subprocess.run(['dpkg-deb', '--build', '--root-owner-group', str(payload), str(deb)], check=True)
        subprocess.run(['dpkg-deb', '--info', str(deb)], check=True)
        if args.formats == 'deb': return
    rpmroot = stage/'rpm'
    for directory in ('BUILD', 'BUILDROOT', 'RPMS', 'SOURCES', 'SPECS', 'SRPMS', 'tmp'):
        (rpmroot/directory).mkdir(parents=True, exist_ok=True)
    # The tar has exactly the same /opt and /usr payload as the Debian package.
    rpm_payload = stage/'rpm-payload'; rpm_payload.mkdir()
    for name in ('opt', 'usr'): shutil.copytree(payload/name, rpm_payload/name)
    archive = shutil.make_archive(str(rpmroot/'SOURCES/clavunora'), 'gztar', root_dir=rpm_payload)
    version, _, prerelease = __version__.partition('rc')
    release = '0.rc'+prerelease if prerelease else '1'
    spec = rpmroot/'SPECS/clavunora.spec'
    spec.write_text(f'''%global __os_install_post %{{nil}}
%global debug_package %{{nil}}
%global _build_id_links none
Name: clavunora
Provides: pianoflow = {version}-{release}
Obsoletes: pianoflow < {version}-{release}
Version: {version}
Release: {release}
Summary: Offline piano practice
License: LicenseRef-Clavunora
URL: https://github.com/Redgion/PianoTrainer
Source0: clavunora.tar.gz
BuildArch: x86_64
AutoReqProv: no
Requires: glibc >= 2.39
Requires: libglvnd-egl, libglvnd-opengl, alsa-lib, pulseaudio-libs, libxcb, libxkbcommon, libxkbcommon-x11, libX11-xcb, xcb-util-cursor
%description
Offline piano practice, with a noncommercial repertoire of 98 pieces.
Dependency and repertoire license notices are included in /opt/clavunora.
%prep
%setup -q -c -n payload
%build
%install
mkdir -p %{{buildroot}}
cp -a opt usr %{{buildroot}}/
%files
/opt/clavunora
/usr/share/applications/clavunora.desktop
/usr/share/icons/hicolor/256x256/apps/clavunora.png
''')
    subprocess.run(['rpmbuild', '--define', '_topdir '+str(rpmroot), '--define', '_tmppath '+str(rpmroot/'tmp'), '-bb', str(spec)], check=True)
    rpms = list((rpmroot/'RPMS').rglob('*.rpm'))
    if len(rpms) != 1: raise RuntimeError('Expected one RPM package')
    rpm = ROOT/'dist'/f'Clavunora-{__version__}-linux-x86_64.rpm'
    shutil.copy2(rpms[0], rpm)
    subprocess.run(['rpm', '-qp', '--info', str(rpm)], check=True)
    print(deb); print(rpm)


if __name__ == '__main__': main()
