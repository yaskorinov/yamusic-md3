import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core
import ".."

Page {
    id: page
    title: "Настройки"

    readonly property var variantNames: ({
        content: "По обложке", tonalSpot: "Спокойная", vibrant: "Яркая", expressive: "Выразительная",
        fidelity: "Точная", neutral: "Нейтральная", monochrome: "Монохром"
    })

    SettingsCard {
        title: "Внешний вид"
        icon: "palette"

        SettingRow {
            title: "Тема"
            ButtonGroup {
                model: [{ text: "Системная", icon: "contrast" }, { text: "Светлая", icon: "light_mode" }, { text: "Тёмная", icon: "dark_mode" }]
                autoSelect: false
                currentIndex: ["system", "light", "dark"].indexOf(Settings.themeMode)
                onActivated: i => Settings.themeMode = ["system", "light", "dark"][i]
            }
        }

        SettingRow {
            title: "Цвет из обложки"
            description: "Приложение перекрашивается под обложку играющего трека"
            Switch { checked: Settings.accentFromCover; onToggled: c => Settings.accentFromCover = c }
        }

        SettingRow {
            title: "Акцентный цвет"
            description: Settings.accentFromCover ? "Когда ничего не играет" : "Основной цвет приложения"
            Row {
                spacing: 8
                Repeater {
                    model: ["#FFCC00", "#FF6D00", "#E8175D", "#9C4DFF", "#6750A4", "#00A3FF", "#00BFA5", "#1DB954"]
                    MorphShape {
                        required property string modelData
                        readonly property bool current: Settings.customSeed.toUpperCase() === modelData
                        width: 32
                        height: 32
                        shape: current ? "cookie6" : "circle"
                        color: modelData
                        MorphShape {
                            anchors.centerIn: parent
                            width: 12; height: 12
                            visible: parent.current
                            shape: "circle"
                            color: "white"
                        }
                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: Settings.customSeed = parent.modelData }
                    }
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 12
            Label { text: "Цветовая схема"; type: "bodyLarge" }
            Flow {
                Layout.fillWidth: true
                spacing: 8
                Repeater {
                    model: ThemeEngine.variants
                    Chip {
                        required property string modelData
                        text: page.variantNames[modelData] ?? modelData
                        selected: Settings.schemeVariant === modelData
                        onClicked: Settings.schemeVariant = modelData
                    }
                }
            }
        }
        SettingRow {
            title: "Размытие фона плеера"
            description: Math.round(Settings.nowPlayingBlur * 100) + " % · 0 — чёткая обложка, 100 — мягкие цветовые пятна"
            Slider {
                width: 220
                from: 0
                to: 1
                stepSize: 0.05
                value: Settings.nowPlayingBlur
                valueText: v => Math.round(v * 100) + " %"
                onMoved: v => Settings.nowPlayingBlur = Math.round(v * 20) / 20
            }
        }
        SettingRow {
            title: "Плавание фона плеера"
            description: Settings.nowPlayingDrift > 0
                ? "Размытая обложка медленно перетекает, пока играет музыка"
                : "Выключено — фон неподвижен"
            Slider {
                width: 220
                from: 0
                to: 1
                stepSize: 0.05
                value: Settings.nowPlayingDrift
                valueText: v => v > 0 ? Math.round(v * 100) + " %" : "выкл"
                onMoved: v => Settings.nowPlayingDrift = Math.round(v * 20) / 20
            }
        }
    }

    SettingsCard {
        title: "Воспроизведение"
        icon: "graphic_eq"

        SettingRow {
            title: "Качество звука"
            description: Settings.quality === "lossless" ? "FLAC, если трек есть без потерь; иначе — AAC 256"
                       : Settings.quality === "high" ? "AAC 256 кбит/с" : "AAC 192 кбит/с — экономит трафик"
            ButtonGroup {
                model: ["Обычное", "Высокое", "Без потерь"]
                autoSelect: false
                currentIndex: ["low", "high", "lossless"].indexOf(Settings.quality)
                onActivated: i => Settings.quality = ["low", "high", "lossless"][i]
            }
        }
    }

    SettingsCard {
        title: "Окно"
        icon: "web_asset"

        SettingRow {
            title: "Компактная боковая панель"
            description: "Только иконки. На узком окне включается сама"
            Switch { checked: Settings.sidebarCollapsed; onToggled: c => Settings.sidebarCollapsed = c }
        }
        SettingRow {
            title: "Кнопки окна"
            description: "Свернуть, развернуть и закрыть — для окружений без тайлинга"
            Switch { checked: Settings.windowButtons; onToggled: c => Settings.windowButtons = c }
        }
        SettingRow {
            title: "Шаг прокрутки колесом"
            description: Settings.wheelStep + " px за щелчок; при быстрой прокрутке шаг растёт до ×2"
            Slider {
                width: 220
                from: 60
                to: 480
                stepSize: 20
                value: Settings.wheelStep
                valueText: v => Math.round(v) + " px"
                onMoved: v => Settings.wheelStep = Math.round(v)
            }
        }
    }

    SettingsCard {
        title: "О программе"
        icon: "info"

        SettingRow {
            title: "YaMusic 0.1"
            description: "Неофициальный клиент Яндекс Музыки в стиле Material 3 Expressive. Шрифты: Google Sans (OFL), Material Symbols (Apache 2.0)"
        }
    }
}
