// Auth (auth.py) — зеркало AuthService: свойства из снимка (watch), сигналы-факты signedIn/signedOut — из событий.
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use ym_services::{AuthEvent, AuthSnapshot};

use super::{Delivery, forward_events, forward_watch};
use crate::context::ctx;
use crate::qt::{QVariantMap, json_to_qvariantmap};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qmap.h");
        type QMap_QString_QVariant = cxx_qt_lib::QMap<cxx_qt_lib::QMapPair_QString_QVariant>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QString, state, READ, NOTIFY = state_changed)]
        #[qproperty(QString, error_text, cxx_name = "errorText", READ, NOTIFY = error_changed)]
        #[qproperty(QString, user_code, cxx_name = "userCode", READ, NOTIFY = code_changed)]
        #[qproperty(QString, verification_url, cxx_name = "verificationUrl", READ, NOTIFY = code_changed)]
        #[qproperty(f64, code_expires_at, cxx_name = "codeExpiresAt", READ, NOTIFY = code_changed)]
        #[qproperty(QString, token_storage, cxx_name = "tokenStorage", READ, NOTIFY = storage_changed)]
        #[qproperty(QMap_QString_QVariant, account, READ, NOTIFY = account_changed)]
        type Auth = super::AuthRust;

        #[qsignal]
        #[cxx_name = "stateChanged"]
        fn state_changed(self: Pin<&mut Auth>);
        #[qsignal]
        #[cxx_name = "errorChanged"]
        fn error_changed(self: Pin<&mut Auth>);
        #[qsignal]
        #[cxx_name = "codeChanged"]
        fn code_changed(self: Pin<&mut Auth>);
        #[qsignal]
        #[cxx_name = "storageChanged"]
        fn storage_changed(self: Pin<&mut Auth>);
        #[qsignal]
        #[cxx_name = "accountChanged"]
        fn account_changed(self: Pin<&mut Auth>);
        #[qsignal]
        #[cxx_name = "signedIn"]
        fn signed_in(self: Pin<&mut Auth>);
        #[qsignal]
        #[cxx_name = "signedOut"]
        fn signed_out(self: Pin<&mut Auth>);

        #[qinvokable]
        #[cxx_name = "startDeviceLogin"]
        fn start_device_login(self: &Auth);
        #[qinvokable]
        #[cxx_name = "cancelLogin"]
        fn cancel_login(self: &Auth);
        #[qinvokable]
        #[cxx_name = "signInWithToken"]
        fn sign_in_with_token(self: &Auth, token: &QString);
        #[qinvokable]
        #[cxx_name = "signOut"]
        fn sign_out(self: &Auth);
        #[qinvokable]
        fn retry(self: &Auth);
        #[qinvokable]
        #[cxx_name = "copyToClipboard"]
        fn copy_to_clipboard(self: &Auth, text: &QString);
    }

    impl cxx_qt::Threading for Auth {}
    impl cxx_qt::Initialize for Auth {}
}

pub struct AuthRust {
    snapshot: AuthSnapshot,
    state: QString,
    error_text: QString,
    user_code: QString,
    verification_url: QString,
    code_expires_at: f64,
    token_storage: QString,
    account: QVariantMap,
}

impl Default for AuthRust {
    fn default() -> Self {
        let snapshot = AuthSnapshot::default();
        Self {
            state: QString::from(snapshot.state.as_str()),
            error_text: QString::default(),
            user_code: QString::default(),
            verification_url: QString::default(),
            code_expires_at: 0.0,
            token_storage: QString::default(),
            account: QVariantMap::default(),
            snapshot,
        }
    }
}

impl qobject::Auth {
    fn start_device_login(&self) {
        ctx().services.auth.start_device_login();
    }

    fn cancel_login(&self) {
        ctx().services.auth.cancel_login();
    }

    fn sign_in_with_token(&self, token: &QString) {
        ctx().services.auth.sign_in_with_token(&token.to_string());
    }

    fn sign_out(&self) {
        ctx().services.auth.sign_out();
    }

    fn retry(&self) {
        ctx().services.auth.retry();
    }

    fn copy_to_clipboard(&self, text: &QString) {
        crate::qt::copy_to_clipboard(&text.to_string());
    }

    /// Снимок сервиса → свойства; сигнал — только для изменившихся групп.
    fn apply(mut self: Pin<&mut Self>, next: AuthSnapshot) {
        let prev = std::mem::replace(&mut self.as_mut().rust_mut().snapshot, next.clone());
        if prev == next {
            return;
        }
        if prev.state != next.state {
            self.as_mut().rust_mut().state = QString::from(next.state.as_str());
            self.as_mut().state_changed();
        }
        if prev.error_text != next.error_text {
            self.as_mut().rust_mut().error_text = QString::from(next.error_text.as_str());
            self.as_mut().error_changed();
        }
        if prev.user_code != next.user_code
            || prev.verification_url != next.verification_url
            || prev.code_expires_at_ms != next.code_expires_at_ms
        {
            let mut this = self.as_mut().rust_mut();
            this.user_code = QString::from(next.user_code.as_str());
            this.verification_url = QString::from(next.verification_url.as_str());
            this.code_expires_at = next.code_expires_at_ms;
            self.as_mut().code_changed();
        }
        if prev.token_storage != next.token_storage {
            self.as_mut().rust_mut().token_storage = QString::from(next.token_storage.as_str());
            self.as_mut().storage_changed();
        }
        if prev.account != next.account {
            let account = match &next.account {
                Some(account) => match serde_json::to_value(account) {
                    Ok(serde_json::Value::Object(map)) => json_to_qvariantmap(&map),
                    _ => QVariantMap::default(),
                },
                None => QVariantMap::default(),
            };
            self.as_mut().rust_mut().account = account;
            self.as_mut().account_changed();
        }
    }

    fn on_snapshot(self: Pin<&mut Self>, snapshot: AuthSnapshot) {
        self.apply(snapshot);
    }

    fn on_event(mut self: Pin<&mut Self>, delivery: Delivery<AuthEvent>) {
        // снимок обновлён до рассылки события: сначала свойства, потом сигнал
        let snapshot = ctx().services.auth.watch().borrow().clone();
        self.as_mut().apply(snapshot);
        match delivery {
            Delivery::Event(AuthEvent::SignedIn(_)) => self.signed_in(),
            Delivery::Event(AuthEvent::SignedOut) => self.signed_out(),
            Delivery::Lagged => {}
        }
    }
}

impl cxx_qt::Initialize for qobject::Auth {
    fn initialize(mut self: Pin<&mut Self>) {
        let auth = &ctx().services.auth;
        let mut watch = auth.watch();
        let snapshot = watch.borrow_and_update().clone();
        self.as_mut().apply(snapshot);
        forward_watch(self.qt_thread(), watch, Self::on_snapshot);
        forward_events(self.qt_thread(), auth.subscribe(), Self::on_event);
    }
}
