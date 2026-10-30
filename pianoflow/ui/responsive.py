# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 redgion and Clavunora contributors
"""Wrapping controls and touch-friendly layout shared by desktop and Android."""
from PySide6.QtCore import QRect, QSize, Qt
from PySide6.QtWidgets import QLayout, QSizePolicy, QListWidget


class FlowLayout(QLayout):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.items = []
        self.setContentsMargins(0, 0, 0, 0)
        self.setSpacing(8)

    def addItem(self, item): self.items.append(item)
    def count(self): return len(self.items)
    def itemAt(self, index): return self.items[index] if index < len(self.items) else None
    def takeAt(self, index): return self.items.pop(index) if index < len(self.items) else None
    def expandingDirections(self): return Qt.Orientation(0)
    def hasHeightForWidth(self): return True
    def heightForWidth(self, width): return self.arrange(QRect(0, 0, width, 0), True)
    def sizeHint(self): return self.minimumSize()
    def minimumSize(self):
        size = QSize()
        for item in self.items:
            if not item.isEmpty(): size = size.expandedTo(item.minimumSize())
        return size
    def addStretch(self, stretch=0): pass
    def addWidget(self, widget, *args):
        widget.setSizePolicy(QSizePolicy.Policy.Preferred, QSizePolicy.Policy.Fixed)
        super().addWidget(widget)
    def setGeometry(self, rect):
        super().setGeometry(rect)
        self.arrange(rect, False)
    def arrange(self, rect, test):
        x, y, height = rect.x(), rect.y(), 0
        for item in self.items:
            if item.isEmpty(): continue
            size = item.sizeHint()
            width = min(size.width(), rect.width())
            if x > rect.x() and x + width > rect.right() + 1:
                x, y, height = rect.x(), y + height + self.spacing(), 0
            if not test: item.setGeometry(QRect(x, y, width, size.height()))
            x += width + self.spacing()
            height = max(height, size.height())
        return y + height - rect.y()


class IconGridList(QListWidget):
    """Resize icon cells using the actual viewport, including after first show."""
    def __init__(self, columns=3, cell_height=220, parent=None):
        super().__init__(parent)
        self.max_columns=columns; self.cell_height=cell_height

    def resizeEvent(self,event):
        super().resizeEvent(event)
        width=self.viewport().width()
        columns=max(1,min(self.max_columns,width//260))
        from PySide6.QtCore import QSize
        self.setGridSize(QSize(max(190,(width-16)//columns-12),self.cell_height))
        self.fit_items()

    def fit_items(self):
        from PySide6.QtCore import QSize
        for i in range(self.count()):
            self.item(i).setSizeHint(QSize(self.gridSize().width()-10,self.cell_height-10))

    def addItem(self,item):
        super().addItem(item)
        self.fit_items()
