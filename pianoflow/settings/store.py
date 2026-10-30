# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Validated, atomic local settings. Never overwrite a corrupt configuration."""
import json
import os
from pathlib import Path
import tempfile
from pianoflow.core.platform import settings_directory

DEFAULTS = {"show_labels": True, "keyboard_size": 88, "midi_port": "", "auto_connect": True, "theme": "light"}


class SettingsStore:
    def __init__(self, path: Path | None = None):
        self.path = path or settings_directory() / "settings.json"
        self.values = DEFAULTS.copy()
        self.error = ""
        self.corrupt = False
        try:
            data = json.loads(self.path.read_text(encoding="utf-8"))
            if not isinstance(data, dict):
                raise ValueError("Ожидался JSON-объект")
            for key, default in DEFAULTS.items():
                value = data.get(key, default)
                if type(value) is type(default):
                    self.values[key] = value
            if self.values["theme"] not in ("light", "dark"):
                self.values["theme"] = "light"
            if self.values["keyboard_size"] not in (49, 61, 88):
                self.values["keyboard_size"] = 88
        except FileNotFoundError:
            pass
        except (OSError, ValueError) as exc:
            self.error = f"Не удалось прочитать настройки: {exc}"
            self.corrupt = True

    def save(self):
        if self.corrupt:
            raise OSError("Повреждённый файл настроек сохранён без изменений: " + str(self.path))
        self.path.parent.mkdir(parents=True, exist_ok=True)
        temporary = None
        try:
            with tempfile.NamedTemporaryFile("w", dir=self.path.parent, delete=False, encoding="utf-8") as f:
                temporary = f.name
                json.dump(self.values, f, ensure_ascii=False, indent=2)
                f.flush()
                os.fsync(f.fileno())
            os.replace(temporary, self.path)
        finally:
            if temporary and os.path.exists(temporary):
                os.unlink(temporary)
