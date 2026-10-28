//! Main window: menus, toolbar, status bar, keyboard shortcuts and layout.

use eframe::egui::{self, Button, Key, KeyboardShortcut, Modifiers, Ui};

use crate::app::{After, App, Mode};
use crate::i18n::{Lang, tr};
use crate::model::AxisId;
use crate::view::ZOOM_PRESETS;

const SC_OPEN_IMAGE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::O);
const SC_OPEN_WS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SC_SAVE_WS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const CTRL_ALT: Modifiers = Modifiers { alt: true, ctrl: true, shift: false, mac_cmd: false, command: true };
const SC_COPY: KeyboardShortcut = KeyboardShortcut::new(CTRL_ALT, Key::C);
const SC_EXPORT: KeyboardShortcut = KeyboardShortcut::new(CTRL_ALT, Key::E);
const SC_TRACE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::T);
const SC_AREA: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);
const SC_CAPTURE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::P);
const SC_ERASER: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::E);
const SC_REORDER: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::R);
const SC_ADD_LINE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::L);
const SC_UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const SC_UNDO2: KeyboardShortcut = KeyboardShortcut::new(Modifiers::ALT, Key::Backspace);
const SC_IMAGE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::I);
const SC_GRID: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::G);
const SC_HELP: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F1);

#[derive(Clone, Copy, PartialEq)]
enum Cmd {
    OpenImage,
    OpenWorkspace,
    SaveWorkspace,
    SaveWorkspaceAs,
    Close,
    Exit,
    Export,
    Copy,
    SetScale,
    DefaultAxes,
    AdjustScale,
    AutoTrace,
    DigitizeArea,
    PointCapture,
    Eraser,
    Reorder,
    AddLine,
    LineColor,
    BkColor,
    AcquireColors,
    RemoveAll,
    Undo,
    Fit,
    ZoomIn,
    ZoomOut,
    ShowImage,
    ShowGrid,
    GridSettings,
    ShowAllLines,
    ConnectPoints,
    Info,
    Magnifier,
    Options,
    Help,
    About,
}

impl App {
    fn needs_image(c: Cmd) -> bool {
        !matches!(
            c,
            Cmd::OpenImage
                | Cmd::OpenWorkspace
                | Cmd::Exit
                | Cmd::Options
                | Cmd::Help
                | Cmd::About
                | Cmd::Info
                | Cmd::Magnifier
                | Cmd::ShowAllLines
                | Cmd::ConnectPoints
        )
    }

    fn enabled(&self, c: Cmd) -> bool {
        match c {
            Cmd::Undo => !self.undo.is_empty(),
            Cmd::Export | Cmd::Copy => self.transform().is_ok(),
            c => !Self::needs_image(c) || self.has_image(),
        }
    }

    fn checked(&self, c: Cmd) -> Option<bool> {
        Some(match c {
            Cmd::SetScale => matches!(self.mode, Mode::SetScale(_)),
            Cmd::AutoTrace => self.mode == Mode::AutoTrace,
            Cmd::DigitizeArea => self.mode == Mode::DigitizeArea,
            Cmd::PointCapture => self.mode == Mode::PointCapture,
            Cmd::Eraser => self.mode == Mode::Eraser,
            Cmd::Reorder => self.mode == Mode::Reorder,
            Cmd::Fit => self.view.fit,
            Cmd::ShowImage => self.settings.show_image,
            Cmd::ShowGrid => self.ws.grid.show_grid,
            Cmd::ShowAllLines => self.settings.show_all_lines,
            Cmd::ConnectPoints => self.settings.connect_points,
            Cmd::Info => self.settings.show_info,
            Cmd::Magnifier => self.settings.show_magnifier,
            _ => return None,
        })
    }

    fn toggle_mode(&mut self, m: Mode) {
        if self.mode == m { self.set_mode(Mode::Normal) } else { self.set_mode(m) }
    }

