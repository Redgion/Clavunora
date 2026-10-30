# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
from pianoflow.ui.responsive import FlowLayout, IconGridList
from dataclasses import replace
from pathlib import Path
from PySide6.QtCore import Qt, Signal, QSize
from PySide6.QtWidgets import (QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPushButton,
    QLineEdit, QComboBox, QListWidget, QListWidgetItem, QFileDialog, QMessageBox,
    QDialog, QFormLayout, QDialogButtonBox, QTableWidget, QTableWidgetItem, QHeaderView, QCheckBox, QTreeWidget, QTreeWidgetItem, QSplitter, QStyle, QScrollArea, QListView, QFrame, QAbstractItemView)
from pianoflow.ui.lesson import ImportThread
from pianoflow.core.notes import note_name
from pianoflow.library.catalog import coverage, manifest

LEVELS = ['Не оценено','1 · Начальный','2 · Лёгкий','3 · Средний','4 · Сложный','5 · Мастер']
from pianoflow.ui.home import FolderGrid, FOLDERS
from pianoflow.ui.artwork import cover_icon


class ScoreEditor(QDialog):
    def __init__(self, song, parent=None):
        super().__init__(parent)
        self.song = song
        self.setWindowTitle('Название, сложность и аппликатура')
        self.resize(720,560)
        layout = QVBoxLayout(self)
        form = QFormLayout()
        self.title = QLineEdit(song.title)
        self.composer = QLineEdit(song.composer)
        self.work = QLineEdit(song.work)
        self.source = QLineEdit(song.source)
        self.level = QComboBox(); self.level.addItems(LEVELS); self.level.setCurrentIndex(song.difficulty)
        for name, control in [('Название',self.title),('Композитор',self.composer),('Произведение / группа',self.work),('Источник / лицензия',self.source),('Сложность',self.level)]:
            form.addRow(name,control)
        layout.addLayout(form)
        layout.addWidget(QLabel('Рука: L или R. Палец: 1–5, 0 — без подсказки. MIDI не определяет палец.'))
        self.table = QTableWidget(len(song.notes),5)
        self.table.setHorizontalHeaderLabels(['Время','Нота','Дорожка','Рука','Палец'])
        self.table.horizontalHeader().setSectionResizeMode(QHeaderView.ResizeMode.Stretch)
        for i,n in enumerate(song.notes):
            for j,value in enumerate((f'{n.start:.3f}',note_name(n.pitch),str(n.track+1),n.hand,str(n.finger))):
                item = QTableWidgetItem(value)
                if j < 3: item.setFlags(item.flags() & ~Qt.ItemFlag.ItemIsEditable)
                self.table.setItem(i,j,item)
        layout.addWidget(self.table)
        batch = QHBoxLayout()
        self.batch_hand = QComboBox(); self.batch_hand.addItems(['Без руки','L','R'])
        batch.addWidget(self.batch_hand)
        apply = QPushButton('Назначить руку выбранным строкам'); apply.clicked.connect(self.assign)
        batch.addWidget(apply); layout.addLayout(batch)
        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Save | QDialogButtonBox.StandardButton.Cancel)
        buttons.accepted.connect(self.validate); buttons.rejected.connect(self.reject); layout.addWidget(buttons)

    def assign(self):
        for i in {item.row() for item in self.table.selectedItems()}:
            self.table.item(i,3).setText(self.batch_hand.currentText() if self.batch_hand.currentIndex() else '')

    def validate(self):
        try:
            notes = []
            for i,n in enumerate(self.song.notes):
                hand = self.table.item(i,3).text().strip().upper()
                finger = int(self.table.item(i,4).text())
                if hand not in ('','L','R') or finger not in range(6) or (finger and not hand):
                    raise ValueError('Укажите руку L/R для пальца 1–5; 0 означает отсутствие подсказки.')
                notes.append(replace(n,hand=hand,finger=finger))
            if not self.title.text().strip(): raise ValueError('Введите название')
            self.song = replace(self.song,title=self.title.text().strip(),composer=self.composer.text(),work=self.work.text(),source=self.source.text(),difficulty=self.level.currentIndex(),notes=tuple(notes))
            self.accept()
        except ValueError as e:
            QMessageBox.warning(self,'Проверьте разметку',str(e))


