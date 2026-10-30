# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Centered folder navigation and a quiet, useful home screen."""
from PySide6.QtCore import Qt, Signal, QSize
from PySide6.QtWidgets import QWidget, QVBoxLayout, QHBoxLayout, QGridLayout, QLabel, QToolButton, QLineEdit, QFrame, QPushButton, QProgressBar
from pianoflow.ui.artwork import folder_icon, cover_icon, medal_icon

FOLDERS = [(f'level:{i+1}', title) for i, title in enumerate(('Начальный','Лёгкий','Средний','Сложный','Мастер'))] + [('favorites','Избранное'),('imports','Мои пьесы'),('all','Все произведения')]


class FolderGrid(QWidget):
    selected = Signal(str)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.grid=QGridLayout(self); self.grid.setContentsMargins(0,0,0,0); self.grid.setHorizontalSpacing(16); self.grid.setVerticalSpacing(12)
        self.tiles=[]; self.columns=0
        for index,(key,title) in enumerate(FOLDERS):
            tile=QToolButton(); tile.setObjectName('folderTile'); tile.setToolButtonStyle(Qt.ToolButtonStyle.ToolButtonTextUnderIcon)
            tile.setIcon(folder_icon(index)); tile.setIconSize(QSize(200,132)); tile.setMinimumSize(170,180)
            tile.setAccessibleName(title); tile.clicked.connect(lambda checked=False,k=key:self.selected.emit(k))
            tile.setText(title); self.tiles.append(tile)
        self.arrange(4)

    def arrange(self, columns):
        if columns==self.columns: return
        self.columns=columns
        for tile in self.tiles: self.grid.removeWidget(tile)
        for i,tile in enumerate(self.tiles): self.grid.addWidget(tile,i//columns,i%columns)
        self.setMinimumHeight(((len(self.tiles)+columns-1)//columns)*192)

    def resizeEvent(self,event):
        super().resizeEvent(event); self.arrange(max(1,min(4,self.width()//210)))

    def refresh(self, store):
        from pianoflow.library.catalog import manifest
        builtin={e['id'] for e in manifest()['entries']} | {f'course-{i}' for i in range(1,7)} | {'birthday-bass','birthday-chords'}
        rows=store.songs()
        for tile,(key,title) in zip(self.tiles,FOLDERS):
            count=sum(s.difficulty==int(key[-1]) if key.startswith('level:') else f if key=='favorites' else i not in builtin if key=='imports' else True for i,s,f in rows)
            tile.setText(f'{title}\n{count} произведений'); tile.setToolTip(f'{title} · {count} произведений')


class HomePage(QWidget):
    folder_selected=Signal(str)
    search_requested=Signal(str)
    continue_requested=Signal(str)
    achievements_requested=Signal()

    def __init__(self,store):
        super().__init__(); self.store=store; self.resume_id=None
        layout=QVBoxLayout(self); layout.setContentsMargins(36,24,36,24); layout.setSpacing(14)
        title=QLabel('Clavunora'); title.setObjectName('homeBrand'); title.setAlignment(Qt.AlignmentFlag.AlignCenter); layout.addWidget(title)
        subtitle=QLabel('Выберите музыку для занятия'); subtitle.setObjectName('homeHeading'); subtitle.setAlignment(Qt.AlignmentFlag.AlignCenter); layout.addWidget(subtitle)
        self.search=QLineEdit(); self.search.setPlaceholderText('Найти произведение или композитора'); self.search.setAccessibleName('Поиск по всей библиотеке'); self.search.setMinimumWidth(280); self.search.setMaximumWidth(660)
        self.search.returnPressed.connect(lambda:self.search_requested.emit(self.search.text())); layout.addWidget(self.search,alignment=Qt.AlignmentFlag.AlignHCenter)
        self.folders=FolderGrid(); self.folders.selected.connect(self.folder_selected); layout.addWidget(self.folders,1)
        panel=QFrame(); panel.setObjectName('card'); panel.setMaximumHeight(180); self.bottom=QHBoxLayout(panel); self.bottom.setContentsMargins(22,18,22,18); self.bottom.setSpacing(28)
        self.resume_art=QLabel(); self.resume_art.setFixedSize(92,76); self.bottom.addWidget(self.resume_art)
        left=QVBoxLayout(); left.setSpacing(6); left.addWidget(QLabel('Продолжить занятие')); self.resume=QLabel(); self.resume.setObjectName('sectionTitle'); left.addWidget(self.resume)
        self.resume_detail=QLabel(); self.resume_detail.setObjectName('muted'); left.addWidget(self.resume_detail)
        self.resume_button=QPushButton('Продолжить →'); self.resume_button.setObjectName('primary'); self.resume_button.clicked.connect(self.continue_song); left.addWidget(self.resume_button,alignment=Qt.AlignmentFlag.AlignLeft); self.bottom.addLayout(left,1)
        self.medal_art=QLabel(); self.medal_art.setFixedSize(80,80); self.bottom.addWidget(self.medal_art)
        right=QVBoxLayout(); right.setSpacing(6); right.addWidget(QLabel('Последнее достижение')); self.latest=QLabel(); self.latest.setObjectName('sectionTitle'); right.addWidget(self.latest)
        self.latest_detail=QLabel(); self.latest_detail.setWordWrap(True); self.latest_detail.setObjectName('muted'); right.addWidget(self.latest_detail)
        more=QPushButton('Все достижения →'); more.clicked.connect(self.achievements_requested); right.addWidget(more,alignment=Qt.AlignmentFlag.AlignLeft); self.bottom.addLayout(right,1); layout.addWidget(panel)

    def resizeEvent(self,event):
        super().resizeEvent(event)
        self.search.setMinimumWidth(min(660,max(220,self.width()-80)))
        self.resume_art.setVisible(self.width()>800); self.medal_art.setVisible(self.width()>800)

    def continue_song(self):
        if self.resume_id: self.continue_requested.emit(self.resume_id)

    def refresh(self,states):
        self.folders.refresh(self.store)
        identifier=self.store.preference('last-song')
        song=self.store.get(identifier) if isinstance(identifier,str) else None
        if not song:
            for row in self.store.attempts():
                song=self.store.get(row['song_id'])
                if song: identifier=row['song_id']; break
        if not song: identifier='course-1'; song=self.store.get(identifier)
        self.resume_id=identifier if song else None
        self.resume.setText(song.title if song else 'Начните с библиотеки')
        self.resume_detail.setText(song.composer or 'Курс с нуля · первые шаги' if song else 'Выберите произведение для занятия')
        self.resume_button.setEnabled(bool(song))
        if song: self.resume_art.setPixmap(cover_icon(song.title,song.difficulty).pixmap(QSize(92,76)))
        unlocked=[s for s in states if s['earned']]
        recent=max(unlocked,key=lambda s:s['earned']) if unlocked else None
        self.medal_art.setPixmap(medal_icon(recent['achievement'].symbol if recent else '♪',bool(recent)).pixmap(QSize(80,80)))
        self.latest.setText('●  '+recent['achievement'].title if recent else 'Ваша первая медаль впереди')
        self.latest_detail.setText(recent['achievement'].description if recent else 'Сыграйте первые верные ноты в занятии.')
