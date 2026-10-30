# Build on the target OS. Qt remains dynamically linked in an onedir bundle.
from pathlib import Path
from PyInstaller.utils.hooks import collect_data_files, copy_metadata
root = Path(SPECPATH).parent
assets = collect_data_files('pianoflow') + copy_metadata('mido')
a = Analysis([str(root / 'main.py')], pathex=[str(root)], binaries=[],
             datas=assets, hiddenimports=['mido.backends.rtmidi', 'rtmidi'],
             hookspath=[], hooksconfig={}, runtime_hooks=[], excludes=[], noarchive=False)
pyz = PYZ(a.pure)
exe = EXE(pyz, a.scripts, [], exclude_binaries=True, name='Clavunora',
          debug=False, bootloader_ignore_signals=False, strip=False, upx=False, console=False, icon=str(root / 'pianoflow/resources/icons/pianoflow.ico'))
coll = COLLECT(exe, a.binaries, a.datas, strip=False, upx=False, name='Clavunora')
