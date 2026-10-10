import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core
import ".."

Page {
    id: page
    title: "Настройки"

    readonly property var backdropModes: [
        { mode: "gauss", text: "Мягкое", hint: "Ровное размытие без пятен и полос: обложка плавно растворяется в цвете" },
        { mode: "glass", text: "Матовое стекло", hint: "Обложка угадывается, как за матовым стеклом; поверх — мелкое зерно" },
        { mode: "palette", text: "Градиент", hint: "Без обложки: плавный градиент из цветов темы" },
        { mode: "blobs", text: "Пятна", hint: "Крупные цветовые пятна — как было раньше" }
    ]

    readonly property var transitions: [
        { mode: "wipe", text: "Шторка", hint: "Новые цвета и фон въезжают сбоку: слева направо — следующий трек, справа налево — предыдущий" },
        { mode: "ripple", text: "Волна от обложки", hint: "Новые цвета и фон расходятся от обложки фигурой с волнистым краем" },
        { mode: "dissolve", text: "Растворение", hint: "Старые цвета и фон спокойно тают, проявляя новые" },
        { mode: "liquid", text: "Перетекание", hint: "Старая картинка растекается и тает неровной волной в сторону перехода" }
    ]

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
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 12
            Label { text: "Смена цветов и фона"; type: "bodyLarge" }
            Flow {
                Layout.fillWidth: true
                spacing: 8
                Repeater {
                    model: page.transitions
                    Chip {
                        required property var modelData
                        text: modelData.text
                        selected: Settings.trackTransition === modelData.mode
                        onClicked: Settings.trackTransition = modelData.mode
                    }
                }
            }
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: (page.transitions.find(m => m.mode === Settings.trackTransition) ?? page.transitions[0]).hint
                type: "bodyMedium"
                color: Theme.fgSurfaceVariant
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 12
            Label { text: "Фон из обложки"; type: "bodyLarge" }
            Flow {
                Layout.fillWidth: true
                spacing: 8
                Repeater {
                    model: page.backdropModes
                    Chip {
                        required property var modelData
                        text: modelData.text
                        selected: Settings.backdropMode === modelData.mode
                        onClicked: Settings.backdropMode = modelData.mode
                    }
                }
            }
            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: (page.backdropModes.find(m => m.mode === Settings.backdropMode) ?? page.backdropModes[0]).hint
                type: "bodyMedium"
                color: Theme.fgSurfaceVariant
            }
        }
        SettingRow {
            title: "Размытие фона плеера"
            enabled: Settings.backdropMode !== "palette"
            opacity: enabled ? 1 : 0.5
            description: Settings.backdropMode === "palette" ? "Градиент от обложки не зависит"
                : Math.round(Settings.nowPlayingBlur * 100) + " % · 0 — чёткая обложка, 100 — только цвет"
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
            title: "Плавание размытия"
            description: Settings.nowPlayingDrift > 0
                ? "Цвета фона медленно переливаются, пока играет музыка"
                : "Выключено — размытие неподвижно"
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
        SettingRow {
            title: "Экономия в фоне"
            description: "Пока окно не в фокусе, декоративные анимации (волна прогресса, фигуры, перелив фона) стоят — меньше нагрузка на процессор. Текст песни продолжает идти"
            Switch { checked: Settings.calmWhenInactive; onToggled: c => Settings.calmWhenInactive = c }
        }
        SettingRow {
            title: "Рамка обложки вращается"
            description: "Пока играет музыка, фигурная рамка обложки медленно крутится; сама картинка стоит"
            Switch { checked: Settings.coverSpin; onToggled: c => Settings.coverSpin = c }
        }
        SettingRow {
            title: "Обложка дышит в такт"
            description: "В полноэкранном плеере обложка слегка пульсирует по громкости музыки"
            Switch { checked: Settings.coverPulse; onToggled: c => Settings.coverPulse = c }
        }
        SettingRow {
            title: "Атмосферный фон"
            description: "Размытая обложка играющего трека за всем окном — слабым цветным свечением"
            Switch { checked: Settings.ambientBackground; onToggled: c => Settings.ambientBackground = c }
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
        SettingRow {
            title: "Плавный переход между треками"
            description: Settings.crossfade > 0
                ? "Следующий трек начинается за " + Settings.crossfade + " с до конца текущего и плавно сменяет его"
                : "Выключен — треки идут друг за другом без пауз (gapless)"
            Slider {
                width: 220
                from: 0
                to: 12
                stepSize: 1
                value: Settings.crossfade
                valueText: v => v > 0 ? Math.round(v) + " с" : "выкл"
                onMoved: v => Settings.crossfade = Math.round(v)
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
            title: "Скрывать в трей при закрытии"
            description: "Закрытое окно прячется в значок в трее, музыка играет дальше. Выход — из меню значка или Ctrl+Q"
            Switch { checked: Settings.closeToTray; onToggled: c => Settings.closeToTray = c }
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
