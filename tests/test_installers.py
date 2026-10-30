# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Portable rebranding updates preserve the existing installation and data."""
import os
from pathlib import Path
import subprocess
import sys
import pytest


@pytest.mark.skipif(sys.platform == 'win32', reason='Linux shell installer')
@pytest.mark.parametrize('upgrading', [False, True])
def test_portable_branding_installation(tmp_path, upgrading):
    source = tmp_path/'download with spaces'
    source.mkdir()
    (source/'_internal/pianoflow').mkdir(parents=True)
    executable = source/'Clavunora'
    executable.write_text('#!/bin/sh\nexit 0\n'); executable.chmod(0o755)
    script = Path(__file__).resolve().parents[1]/'packaging/linux/install.sh'
    (source/'install.sh').write_bytes(script.read_bytes())
    data_home = tmp_path/'user data'
    applications = data_home/'applications'
    applications.mkdir(parents=True)
    legacy = data_home/'PianoFlow'
    expected = data_home/'Clavunora'
    launcher = applications/'Clavunora.desktop'
    if upgrading:
        (legacy/'_internal/pianoflow').mkdir(parents=True)
        (legacy/'PianoFlow').write_text('previous binary')
        (legacy/'retained-user-file.txt').write_text('keep me')
        launcher = applications/'PianoFlow.desktop'
        launcher.write_text('[Desktop Entry]\nName=PianoFlow\n')
        expected = legacy
    env = dict(os.environ, XDG_DATA_HOME=str(data_home))
    subprocess.run(['sh', str(source/'install.sh')], env=env, check=True)
    assert (expected/'Clavunora').exists()
    assert 'Name=Clavunora' in launcher.read_text()
    assert str(expected/'Clavunora') in launcher.read_text()
    assert len(list(applications.glob('*.desktop'))) == 1
    if upgrading:
        assert (legacy/'retained-user-file.txt').read_text() == 'keep me'
