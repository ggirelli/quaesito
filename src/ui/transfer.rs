//! Quiz imports and exports with progress: the browser sends and fetches the files itself, so
//! the admin page can tell how far along they are, and what the server is doing meanwhile.

use dioxus::prelude::*;
use serde::Deserialize;

use crate::model::{ImportOutcome, ImportProgress};

/// The id of the admin page's file input that [`import`] sends from.
pub const IMPORT_INPUT: &str = "import-file";

/// An import or export under way, as the admin page shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Transfer {
    /// What is moving, e.g. "Importing movie-night.zip".
    pub what: String,
    /// What is happening now, e.g. "Uploading… 40%".
    pub step: String,
    /// How far along that is, from 0 to 1, when known.
    pub fraction: Option<f64>,
}

/// The transfer under way on the admin page, if any; provided by its dashboard.
#[derive(Clone, Copy)]
pub struct Transfers(pub Signal<Option<Transfer>>);

/// What the browser reports while sending or fetching a file.
#[derive(Deserialize)]
enum Event {
    /// Bytes sent or received so far, of `total` when known.
    Bytes {
        loaded: f64,
        total: Option<f64>,
    },
    /// The whole file is sent: the server is reading it.
    Sent,
    /// One line of the import's answer.
    Line(ImportProgress),
    /// The exported file is handed to the browser to save.
    Saved,
    Failed(String),
    /// The script is done; anything after this would never come.
    End,
}

/// Sends the file chosen in the input [`IMPORT_INPUT`], reporting progress line by line.
const IMPORT_JS: &str = r"
const url = await dioxus.recv();
const input = document.getElementById('import-file');
const file = input && input.files[0];
if (!file) {
  dioxus.send({ Failed: 'Choose a file to import.' });
} else {
  await new Promise((done) => {
    const xhr = new XMLHttpRequest();
    let read = 0;
    const lines = () => {
      let end;
      while ((end = xhr.responseText.indexOf('\n', read)) >= 0) {
        const line = xhr.responseText.slice(read, end);
        read = end + 1;
        try {
          dioxus.send({ Line: JSON.parse(line) });
        } catch {
          dioxus.send({ Failed: 'The server answered something unexpected.' });
        }
      }
    };
    xhr.upload.onprogress = (e) =>
      dioxus.send({ Bytes: { loaded: e.loaded, total: e.lengthComputable ? e.total : null } });
    xhr.upload.onload = () => dioxus.send('Sent');
    xhr.onprogress = lines;
    xhr.onload = () => {
      if (xhr.status === 200) {
        lines();
      } else if (xhr.status === 401) {
        dioxus.send({ Failed: 'Your login has expired: log in again to import.' });
      } else {
        dioxus.send({ Failed: xhr.responseText || 'The import failed.' });
      }
      done();
    };
    xhr.onerror = () => {
      dioxus.send({ Failed: 'Could not reach the server.' });
      done();
    };
    xhr.open('POST', url);
    xhr.send(file);
  });
}
dioxus.send('End');
";

/// Fetches an export, reporting progress, then hands it to the browser to save.
const EXPORT_JS: &str = r#"
const url = await dioxus.recv();
await new Promise((done) => {
  const xhr = new XMLHttpRequest();
  xhr.responseType = 'blob';
  xhr.onprogress = (e) =>
    dioxus.send({ Bytes: { loaded: e.loaded, total: e.lengthComputable ? e.total : null } });
  xhr.onload = () => {
    if (xhr.status === 200) {
      const disposition = xhr.getResponseHeader('Content-Disposition') || '';
      const named = /filename="([^"]+)"/.exec(disposition);
      const link = document.createElement('a');
      link.href = URL.createObjectURL(xhr.response);
      link.download = named ? named[1] : 'quiz.zip';
      document.body.appendChild(link);
      link.click();
      link.remove();
      setTimeout(() => URL.revokeObjectURL(link.href), 60000);
      dioxus.send('Saved');
    } else if (xhr.status === 401) {
      dioxus.send({ Failed: 'Your login has expired: log in again to export.' });
    } else {
      dioxus.send({ Failed: 'The export failed.' });
    }
    done();
  };
  xhr.onerror = () => {
    dioxus.send({ Failed: 'Could not reach the server.' });
    done();
  };
  xhr.open('GET', url);
  xhr.send();
});
dioxus.send('End');
"#;