    fn run(&mut self, c: Cmd) {
        if !self.enabled(c) {
            return;
        }
        match c {
            Cmd::OpenImage => self.guard(After::OpenImage(None)),
            Cmd::OpenWorkspace => self.guard(After::OpenWorkspace(None)),
            Cmd::SaveWorkspace => {
                self.save_workspace(false);
            }
            Cmd::SaveWorkspaceAs => {
                self.save_workspace(true);
            }
            Cmd::Close => self.guard(After::Close),
            Cmd::Exit => self.guard(After::Exit),
            Cmd::Export => self.open_export(),
            Cmd::Copy => self.copy_to_clipboard(),
            Cmd::SetScale => {
                if matches!(self.mode, Mode::SetScale(_)) {
                    self.set_mode(Mode::Normal)
                } else {
                    self.set_mode(Mode::SetScale(AxisId::Xmin))
                }
            }
            Cmd::DefaultAxes => self.set_default_axes(),
            Cmd::AdjustScale => self.open_adjust_scale(),
            Cmd::AutoTrace => self.toggle_mode(Mode::AutoTrace),
            Cmd::DigitizeArea => {
                if self.mode == Mode::DigitizeArea {
                    self.set_mode(Mode::Normal);
                } else {
                    self.open_digitize_area();
                }
            }
            Cmd::PointCapture => self.toggle_mode(Mode::PointCapture),
            Cmd::Eraser => self.toggle_mode(Mode::Eraser),
            Cmd::Reorder => self.toggle_mode(Mode::Reorder),
            Cmd::AddLine => self.open_line_props(None),
            Cmd::LineColor => self.set_mode(Mode::PickColor { line: true }),
            Cmd::BkColor => self.set_mode(Mode::PickColor { line: false }),
            Cmd::AcquireColors => {
                self.checkpoint();
                self.acquire_colors();
            }
            Cmd::RemoveAll => self.dialogs.confirm_remove_all = true,
            Cmd::Undo => self.do_undo(),
            Cmd::Fit => self.view.fit = true,
            Cmd::ZoomIn | Cmd::ZoomOut => {
                let s = if c == Cmd::ZoomIn { self.view.zoom_in_step() } else { self.view.zoom_out_step() };
                let anchor = self.hover.map(|h| self.view.to_screen(h)).unwrap_or(self.view.rect.center());
                self.view.zoom_at(anchor, s);
            }
            Cmd::ShowImage => self.settings.show_image ^= true,
            Cmd::ShowGrid => {
                self.ws.grid.show_grid ^= true;
                self.modified = true;
            }
            Cmd::GridSettings => self.open_grid_settings(),
            Cmd::ShowAllLines => self.settings.show_all_lines ^= true,
            Cmd::ConnectPoints => self.settings.connect_points ^= true,
            Cmd::Info => self.settings.show_info ^= true,
            Cmd::Magnifier => self.settings.show_magnifier ^= true,
            Cmd::Options => self.open_options(),
            Cmd::Help => self.show_help = true,
            Cmd::About => self.show_about = true,
        }
    }

    fn menu_item(&mut self, ui: &mut Ui, c: Cmd, text: &str, sc: Option<KeyboardShortcut>) {
        let mut b = match self.checked(c) {
            Some(on) => Button::new(text).selected(on),
            None => Button::new(text),
        };
        if let Some(sc) = sc {
            b = b.shortcut_text(ui.ctx().format_shortcut(&sc));
        }
        if ui.add_enabled(self.enabled(c), b).clicked() {
            self.run(c);
            ui.close();
        }
    }

