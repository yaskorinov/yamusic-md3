// Wave (wave.py) — зеркало WaveService: свойства из снимка, сигнал — по изменившейся группе.
use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use ym_services::WaveSnapshot;

use super::forward_watch;
use crate::context::ctx;
use crate::qt::{QVariantList, QVariantMap, json_to_qvariantlist};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qmap.h");
        type QMap_QString_QVariant = cxx_qt_lib::QMap<cxx_qt_lib::QMapPair_QString_QVariant>;
        include!("cxx-qt-lib/qlist.h");
        type QList_QVariant = cxx_qt_lib::QList<cxx_qt_lib::QVariant>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QList_QVariant, groups, READ, NOTIFY = groups_changed)]
        #[qproperty(QMap_QString_QVariant, selection, READ, NOTIFY = selection_changed)]
        #[qproperty(bool, active, READ, NOTIFY = state_changed)]
        #[qproperty(bool, loading, READ, NOTIFY = state_changed)]
        #[qproperty(QString, station_title, cxx_name = "stationTitle", READ, NOTIFY = state_changed)]
        #[qproperty(QString, error_text, cxx_name = "errorText", READ, NOTIFY = error_changed)]
        type Wave = super::WaveRust;

        #[qsignal]
        #[cxx_name = "groupsChanged"]
        fn groups_changed(self: Pin<&mut Wave>);
        #[qsignal]
        #[cxx_name = "selectionChanged"]
        fn selection_changed(self: Pin<&mut Wave>);
        #[qsignal]
        #[cxx_name = "stateChanged"]
        fn state_changed(self: Pin<&mut Wave>);
        #[qsignal]
        #[cxx_name = "errorChanged"]
        fn error_changed(self: Pin<&mut Wave>);

        /// Большая кнопка: «Моя волна» или пауза/продолжение, если она уже играет.
        #[qinvokable]
        fn play(self: &Wave);
        /// Волна от исполнителя/трека: сид artist:ID или track:ID.
        #[qinvokable]
        #[cxx_name = "playStation"]
        fn play_station(self: &Wave, seed: &QString, title: &QString);
        #[qinvokable]
        fn select(self: &Wave, key: &QString, seed: &QString);
    }

    impl cxx_qt::Threading for Wave {}
    impl cxx_qt::Initialize for Wave {}
}

#[derive(Default)]
pub struct WaveRust {
    snapshot: WaveSnapshot,
    groups: QVariantList,
    selection: QVariantMap,
    active: bool,
    loading: bool,
    station_title: QString,
    error_text: QString,
}

impl qobject::Wave {
    fn play(&self) {
        ctx().services.wave.play();
    }

    fn play_station(&self, seed: &QString, title: &QString) {
        ctx().services.wave.play_station(seed.to_string(), title.to_string());
    }

    fn select(&self, key: &QString, seed: &QString) {
        ctx().services.wave.select(key.to_string(), seed.to_string());
    }

    fn apply(mut self: Pin<&mut Self>, next: WaveSnapshot) {
        let prev = std::mem::replace(&mut self.as_mut().rust_mut().snapshot, next.clone());
        if prev == next {
            return;
        }
        if !Arc::ptr_eq(&prev.groups, &next.groups) && prev.groups != next.groups {
            self.as_mut().rust_mut().groups = match serde_json::to_value(&*next.groups) {
                Ok(serde_json::Value::Array(items)) => json_to_qvariantlist(&items),
                _ => QVariantList::default(),
            };
            self.as_mut().groups_changed();
        }
        if prev.selection != next.selection {
            let mut selection = QVariantMap::default();
            for (key, seed) in &next.selection {
                selection.insert(QString::from(key.as_str()), QVariant::from(&QString::from(seed.as_str())));
            }
            self.as_mut().rust_mut().selection = selection;
            self.as_mut().selection_changed();
        }
        if prev.active != next.active || prev.loading != next.loading || prev.station_title != next.station_title {
            let mut this = self.as_mut().rust_mut();
            this.active = next.active;
            this.loading = next.loading;
            this.station_title = QString::from(next.station_title.as_str());
            self.as_mut().state_changed();
        }
        if prev.error_text != next.error_text {
            self.as_mut().rust_mut().error_text = QString::from(next.error_text.as_str());
            self.as_mut().error_changed();
        }
    }
}

impl cxx_qt::Initialize for qobject::Wave {
    fn initialize(mut self: Pin<&mut Self>) {
        let mut watch = ctx().services.wave.watch();
        let snapshot = watch.borrow_and_update().clone();
        self.as_mut().apply(snapshot);
        forward_watch(self.qt_thread(), watch, Self::apply);
    }
}
