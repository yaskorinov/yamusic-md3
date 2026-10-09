import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core
import ".."

// Аккаунт: вход по коду устройства (ya.ru/device) или токеном; карточка аккаунта после входа.
Page {
    id: page
    title: "Аккаунт"

    readonly property string st: Auth.state
    property bool tokenMode: false

    // Обратный отсчёт кода
    property real now: Date.now()
    Timer { interval: 1000; repeat: true; running: page.st === "awaitingUser"; onTriggered: page.now = Date.now() }
    readonly property real secondsLeft: Math.max(0, (Auth.codeExpiresAt - now) / 1000)

    // ---- Не выполнен вход ----
    ColumnLayout {
        visible: page.st === "signedOut" || page.st === "error"
        Layout.fillWidth: true
        Layout.maximumWidth: 560
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: 24
        spacing: 16

        EmptyState {
            Layout.alignment: Qt.AlignHCenter
            icon: page.st === "error" ? "cloud_off" : "account_circle"
            shape: page.st === "error" ? "flower6" : "cookie9"
            title: page.st === "error" ? "Не получилось" : "Войдите в Яндекс"
            text: Auth.errorText !== "" ? Auth.errorText
                : "Чтобы слушать музыку, нужен аккаунт Яндекса с подпиской Плюс. Пароль вводится только на сайте Яндекса"
        }

        RowLayout {
            Layout.alignment: Qt.AlignHCenter
            spacing: 12
            Button {
                text: page.st === "error" ? "Повторить" : "Войти по коду"
                icon: page.st === "error" ? "refresh" : "qr_code_2"
                size: "m"
                onClicked: page.st === "error" ? Auth.retry() : Auth.startDeviceLogin()
            }
            Button {
                visible: page.st !== "error"
                text: page.tokenMode ? "Скрыть" : "У меня есть токен"
                style: "text"
                size: "m"
                onClicked: page.tokenMode = !page.tokenMode
            }
        }

        ColumnLayout {
            visible: page.tokenMode && page.st !== "error"
            Layout.fillWidth: true
            Layout.topMargin: 8
            spacing: 12
            SearchField {
                id: tokenField
                Layout.fillWidth: true
                leadingIcon: "key"
                placeholder: "OAuth-токен Яндекс Музыки"
                echoMode: TextInput.Password
                onAccepted: t => Auth.signInWithToken(t)
            }
            Button {
                Layout.alignment: Qt.AlignRight
                text: "Войти с токеном"
                style: "tonal"
                enabled: tokenField.text.length > 10
                onClicked: Auth.signInWithToken(tokenField.text)
            }
        }
    }

    // ---- Код для ya.ru/device ----
    ColumnLayout {
        visible: page.st === "awaitingUser"
        Layout.fillWidth: true
        Layout.maximumWidth: 560
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: 16
        spacing: 20

        Label {
            Layout.alignment: Qt.AlignHCenter
            text: "Откройте " + Auth.verificationUrl.replace("https://", "") + " и введите код"
            type: "titleLarge"
        }

        Row {
            Layout.alignment: Qt.AlignHCenter
            spacing: 8
            Repeater {
                model: Auth.userCode.split("")
                Rectangle {
                    required property string modelData
                    required property int index
                    width: 56
                    height: 72
                    radius: Theme.shape.large
                    color: Theme.primaryContainer
                    Label {
                        anchors.centerIn: parent
                        text: parent.modelData
                        type: "displaySmall"
                        weight: 600
                        color: Theme.fgPrimaryContainer
                    }
                }
            }
        }

        RowLayout {
            Layout.alignment: Qt.AlignHCenter
            spacing: 12
            Button {
                text: "Открыть страницу"
                icon: "open_in_new"
                onClicked: Qt.openUrlExternally(Auth.verificationUrl)
            }
            Button {
                text: copied.running ? "Скопировано" : "Скопировать код"
                icon: copied.running ? "check" : "content_copy"
                style: "tonal"
                onClicked: { Auth.copyToClipboard(Auth.userCode); copied.restart() }
                Timer { id: copied; interval: 1500 }
            }
        }

        RowLayout {
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: 8
            spacing: 12
            LoadingIndicator { size: 36 }
            Label {
                text: "Ждём подтверждения · код действует ещё "
                      + Math.floor(page.secondsLeft / 60) + ":" + String(Math.floor(page.secondsLeft % 60)).padStart(2, "0")
                type: "bodyMedium"
                color: Theme.fgSurfaceVariant
            }
        }

        Button {
            Layout.alignment: Qt.AlignHCenter
            text: "Отмена"
            style: "text"
            onClicked: Auth.cancelLogin()
        }
    }

    // ---- Ожидание ----
    ColumnLayout {
        visible: page.st === "checking" || page.st === "requestingCode" || page.st === "signingIn"
        Layout.alignment: Qt.AlignHCenter
        Layout.topMargin: 64
        spacing: 16
        LoadingIndicator { Layout.alignment: Qt.AlignHCenter; size: 72; contained: true }
        Label {
            Layout.alignment: Qt.AlignHCenter
            text: page.st === "requestingCode" ? "Получаем код…" : page.st === "signingIn" ? "Входим…" : "Проверяем сессию…"
            type: "titleMedium"
            color: Theme.fgSurfaceVariant
        }
    }

    // ---- Вошли ----
    Rectangle {
        visible: page.st === "signedIn"
        Layout.fillWidth: true
        Layout.maximumWidth: 640
        implicitHeight: signedInRow.implicitHeight + 48
        radius: Theme.shape.extraLarge
        color: Theme.surfaceContainerHigh

        RowLayout {
            id: signedInRow
            anchors.fill: parent
            anchors.margins: 24
            spacing: 20

            Avatar { size: 72; name: Auth.account.displayName ?? "" }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4
                Label { Layout.fillWidth: true; text: Auth.account.displayName ?? ""; type: "headlineSmall"; weight: 600 }
                Label { Layout.fillWidth: true; text: Auth.account.login ?? ""; type: "bodyMedium"; color: Theme.fgSurfaceVariant }
                RowLayout {
                    Layout.topMargin: 6
                    spacing: 8
                    Chip {
                        text: Auth.account.hasPlus ? "Плюс активен" : "Без Плюса"
                        icon: Auth.account.hasPlus ? "verified" : "block"
                        selected: false
                    }
                    Chip {
                        text: Auth.tokenStorage === "keyring" ? "Токен в связке ключей" : "Токен в файле"
                        icon: Auth.tokenStorage === "keyring" ? "lock" : "description"
                    }
                }
            }
            Button { text: "Выйти"; icon: "logout"; style: "outlined"; onClicked: Auth.signOut() }
        }
    }

    Label {
        visible: page.st === "signedIn" && Auth.tokenStorage === "file"
        Layout.fillWidth: true
        Layout.maximumWidth: 640
        text: "Связка ключей (gnome-keyring) не настроена, поэтому токен лежит в файле с доступом только для вашего пользователя. Создайте связку ключей по умолчанию (например, в Seahorse) и войдите заново — токен переедет туда."
        type: "bodySmall"
        color: Theme.fgSurfaceVariant
        wrapMode: Text.WordWrap
        elide: Text.ElideNone
        lineHeightMode: Text.ProportionalHeight
        lineHeight: 1.25
    }
}