    fn menus(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tr("File"), |ui| {
                self.menu_item(ui, Cmd::OpenImage, tr("Open Image"), Some(SC_OPEN_IMAGE));
                ui.separator();
                self.menu_item(ui, Cmd::Export, tr("Export data"), Some(SC_EXPORT));
                self.menu_item(ui, Cmd::Copy, tr("Copy data to buffer"), Some(SC_COPY));
                ui.separator();
                self.menu_item(ui, Cmd::OpenWorkspace, tr("Open Workspace"), Some(SC_OPEN_WS));
                self.menu_item(ui, Cmd::SaveWorkspaceAs, tr("Save Workspace As..."), None);
                self.menu_item(ui, Cmd::SaveWorkspace, tr("Save Workspace"), Some(SC_SAVE_WS));
                self.menu_item(ui, Cmd::Close, tr("Close"), None);
                if !self.settings.recent.is_empty() {
                    ui.separator();
                    let recent = self.settings.recent.clone();
                    for (i, p) in recent.iter().enumerate() {
                        if ui.button(format!("{}: {}", i + 1, p)).clicked() {
                            self.open_path(std::path::Path::new(p));
                            ui.close();
                        }
                    }
                }
                ui.separator();
                self.menu_item(ui, Cmd::Exit, tr("Exit"), None);
            });
            ui.menu_button(tr("Operations"), |ui| {
                self.menu_item(ui, Cmd::SetScale, tr("Set the scale"), None);
                self.menu_item(ui, Cmd::DefaultAxes, tr("Set default axes"), None);
                self.menu_item(ui, Cmd::AdjustScale, tr("Adjust the scale"), None);
                ui.separator();
                self.menu_item(ui, Cmd::AutoTrace, tr("Auto trace lines"), Some(SC_TRACE));
                self.menu_item(ui, Cmd::DigitizeArea, tr("Digitize area"), Some(SC_AREA));
                self.menu_item(ui, Cmd::PointCapture, tr("Point capture mode"), Some(SC_CAPTURE));
                self.menu_item(ui, Cmd::Eraser, tr("Data points eraser"), Some(SC_ERASER));
                ui.separator();
                self.menu_item(ui, Cmd::Reorder, tr("Reorder tool"), Some(SC_REORDER));
                ui.separator();
                self.menu_item(ui, Cmd::AddLine, tr("Add line"), Some(SC_ADD_LINE));
                self.menu_item(ui, Cmd::LineColor, tr("Set line color"), None);
                self.menu_item(ui, Cmd::BkColor, tr("Set background color"), None);
                self.menu_item(ui, Cmd::AcquireColors, tr("Automatically acquire colors"), None);
                ui.separator();
                self.menu_item(ui, Cmd::RemoveAll, tr("Remove all data"), None);
                ui.separator();
                self.menu_item(ui, Cmd::Undo, tr("Undo"), Some(SC_UNDO));
            });
            ui.menu_button(tr("View"), |ui| {
                self.menu_item(ui, Cmd::Fit, tr("Fit to window"), None);
                self.menu_item(ui, Cmd::ZoomIn, tr("Zoom in"), None);
                self.menu_item(ui, Cmd::ZoomOut, tr("Zoom out"), None);
                ui.add_enabled_ui(self.has_image(), |ui| {
                    ui.menu_button(tr("Zoom"), |ui| {
                        for z in ZOOM_PRESETS {
                            let on = !self.view.fit && (self.view.scale * 100.0 - z).abs() < 0.01;
                            if ui.add(Button::new(format!("{z:.0}%")).selected(on)).clicked() {
                                let c = self.view.rect.center();
                                self.view.zoom_at(c, z / 100.0);
                                ui.close();
                            }
                        }
                    });
                });
                ui.separator();
                self.menu_item(ui, Cmd::ShowImage, tr("Show image"), Some(SC_IMAGE));
                self.menu_item(ui, Cmd::ShowGrid, tr("Show grid"), Some(SC_GRID));
                self.menu_item(ui, Cmd::GridSettings, tr("Grid settings..."), None);
                ui.separator();
                self.menu_item(ui, Cmd::ShowAllLines, tr("Show all lines"), None);
                self.menu_item(ui, Cmd::ConnectPoints, tr("Connect points"), None);
                ui.separator();
                self.menu_item(ui, Cmd::Info, tr("Information pane"), None);
                self.menu_item(ui, Cmd::Magnifier, tr("Magnifier"), None);
            });
            ui.menu_button(tr("Settings"), |ui| {
                self.menu_item(ui, Cmd::Options, tr("Options ..."), None);
                ui.menu_button(tr("Language"), |ui| {
                    for l in Lang::ALL {
                        if ui.add(Button::new(l.name()).selected(self.settings.lang == l)).clicked() {
                            self.settings.lang = l;
                            crate::i18n::set_lang(l);
                            ui.close();
                        }
                    }
                });
            });
            ui.menu_button(tr("Help"), |ui| {
                self.menu_item(ui, Cmd::Help, tr("Contents"), Some(SC_HELP));
                ui.separator();
                self.menu_item(ui, Cmd::About, tr("About GetData ..."), None);
            });
        });
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            use crate::icons::Icon;
            let groups: &[&[(Cmd, Icon, &str)]] = &[
                &[
                    (Cmd::OpenImage, Icon::OpenImage, "Open Image"),
                    (Cmd::OpenWorkspace, Icon::OpenWorkspace, "Open Workspace"),
                    (Cmd::SaveWorkspace, Icon::Save, "Save Workspace"),
                ],
                &[(Cmd::Copy, Icon::Copy, "Copy data to buffer")],
                &[
                    (Cmd::SetScale, Icon::SetScale, "Set the scale"),
                    (Cmd::AutoTrace, Icon::AutoTrace, "Auto trace lines"),
                    (Cmd::DigitizeArea, Icon::DigitizeArea, "Digitize area"),
                    (Cmd::PointCapture, Icon::PointCapture, "Point capture mode"),
                    (Cmd::Eraser, Icon::Eraser, "Eraser"),
                    (Cmd::Reorder, Icon::Reorder, "Reorder tool"),
                ],
                &[(Cmd::Undo, Icon::Undo, "Undo")],
                &[(Cmd::Fit, Icon::Fit, "Fit to window"), (Cmd::ZoomIn, Icon::ZoomIn, "Zoom in"), (Cmd::ZoomOut, Icon::ZoomOut, "Zoom out")],
                &[
                    (Cmd::ShowImage, Icon::ShowImage, "Show image"),
                    (Cmd::ShowGrid, Icon::ShowGrid, "Show grid"),
                    (Cmd::ShowAllLines, Icon::ShowAllLines, "Show all lines"),
                    (Cmd::ConnectPoints, Icon::ConnectPoints, "Connect points"),
                ],
            ];
            for (gi, g) in groups.iter().enumerate() {
                if gi > 0 {
                    ui.separator();
                }
                for &(c, icon, tip) in g.iter() {
                    let b = Button::new("").min_size(egui::vec2(30.0, 28.0));
                    let b = match self.checked(c) {
                        Some(on) => b.selected(on),
                        None => b,
                    };
                    let enabled = self.enabled(c);
                    let r = ui.add_enabled(enabled, b);
                    let color = if enabled { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
                    crate::icons::paint(ui.painter(), r.rect.shrink(6.0), icon, color);
                    if r.on_hover_text(tr(tip)).clicked() {
                        self.run(c);
                    }
                }
            }
        });
    }

    fn mode_hint(&self) -> &'static str {
        match &self.mode {
            Mode::Normal => tr("Shift + left mouse button to set/remove points"),
            Mode::SetScale(AxisId::Xmin) => tr("Click left mouse button to set Xmin; ESC to abort, SPACE to skip Xmin"),
            Mode::SetScale(AxisId::Xmax) => tr("Click left mouse button to set Xmax; ESC to abort, SPACE to skip Xmax"),
            Mode::SetScale(AxisId::Ymin) => tr("Click left mouse button to set Ymin; ESC to abort, SPACE to skip Ymin"),
            Mode::SetScale(AxisId::Ymax) => tr("Click left mouse button to set Ymax; ESC to abort, SPACE to skip Ymax"),
            Mode::PickColor { line: true } => tr("Click left mouse button to choose line color"),
            Mode::PickColor { line: false } => tr("Click left mouse button to choose background color"),
            Mode::AutoTrace => {
                tr("Left mouse button: auto trace in all directions, right mouse button: choose direction for auto trace")
            }
            Mode::DigitizeArea => {
                if self.ws.digitize_area.area_type == crate::model::AreaType::Rectangular {
                    tr("Hold left mouse button and choose the area to digitize; CTRL to move the grid; SHIFT to rotate the grid")
                } else {
                    tr("Hold left mouse button and choose the area to digitize; CTRL to move the grid")
                }
            }
            Mode::PointCapture => tr(
                "Left/right button to set/select and move data points; Ctrl: accurate cursor positioning; Shift: middle of lines only",
            ),
            Mode::Eraser => tr("Shift + left mouse button to change eraser size"),
            Mode::Reorder => tr(
                "Hold Ctrl to start reordering from the middle of the line; Shift + left mouse button to change reorder tool size",
            ),
        }
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let text = self.message.clone().unwrap_or_else(|| self.mode_hint().to_string());
            ui.label(text);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(img) = &self.image {
                    ui.monospace(format!("{}x{}x24", img.width(), img.height()));
                    ui.separator();
                    let z = self.view.scale * 100.0;
                    ui.monospace(if z < 100.0 && z.fract() != 0.0 { format!("{z:.1}%") } else { format!("{z:.0}%") });
                    if let Some(h) = self.hover {
                        ui.separator();
                        ui.monospace(format!("({},{})", h.x.floor() as i32, h.y.floor() as i32));
                    }
                }
            });
        });
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.dialogs.any_open() || ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        let table = [
            (SC_COPY, Cmd::Copy),
            (SC_EXPORT, Cmd::Export),
            (SC_OPEN_WS, Cmd::OpenWorkspace),
            (SC_SAVE_WS, Cmd::SaveWorkspace),
            (SC_TRACE, Cmd::AutoTrace),
            (SC_AREA, Cmd::DigitizeArea),
            (SC_CAPTURE, Cmd::PointCapture),
            (SC_ERASER, Cmd::Eraser),
            (SC_REORDER, Cmd::Reorder),
            (SC_ADD_LINE, Cmd::AddLine),
            (SC_UNDO, Cmd::Undo),
            (SC_UNDO2, Cmd::Undo),
            (SC_IMAGE, Cmd::ShowImage),
            (SC_GRID, Cmd::ShowGrid),
            (SC_HELP, Cmd::Help),
            (SC_OPEN_IMAGE, Cmd::OpenImage),
        ];
        for (sc, c) in table {
            if ctx.input_mut(|i| i.consume_shortcut(&sc)) {
                self.run(c);
            }
        }
        let (plus, minus, esc, space, arrows, pgup, pgdn) = ctx.input(|i| {
            let k = |k| i.key_pressed(k);
            (
                k(Key::Plus) || k(Key::Equals),
                k(Key::Minus),
                k(Key::Escape),
                k(Key::Space),
                [k(Key::ArrowLeft), k(Key::ArrowRight), k(Key::ArrowUp), k(Key::ArrowDown)],
                k(Key::PageUp),
                k(Key::PageDown),
            )
        });
        if plus || pgup {
            self.run(Cmd::ZoomIn);
        }
        if minus || pgdn {
            self.run(Cmd::ZoomOut);
        }
        if esc {
            if self.moving.is_some() {
                self.moving = None;
            } else if self.area.is_some() {
                self.area = None;
            } else {
                self.set_mode(Mode::Normal);
            }
        }
        if space && let Mode::SetScale(a) = self.mode {
            self.advance_scale(a, true);
        }
        let step = 40.0;
        let d = [(-step, 0.0), (step, 0.0), (0.0, -step), (0.0, step)];
        for (pressed, (dx, dy)) in arrows.into_iter().zip(d) {
            if pressed {
                self.view.pan(egui::vec2(-dx, -dy));
            }
        }
    }

    fn help_window(&mut self, ctx: &egui::Context) {
        let ru = crate::i18n::lang() == Lang::Russian;
        egui::Window::new(tr("Help")).open(&mut self.show_help).default_width(520.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(if ru { HELP_RU } else { HELP_EN });
            });
        });
        egui::Window::new(tr("About GetData ..."))
            .open(&mut self.show_about)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("GetData");
                ui.label(format!("{} {}", tr("GetData version"), env!("CARGO_PKG_VERSION")));
                ui.label(tr("Rust/egui"));
                ui.add_space(6.0);
                ui.label("Formats: GDW workspaces; BMP, JPEG, TIFF, PCX, PNG, GIF images; TXT, XLSX, XLS, XML, EPS, DXF export.");
            });
    }
}

