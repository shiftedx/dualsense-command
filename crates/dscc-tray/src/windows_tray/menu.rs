use super::{
    TrayHealthSummary, CMD_CHECK_UPDATES, CMD_OPEN_BUTTON_MAPPING, CMD_OPEN_HAPTICS, CMD_OPEN_UI,
    CMD_QUIT, CMD_RESTART, CMD_START, CMD_STOP, COLOR_ACTUATION, COLOR_OVERDRIVE, COLOR_READY,
    COLOR_TUNGSTEN, MENU_HEADER_HEIGHT, MENU_ITEM_HEIGHT, MENU_READOUT_HEIGHT,
    MENU_SEPARATOR_HEIGHT, MENU_WIDTH,
};
use windows_sys::Win32::Foundation::{COLORREF, HWND, RECT};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TrayMenuKind {
    Header,
    Readout,
    Action,
    Separator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TrayMenuAccent {
    Brand,
    Ready,
    Danger,
    Neutral,
}

#[derive(Debug)]
pub(super) struct TrayMenuDescriptor {
    pub(super) kind: TrayMenuKind,
    pub(super) label: String,
    pub(super) detail: String,
    pub(super) accent: TrayMenuAccent,
}

impl TrayMenuDescriptor {
    pub(super) fn new(
        kind: TrayMenuKind,
        label: impl Into<String>,
        detail: impl Into<String>,
        accent: TrayMenuAccent,
    ) -> Self {
        Self {
            kind,
            label: label.into(),
            detail: detail.into(),
            accent,
        }
    }
}

#[derive(Debug)]
pub(super) struct TrayMenuEntry {
    pub(super) command: usize,
    pub(super) descriptor: TrayMenuDescriptor,
    pub(super) disabled: bool,
}

#[derive(Debug)]
pub(super) struct TrayPopupState {
    pub(super) owner: HWND,
    pub(super) entries: Vec<TrayMenuEntry>,
    pub(super) hover_index: Option<usize>,
}

impl TrayPopupState {
    pub(super) fn new(owner: HWND, entries: Vec<TrayMenuEntry>) -> Self {
        Self {
            owner,
            entries,
            hover_index: None,
        }
    }

    pub(super) fn height(&self) -> i32 {
        tray_menu_height(&self.entries)
    }

    pub(super) fn item_rect(&self, index: usize) -> Option<RECT> {
        if index >= self.entries.len() {
            return None;
        }

        let top = self
            .entries
            .iter()
            .take(index)
            .map(|entry| menu_item_height(entry.descriptor.kind) as i32)
            .sum::<i32>();
        let bottom = top + menu_item_height(self.entries[index].descriptor.kind) as i32;
        Some(RECT {
            left: 0,
            top,
            right: MENU_WIDTH as i32,
            bottom,
        })
    }

    pub(super) fn item_at(&self, y: i32) -> Option<usize> {
        let mut top = 0;
        for (index, entry) in self.entries.iter().enumerate() {
            let bottom = top + menu_item_height(entry.descriptor.kind) as i32;
            if y >= top && y < bottom {
                return Some(index);
            }
            top = bottom;
        }
        None
    }
}

pub(super) fn tray_menu_entries(
    summary: &TrayHealthSummary,
    owned_agent: bool,
) -> Vec<TrayMenuEntry> {
    let mut entries = vec![
        TrayMenuEntry {
            command: 0,
            descriptor: TrayMenuDescriptor::new(
                TrayMenuKind::Header,
                "DualSense Command Center",
                "Local haptics control",
                TrayMenuAccent::Brand,
            ),
            disabled: true,
        },
        TrayMenuEntry {
            command: 0,
            descriptor: TrayMenuDescriptor::new(
                TrayMenuKind::Readout,
                summary.agent_label.clone(),
                summary.agent_detail.clone(),
                summary.agent_accent,
            ),
            disabled: true,
        },
    ];

    entries.extend([
        TrayMenuEntry {
            command: 0,
            descriptor: TrayMenuDescriptor::new(
                TrayMenuKind::Readout,
                summary.profile_label.clone(),
                summary.profile_detail.clone(),
                summary.profile_accent,
            ),
            disabled: true,
        },
        TrayMenuEntry {
            command: 0,
            descriptor: TrayMenuDescriptor::new(
                TrayMenuKind::Readout,
                summary.controller_label.clone(),
                summary.controller_detail.clone(),
                summary.controller_accent,
            ),
            disabled: true,
        },
        TrayMenuEntry {
            command: 0,
            descriptor: TrayMenuDescriptor::new(
                TrayMenuKind::Readout,
                summary.diagnostics_label.clone(),
                summary.diagnostics_detail.clone(),
                summary.diagnostics_accent,
            ),
            disabled: true,
        },
        TrayMenuEntry {
            command: 0,
            descriptor: separator_descriptor(),
            disabled: true,
        },
        TrayMenuEntry {
            command: CMD_OPEN_UI,
            descriptor: TrayMenuDescriptor::new(
                TrayMenuKind::Action,
                "Dashboard",
                "Open controller and profile overview",
                TrayMenuAccent::Brand,
            ),
            disabled: false,
        },
        TrayMenuEntry {
            command: CMD_OPEN_HAPTICS,
            descriptor: TrayMenuDescriptor::new(
                TrayMenuKind::Action,
                "Triggers & Haptics",
                "Open adaptive trigger tuning",
                TrayMenuAccent::Brand,
            ),
            disabled: false,
        },
        TrayMenuEntry {
            command: CMD_OPEN_BUTTON_MAPPING,
            descriptor: TrayMenuDescriptor::new(
                TrayMenuKind::Action,
                "Button Mapping",
                "Open button layout editor",
                TrayMenuAccent::Brand,
            ),
            disabled: false,
        },
        TrayMenuEntry {
            command: CMD_CHECK_UPDATES,
            descriptor: TrayMenuDescriptor::new(
                TrayMenuKind::Action,
                "Check for Updates...",
                "Open latest GitHub release",
                TrayMenuAccent::Neutral,
            ),
            disabled: false,
        },
        TrayMenuEntry {
            command: 0,
            descriptor: separator_descriptor(),
            disabled: true,
        },
    ]);

    if summary.agent_running && owned_agent {
        entries.extend([
            TrayMenuEntry {
                command: CMD_STOP,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Action,
                    "Stop Agent",
                    "Stop local runtime",
                    TrayMenuAccent::Danger,
                ),
                disabled: false,
            },
            TrayMenuEntry {
                command: CMD_RESTART,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Action,
                    "Restart Agent",
                    "Refresh local runtime",
                    TrayMenuAccent::Brand,
                ),
                disabled: false,
            },
            TrayMenuEntry {
                command: 0,
                descriptor: separator_descriptor(),
                disabled: true,
            },
        ]);
    } else if !summary.agent_running {
        entries.extend([
            TrayMenuEntry {
                command: CMD_START,
                descriptor: TrayMenuDescriptor::new(
                    TrayMenuKind::Action,
                    "Start Agent",
                    "Launch local runtime",
                    TrayMenuAccent::Ready,
                ),
                disabled: false,
            },
            TrayMenuEntry {
                command: 0,
                descriptor: separator_descriptor(),
                disabled: true,
            },
        ]);
    }

    entries.push(TrayMenuEntry {
        command: CMD_QUIT,
        descriptor: TrayMenuDescriptor::new(
            TrayMenuKind::Action,
            "Quit DSCC",
            if owned_agent {
                "Stop owned agent and tray"
            } else {
                "Close tray"
            },
            TrayMenuAccent::Danger,
        ),
        disabled: false,
    });
    entries
}

fn separator_descriptor() -> TrayMenuDescriptor {
    TrayMenuDescriptor::new(TrayMenuKind::Separator, "", "", TrayMenuAccent::Neutral)
}

pub(super) fn menu_item_height(kind: TrayMenuKind) -> u32 {
    match kind {
        TrayMenuKind::Header => MENU_HEADER_HEIGHT,
        TrayMenuKind::Readout => MENU_READOUT_HEIGHT,
        TrayMenuKind::Action => MENU_ITEM_HEIGHT,
        TrayMenuKind::Separator => MENU_SEPARATOR_HEIGHT,
    }
}

pub(super) fn tray_menu_height(entries: &[TrayMenuEntry]) -> i32 {
    entries
        .iter()
        .map(|entry| menu_item_height(entry.descriptor.kind) as i32)
        .sum()
}

pub(super) fn menu_accent_color(accent: TrayMenuAccent) -> COLORREF {
    match accent {
        TrayMenuAccent::Brand => COLOR_ACTUATION,
        TrayMenuAccent::Ready => COLOR_READY,
        TrayMenuAccent::Danger => COLOR_OVERDRIVE,
        TrayMenuAccent::Neutral => COLOR_TUNGSTEN,
    }
}
