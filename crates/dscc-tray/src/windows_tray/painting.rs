use super::{
    menu_accent_color, wide_null, wide_text, TrayMenuAccent, TrayMenuDescriptor, COLOR_ACTUATION,
    COLOR_CARBON, COLOR_DISABLED, COLOR_HAPTIC, COLOR_LINE, COLOR_OBSIDIAN, COLOR_OVERDRIVE,
    COLOR_SELECTED, COLOR_TUNGSTEN, COLOR_WHITE, MENU_CORNER_RADIUS,
};
use std::ptr::null_mut;
use windows_sys::Win32::{
    Foundation::{COLORREF, HWND, RECT},
    Graphics::Gdi::{
        CreateFontW, CreatePen, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, DrawTextW,
        Ellipse, FillRect, RoundRect, SelectObject, SetBkMode, SetTextColor, SetWindowRgn,
        CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DT_END_ELLIPSIS, DT_LEFT,
        DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD,
        OUT_DEFAULT_PRECIS, PS_SOLID, TRANSPARENT,
    },
};
pub(super) unsafe fn apply_popup_shape(hwnd: HWND, width: i32, height: i32) {
    let region = CreateRoundRectRgn(
        0,
        0,
        width + 1,
        height + 1,
        MENU_CORNER_RADIUS,
        MENU_CORNER_RADIUS,
    );
    if region.is_null() {
        return;
    }

    if SetWindowRgn(hwnd, region, 1) == 0 {
        DeleteObject(region);
    }
}

pub(super) unsafe fn draw_menu_header(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    rect: RECT,
    descriptor: &TrayMenuDescriptor,
) {
    fill_rect(hdc, rect, COLOR_OBSIDIAN);
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.bottom - 1,
            right: rect.right,
            bottom: rect.bottom,
        },
        COLOR_LINE,
    );

    let title_rect = RECT {
        left: rect.left + 28,
        top: rect.top + 4,
        right: rect.right - 14,
        bottom: rect.top + 20,
    };
    draw_text_line(
        hdc,
        &descriptor.label,
        title_rect,
        COLOR_WHITE,
        13,
        FW_SEMIBOLD,
    );

    draw_dot(hdc, rect.left + 14, rect.top + 12, 9, COLOR_ACTUATION);
    let status_rect = RECT {
        left: rect.left + 28,
        top: rect.top + 19,
        right: rect.right - 14,
        bottom: rect.bottom - 3,
    };
    draw_text_line(
        hdc,
        &descriptor.detail,
        status_rect,
        COLOR_TUNGSTEN,
        9,
        FW_NORMAL,
    );
}

pub(super) unsafe fn draw_menu_readout(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    rect: RECT,
    descriptor: &TrayMenuDescriptor,
) {
    fill_rect(hdc, rect, COLOR_CARBON);
    draw_dot(
        hdc,
        rect.left + 16,
        rect.top + 12,
        8,
        menu_accent_color(descriptor.accent),
    );

    let label_rect = RECT {
        left: rect.left + 32,
        top: rect.top + 2,
        right: rect.right - 14,
        bottom: rect.top + 17,
    };
    draw_text_line(
        hdc,
        &descriptor.label,
        label_rect,
        COLOR_HAPTIC,
        11,
        FW_SEMIBOLD,
    );

    let detail_rect = RECT {
        left: rect.left + 32,
        top: rect.top + 16,
        right: rect.right - 14,
        bottom: rect.bottom - 3,
    };
    draw_text_line(
        hdc,
        &descriptor.detail,
        detail_rect,
        COLOR_TUNGSTEN,
        9,
        FW_NORMAL,
    );
}

pub(super) unsafe fn draw_menu_action(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    rect: RECT,
    descriptor: &TrayMenuDescriptor,
    selected: bool,
    disabled: bool,
) {
    fill_rect(
        hdc,
        rect,
        if selected {
            COLOR_SELECTED
        } else {
            COLOR_CARBON
        },
    );

    let label_color = if disabled {
        COLOR_DISABLED
    } else if descriptor.accent == TrayMenuAccent::Danger {
        COLOR_OVERDRIVE
    } else {
        COLOR_HAPTIC
    };
    let detail_color = if disabled {
        COLOR_DISABLED
    } else {
        COLOR_TUNGSTEN
    };
    let label_rect = RECT {
        left: rect.left + 18,
        top: rect.top + 1,
        right: rect.right - 14,
        bottom: rect.top + 16,
    };
    draw_text_line(
        hdc,
        &descriptor.label,
        label_rect,
        label_color,
        11,
        FW_SEMIBOLD,
    );

    let detail_rect = RECT {
        left: rect.left + 18,
        top: rect.top + 15,
        right: rect.right - 14,
        bottom: rect.bottom - 1,
    };
    draw_text_line(
        hdc,
        &descriptor.detail,
        detail_rect,
        detail_color,
        9,
        FW_NORMAL,
    );
}