const HELP_EN: &str = "Quick start\n\n\
1. Open an image (File => Open Image, or drop a file on the window).\n\n\
2. Set the scale: Operations => Set the scale, then click Xmin, Xmax, Ymin and Ymax and enter their values. \
Axes need not be orthogonal. Log scales are set in Operations => Adjust the scale. \
Operations => Set default axes uses image pixels as coordinates.\n\n\
3. Digitize:\n\
 • Auto trace lines (Ctrl+T): click a solid line; right click to choose the direction.\n\
 • Digitize area (Ctrl+D): set the grid, then drag over the area (Shift rotates the grid, Ctrl moves it). \
Works for dashed lines too.\n\
 • Point capture mode (Ctrl+P): left click sets points, right click selects a point to move; \
Ctrl: sub-pixel position; Shift: middle of the line. Double click in the magnifier sets a precise point.\n\
 • Eraser (Ctrl+E) removes points; Shift+drag changes its size.\n\
 • Reorder tool (Ctrl+R): drag over the points in the right order; Ctrl continues from the middle of the line.\n\n\
Line and background colours are acquired automatically; set them with Operations => Set line/background colour \
or by clicking the colour boxes in the Information pane. The identification method is in Settings => Options => Digitize.\n\n\
4. Copy the data (Ctrl+Alt+C) or export it (Ctrl+Alt+E) to TXT, XLSX, XLS, XML, EPS or DXF.\n\n\
Mouse wheel zooms, the middle button (or the left one in normal mode) pans the image.";

