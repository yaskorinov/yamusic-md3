import QtQuick

// Поверхность-контейнер MD3 (плавающие панели референса).
Rectangle {
    property string level: "container"   // lowest | low | container | high | highest | surface
    property int elevation: 0

    radius: Theme.shape.extraLarge
    color: ({
        lowest: Theme.surfaceContainerLowest,
        low: Theme.surfaceContainerLow,
        container: Theme.surfaceContainer,
        high: Theme.surfaceContainerHigh,
        highest: Theme.surfaceContainerHighest,
        surface: Theme.surface
    })[level] ?? Theme.surfaceContainer
}
