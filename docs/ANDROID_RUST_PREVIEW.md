# Clavunora Android — первый тестовый APK

Первая цель проверки — Nothing Phone (4a), ARM64. Сначала создаётся APK, затем проверяются запуск, интерфейс, звук и MIDI на телефоне. Успешная компиляция не означает аппаратную проверку.

Приложение использует текущую Rust-логику и egui, Android NativeActivity, системный звук через CPAL и Android MIDI через midir/AMidi. Минимум Android 10 (API 29), target API 36. Это отдельный пакет org.redgion.clavunora.preview: он не заменяет старое Python-приложение.

Библиотека входит в APK и копируется в закрытую папку приложения. Профиль сохраняется там же. Есть обработка нескольких касаний, независимое отпускание нот и отмена жестов. При потере фокуса занятие приостанавливается; автоматического продолжения нет.

Системный выбор файлов Android ещё не реализован. В тестовой сборке эта операция выдаёт понятное сообщение; готовность импорта через Android Documents UI не заявляется. BLE-поиск, задержка звука, USB-MIDI, работа в фоне/после блокировки и поворот экрана требуют проверки и дальнейшей доработки.

Сборка: workflow rust-android.yml в ветке codex/android-preview. Создаёт ARM64 APK, проверяет его подпись и прикладывает соответствующие исходники с Cargo.lock и зависимостями. Первая сборка использует тестовую подпись; для выдачи обновляемых APK используется локальный ключ, не включаемый в исходники.

Это preview для проверки, не публикация в Google Play и не окончательный Android 1.0.

## Phone layout update

The activity requests sensorLandscape (both horizontal directions). System Back returns from performance to the lesson, then from a section to Home.
Only on Home it pauses playback, retries saves and moves the task to the background.
Dirty editor changes still require confirmation; active backups cannot be interrupted.
Short windows use a compact header and padding, wrapping practice/settings controls,
and bounded piano height. Oversized content remains scrollable on both axes.
Host layout regression covers 640×320 and 800×360 logical-pixel piano views.
Real-device rotation, system gestures and safe-area behavior still need verification.
