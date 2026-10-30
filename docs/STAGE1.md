# Этап 1 — отчёт

Дата: 24 октября 2026, среда Fedora 44 KDE, Python 3.14.8, PySide6 6.11.2.

## Файлы

- `.gitignore`, `pyproject.toml` — правила Git и метаданные/зависимости.
- `main.py`, `pianoflow/__main__.py`, `pianoflow/app.py` — точки запуска.
- `pianoflow/__init__.py` — версия пакета.
- `pianoflow/core/__init__.py`, `notes.py`, `state.py` — ноты, аккорды, состояние.
- `pianoflow/midi/__init__.py`, `service.py` — MIDI worker и сигналы.
- `pianoflow/settings/__init__.py`, `store.py` — JSON-настройки.
- `pianoflow/ui/__init__.py`, `keyboard.py`, `theme.py`, `window.py` — интерфейс.
- `tests/conftest.py`, `test_state.py`, `test_settings.py`, `test_ui.py`, `test_midi.py` — проверки.
- `scripts/run.sh`, `assets/pianoflow.desktop` — запуск/desktop-шаблон.
- `README.md`, `CHANGELOG.md`, `docs/STAGE1.md`, `docs/freeplay.png` — документация/скриншот.

В `.venv` установлены mido 1.3.3 и pytest 9.1.1. PySide6 используется системный
через `--system-site-packages`. Виртуальное окружение не включено в Git.

## Проверено

- `python -m compileall -q pianoflow tests` — успешно.
- `python -m pytest -q` — **13 passed**.
- Offscreen-рендер 5 страниц в 1000×700 и 1920×1080.
- QtTest: одновременные клавиши, педаль, mouse hit test чёрной клавиши,
  смена страниц со сбросом компьютерных нот, изменение/сохранение настроек.
- MIDI worker на fake backend: порт, callback из отдельного потока, исчезновение
  порта, закрытие потока. Это не проверка физического USB/Bluetooth оборудования.
- Отдельный запуск настоящего MainWindow с worker: корректное сообщение об
  отсутствии RtMidi; окно отрисовано и закрыто, поток завершился.
- `docs/freeplay.png` создан Qt и просмотрен.

## Ограничения

Сборка python-rtmidi 1.5.8 под Python 3.14 остановилась из-за отсутствия C++
компилятора. `sudo -n dnf install ...` не выполнилась: нужен пароль администратора.
Пользователю следует выполнить команды установки из README. MIDI-код реализован
и проверен через имитацию; физическое пианино, реальная задержка и дисплейная
сессия Wayland/X11 не проверены.

MIDI-импорт/падающие ноты/обучение/звуковой движок/БД не входят в первый этап.
Метроном ещё не реализован. Текущий счётчик нот не сохраняется.

## Следующий этап

1. MIDI parser с tempo map и обработкой повреждённых файлов, тестами темпа/длительностей.
2. Фоновый импорт и дорожки с ручным назначением рук.
3. Падающие ноты, музыкальные часы, pause/seek/speed.
4. Далее отдельным этапом: ожидание аккордов и проверка исполнения.

## Технические источники

- [Mido API: callback-порты](https://mido.readthedocs.io/en/latest/api.html)
- [Qt: сигналы и привязка слотов к потоку](https://doc.qt.io/qtforpython-6/tutorials/basictutorial/signals_and_slots.html)