pub(super) unsafe fn draw_menu_separator(hdc: windows_sys::Win32::Graphics::Gdi::HDC, rect: RECT) {
    fill_rect(hdc, rect, COLOR_CARBON);
    let top = rect.top + ((rect.bottom - rect.top) / 2);
    fill_rect(
        hdc,
        RECT {
            left: rect.left + 12,
            top,
            right: rect.right - 12,
            bottom: top + 1,
        },
        COLOR_LINE,
    );
}

pub(super) unsafe fn draw_round_panel(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    rect: RECT,
    fill: COLORREF,
    stroke: COLORREF,
) {
    let brush = CreateSolidBrush(fill);
    let pen = CreatePen(PS_SOLID, 1, stroke);
    if brush.is_null() || pen.is_null() {
        if !brush.is_null() {
            DeleteObject(brush);
        }
        if !pen.is_null() {
            DeleteObject(pen);
        }
        fill_rect(hdc, rect, fill);
        return;
    }

    let previous_brush = SelectObject(hdc, brush);
    let previous_pen = SelectObject(hdc, pen);
    RoundRect(
        hdc,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        MENU_CORNER_RADIUS,
        MENU_CORNER_RADIUS,
    );
    if !previous_pen.is_null() {
        SelectObject(hdc, previous_pen);
    }
    if !previous_brush.is_null() {
        SelectObject(hdc, previous_brush);
    }
    DeleteObject(pen);
    DeleteObject(brush);
}

pub(super) unsafe fn draw_round_panel_outline(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    rect: RECT,
    color: COLORREF,
) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.top + 1,
        },
        color,
    );
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.bottom - 1,
            right: rect.right,
            bottom: rect.bottom,
        },
        color,
    );
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.top,
            right: rect.left + 1,
            bottom: rect.bottom,
        },
        color,
    );
    fill_rect(
        hdc,
        RECT {
            left: rect.right - 1,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        },
        color,
    );
}

unsafe fn fill_rect(hdc: windows_sys::Win32::Graphics::Gdi::HDC, rect: RECT, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    if !brush.is_null() {
        FillRect(hdc, &rect, brush);
        DeleteObject(brush);
    }
}

unsafe fn draw_dot(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    left: i32,
    top: i32,
    size: i32,
    color: COLORREF,
) {
    let brush = CreateSolidBrush(color);
    if brush.is_null() {
        return;
    }
    let previous = SelectObject(hdc, brush);
    Ellipse(hdc, left, top, left + size, top + size);
    if !previous.is_null() {
        SelectObject(hdc, previous);
    }
    DeleteObject(brush);
}

unsafe fn draw_text_line(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    text: &str,
    mut rect: RECT,
    color: COLORREF,
    height: i32,
    weight: u32,
) {
    let face = wide_null("Segoe UI");
    let font = CreateFontW(
        -height,
        0,
        0,
        0,
        weight as i32,
        0,
        0,
        0,
        u32::from(DEFAULT_CHARSET),
        u32::from(OUT_DEFAULT_PRECIS),
        u32::from(CLIP_DEFAULT_PRECIS),
        u32::from(CLEARTYPE_QUALITY),
        u32::from(FF_DONTCARE),
        face.as_ptr(),
    );
    let previous = if font.is_null() {
        null_mut()
    } else {
        SelectObject(hdc, font)
    };
    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(hdc, color);
    let text = wide_text(text);
    DrawTextW(
        hdc,
        text.as_ptr(),
        text.len() as i32,
        &mut rect,
        DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
    );
    if !previous.is_null() {
        SelectObject(hdc, previous);
    }
    if !font.is_null() {
        DeleteObject(font);
    }
}
