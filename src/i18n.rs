//! Interface languages.  UI code uses English text as the key; `tr` maps it to
//! the current language.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum Lang {
    #[default]
    English,
    Russian,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::English, Lang::Russian];

    pub fn name(self) -> &'static str {
        match self {
            Lang::English => "English",
            Lang::Russian => "Русский",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_lang(l: Lang) {
    CURRENT.store(l as u8, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == 1 { Lang::Russian } else { Lang::English }
}

const RU: &[(&str, &str)] = &[
    // Menus
    ("File", "Файл"),
    ("Open Image", "Открыть изображение"),
    ("Export data", "Экспорт данных"),
    ("Copy data to buffer", "Копировать данные в буфер"),
    ("Open Workspace", "Открыть рабочее пространство"),
    ("Save Workspace As...", "Сохранить рабочее пространство как..."),
    ("Save Workspace", "Сохранить рабочее пространство"),
    ("Close", "Закрыть"),
    ("Exit", "Выход"),
    ("Recent files", "Последние файлы"),
    ("Operations", "Команды"),
    ("Set the scale", "Установить систему координат"),
    ("Set default axes", "Установить оси по умолчанию"),
    ("Adjust the scale", "Параметры системы координат"),
    ("Auto trace lines", "Авто трассировка линий"),
    ("Digitize area", "Оцифровка области"),
    ("Point capture mode", "Режим установки точек"),
    ("Data points eraser", "Ластик точек данных"),
    ("Reorder tool", "Упорядочить точки"),
    ("Add line", "Добавить линию"),
    ("Set line color", "Выбрать цвет линии"),
    ("Set background color", "Выбрать цвет фона"),
    ("Automatically acquire colors", "Автоматически извлечь цвета"),
    ("Remove all data", "Удалить все данные"),
    ("Undo", "Отмена"),
    ("View", "Вид"),
    ("Fit to window", "По размеру окна"),
    ("Zoom in", "Увеличение"),
    ("Zoom out", "Уменьшение"),
    ("Zoom", "Масштаб"),
    ("Show image", "Показывать изображение"),
    ("Show grid", "Показывать сетку"),
    ("Grid settings...", "Параметры сетки..."),
    ("Show all lines", "Показывать все линии"),
    ("Connect points", "Соединять точки"),
    ("Information pane", "Окно информации"),
    ("Magnifier", "Лупа"),
    ("Settings", "Установки"),
    ("Options ...", "Параметры ..."),
    ("Language", "Язык"),
    ("Help", "Справка"),
    ("Contents", "Содержание"),
    ("About GetData ...", "О программе ..."),
    ("Eraser", "Ластик"),
    // Information pane
    ("Information", "Информация"),
    ("Current status", "Текущее состояние"),
    ("Data", "Данные"),
    ("Current point", "Текущая точка"),
    ("Current line", "Текущая линия"),
    ("Line", "Линия"),
    ("Background", "Фон"),
    ("Properties", "Свойства"),
    ("data points:", "точки данных:"),
    ("data points: {}", "точки данных: {}"),
    ("total data points:", "всего точек данных:"),
    ("Coordinate system", "Система координат"),
    ("Default axes", "Оси по умолчанию"),
    ("Line #{} caption:", "Название линии #{}:"),
    ("Caption for the new line (#{}):", "Название для новой линии (#{}):"),
    ("Line #{}", "Линия #{}"),
    // Dialogs
    ("OK", "OK"),
    ("Cancel", "Отмена"),
    ("X axis", "ось X"),
    ("Y axis", "ось Y"),
    ("Xmin value", "значение Xmin"),
    ("Xmax value", "значение Xmax"),
    ("Ymin value", "значение Ymin"),
    ("Ymax value", "значение Ymax"),
    ("Log scale along X axis", "Log масштаб вдоль оси X"),
    ("Log scale along Y axis", "Log масштаб вдоль оси Y"),
    ("Options", "Параметры"),
    ("Auto trace", "Авто трассировка"),
    ("Distance between data points:", "Расстояние между точками данных:"),
    ("Smooth lines", "Сглаживать линии"),
    ("Lines", "Линии"),
    ("Current line only", "Текущая линия"),
    ("All lines", "Все линии"),
    ("Print line caption", "Выводить название линии"),
    ("Output format", "Формат вывода"),
    ("X-value based sorting", "Сортировать по X"),
    ("Print header", "Выводить заголовок"),
    ("Separator", "Разделитель"),
    ("Space separated", "Пробелы"),
    ("Tab separated", "Табуляция"),
    ("Export to text file", "Экспорт в текстовый файл"),
    ("Export to clipboard", "Экспорт в буфер"),
    ("Digitize", "Оцифровка"),
    ("Area", "Область"),
    ("Rectangular area", "Прямоугольная область"),
    ("Area is related to axes", "Область задается осями координат"),
    ("Grids", "Сетка"),
    ("X grid", "Сетка по X"),
    ("Y grid", "Сетка по Y"),
    ("Grid settings", "Параметры сетки"),
    ("Grid Settings", "Параметры сетки"),
    ("Shift X grid to make it go through x =", "Сдвигать сетку по Х, проходит через x ="),
    ("Shift Y grid to make it go through y =", "Сдвигать сетку по Y, проходит через y ="),
    ("points", "точки"),
    ("x axis values", "значения по оси x"),
    ("y axis values", "значения по оси y"),
    ("Line properties", "Свойства линии"),
    ("Line type", "Тип линии"),
    ("polyline", "полилиния"),
    ("closed polyline", "замкнутая полилиния"),
    ("separate points", "отдельные точки"),
    ("Points color:", "Цвет точек:"),
    ("Change color", "Изменить цвет"),
    ("Remove line", "Удалить линию"),
    ("Data points are placed", "Располагать точки данных"),
    ("in the middle of a line", "в середине линии"),
    ("at the area boundary", "на границе области"),
    ("Line/background identification method:", "Метод идентификации линия/фон:"),
    ("by color", "по цвету"),
    ("by intensity", "по яркости"),
    ("combined", "комбинированный"),
    ("Ignore single pixels/very thin lines", "Игнорировать отдельные точки/тонкие линии"),
    ("Auto trace: choose direction", "Авто трассировка: выбрать направление"),
    ("Yes", "Да"),
    ("No", "Нет"),
    // Messages
    ("Save changes to current workspace?", "Сохранить изменения рабочего пространства?"),
    ("Please enter a number.", "Пожалуйста, введите число."),
    (
        "X axis: log scale not allowed (both Xmin and Xmax should be > 0)",
        "Ось X: log масштаб невозможен (Xmin и Xmax должны быть > 0)",
    ),
    (
        "Y axis: log scale not allowed (both Ymin and Ymax should be > 0)",
        "Ось Y: log масштаб невозможен (Ymin и Ymax должны быть > 0)",
    ),
    (
        "Export not possible: Xmin, Xmax, Ymin and Ymax should be set",
        "Экспорт невозможен: должны быть поставлены точки Xmin, Xmax, Ymin и Ymax",
    ),
    (
        "Warning: this operation can not be undone!\nAre you sure you want to completely remove this line and corresponding data points?",
        "Внимание: данное действие нельзя отменить!\nВы уверены, что хотите полностью удалить эту линию и соответствующие точки данных?",
    ),
    (
        "Warning: this operation can not be undone!\nAre you sure you want to completely remove all lines and data points?",
        "Внимание: данное действие нельзя отменить!\nВы уверены, что хотите полностью удалить все линии и точки данных?",
    ),
    ("Unable to open file \"{}\".\nTry to locate this file?", "Невозможно открыть файл \"{}\".\nПопытаться найти этот файл?"),
    ("Unable to open file \"{}\"", "Невозможно открыть файл \"{}\""),
    ("File \"{}\" is not a valid GetData workspace.", "Файл \"{}\" не является файлом рабочего пространства GetData."),
    ("Shift + left mouse button to change eraser size", "Shift + левая кнопка мышки: изменение размера ластика"),
    ("Shift + left mouse button to set/remove points", "Shift + щелчок левой кнопкой мышки: поставить/удалить точку"),
    ("Left/right mouse button to set left/right boundary", "Левая/правая кнопка мышки: установка левой/правой границы"),
    ("Data copied to buffer", "Данные скопированы в буфер"),
    (
        "Left/right button to set/select and move data points; Ctrl: accurate cursor positioning; Shift: middle of lines only",
        "Левая/правая кнопка: поставить/выделить и передвинуть точку; Ctrl: медленное движение; Shift: центр линий",
    ),
    ("Workspace saved", "Рабочее пространство сохранено"),
    (
        "Click left mouse button to set Xmin; ESC to abort, SPACE to skip Xmin",
        "Щелчок левой кнопкой мышки: поставить Xmin; ESC - прервать, ПРОБЕЛ - пропустить Xmin",
    ),
    (
        "Click left mouse button to set Xmax; ESC to abort, SPACE to skip Xmax",
        "Щелчок левой кнопкой мышки: поставить Xmax; ESC - прервать, ПРОБЕЛ - пропустить Xmax",
    ),
    (
        "Click left mouse button to set Ymin; ESC to abort, SPACE to skip Ymin",
        "Щелчок левой кнопкой мышки: поставить Ymin; ESC - прервать, ПРОБЕЛ - пропустить Ymin",
    ),
    (
        "Click left mouse button to set Ymax; ESC to abort, SPACE to skip Ymax",
        "Щелчок левой кнопкой мышки: поставить Ymax; ESC - прервать, ПРОБЕЛ - пропустить Ymax",
    ),
    (
        "Left mouse button: auto trace in all directions, right mouse button: choose direction for auto trace",
        "Левая кнопка мышки: авто трассировка во всех направлениях, правая кнопка мышки: выбрать направление для авто трассировки",
    ),
    ("Click left mouse button to choose line color", "Щелчок левой кнопкой мышки: выбрать цвет линии"),
    ("Click left mouse button to choose background color", "Щелчок левой кнопкой мышки: выбрать цвет фона"),
    (
        "Hold left mouse button and choose the area to digitize; CTRL to move the grid",
        "При нажатой левой кнопке мышки выберите область для оцифровки; CTRL - подвинуть сетку",
    ),
    (
        "Hold left mouse button and choose the area to digitize; CTRL to move the grid; SHIFT to rotate the grid",
        "При нажатой левой кнопке мышки выберите область для оцифровки; CTRL - подвинуть сетку; Shift - поворот сетки",
    ),
    (
        "Hold Ctrl to start reordering from the middle of the line; Shift + left mouse button to change reorder tool size",
        "Держите нажатым Ctrl, чтобы начать упорядочивать с середины линии; Shift + левая кнопка мышки: изменить размер",
    ),
    ("No data: set Xmin, Xmax, Ymin and Ymax", "Нет данных: установите Xmin, Xmax, Ymin и Ymax"),
    ("No data: Xmin = Xmax", "Нет данных: Xmin = Xmax"),
    ("No data: Ymin = Ymax", "Нет данных: Ymin = Ymax"),
    ("No data: axes OX and OY are collinear", "Нет данных: оси OX и OY коллинеарны"),
    ("No data: log scale along X axis not allowed", "Нет данных: log масштаб по оси X невозможен"),
    ("No data: log scale along Y axis not allowed", "Нет данных: log масштаб по оси Y невозможен"),
    ("Set Xmin", "Поставить Xmin"),
    ("Set Xmax", "Поставить Xmax"),
    ("Set Ymin", "Поставить Ymin"),
    ("Set Ymax", "Поставить Ymax"),
    ("Remove Xmin", "Удалить Xmin"),
    ("Remove Xmax", "Удалить Xmax"),
    ("Remove Ymin", "Удалить Ymin"),
    ("Remove Ymax", "Удалить Ymax"),
    ("Set point", "Поставить точку"),
    ("Remove point", "Удалить точку"),
    ("Set point in the middle of a line", "Поставить точку в центре линии"),
    ("All directions", "Во всех направлениях"),
    ("Choose direction", "Выбрать направление"),
    ("Auto trace line", "Авто трассировка линии"),
    ("Choose starting point", "Выберите начальную точку"),
    ("Choose line color", "Выберите цвет линии"),
    ("Choose background color", "Выберите цвет фона"),
    ("dx should not be less than {}", "dx не должно быть меньше, чем {}"),
    ("dy should not be less than {}", "dy не должно быть меньше, чем {}"),
    ("x(n+1)/x(n) should not be less than {}", "x(n+1)/x(n) не должно быть меньше, чем {}"),
    ("y(n+1)/y(n) should not be less than {}", "y(n+1)/y(n) не должно быть меньше, чем {}"),
    ("Error: Xmin = Xmax", "Ошибка: Xmin = Xmax"),
    ("Error: Ymin = Ymax", "Ошибка: Ymin = Ymax"),
    ("Ready", " "),
    ("Generated by {} on {}\nfrom file '{}'\n\n", "Создано программой {}, дата создания {},\nна основе файла '{}'\n\n"),
    ("Image files", "Изображения"),
    ("All files", "Все файлы"),
    ("GetData Workspace", "Рабочее пространство GetData"),
    ("Error", "Ошибка"),
    ("GetData version", "GetData версия"),
    ("Press ESC to abort", "ESC для отмены"),
];

fn ru_map() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| RU.iter().copied().collect())
}

/// Translate an English UI string to the current language.
pub fn tr(en: &'static str) -> &'static str {
    match lang() {
        Lang::English => en,
        Lang::Russian => ru_map().get(en).copied().unwrap_or(en),
    }
}

/// Translate a template with `{}` placeholders and substitute arguments in order.
pub fn trf(en: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut it = args.iter();
    let mut parts = tr(en).split("{}").peekable();
    while let Some(p) = parts.next() {
        out.push_str(p);
        if parts.peek().is_some() {
            if let Some(a) = it.next() {
                out.push_str(&a.to_string());
            }
        }
    }
    out
}
