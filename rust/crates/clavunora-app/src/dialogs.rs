// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Native pickers on Windows/macOS; existing optional desktop pickers on Linux.
use anyhow::Result;
use std::path::PathBuf;

pub enum Kind {
    Score,
    BackupOpen,
    BackupSave,
}
pub fn choose(kind: Kind) -> Result<Option<PathBuf>> {
    let (title, extensions) = match kind {
        Kind::Score => (
            "Импорт нот в Clavunora",
            &["mid", "midi", "musicxml", "xml", "mxl"][..],
        ),
        Kind::BackupOpen => ("Восстановить Clavunora из копии", &["clavunora-backup"][..]),
        Kind::BackupSave => (
            "Создать резервную копию Clavunora",
            &["clavunora-backup"][..],
        ),
    };
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let picker = rfd::FileDialog::new()
            .set_title(title)
            .add_filter("Clavunora", extensions);
        Ok(if matches!(kind, Kind::BackupSave) {
            picker
                .set_file_name("Clavunora.clavunora-backup")
                .save_file()
        } else {
            picker.pick_file()
        })
    }
    #[cfg(target_os = "android")]
    {
        let _ = (title, extensions);
        anyhow::bail!("Системный выбор файла Android пока недоступен в этой тестовой сборке")
    }
    #[cfg(target_os = "linux")]
    {
        use anyhow::Context;
        use std::process::Command;
        let save = matches!(kind, Kind::BackupSave);
        let filter = extensions
            .iter()
            .map(|s| format!("*.{s}"))
            .collect::<Vec<_>>()
            .join(" ");
        let initial = if save {
            "Clavunora.clavunora-backup"
        } else {
            "."
        };
        let output = match Command::new("kdialog")
            .args([
                if save {
                    "--getsavefilename"
                } else {
                    "--getopenfilename"
                },
                initial,
                &format!("{filter}|Clavunora"),
                "--title",
                title,
            ])
            .output()
        {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut command = Command::new("zenity");
                command.args([
                    "--file-selection",
                    &format!("--title={title}"),
                    &format!("--file-filter=Clavunora | {filter}"),
                ]);
                if save {
                    command.args([
                        "--save",
                        "--confirm-overwrite",
                        "--filename=Clavunora.clavunora-backup",
                    ]);
                }
                command
                    .output()
                    .context("Диалог недоступен. Укажите полный путь вручную")?
            }
            Err(e) => return Err(e.into()),
        };
        if !output.status.success() {
            if output.status.code() == Some(1) {
                return Ok(None);
            }
            anyhow::bail!("Диалог не открылся. Укажите полный путь вручную");
        }
        let value = String::from_utf8(output.stdout)?
            .trim_end_matches(['\n', '\r'])
            .to_string();
        Ok((!value.is_empty()).then(|| PathBuf::from(value)))
    }
}
