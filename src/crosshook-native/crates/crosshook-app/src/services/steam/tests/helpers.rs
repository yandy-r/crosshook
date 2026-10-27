use std::fmt::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crosshook_core::metadata::MetadataStore;
use crosshook_core::settings::SettingsStore;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

use super::super::SteamService;

pub(super) fn service(root: &Path, metadata: MetadataStore) -> SteamService {
    SteamService::new(
        metadata,
        SettingsStore::with_base_path(root.join("settings")),
    )
}

pub(super) fn create_proton(root: &Path, name: &str) {
    let tool = root.join("compatibilitytools.d").join(name);
    std::fs::create_dir_all(root.join("steamapps")).unwrap();
    std::fs::create_dir_all(&tool).unwrap();
    std::fs::write(tool.join("proton"), b"#!/bin/sh\n").unwrap();
}

pub(super) const PNG: &[u8] =
    b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR\x00\x00\x00\x01\x00\x00\x00\x01";

#[derive(Clone, Default)]
pub(super) struct TraceCapture(pub Arc<Mutex<String>>);

struct Fields<'a>(&'a mut String);
impl Visit for Fields<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        write!(self.0, "{}={value:?};", field.name()).unwrap();
    }
}

impl Subscriber for TraceCapture {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, attrs: &Attributes<'_>) -> Id {
        let mut output = self.0.lock().unwrap();
        output.push_str(attrs.metadata().name());
        attrs.record(&mut Fields(&mut output));
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, record: &Record<'_>) {
        record.record(&mut Fields(&mut self.0.lock().unwrap()));
    }
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        event.record(&mut Fields(&mut self.0.lock().unwrap()));
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}