const HELP_RU: &str = "Быстрый старт\n\n\
1. Откройте изображение (Файл => Открыть изображение, или перетащите файл в окно).\n\n\
2. Установите систему координат: Команды => Установить систему координат, затем щёлкните точки Xmin, Xmax, Ymin и Ymax \
и введите их значения. Оси могут быть неортогональными. Логарифмический масштаб задаётся в Команды => Параметры системы координат. \
Команды => Установить оси по умолчанию – координатами будут пиксели изображения.\n\n\
3. Оцифровка:\n\
 • Авто трассировка линий (Ctrl+T): щёлкните по сплошной линии; правая кнопка – выбор направления.\n\
 • Оцифровка области (Ctrl+D): задайте сетку и выделите область мышью (Shift – поворот сетки, Ctrl – сдвиг). \
Подходит и для пунктирных линий.\n\
 • Режим установки точек (Ctrl+P): левая кнопка ставит точку, правая выделяет точку для перемещения; \
Ctrl – субпиксельная точность; Shift – центр линии. Двойной щелчок в лупе ставит точную точку.\n\
 • Ластик (Ctrl+E) удаляет точки; Shift+перетаскивание меняет размер.\n\
 • Упорядочить точки (Ctrl+R): проведите по точкам в нужном порядке; Ctrl – продолжить с середины линии.\n\n\
Цвета линии и фона определяются автоматически; их можно задать через Команды => Выбрать цвет линии/фона \
или щелчком по образцам цвета в окне информации. Метод идентификации – Установки => Параметры => Оцифровка.\n\n\
4. Скопируйте данные (Ctrl+Alt+C) или экспортируйте их (Ctrl+Alt+E) в TXT, XLSX, XLS, XML, EPS или DXF.\n\n\
Колесо мыши – масштаб, средняя кнопка (или левая в обычном режиме) – перемещение изображения.";

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Files dropped on the window.
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if let Some(p) = dropped.into_iter().next() {
            self.open_path(&p);
        }
        // Closing the window: ask to save first.
        if ctx.input(|i| i.viewport().close_requested()) && !self.exit_confirmed && self.modified && self.has_image() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialogs.save_changes = Some(After::Exit);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.window_title()));
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);
        if self.hover.is_some() || matches!(self.mode, Mode::Normal) {
            // A transient message disappears when the mode changes or after a while.
        }
        egui::Panel::top("menu").show(ui, |ui| self.menus(ui));
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        if self.settings.show_info || self.settings.show_magnifier {
            egui::Panel::right("side").resizable(true).default_size(300.0).min_size(220.0).show(ui, |ui| {
                if self.settings.show_magnifier {
                    egui::CollapsingHeader::new(tr("Magnifier")).default_open(true).show(ui, |ui| self.magnifier(ui));
                }
                if self.settings.show_info {
                    egui::ScrollArea::vertical().id_salt("info_scroll").show(ui, |ui| self.info_pane(ui));
                }
            });
        }
        egui::CentralPanel::no_frame().show(ui, |ui| self.canvas(ui));
        self.show_dialogs(&ctx);
        self.help_window(&ctx);
        if ctx.input(|i| i.pointer.any_click()) {
            self.message = None;
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.save_settings(storage);
    }
}