class LibraryPage(QWidget):
    open_song = Signal(str)

    def __init__(self, store, parent=None):
        super().__init__(parent)
        self.store = store
        self.importer = None
        self._folder = store.preference('library-folder', 'root')
        self.folder_titles = dict(FOLDERS) | {'root':'Библиотека', 'course':'Курс с нуля', 'level:0':'Без сложности'}
        if self._folder not in self.folder_titles: self._folder='root'
        layout = QVBoxLayout(self); layout.setContentsMargins(30,25,30,25); layout.setSpacing(14)
        title = QLabel('Библиотека'); title.setObjectName('title'); layout.addWidget(title)
        self.catalog_status = QLabel(); self.catalog_status.setObjectName('muted'); self.catalog_status.setWordWrap(True); layout.addWidget(self.catalog_status)
        row = QHBoxLayout()
        self.back = QPushButton('← Все папки'); self.back.clicked.connect(lambda:self.select_folder('root')); row.addWidget(self.back)
        self.breadcrumb = QLabel(); row.addWidget(self.breadcrumb,1)
        course=QPushButton('Курс с нуля'); course.clicked.connect(lambda:self.select_folder('course')); row.addWidget(course)
        add=QPushButton('Импорт…'); add.clicked.connect(self.import_file); row.addWidget(add); layout.addLayout(row)
        self.search = QLineEdit(); self.search.setPlaceholderText('Найти произведение или композитора'); layout.addWidget(self.search)
        self.folders=FolderGrid(); self.folders.selected.connect(self.select_folder); layout.addWidget(self.folders)
        self.list = IconGridList(3,216); self.list.setObjectName('songGrid'); self.list.setWordWrap(True)
        self.list.setViewMode(QListView.ViewMode.IconMode); self.list.setResizeMode(QListView.ResizeMode.Adjust)
        self.list.setMovement(QListView.Movement.Static); self.list.setIconSize(QSize(200,116)); self.list.setGridSize(QSize(232,196)); self.list.setSpacing(8)
        self.list.setAccessibleName('Произведения в выбранной папке'); layout.addWidget(self.list,1)
        self.selection_title=QLabel(); self.selection_title.setObjectName('sectionTitle'); self.selection_title.setWordWrap(True)
        self.info = QLabel(''); self.info.setWordWrap(True); self.info.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        self.details=QFrame(); self.details.setObjectName('card'); inner=QVBoxLayout(self.details); inner.setContentsMargins(18,14,18,14); inner.addWidget(self.selection_title)
        details = QScrollArea(); details.setWidgetResizable(True); details.setMinimumHeight(85); details.setMaximumHeight(125); details.setWidget(self.info); inner.addWidget(details)
        row = FlowLayout()
        for text, action in [('Играть',self.play),('Разметка…',self.edit),('☆ Избранное',self.favorite),('Источник',self.show_source),('План 100',self.show_catalog),('Удалить',self.delete)]:
            control = QPushButton(text); control.clicked.connect(action)
            if text=='Играть': control.setObjectName('primary')
            row.addWidget(control)
        inner.addLayout(row); layout.addWidget(self.details)
        self.empty=QLabel('В этой папке пока нет произведений. Добавьте MIDI или измените поиск.'); self.empty.setWordWrap(True); self.empty.setObjectName('muted'); layout.addWidget(self.empty)
        self.search.textChanged.connect(self.refresh)
        self.list.currentItemChanged.connect(self.describe); self.list.itemActivated.connect(lambda _: self.play())
        self.refresh()

    def current_folder(self):
        return self._folder

    def select_folder(self, key):
        self._folder=key if key in self.folder_titles else 'all'
        self.search.blockSignals(True); self.search.clear(); self.search.blockSignals(False)
        self.store.set_preference('library-folder', self._folder)
        self.refresh()

    def matches_folder(self, key, identifier, song, favorite):
        if key.startswith('level:'): return song.difficulty == int(key.split(':')[1])
        if key == 'course': return identifier.startswith('course-')
        if key == 'favorites': return favorite
        if key == 'imports': return identifier not in self.builtin_ids
        return True

    def selected(self):
        item = self.list.currentItem()
        return item.data(Qt.ItemDataRole.UserRole) if item else None

    def refresh(self, *_):
        selected = self.selected(); self.list.clear()
        songs = self.store.songs()
        installed = {identifier for identifier, _, _ in songs}
        entries = manifest()['entries']
        count = sum(e['id'] in installed for e in entries)
        self.catalog_status.setText(f'{count} из 100 произведений доступно · Курс с нуля и ваши пьесы считаются отдельно')
        self.builtin_ids = {e['id'] for e in entries} | {f'course-{i}' for i in range(1,7)} | {'birthday-bass','birthday-chords'}
        folder = self.current_folder(); query=self.search.text().strip().casefold()
        hub=folder=='root' and not query
        self.folders.refresh(self.store); self.folders.setVisible(hub); self.list.setVisible(not hub); self.details.setVisible(not hub); self.back.setVisible(not hub)
        self.breadcrumb.setText('Библиотека' if hub else 'Библиотека  /  '+('Результаты поиска' if folder=='root' else self.folder_titles.get(folder,'Все произведения')))
        songs.sort(key=lambda x:(x[1].difficulty or 6,x[1].title.casefold()))
        for identifier,song,favorite in songs:
            if query not in (song.title+' '+song.composer+' '+song.work).casefold(): continue
            if not self.matches_folder(folder, identifier, song, favorite): continue
            passed = self.store.preference('passed:'+identifier,False)
            item = QListWidgetItem(cover_icon(song.title,song.difficulty), f'{"★ " if favorite else ""}{"✓ " if passed else ""}{song.title}\n{song.composer}\n{LEVELS[song.difficulty]}')
            item.setToolTip(f'{song.title} · {song.composer} · {song.duration:.0f} сек')
            item.setData(Qt.ItemDataRole.UserRole,identifier); self.list.addItem(item)
            if identifier == selected: self.list.setCurrentItem(item)
        if not self.list.currentItem() and self.list.count(): self.list.setCurrentRow(0)
        self.empty.setVisible(not hub and not self.list.count())
        self.describe()

    def describe(self, *_):
        song = self.store.get(self.selected()) if self.selected() else None
        self.selection_title.setText(song.title if song else 'Нет подходящих произведений')
        self.info.setText((f'{song.composer} · {song.duration:.0f} сек · {LEVELS[song.difficulty]}\n'+coverage(song)) if song else 'Измените поиск или импортируйте MIDI.')

    def show_source(self):
        song=self.store.get(self.selected()) if self.selected() else None
        if song: QMessageBox.information(self,'О произведении',song.title+'\n\n'+song.description+'\n\n'+song.source)

    def show_catalog(self):
        dialog = QDialog(self); dialog.setWindowTitle('Репертуар: 100 произведений'); dialog.resize(800, 600)
        layout = QVBoxLayout(dialog)
        layout.addWidget(QLabel('Доступные партитуры открываются в библиотеке. Остальные ещё готовятся.'))
        listing = QListWidget(); layout.addWidget(listing)
        installed = {i for i, _, _ in self.store.songs()}
        for entry in manifest()['entries']:
            status = 'Ожидает разрешения' if entry.get('status') == 'rights-pending' else 'Доступно' if entry['id'] in installed else 'Подготовка' if not entry.get('score') else 'Удалено из библиотеки'
            listing.addItem(f'{entry["number"]}. {entry["title"]}\nУровень {entry["difficulty"]} · {status}')
        close = QDialogButtonBox(QDialogButtonBox.StandardButton.Close)
        close.rejected.connect(dialog.reject); layout.addWidget(close); dialog.exec()

    def play(self):
        if self.selected(): self.open_song.emit(self.selected())

    def import_file(self):
        path,_ = QFileDialog.getOpenFileName(self,'Добавить MIDI','','Музыка (*.mid *.midi *.musicxml *.xml)')
        if path and not (self.importer and self.importer.isRunning()):
            self.info.setText('Импорт…')
            self.importer = ImportThread(path,self)
            self.importer.loaded.connect(self.imported)
            self.importer.failed.connect(lambda e:self.info.setText('Ошибка импорта: '+e))
            self.importer.start()

    def imported(self,song):
        try:
            self.store.add(song); self.select_folder('imports')
        except Exception as e: self.info.setText('Не удалось сохранить: '+str(e))

    def edit(self):
        identifier = self.selected()
        if not identifier: return
        dialog = ScoreEditor(self.store.get(identifier),self)
        if dialog.exec():
            self.store.update(identifier,dialog.song); self.refresh()

    def favorite(self):
        identifier = self.selected()
        if identifier:
            favorite = next(f for i,s,f in self.store.songs() if i==identifier)
            self.store.favorite(identifier,not favorite); self.refresh()

    def delete(self):
        identifier = self.selected()
        if identifier and QMessageBox.question(self,'Удалить из библиотеки?','Исходный MIDI-файл останется на месте. История занятий сохранится.') == QMessageBox.StandardButton.Yes:
            self.store.delete(identifier); self.refresh()

    def shutdown(self):
        if self.importer: self.importer.wait()