/// `done` of `total` as a fraction, if there is a total.
fn share(done: f64, total: Option<f64>) -> Option<f64> {
    total
        .filter(|t| *t > 0.0)
        .map(|t| (done / t).clamp(0.0, 1.0))
}

/// A step's name with its percentage, when known: "Uploading… 40%".
fn percent(name: &str, fraction: Option<f64>) -> String {
    match fraction {
        Some(f) => format!("{name}… {:.0}%", f * 100.0),
        None => format!("{name}…"),
    }
}

/// Imports the file chosen in [`IMPORT_INPUT`] (named `name`), showing its progress in
/// `transfers` until it ends.
///
/// # Errors
/// Fails with a message for the admin when the file cannot be sent or imported.
pub async fn import(
    token: String,
    consent: bool,
    name: &str,
    Transfers(mut transfer): Transfers,
) -> Result<ImportOutcome, String> {
    let what = format!("Importing {name}");
    let mut show = |step: String, fraction: Option<f64>| {
        transfer.set(Some(Transfer {
            what: what.clone(),
            step,
            fraction,
        }));
    };
    show(percent("Uploading", Some(0.0)), Some(0.0));
    let mut script = document::eval(IMPORT_JS);
    let _ = script.send(format!("/import?token={token}&consent={consent}"));
    let result = loop {
        match script.recv::<Event>().await {
            Ok(Event::Bytes { loaded, total }) => {
                let fraction = share(loaded, total);
                show(percent("Uploading", fraction), fraction);
            }
            Ok(Event::Sent) => show("Reading the quiz…".to_owned(), None),
            Ok(Event::Line(ImportProgress::Media { done, total })) if total > 0 => {
                #[allow(clippy::cast_precision_loss, reason = "media counts are small")]
                let fraction = share(done as f64, Some(total as f64));
                show(
                    format!("Processing pictures and sounds: {done} of {total}"),
                    fraction,
                );
            }
            Ok(Event::Line(ImportProgress::Media { .. })) => {}
            Ok(Event::Line(ImportProgress::Done(outcome))) => break Ok(outcome),
            Ok(Event::Line(ImportProgress::Failed(err)) | Event::Failed(err)) => break Err(err),
            Ok(Event::Saved | Event::End) | Err(_) => {
                break Err("The import stopped before it finished.".to_owned());
            }
        }
    };
    transfer.set(None);
    result
}

/// Forgets the file chosen for import, so choosing the same one again imports it again.
pub fn clear_import_input() {
    document::eval(&format!(
        "const input = document.getElementById('{IMPORT_INPUT}'); if (input) {{ input.value = ''; }}"
    ));
}

/// Exports quiz `id` (titled `title`) and has the browser save it, showing its progress in
/// `transfers` until it ends.
///
/// # Errors
/// Fails with a message for the admin when the export cannot be fetched.
pub async fn export(
    token: String,
    id: String,
    title: &str,
    Transfers(mut transfer): Transfers,
) -> Result<(), String> {
    let what = format!("Exporting {title}");
    let mut show = |step: String, fraction: Option<f64>| {
        transfer.set(Some(Transfer {
            what: what.clone(),
            step,
            fraction,
        }));
    };
    show("Packing pictures and sounds…".to_owned(), None);
    let mut script = document::eval(EXPORT_JS);
    let _ = script.send(format!("/export/{id}?token={token}"));
    let result = loop {
        match script.recv::<Event>().await {
            Ok(Event::Bytes { loaded, total }) => {
                let fraction = share(loaded, total);
                show(percent("Downloading", fraction), fraction);
            }
            Ok(Event::Saved) => break Ok(()),
            Ok(Event::Failed(err)) => break Err(err),
            Ok(Event::Sent | Event::Line(_) | Event::End) | Err(_) => {
                break Err("The export stopped before it finished.".to_owned());
            }
        }
    };
    transfer.set(None);
    result
}

/// The transfer under way, with its progress bar.
#[component]
pub fn TransferStatus() -> Element {
    let Transfers(transfer) = use_context();
    let Some(transfer) = transfer() else {
        return rsx! {};
    };
    rsx! {
        div { class: "transfer stack-tight", role: "status",
            div { class: "row between",
                strong { "{transfer.what}" }
                span { class: "muted small", "{transfer.step}" }
            }
            // Without a value, the bar shows that something is happening, of unknown length.
            progress { max: 1, value: transfer.fraction }
        }
    }
}