class StatisticsPage(QWidget):
    practice_error = Signal(str,float)

    def __init__(self,store,parent=None):
        super().__init__(parent); self.store=store
        layout=QVBoxLayout(self); layout.setContentsMargins(30,25,30,25)
        title=QLabel('История занятий'); title.setObjectName('title'); layout.addWidget(title)
        self.summary=QLabel(); self.summary.setObjectName('sectionTitle'); layout.addWidget(self.summary)
        from pianoflow.ui.staff import ProgressChart
        self.chart=ProgressChart(); self.chart.setMaximumHeight(220); self.chart.setMinimumHeight(180); layout.addWidget(self.chart)
        self.list=QTableWidget(0,6); self.list.setHorizontalHeaderLabels(['Дата','Произведение','Режим','Точность','Скорость','Статус'])
        self.list.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows); self.list.setSelectionMode(QAbstractItemView.SelectionMode.SingleSelection); self.list.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self.list.verticalHeader().hide(); self.list.horizontalHeader().setSectionResizeMode(QHeaderView.ResizeMode.ResizeToContents); self.list.horizontalHeader().setSectionResizeMode(1,QHeaderView.ResizeMode.Stretch); layout.addWidget(self.list,1)
        self.details=QListWidget(); self.details.setMaximumHeight(150); layout.addWidget(self.details)
        layout.addWidget(QLabel('Дважды нажмите на ошибку, чтобы разучить этот фрагмент.'))
        self.list.currentCellChanged.connect(lambda row,*_:self.describe(row))
        self.details.itemDoubleClicked.connect(self.open_error)
        self.refresh()

    def refresh(self):
        import datetime
        self.rows=self.store.attempts(); self.list.setRowCount(0); self.details.clear()
        complete=sum(bool(r['report'].get('complete')) for r in self.rows)
        best=max((r['report'].get('accuracy',0) for r in self.rows if r['report'].get('complete')),default=0)
        self.summary.setText(f'Занятий: {len(self.rows)}     Завершено: {complete}     Лучшая точность: {best:g}%')
        self.chart.values=[r['report'].get('accuracy',0) for r in self.rows if r['report'].get('complete')][:20][::-1]
        self.chart.update()
        self.chart.dates=[r['created'] for r in self.rows if r['report'].get('complete')][:20][::-1]
        self.list.setRowCount(len(self.rows))
        for index,row in enumerate(self.rows):
            r=row['report']; date=datetime.datetime.fromtimestamp(row['created']).strftime('%d.%m.%Y %H:%M')
            values=[date,r.get('title','Занятие'),r.get('mode',''),f"{r.get('accuracy',0):g}%",f"{r.get('speed',1)*100:g}%",'Завершено' if r.get('complete') else 'Прервано']
            for column,value in enumerate(values): self.list.setItem(index,column,QTableWidgetItem(value))
            self.list.setRowHeight(index,40)
        if self.rows: self.list.setCurrentCell(0,0)

    def describe(self,index):
        self.details.clear()
        if not 0<=index<len(self.rows): return
        report=self.rows[index]['report']
        for m in report.get('mistakes',[]):
            item=QListWidgetItem(f"{m['position']:.2f} с · {note_name(m['pitch'])} · {m['kind']}" + (f" ({m['delta']:+.2f} с)" if m.get('delta') else ''))
            item.setData(Qt.ItemDataRole.UserRole,m['position']); self.details.addItem(item)
        if not report.get('mistakes'): self.details.addItem('Ошибок не зарегистрировано.' if report.get('complete') else 'Незавершённая попытка — итоговая оценка ограничена.')

    def open_error(self,item):
        pos=item.data(Qt.ItemDataRole.UserRole); index=self.list.currentRow()
        if pos is not None and 0<=index<len(self.rows):
            import hashlib
            from pianoflow.library.store import encode
            row=self.rows[index]; song=self.store.get(row['song_id'])
            if song and row['report'].get('score_version') != hashlib.sha256(encode(song).encode()).hexdigest():
                QMessageBox.information(self,'Аранжировка изменилась','Разметка или ноты изменились после этой попытки. Откройте песню из библиотеки для новой практики.'); return
            self.practice_error.emit(row['song_id'],pos)
